//! Minimal read-only ISO9660 access to the GOG CD images (`game.gog`).
//!
//! Both Magic Carpet discs are plain ISO9660 level-1 filesystems; the
//! MC1 image is "cooked" (2048-byte sectors) while the MC2 image is a
//! raw MODE1 dump (2352-byte sectors, user data at offset 16, followed
//! by redbook audio tracks the filesystem never references). Sector
//! layout is auto-detected from the MODE1 sync pattern.
//!
//! `open` walks the directory tree once and keeps a path → extent map;
//! `read` opens the image per call, so the handle is cheap to share.

use std::collections::BTreeMap;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const COOKED: u32 = 2048;
const RAW: u32 = 2352;
const RAW_DATA_OFFSET: u64 = 16;
const MODE1_SYNC: [u8; 12] = [
    0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00,
];

/// One file inside the image: logical block address and byte size.
#[derive(Clone, Copy, Debug)]
struct Extent {
    lba: u32,
    size: u32,
}

pub struct IsoImage {
    path: PathBuf,
    sector_size: u32,
    data_offset: u64,
    /// Uppercase `DIR/FILE.EXT` path → extent, version suffixes stripped.
    files: BTreeMap<String, Extent>,
}

impl IsoImage {
    pub fn open(path: &Path) -> io::Result<IsoImage> {
        let mut f = std::fs::File::open(path)?;
        let mut sync = [0u8; 12];
        f.read_exact(&mut sync)?;
        let (sector_size, data_offset) = if sync == MODE1_SYNC {
            (RAW, RAW_DATA_OFFSET)
        } else {
            (COOKED, 0)
        };

        let mut iso = IsoImage {
            path: path.to_path_buf(),
            sector_size,
            data_offset,
            files: BTreeMap::new(),
        };
        // Primary volume descriptor at sector 16; the root directory
        // record sits at its offset 156.
        let pvd = iso.read_sectors(&mut f, 16, 2048)?;
        if pvd[0] != 1 || &pvd[1..6] != b"CD001" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: no ISO9660 volume descriptor", path.display()),
            ));
        }
        let root = DirRecord::parse(&pvd[156..])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad root record"))?;
        iso.walk(&mut f, &root, "")?;
        Ok(iso)
    }

    /// Uppercase paths of every file in the image, sorted.
    /// Path of the image file on disk (for consumers that need raw
    /// sector access outside the ISO filesystem — the redbook rip).
    pub fn image_path(&self) -> &Path {
        &self.path
    }

    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    pub fn contains(&self, rel: &str) -> bool {
        self.files.contains_key(&rel.to_ascii_uppercase())
    }

    /// Read one file by its `DIR/FILE.EXT` path (case-insensitive).
    pub fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        let extent = self.files.get(&rel.to_ascii_uppercase()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{}: no {rel} in image", self.path.display()),
            )
        })?;
        let mut f = std::fs::File::open(&self.path)?;
        self.read_sectors(&mut f, extent.lba, extent.size as usize)
    }

    /// Read `len` bytes of user data starting at sector `lba`.
    fn read_sectors(&self, f: &mut std::fs::File, lba: u32, len: usize) -> io::Result<Vec<u8>> {
        if self.sector_size == COOKED {
            f.seek(SeekFrom::Start(lba as u64 * COOKED as u64))?;
            let mut buf = vec![0u8; len];
            f.read_exact(&mut buf)?;
            return Ok(buf);
        }
        let mut buf = Vec::with_capacity(len);
        let mut sector = vec![0u8; COOKED as usize];
        for i in 0..len.div_ceil(COOKED as usize) {
            f.seek(SeekFrom::Start(
                (lba as u64 + i as u64) * self.sector_size as u64 + self.data_offset,
            ))?;
            f.read_exact(&mut sector)?;
            let want = (len - buf.len()).min(COOKED as usize);
            buf.extend_from_slice(&sector[..want]);
        }
        Ok(buf)
    }

    fn walk(&mut self, f: &mut std::fs::File, dir: &DirRecord, prefix: &str) -> io::Result<()> {
        let data = self.read_sectors(f, dir.extent.lba, dir.extent.size as usize)?;
        let mut pos = 0usize;
        while pos < data.len() {
            if data[pos] == 0 {
                // Records never span sectors; a zero length byte pads to
                // the next sector boundary.
                pos = (pos / 2048 + 1) * 2048;
                continue;
            }
            let Some(rec) = DirRecord::parse(&data[pos..]) else {
                break;
            };
            pos += rec.len;
            if rec.name.is_empty() {
                continue; // "." and ".." entries
            }
            let path = if prefix.is_empty() {
                rec.name.clone()
            } else {
                format!("{prefix}/{}", rec.name)
            };
            if rec.is_dir {
                self.walk(f, &rec, &path)?;
            } else {
                self.files.insert(path, rec.extent);
            }
        }
        Ok(())
    }
}

struct DirRecord {
    len: usize,
    extent: Extent,
    is_dir: bool,
    name: String,
}

impl DirRecord {
    fn parse(raw: &[u8]) -> Option<DirRecord> {
        let len = *raw.first()? as usize;
        if len < 34 || raw.len() < len {
            return None;
        }
        let lba = u32::from_le_bytes(raw[2..6].try_into().unwrap());
        let size = u32::from_le_bytes(raw[10..14].try_into().unwrap());
        let name_len = raw[32] as usize;
        let name_raw = raw.get(33..33 + name_len)?;
        // "\0"/"\x01" are the . and .. self-references; real names drop
        // the ";1" version suffix and any trailing dot.
        let name = if name_raw == [0] || name_raw == [1] {
            String::new()
        } else {
            String::from_utf8_lossy(name_raw)
                .split(';')
                .next()
                .unwrap_or_default()
                .trim_end_matches('.')
                .to_ascii_uppercase()
        };
        Some(DirRecord {
            len,
            extent: Extent { lba, size },
            is_dir: raw[25] & 2 != 0,
            name,
        })
    }
}
