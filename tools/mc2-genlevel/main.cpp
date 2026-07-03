// mc2-genlevel — standalone Magic Carpet 2 terrain generation oracle.
//
// Runs the original (reverse-engineered) generation algorithm, carved
// verbatim out of remc2 (see vendor/PROVENANCE.md), over one decompressed
// MC2 level and writes the resulting terrain arrays.
//
// Usage: mc2-genlevel <level.bin> <out.bin>
//   <level.bin>  one decompressed LEVELS.DAT entry (26,116 bytes)
//   <out.bin>    0x70000-byte output, laid out like the engine's terrain
//                block (matching remc2's regression memimage layout):
//                  +0x00000  terrain type   (256x256 u8)
//                  +0x10000  heightmap      (256x256 u8)
//                  +0x20000  shading        (256x256 u8)
//                  +0x30000  angle          (256x256 u8)
//                  +0x40000  zeros          (array not produced here)
//                  +0x50000  entity index   (256x256 i16 LE)

#include "Terrain.h"

#include <cstdio>
#include <cstdlib>

D41A0_stub D41A0_0;
bool isCaveLevel_D41B6 = false;
static uint8_t second_heightmap_storage[65536];
uint8_t* x_BYTE_14B4E0_second_heightmap = second_heightmap_storage;
// Generation-time scratch (the engine reuses its screen buffer here).
static uint8_t scratch_screen_buffer[25 * 2401 + 4096];
uint8_t* pdwScreenBuffer_351628 = scratch_screen_buffer;

static const size_t LEVEL_SIZE = 26116;

static uint16_t u16le(const uint8_t* p) { return (uint16_t)(p[0] | (p[1] << 8)); }
static uint32_t u32le(const uint8_t* p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

int main(int argc, char** argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: mc2-genlevel <level.bin> <out.bin>\n");
        return 2;
    }

    uint8_t level[LEVEL_SIZE];
    FILE* in = fopen(argv[1], "rb");
    if (!in || fread(level, 1, LEVEL_SIZE, in) != LEVEL_SIZE) {
        fprintf(stderr, "error: cannot read %zu bytes from %s\n", LEVEL_SIZE, argv[1]);
        return 1;
    }
    fclose(in);

    if (u16le(level + 0x00) != 2) {
        fprintf(stderr, "error: level version %u != 2\n", u16le(level + 0x00));
        return 1;
    }

    Type_Level_2FECE lvl;
    memset(&lvl, 0, sizeof(lvl));
    uint8_t map_type = level[0x06];
    lvl.MapType = map_type == 2   ? MapType_t::Cave
                  : map_type == 1 ? MapType_t::Night
                                  : MapType_t::Day;
    lvl.seed_0x2FEE5 = u16le(level + 0x17);
    lvl.offset_0x2FEE9 = u16le(level + 0x1B);
    lvl.raise_0x2FEED = u16le(level + 0x1F);
    lvl.gnarl_0x2FEF1 = u16le(level + 0x23);
    lvl.river_0x2FEF5 = u32le(level + 0x27);
    lvl.lriver_0x2FEF9 = u16le(level + 0x2B);
    lvl.source_0x2FEFD = u16le(level + 0x2F);
    lvl.snLin_0x2FF01 = u16le(level + 0x33);
    lvl.snFlt_0x2FF05 = u16le(level + 0x37);
    lvl.bhLin_0x2FF09 = u16le(level + 0x3B);
    lvl.bhFlt_0x2FF0D = u16le(level + 0x3F);
    lvl.rkSte_0x2FF11 = u16le(level + 0x43);

    isCaveLevel_D41B6 = (map_type == 2);
    D41A0_0.terrain_2FECE = lvl;

    GenerateLevelMap_43830(&lvl);

    FILE* out = fopen(argv[2], "wb");
    if (!out) {
        fprintf(stderr, "error: cannot open %s\n", argv[2]);
        return 1;
    }
    fwrite(mapTerrainType_10B4E0, 1, 0x10000, out);
    fwrite(mapHeightmap_11B4E0, 1, 0x10000, out);
    fwrite(mapShading_12B4E0, 1, 0x10000, out);
    fwrite(mapAngle_13B4E0, 1, 0x10000, out);
    fwrite(x_BYTE_14B4E0_second_heightmap, 1, 0x10000, out);
    fwrite(mapEntityIndex_15B4E0, 2, 0x10000, out); // little-endian host
    fclose(out);
    return 0;
}
