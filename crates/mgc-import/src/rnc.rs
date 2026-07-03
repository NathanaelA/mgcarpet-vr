//! RNC ("Rob Northen Compression" / ProPack) decompressor.
//!
//! Both Magic Carpet games ship most of their data RNC-packed. This is a
//! Rust port of remc2's `utilities/DataFileRNC.cpp` (GPL, itself derived
//! from the decompiled original ProPack in lab313ru/rnc_propack_source),
//! kept structurally close to the reference so the two can be diffed.
//!
//! Container layout (all multi-byte fields big-endian):
//! ```text
//! offset size  field
//! 0      3     signature "RNC"
//! 3      1     method (1 or 2, low two bits)
//! 4      4     unpacked size
//! 8      4     packed size (payload after the 18-byte header)
//! 12     2     CRC-16 of the unpacked data
//! 14     2     CRC-16 of the packed payload
//! 16     1     leeway
//! 17     1     chunk count
//! ```

pub const RNC_HEADER_SIZE: usize = 0x12;

/// Sanity cap on the claimed unpacked size, so a corrupt header cannot
/// make us allocate the moon. The original engine's whole buffer was
/// 0x90000 bytes; 64 MiB is generous for any Bullfrog-era file.
const MAX_UNPACKED_SIZE: u32 = 64 * 1024 * 1024;

/// How many bytes past the end of the packed payload the bit readers may
/// consume (as zeros) before giving up. See `M1::read_byte`.
const OVERRUN_LIMIT: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub enum RncError {
    /// Data does not start with an RNC signature.
    NotRnc,
    /// Data is shorter than the header claims.
    Truncated,
    /// Method byte is not 1 or 2.
    UnsupportedMethod(u8),
    /// Header flags a "locked" archive (never seen in game data).
    Locked,
    /// Header flags an encryption key requirement (never seen in game data).
    Encrypted,
    /// Claimed unpacked size exceeds the sanity cap.
    TooLarge(u32),
    PackedCrcMismatch {
        expected: u16,
        actual: u16,
    },
    UnpackedCrcMismatch {
        expected: u16,
        actual: u16,
    },
    /// A back-reference points before the start of the output.
    BadMatchOffset,
    /// The bit stream ran out of input mid-decode.
    UnexpectedEof,
    /// No Huffman code in the current table matches the input.
    BadHuffmanCode,
}

impl std::fmt::Display for RncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRnc => write!(f, "not an RNC container"),
            Self::Truncated => write!(f, "data shorter than header claims"),
            Self::UnsupportedMethod(m) => write!(f, "unsupported RNC method {m}"),
            Self::Locked => write!(f, "locked RNC archive"),
            Self::Encrypted => write!(f, "encrypted RNC archive"),
            Self::TooLarge(n) => write!(f, "unpacked size {n} exceeds sanity cap"),
            Self::PackedCrcMismatch { expected, actual } => {
                write!(
                    f,
                    "packed CRC mismatch: header {expected:#06x}, data {actual:#06x}"
                )
            }
            Self::UnpackedCrcMismatch { expected, actual } => {
                write!(
                    f,
                    "unpacked CRC mismatch: header {expected:#06x}, data {actual:#06x}"
                )
            }
            Self::BadMatchOffset => write!(f, "back-reference before start of output"),
            Self::UnexpectedEof => write!(f, "unexpected end of packed data"),
            Self::BadHuffmanCode => write!(f, "invalid Huffman code in stream"),
        }
    }
}

impl std::error::Error for RncError {}

#[derive(Debug, Clone, Copy)]
pub struct RncHeader {
    pub method: u8,
    pub unpacked_size: u32,
    pub packed_size: u32,
    pub unpacked_crc: u16,
    pub packed_crc: u16,
    pub leeway: u8,
    pub chunks: u8,
}

/// True if `data` starts with an RNC signature.
pub fn is_rnc(data: &[u8]) -> bool {
    data.len() >= RNC_HEADER_SIZE && data.starts_with(b"RNC")
}

