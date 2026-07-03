// Shim header satisfying vendor/Terrain.cpp's `#include "Terrain.h"`
// outside the remc2 tree. Type definitions are copied from remc2's
// global_types.h / axis_3d.h / BasicTerrain.h (GPL-3.0, see
// vendor/PROVENANCE.md); structs referenced but never dereferenced on
// the generation path are stubbed with just the members the compiler
// needs to see.
#pragma once

#include <cstdint>
#include <cstring>
#include <cstdlib>

#define __int16 short
#define __int8 char
#define qmemcpy memcpy

typedef struct {
    uint8_t x;
    uint8_t y;
} baxis_2d;

typedef union {
    baxis_2d _axis_2d;
    uint16_t word;
} uaxis_2d;

typedef struct {
    uint16_t x;
    uint16_t y;
    int16_t z;
} axis_3d;

typedef struct {
    int16_t yaw;
    int16_t pitch;
    int16_t roll;
    int16_t fov;
} axis_4d;

// Minimal stand-ins for entity types referenced only by helper
// functions that the generation path never calls.
typedef struct {
    int16_t word_160_0xc_12;
} type_str_160;

typedef struct {
    axis_4d array_0x52_82;
    type_str_160* dword_0xA0_160x;
} type_entity_0x6E8E;

enum class MapType_t : uint8_t { Day = 0, Night = 1, Cave = 2 };

// Minimal Type_Level_2FECE: exactly the fields GenerateLevelMap_43830
// reads, with the field types from BasicTerrain.h.
typedef struct {
    MapType_t MapType;
    uint16_t seed_0x2FEE5;
    uint16_t offset_0x2FEE9;
    uint16_t raise_0x2FEED;
    uint16_t gnarl_0x2FEF1;
    uint32_t river_0x2FEF5;
    uint16_t lriver_0x2FEF9;
    uint16_t source_0x2FEFD;
    uint16_t snLin_0x2FF01;
    uint16_t snFlt_0x2FF05;
    uint16_t bhLin_0x2FF09;
    uint16_t bhFlt_0x2FF0D;
    uint16_t rkSte_0x2FF11;
} Type_Level_2FECE;

// Stub of the global game-state blob: Terrain.cpp writes .rand_0x8 and
// reads .terrain_2FECE.MapType.
typedef struct {
    uint16_t rand_0x8;
    Type_Level_2FECE terrain_2FECE;
} D41A0_stub;

extern D41A0_stub D41A0_0;
extern bool isCaveLevel_D41B6;

#include "vendor/Unk_D47E0.h"
#include "vendor/Unk_D4A30.h"

extern char building_F2CD0x[2800][2];
extern uint8_t MapBasicHeight_D41B7;
extern uint16_t rand2_17B4E0;
extern uint8_t mapTerrainType_10B4E0[65536];
extern uint8_t mapHeightmap_11B4E0[65536];
extern uint8_t mapShading_12B4E0[65536];
extern uint8_t mapAngle_13B4E0[65536];
extern __int16 mapEntityIndex_15B4E0[65536];
extern uint8_t* x_BYTE_14B4E0_second_heightmap;
extern bool lowDiffHeightmap_D47DC;
// The original engine reuses the VGA screen buffer as scratch memory
// during generation (max index seen: 25 * 2401).
extern uint8_t* pdwScreenBuffer_351628;

// Prototypes as in remc2's Terrain.h; all definitions live in
// vendor/Terrain.cpp.
void GenerateLevelMap_43830(Type_Level_2FECE* a2x);
void sub_B5E70_decompress_terrain_map_level(__int16 a1, unsigned __int16 a2, __int16 a3, int32_t a4);
void sub_44DB0_truncTerrainHeight(int16_t mapEntityIndex_15B4E0[], uint8_t mapHeightmap_11B4E0[]);
int sub_B5C60_getTerrainAlt2(uint16_t a1, uint16_t a2);
void sub_44E40(int a1, uint8_t a2);
void sub_45AA0_setMax4Tiles();
void sub_440D0(unsigned __int16 a1);
void sub_45060(uint8_t a1, uint8_t a2);
void sub_44320();
void sub_45210(uint8_t a1, uint8_t a2);
void sub_454F0(uint8_t a1, uint8_t a2);
void sub_45600(uint8_t a1);
void sub_43FC0();
void sub_43970();
void sub_43EE0();
void sub_44580();
void sub_43B40();
void sub_43D50();
void sub_44D00();
void sub_B5EFA(__int16 a1, uaxis_2d* a2, int32_t a3, int16_t* nextRand);
void sub_B5F8F(__int16 a1, uaxis_2d* a2, int32_t a3, int16_t* nextRand);
void sub_44EE0_smooth_tiles(uaxis_2d a2x);
unsigned int sub_439A0(uint16_t index);
void sub_43BB0();
int sub_1B7A0_tile_compare(axis_3d* a1);
int sub_1B830(axis_3d* a1);
uint8_t sub_45BE0(uint8_t a2, uaxis_2d a3x);
bool sub_33F70(uint16_t inAxis);
void sub_45DC0(uint8_t a2, uaxis_2d a3, unsigned __int8 a4);
void sub_462A0(uaxis_2d a1x, uaxis_2d a2x);
int getTerrainAlt_10C40(axis_3d* a1);
bool sub_11E70(type_entity_0x6E8E* a1, axis_3d* a2);
int sub_10C60(axis_3d* a1);
int sub_B5D68(uint16_t a1, uint16_t a2);
uint32_t sub_10590_terrain_tile_type(char tileType);
signed int sub_104A0(axis_3d* axis3d);
uint32_t sub_104D0_terrain_tile_is_water(axis_3d* axis3d);