pub fn parse_header(data: &[u8]) -> Result<RncHeader, RncError> {
    if !is_rnc(data) {
        return Err(RncError::NotRnc);
    }
    let be32 = |o: usize| u32::from_be_bytes(data[o..o + 4].try_into().unwrap());
    let be16 = |o: usize| u16::from_be_bytes(data[o..o + 2].try_into().unwrap());
    Ok(RncHeader {
        method: data[3] & 3,
        unpacked_size: be32(4),
        packed_size: be32(8),
        unpacked_crc: be16(12),
        packed_crc: be16(14),
        leeway: data[16],
        chunks: data[17],
    })
}

/// CRC-16/ARC (polynomial 0xA001 reflected, init 0) — the checksum RNC
/// containers use for both packed and unpacked data.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &b in data {
        crc ^= b as u16;
        crc = (crc >> 8) ^ CRC_TABLE[(crc & 0xFF) as usize];
    }
    crc
}

/// Decompress a full RNC container (header + payload), verifying both CRCs.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, RncError> {
    let header = parse_header(data)?;

    if header.unpacked_size > MAX_UNPACKED_SIZE {
        return Err(RncError::TooLarge(header.unpacked_size));
    }
    let payload = data[RNC_HEADER_SIZE..]
        .get(..header.packed_size as usize)
        .ok_or(RncError::Truncated)?;

    let actual_packed_crc = crc16(payload);
    if actual_packed_crc != header.packed_crc {
        return Err(RncError::PackedCrcMismatch {
            expected: header.packed_crc,
            actual: actual_packed_crc,
        });
    }

    let out = match header.method {
        1 => unpack_m1(payload, header.unpacked_size as usize)?,
        2 => unpack_m2(payload, header.unpacked_size as usize)?,
        m => return Err(RncError::UnsupportedMethod(m)),
    };

    let actual_unpacked_crc = crc16(&out);
    if actual_unpacked_crc != header.unpacked_crc {
        return Err(RncError::UnpackedCrcMismatch {
            expected: header.unpacked_crc,
            actual: actual_unpacked_crc,
        });
    }
    Ok(out)
}

/// Copy `count` bytes from `offset` bytes behind the write head, byte by
/// byte (overlapping copies are the point: offset 1 repeats the last byte).
fn copy_from_window(out: &mut Vec<u8>, offset: usize, count: usize) -> Result<(), RncError> {
    if offset == 0 || offset > out.len() {
        return Err(RncError::BadMatchOffset);
    }
    for _ in 0..count {
        let b = out[out.len() - offset];
        out.push(b);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Method 1: Huffman-coded LZ. Bits are consumed LSB-first from a 16-bit
// buffer that always holds the next two unread bytes in its upper half —
// a quirk the reference preserves from the original assembly, and which
// `decode_code` depends on (it peeks at the buffer without consuming).
// ---------------------------------------------------------------------------

struct M1<'a> {
    src: &'a [u8],
    pos: usize,
    bit_buffer: u32,
    bit_count: u32,
}

impl<'a> M1<'a> {
    fn new(src: &'a [u8]) -> Self {
        Self {
            src,
            pos: 0,
            bit_buffer: 0,
            bit_count: 0,
        }
    }

    /// Reads past the payload end yield zeros, up to [`OVERRUN_LIMIT`]:
    /// many real files end mid-refill of the 16-bit buffer, and the
    /// reference tolerates that (it reads leftover heap; the surplus bits
    /// never reach the output, which the unpacked CRC ultimately proves).
    /// The limit keeps corrupt streams from decoding zeros forever.
    fn read_byte(&mut self) -> Result<u8, RncError> {
        if self.pos >= self.src.len() + OVERRUN_LIMIT {
            return Err(RncError::UnexpectedEof);
        }
        let b = self.src.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        Ok(b)
    }

    /// Unconsumed byte `i` positions ahead of the cursor; past-the-end
    /// reads yield 0 (the reference reads leftover heap here, but those
    /// bits are never used by well-formed streams).
    fn peek(&self, i: usize) -> u32 {
        self.src.get(self.pos + i).copied().unwrap_or(0) as u32
    }

    fn bits(&mut self, count: u32) -> Result<u32, RncError> {
        let mut bits = 0u32;
        let mut cur = 1u32;
        for _ in 0..count {
            if self.bit_count == 0 {
                let b1 = self.read_byte()? as u32;
                let b2 = self.read_byte()? as u32;
                self.bit_buffer = (self.peek(1) << 24) | (self.peek(0) << 16) | (b2 << 8) | b1;
                self.bit_count = 16;
            }
            if self.bit_buffer & 1 != 0 {
                bits |= cur;
            }
            self.bit_buffer >>= 1;
            cur <<= 1;
            self.bit_count -= 1;
        }
        Ok(bits)
    }

    /// After a literal run consumes raw bytes from the cursor, the bit
    /// buffer's look-ahead half is stale; rebuild it from the bytes now
    /// ahead of the cursor while keeping the already-buffered low bits.
    fn reprime(&mut self) {
        let ahead = (self.peek(2) << 16) | (self.peek(1) << 8) | self.peek(0);
        let kept = self.bit_buffer & ((1u32 << self.bit_count) - 1);
        self.bit_buffer = ((ahead as u64) << self.bit_count) as u32 | kept;
    }

    /// Read one Huffman table: a 5-bit leaf count, then a 4-bit code
    /// length per leaf, expanded to canonical codes (bit-reversed for
    /// LSB-first matching).
    fn read_table(&mut self) -> Result<HufTable, RncError> {
        let mut table = HufTable::default();
        let mut leaves = self.bits(5)? as usize;
        if leaves == 0 {
            return Ok(table);
        }
        leaves = leaves.min(16);
        for i in 0..leaves {
            table.depth[i] = self.bits(4)? as u16;
        }
        // Canonical code assignment, shortest codes first.
        let mut val = 0u32;
        let mut div = 0x8000_0000u32;
        for bits_count in 1..=16u16 {
            for i in 0..leaves {
                if table.depth[i] == bits_count {
                    table.code[i] = inverse_bits(val / div, bits_count as u32);
                    val = val.wrapping_add(div);
                }
            }
            div >>= 1;
        }
        Ok(table)
    }

    /// Match the buffered bits against a table and decode one value.
    fn decode_code(&mut self, table: &HufTable) -> Result<u32, RncError> {
        for i in 0..16usize {
            let depth = table.depth[i];
            if depth != 0 && table.code[i] == self.bit_buffer & ((1u32 << depth) - 1) {
                self.bits(depth as u32)?;
                if i < 2 {
                    return Ok(i as u32);
                }
                return Ok(self.bits(i as u32 - 1)? | (1u32 << (i - 1)));
            }
        }
        Err(RncError::BadHuffmanCode)
    }
}

#[derive(Default)]
struct HufTable {
    code: [u32; 16],
    depth: [u16; 16],
}

fn inverse_bits(mut value: u32, count: u32) -> u32 {
    let mut out = 0u32;
    for _ in 0..count {
        out = (out << 1) | (value & 1);
        value >>= 1;
    }
    out
}

fn unpack_m1(payload: &[u8], unpacked_size: usize) -> Result<Vec<u8>, RncError> {
    let mut s = M1::new(payload);
    let mut out = Vec::with_capacity(unpacked_size);

    // Two header flag bits: "locked" and "needs encryption key".
    if s.bits(1)? != 0 {
        return Err(RncError::Locked);
    }
    if s.bits(1)? != 0 {
        return Err(RncError::Encrypted);
    }

    let mut processed = 0usize;
    while processed < unpacked_size {
        let raw_table = s.read_table()?;
        let len_table = s.read_table()?;
        let pos_table = s.read_table()?;

        let mut subchunks = s.bits(16)?;
        while subchunks > 0 {
            subchunks -= 1;

            let literal_len = s.decode_code(&raw_table)? as usize;
            processed += literal_len;
            if literal_len > 0 {
                for _ in 0..literal_len {
                    let b = s.read_byte()?;
                    out.push(b);
                }
                s.reprime();
            }

            if subchunks > 0 {
                // Reference quirk kept as-is: offsets come from the table
                // named "len", counts from the one named "pos".
                let match_offset = s.decode_code(&len_table)? as usize + 1;
                let match_count = s.decode_code(&pos_table)? as usize + 2;
                processed += match_count;
                copy_from_window(&mut out, match_offset, match_count)?;
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Method 2: bitwise-tagged LZ, bits consumed MSB-first one byte at a time.
// Literal bytes and bit-buffer refills share one cursor, interleaved.
// ---------------------------------------------------------------------------

struct M2<'a> {
    src: &'a [u8],
    pos: usize,
    bit_buffer: u8,
    bit_count: u32,
}

impl<'a> M2<'a> {
    fn new(src: &'a [u8]) -> Self {
        Self {
            src,
            pos: 0,
            bit_buffer: 0,
            bit_count: 0,
        }
    }

    /// Same bounded past-the-end tolerance as `M1::read_byte` (the final
    /// end-of-block marker's trailing bit can need a refill byte that
    /// isn't there).
    fn read_byte(&mut self) -> Result<u8, RncError> {
        if self.pos >= self.src.len() + OVERRUN_LIMIT {
            return Err(RncError::UnexpectedEof);
        }
        let b = self.src.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        Ok(b)
    }

    fn bit(&mut self) -> Result<u32, RncError> {
        if self.bit_count == 0 {
            self.bit_buffer = self.read_byte()?;
            self.bit_count = 8;
        }
        let bit = (self.bit_buffer >> 7) as u32;
        self.bit_buffer <<= 1;
        self.bit_count -= 1;
        Ok(bit)
    }

    fn bits(&mut self, count: u32) -> Result<u32, RncError> {
        let mut bits = 0u32;
        for _ in 0..count {
            bits = (bits << 1) | self.bit()?;
        }
        Ok(bits)
    }

    fn match_count(&mut self) -> Result<usize, RncError> {
        let mut count = self.bit()? as usize + 4;
        if self.bit()? != 0 {
            count = ((count - 1) << 1) + self.bit()? as usize;
        }
        Ok(count)
    }

    fn match_offset(&mut self) -> Result<usize, RncError> {
        let mut offset = 0u32;
        if self.bit()? != 0 {
            offset = self.bit()?;
            if self.bit()? != 0 {
                offset = ((offset << 1) | self.bit()?) | 4;
                if self.bit()? == 0 {
                    offset = (offset << 1) | self.bit()?;
                }
            } else if offset == 0 {
                offset = self.bit()? + 2;
            }
        }
        Ok((((offset << 8) | self.read_byte()? as u32) + 1) as usize)
    }
}

fn unpack_m2(payload: &[u8], unpacked_size: usize) -> Result<Vec<u8>, RncError> {
    let mut s = M2::new(payload);
    let mut out = Vec::with_capacity(unpacked_size);

    if s.bit()? != 0 {
        return Err(RncError::Locked);
    }
    if s.bit()? != 0 {
        return Err(RncError::Encrypted);
    }

    let mut processed = 0usize;
    while processed < unpacked_size {
        loop {
            if s.bit()? == 0 {
                // Literal byte.
                let b = s.read_byte()?;
                out.push(b);
                processed += 1;
                if processed >= unpacked_size {
                    break;
                }
            } else if s.bit()? != 0 {
                if s.bit()? != 0 {
                    let (count, escape);
                    if s.bit()? != 0 {
                        count = s.read_byte()? as usize + 8;
                        escape = count == 8;
                    } else {
                        count = 3;
                        escape = false;
                    }
                    if escape {
                        // End-of-block marker; realign and resume the
                        // outer loop.
                        s.bit()?;
                        break;
                    }
                    let offset = s.match_offset()?;
                    processed += count;
                    copy_from_window(&mut out, offset, count)?;
                } else {
                    let count = 2usize;
                    let offset = s.read_byte()? as usize + 1;
                    processed += count;
                    copy_from_window(&mut out, offset, count)?;
                }
                if processed >= unpacked_size {
                    break;
                }
            } else {
                let count = s.match_count()?;
                if count != 9 {
                    let offset = s.match_offset()?;
                    processed += count;
                    copy_from_window(&mut out, offset, count)?;
                } else {
                    let literal_len = ((s.bits(4)? as usize) << 2) + 12;
                    processed += literal_len;
                    for _ in 0..literal_len {
                        let b = s.read_byte()?;
                        out.push(b);
                    }
                }
                if processed >= unpacked_size {
                    break;
                }
            }
        }
    }
    Ok(out)
}

// CRC-16/ARC table, identical to the reference (and to every standard
// implementation of the 0xA001 reflected polynomial).
#[rustfmt::skip]
const CRC_TABLE: [u16; 256] = [
    0x0000, 0xC0C1, 0xC181, 0x0140, 0xC301, 0x03C0, 0x0280, 0xC241,
    0xC601, 0x06C0, 0x0780, 0xC741, 0x0500, 0xC5C1, 0xC481, 0x0440,
    0xCC01, 0x0CC0, 0x0D80, 0xCD41, 0x0F00, 0xCFC1, 0xCE81, 0x0E40,
    0x0A00, 0xCAC1, 0xCB81, 0x0B40, 0xC901, 0x09C0, 0x0880, 0xC841,
    0xD801, 0x18C0, 0x1980, 0xD941, 0x1B00, 0xDBC1, 0xDA81, 0x1A40,
    0x1E00, 0xDEC1, 0xDF81, 0x1F40, 0xDD01, 0x1DC0, 0x1C80, 0xDC41,
    0x1400, 0xD4C1, 0xD581, 0x1540, 0xD701, 0x17C0, 0x1680, 0xD641,
    0xD201, 0x12C0, 0x1380, 0xD341, 0x1100, 0xD1C1, 0xD081, 0x1040,
    0xF001, 0x30C0, 0x3180, 0xF141, 0x3300, 0xF3C1, 0xF281, 0x3240,
    0x3600, 0xF6C1, 0xF781, 0x3740, 0xF501, 0x35C0, 0x3480, 0xF441,
    0x3C00, 0xFCC1, 0xFD81, 0x3D40, 0xFF01, 0x3FC0, 0x3E80, 0xFE41,
    0xFA01, 0x3AC0, 0x3B80, 0xFB41, 0x3900, 0xF9C1, 0xF881, 0x3840,
    0x2800, 0xE8C1, 0xE981, 0x2940, 0xEB01, 0x2BC0, 0x2A80, 0xEA41,
    0xEE01, 0x2EC0, 0x2F80, 0xEF41, 0x2D00, 0xEDC1, 0xEC81, 0x2C40,
    0xE401, 0x24C0, 0x2580, 0xE541, 0x2700, 0xE7C1, 0xE681, 0x2640,
    0x2200, 0xE2C1, 0xE381, 0x2340, 0xE101, 0x21C0, 0x2080, 0xE041,
    0xA001, 0x60C0, 0x6180, 0xA141, 0x6300, 0xA3C1, 0xA281, 0x6240,
    0x6600, 0xA6C1, 0xA781, 0x6740, 0xA501, 0x65C0, 0x6480, 0xA441,
    0x6C00, 0xACC1, 0xAD81, 0x6D40, 0xAF01, 0x6FC0, 0x6E80, 0xAE41,
    0xAA01, 0x6AC0, 0x6B80, 0xAB41, 0x6900, 0xA9C1, 0xA881, 0x6840,
    0x7800, 0xB8C1, 0xB981, 0x7940, 0xBB01, 0x7BC0, 0x7A80, 0xBA41,
    0xBE01, 0x7EC0, 0x7F80, 0xBF41, 0x7D00, 0xBDC1, 0xBC81, 0x7C40,
    0xB401, 0x74C0, 0x7580, 0xB541, 0x7700, 0xB7C1, 0xB681, 0x7640,
    0x7200, 0xB2C1, 0xB381, 0x7340, 0xB101, 0x71C0, 0x7080, 0xB041,
    0x5000, 0x90C1, 0x9181, 0x5140, 0x9301, 0x53C0, 0x5280, 0x9241,
    0x9601, 0x56C0, 0x5780, 0x9741, 0x5500, 0x95C1, 0x9481, 0x5440,
    0x9C01, 0x5CC0, 0x5D80, 0x9D41, 0x5F00, 0x9FC1, 0x9E81, 0x5E40,
    0x5A00, 0x9AC1, 0x9B81, 0x5B40, 0x9901, 0x59C0, 0x5880, 0x9841,
    0x8801, 0x48C0, 0x4980, 0x8941, 0x4B00, 0x8BC1, 0x8A81, 0x4A40,
    0x4E00, 0x8EC1, 0x8F81, 0x4F40, 0x8D01, 0x4DC0, 0x4C80, 0x8C41,
    0x4400, 0x84C1, 0x8581, 0x4540, 0x8701, 0x47C0, 0x4680, 0x8641,
    0x8201, 0x42C0, 0x4380, 0x8341, 0x4100, 0x81C1, 0x8081, 0x4040,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a valid container around a raw payload.
    fn container(method: u8, unpacked_size: u32, unpacked_crc: u16, payload: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"RNC");
        data.push(method);
        data.extend_from_slice(&unpacked_size.to_be_bytes());
        data.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        data.extend_from_slice(&unpacked_crc.to_be_bytes());
        data.extend_from_slice(&crc16(payload).to_be_bytes());
        data.push(0); // leeway
        data.push(1); // chunks
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn crc16_known_answer() {
        // CRC-16/ARC check value.
        assert_eq!(crc16(b"123456789"), 0xBB3D);
        assert_eq!(crc16(b""), 0x0000);
    }

    #[test]
    fn rejects_non_rnc() {
        assert!(!is_rnc(b"MZ\x90\x00"));
        assert_eq!(decompress(&[0u8; 32]).unwrap_err(), RncError::NotRnc);
    }

    #[test]
    fn rejects_truncated_payload() {
        let mut data = container(2, 2, 0, &[0x00, 0x41, 0x42]);
        data.truncate(data.len() - 1);
        assert_eq!(decompress(&data).unwrap_err(), RncError::Truncated);
    }

    #[test]
    fn detects_packed_crc_mismatch() {
        let mut data = container(2, 2, 0, &[0x00, 0x41, 0x42]);
        let last = data.len() - 1;
        data[last] ^= 0xFF;
        assert!(matches!(
            decompress(&data).unwrap_err(),
            RncError::PackedCrcMismatch { .. }
        ));
    }

    /// Hand-assembled method-2 stream: flag bits 0,0 then two literals.
    /// Bit source and literal bytes share the cursor, so the payload is
    /// one bit-carrier byte (0x00 = both flags clear + two literal tags)
    /// followed by the literal bytes themselves.
    #[test]
    fn m2_literals_roundtrip() {
        let payload = [0b0000_0000, b'A', b'B'];
        let data = container(2, 2, crc16(b"AB"), &payload);
        assert_eq!(decompress(&data).unwrap(), b"AB");
    }

    /// Method-2 back-reference: three literals "ABA", then a 2-byte match
    /// at offset 2 reproducing "BA" → "ABABA".
    ///
    /// Bit stream (MSB-first): 00 (flags) 0 0 0 (literals) 110 (2-byte
    /// match) → carrier byte 0b0000_0110, then literals A, B, A, then the
    /// match's offset byte (offset-1 = 1).
    #[test]
    fn m2_backreference() {
        let payload = [0b0000_0110, b'A', b'B', b'A', 0x01];
        let data = container(2, 5, crc16(b"ABABA"), &payload);
        assert_eq!(decompress(&data).unwrap(), b"ABABA");
    }

    /// Method-1 container with zero unpacked bytes exercises header-flag
    /// parsing and the 16-bit buffer priming without needing hand-built
    /// Huffman tables (real M1 coverage comes from the gamedata
    /// integration test).
    #[test]
    fn m1_empty_stream() {
        let payload = [0x00, 0x00];
        let data = container(1, 0, 0, &payload);
        assert_eq!(decompress(&data).unwrap(), b"");
    }

    #[test]
    fn header_fields_parse() {
        let data = container(2, 2, crc16(b"AB"), &[0b0000_0000, b'A', b'B']);
        let h = parse_header(&data).unwrap();
        assert_eq!(h.method, 2);
        assert_eq!(h.unpacked_size, 2);
        assert_eq!(h.packed_size, 3);
        assert_eq!(h.chunks, 1);
    }
}
