//! MC1 load-time terrain features ("GenerateFeatures").
//!
//! Port of remc1's `GenerateFeatures_36430_367F0` (sub_main.cpp:43043):
//! the entity-driven post-generation phase that carves craters and
//! canyons, raises walls and ridges, paints tracks, and flattens/paints
//! building footprints into the pristine generated terrain. Baked
//! `.mgcl` terrain stays pristine by design (docs/FORMAT.md); the
//! engine applies these modifications at level load from `things.json`.
//!
//! Machinery (line references are remc1 sub_main.cpp):
//!
//! - Level entities with `class == 10 && dis_id == 0xFFFF` are terrain
//!   features, consumed in slot order 1..1999. Chained models (28
//!   walls, 29 tracks, 31 canyons, 50 ridges, with `swi_id != 0` as
//!   the not-yet-processed flag) run a polyline walker (sub_362C0,
//!   :42972): root-first via `parent` links, then one segment function
//!   per parent→child pair. Everything else spawns a runtime *event*
//!   through its per-model creator (`off_97D12`, :5075); model 45
//!   (building) additionally gets the footprint fix-up sub_36DF0.
//! - The event loop (sub_36620, :43181) then sweeps the 1000-slot
//!   event pool to fixpoint: craters dig ring by ring, canyon heads
//!   walk and spawn diggers, buildings flatten and paint over 30
//!   ticks, and every non-feature event is purged. Dispatch is by the
//!   entity's byte-70 tick index, not its model.
//! - Determinism: the pool allocates slots 1,2,3,… (free stack built
//!   999→1; frees push back LIFO), and each event seeds a per-entity
//!   LCG from `slot + global_rand`. Two behaviors depend on the slot
//!   number itself: digger radius growth (`slot % 3`, sub_25670) and
//!   dither draws — so slot churn from events that are spawned only to
//!   be purged is load-bearing and reproduced exactly.
//! - PRNG streams (all `x = 9377x + 9439`): the global u32 `rand_4` is
//!   the level seed at scan time and is advanced exactly once at event
//!   loop entry; retiling draws the u16 `pseudoRand` stream whose
//!   post-generation state is replayed from the height plane
//!   ([`post_generation_pseudo_rand`], the generator's shading pass
//!   reset it to 0 and drew once per flat tile).
//!
//! Deliberately omitted (terrain-neutral at load): damage broadcasts
//! (sub_127E0/sub_120B0 — they write damage fields on pool entities;
//! relevant once entities persist), sounds, and the surviving building
//! entities themselves (the entity track will need them; the terrain
//! effect is complete without).
//!
//! Entity-table indices: `things.json` slots are 0-based file order;
//! the engine indexes the same records 1-based (its record 1 = file
//! offset 0x442 = our slot 0), and `parent`/`child` values are those
//! 1-based indices. The pass rebuilds the 1-based table.

use crate::mc1::corners;
use crate::mc1::tables::{ATAN, BIT_SQRT, COS, PAINT_AC, PAINT_BC, PAINT_EC, PAINT_FC, SIN};
use mgc_formats::Thing;

use crate::chassis::{ChassisParams, RandWidth};
use crate::verbs::{VerbKind, VerbSet};

/// Cells in the 256x256 terrain grid.
const GRID: usize = 0x10000;

// THING-table capacity is chassis data (ChassisParams::
// level_table_slots); the feature/disposition scans are len-driven.
// Runtime pool size lives in chassis::ChassisParams::pool_slots
// (slot 0 never allocated); sizing/iteration read `ent.len()`.

/// The four terrain planes the feature pass mutates, engine layout
/// (index = tile_y * 256 + tile_x).
pub struct TerrainPlanes<'a> {
    pub height: &'a mut [u8],
    pub tile_type: &'a mut [u8],
    pub shading: &'a mut [u8],
    pub angle: &'a mut [u8],
}

/// Owned form of the terrain planes — what the runtime world keeps and
/// mutates across ticks (`mgc_sim::world`).
#[derive(Clone)]
pub struct Planes {
    pub height: Vec<u8>,
    pub tile_type: Vec<u8>,
    pub shading: Vec<u8>,
    pub angle: Vec<u8>,
    /// MC2 cave second heightmap (`x_BYTE_14B4E0`): the CEILING, world
    /// height = 32 * value like the floor. EMPTY everywhere except MC2
    /// cave levels (retail's `sub_43D50` never writes it off-cave) —
    /// and hash-transparent when empty, so the MC1/MC2 non-cave golden
    /// streams are unchanged by the field. On caves, `angle` bit 3
    /// means SEALED rock (ceiling pinned to floor−1) — the OPPOSITE of
    /// its non-cave open-sea meaning. Trace:
    /// docs/traces/mc2-cave-terrain-foundation.md.
    pub ceiling: Vec<u8>,
}

impl std::hash::Hash for Planes {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Planes {
            height,
            tile_type,
            shading,
            angle,
            ceiling,
        } = self;
        height.hash(state);
        tile_type.hash(state);
        shading.hash(state);
        angle.hash(state);
        // Hash-when-present (the FeatureAssets pattern): empty =
        // absent, not "a zero-length plane".
        if !ceiling.is_empty() {
            ceiling.hash(state);
        }
    }
}

/// One building-footprint entry from `BUILD?-0.TAB` (6 bytes on disk:
/// u32 offset into the DAT blob, u8 width, u8 height in tiles).
#[derive(Clone, Copy, Hash)]
pub struct BuildDef {
    pub offset: u32,
    pub w: u8,
    pub h: u8,
}

/// One MC2 `BLDGPRM.DAT` record (4 bytes; remc2
/// Type_D93C0_Bldgprmbuffer.h + loader sub_539A0 :38319): production
/// rate, flag bits (0x10 = GenerateEvents pass F/G split, 8 = no
/// mana/production, 4 = no cave second-heightmap raise, 1 =
/// enterable), and the objective-chain / font index byte.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct BldgParam {
    pub rate: u16,
    pub flags: u8,
    pub chain: u8,
}

/// Parsed game data the feature pass needs: the SEARCH.DAT ring table
/// and the building footprint RLE maps. `bldgprm` = MC2's building
/// parameter table, `spells` = MC2's SPELLS.DAT (both empty on MC1 —
/// and hash-transparent when empty, so the MC1 goldens' hash stream
/// is unchanged by the fields).
#[derive(Clone)]
pub struct FeatureAssets {
    /// Per ring 0..31: (dx, dy) byte deltas from the dig center, in the
    /// original's row-major emission order (sub_11540, :16784).
    pub rings: Vec<Vec<(u8, u8)>>,
    pub build_tab: Vec<BuildDef>,
    pub build_dat: Vec<u8>,
    pub bldgprm: Vec<BldgParam>,
    /// MC2's spell table ([`crate::mc2::spells`]): the par1-authored
    /// class-10 overrides + class-15 cast costs.
    pub spells: Vec<crate::mc2::spells::Mc2SpellRow>,
    /// MC2's DERIVED sprite-extent pairs (speed_6, rotSpeed_8) per
    /// particle-param row ([`crate::mc2::derive_sprite_extents`] —
    /// retail computes these at load from the sprite bitmaps,
    /// EF:44870-44910). Empty = pre-dims caller → the static table's
    /// raw values stand (the old zero-box behavior).
    pub mc2_sprite_ext: Vec<(u16, u16)>,
}

impl std::hash::Hash for FeatureAssets {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let FeatureAssets {
            rings,
            build_tab,
            build_dat,
            bldgprm,
            spells,
            mc2_sprite_ext,
        } = self;
        rings.hash(state);
        build_tab.hash(state);
        build_dat.hash(state);
        // Only when present — an absent table hashes exactly like the
        // pre-field struct (MC1 goldens hold).
        if !bldgprm.is_empty() {
            bldgprm.hash(state);
        }
        if !spells.is_empty() {
            spells.hash(state);
        }
        if !mc2_sprite_ext.is_empty() {
            mc2_sprite_ext.hash(state);
        }
    }
}

impl FeatureAssets {
    /// `search` = decompressed SEARCH.DAT (1024 bytes, 32x32 ring-index
    /// grid); `build_tab`/`build_dat` = decompressed BUILD?-0.TAB/DAT.
    pub fn parse(search: &[u8], build_tab: &[u8], build_dat: &[u8]) -> Result<Self, String> {
        if search.len() != 1024 {
            return Err(format!(
                "search grid: expected 1024 bytes, got {}",
                search.len()
            ));
        }
        // Center = the first value-0 cell in row-major scan; ring j's
        // entries are all value-j cells in the same scan order.
        let c = search
            .iter()
            .position(|&v| v == 0)
            .ok_or("search grid has no ring-0 cell")?;
        let (cx, cy) = ((c % 32) as u8, (c / 32) as u8);
        let mut rings = vec![Vec::new(); 32];
        for (j, ring) in rings.iter_mut().enumerate() {
            for y in 0..32u8 {
                for x in 0..32u8 {
                    if search[y as usize * 32 + x as usize] == j as u8 {
                        ring.push((x.wrapping_sub(cx), y.wrapping_sub(cy)));
                    }
                }
            }
        }
        if build_tab.len() % 6 != 0 {
            return Err(format!(
                "build tab: {} bytes is not 6-byte entries",
                build_tab.len()
            ));
        }
        let tab: Vec<BuildDef> = build_tab
            .chunks_exact(6)
            .map(|e| BuildDef {
                offset: u32::from_le_bytes(e[0..4].try_into().unwrap()),
                w: e[4],
                h: e[5],
            })
            .collect();
        for (i, b) in tab.iter().enumerate() {
            if (b.offset as usize) >= build_dat.len() && (b.w != 0 || b.h != 0) {
                return Err(format!("build tab entry {i} offset {} past dat", b.offset));
            }
        }
        Ok(Self {
            rings,
            build_tab: tab,
            build_dat: build_dat.to_vec(),
            bldgprm: Vec::new(),
            spells: Vec::new(),
            mc2_sprite_ext: Vec::new(),
        })
    }

    /// Attach MC2's `BLDGPRM.DAT` table (4-byte records; the loader
    /// reads 76 x 4 of the 77-record file, sub_539A0 :38319 — we take
    /// every whole record present).
    pub fn with_bldgprm(mut self, bytes: &[u8]) -> Self {
        self.bldgprm = bytes
            .chunks_exact(4)
            .map(|r| BldgParam {
                rate: u16::from_le_bytes([r[0], r[1]]),
                flags: r[2],
                chain: r[3],
            })
            .collect();
        self
    }

    /// Attach MC2's `SPELLS.DAT` table (`spells.bin`, 26 x 80 bytes;
    /// [`crate::mc2::spells::parse`]). A malformed blob is a bake bug
    /// — surface it instead of silently running on ctor defaults.
    /// Retail's LevelInit.cpp:12-21 patch of rows 4 and 19 (Day vs
    /// non-Day, tier-0 life + hintText) is applied later, by
    /// `World::set_mc2_night_shade` — the seam that declares the
    /// level's environment ([`crate::mc2::spells::level_init_patch`]).
    pub fn with_spells(mut self, bytes: &[u8]) -> Result<Self, String> {
        self.spells = crate::mc2::spells::parse(bytes)?;
        Ok(self)
    }

    /// Attach the derived MC2 sprite extents (the retail load-time
    /// pass over the sprite bitmaps — feed
    /// [`crate::mc2::derive_sprite_extents`] with the baked sprite
    /// index dims).
    pub fn with_mc2_sprite_ext(mut self, ext: Vec<(u16, u16)>) -> Self {
        self.mc2_sprite_ext = ext;
        self
    }
}

/// The engine's LCG, 32-bit state (`rand_4` and per-entity streams).
#[inline]
pub(crate) fn lcg32(s: &mut u32) -> u32 {
    *s = s.wrapping_mul(9377).wrapping_add(9439);
    *s
}

/// Tile index from u8 coordinates (low byte = x, high byte = y).
#[inline]
pub(crate) fn tile(x: u8, y: u8) -> usize {
    ((y as usize) << 8) | x as usize
}

#[inline]
fn tx(t: usize) -> u8 {
    t as u8
}
#[inline]
fn ty(t: usize) -> u8 {
    (t >> 8) as u8
}
/// Move a packed tile index by wrapping each byte axis independently.
#[inline]
fn step(t: usize, dx: i32, dy: i32) -> usize {
    tile(tx(t).wrapping_add(dx as u8), ty(t).wrapping_add(dy as u8))
}

/// Replay the generator's final shading pass on the pristine height
/// plane to recover the u16 `pseudoRand` state at GenerateFeatures
/// time (the pass reset the stream to 0, then drew once per flat cell
/// — `sub_329C0`, mirrored by mc1_terrain's `shading_pass`).
pub fn post_generation_pseudo_rand(height: &[u8]) -> u16 {
    let mut s = 0u16;
    for i in 0..=0xFFFFu16 {
        let hi = height[step(i as usize, -1, -1)];
        let lo = height[step(i as usize, 1, 1)];
        if hi.wrapping_sub(lo).wrapping_add(32) == 32 {
            s = s.wrapping_mul(9377).wrapping_add(9439);
        }
    }
    s
}

/// One record of the original 18-byte THING_INIT table (1-based copy).
/// The runtime world keeps this table live: dispositions scan it and
/// one-shot spawns zero the class (`sub_37440_37800`).
#[derive(Clone, Copy, Default)]
pub(crate) struct Rec {
    pub(crate) class: u16,
    pub(crate) model: u16,
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) dis_id: u16,
    /// Switch size (`data_10`): trigger volume radius in tiles.
    pub(crate) swi_sz: u16,
    pub(crate) swi_id: u16,
    pub(crate) parent: u16,
    pub(crate) child: u16,
    /// MC2 `par3_18` (the third context parameter; 0 on MC1 records) —
    /// the cave pit/hill depth seed and the tube-carver radius nibble.
    pub(crate) par3: u16,
}

impl std::hash::Hash for Rec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // par3 is STATIC level input (never mutated at runtime, unlike
        // class/swi_id) — excluded so plumbing it into Rec left every
        // pinned MC2 state-hash golden untouched.
        let Rec {
            class,
            model,
            x,
            y,
            dis_id,
            swi_sz,
            swi_id,
            parent,
            child,
            par3: _,
        } = self;
        class.hash(state);
        model.hash(state);
        x.hash(state);
        y.hash(state);
        dis_id.hash(state);
        swi_sz.hash(state);
        swi_id.hash(state);
        parent.hash(state);
        child.hash(state);
    }
}

/// Runtime event entity — the subset of remc1's 164-byte
/// `Type_AE400_29795` the load-time feature path uses. Names keep the
/// original byte offsets for traceability.
#[derive(Clone, Copy, Default, Hash)]
pub(crate) struct Ent {
    /// Per-entity LCG (offset 4), seeded `slot + global_rand` at alloc.
    pub(crate) rand: u32,
    pub(crate) max_life: u32,
    pub(crate) act_life: i32,
    /// Flags (offset 16). Bit 0 (0x1) = active, bit 1 (0x2) =
    /// dug/second-phase, bit 2 (0x4) = linked into the tile map,
    /// bit 10 (0x400) = marked dead.
    pub(crate) flags: u32,
    pub(crate) next20: u16,
    pub(crate) prev22: u16,
    /// The disposition this event fires / entity link (offset 24, from
    /// the THING's `swi_id`). NewEvent defaults it to the OWN slot —
    /// for projectiles/effects the cast/thunk overwrites it with the
    /// caster's id, and +24 equality is the engine's only friendly-
    /// fire rule (owner immunity).
    pub(crate) id24: u16,
    /// Killer id latch (offset 38) and attacker latch (offset 40) —
    /// written by the damage inbox block, read by DEATH's kill credit
    /// and the aggro retarget.
    pub(crate) f38: u16,
    pub(crate) f40: u16,
    /// Vertical velocity (offset 46): mana-ball gravity, fire flicker.
    pub(crate) f46: i16,
    /// Damage-response countdown (offset 50): a blast near a castle
    /// arms 30 ticks (sub_127E0 :17522); expiry sends the castle to
    /// the repaint sub-state (:55987-93). The downgrade arms 5.
    pub(crate) f50: i16,
    /// Explosion class/model a projectile detonates into (offsets
    /// 68/69). NewEvent defaults +68 = 10 (:43879), +69 = 0 (fire).
    pub(crate) f68: u8,
    pub(crate) f69: u8,
    /// Damage mailboxes (offsets 90..124): six {u32 amount, u16
    /// source-id} channels. ch0 = physical damage, ch1 = mana-ball
    /// claim, ch3 = mana steal, ch4 = grip/attract, ch5 = balloon
    /// recall. Writers accumulate while a source is pending and
    /// overwrite stale amounts (readers clear the source but NOT the
    /// amount — :17301-05).
    pub(crate) mail: [(u32, u16); 6],
    /// Mana-ball owner (offset 144): the wizard whose collection claim
    /// (ch1) tagged the ball; corpses pass theirs to the dropped ball.
    pub(crate) f144: u16,
    /// Generic counter (offset 26): crater ring counter, wall run
    /// length, trigger rearm/debounce countdown.
    pub(crate) f26: i16,
    pub(crate) f28: u16,
    /// Wall step dx/dy (offsets 30/32); canyon/ridge heading (30).
    pub(crate) f30: u16,
    pub(crate) f32: u16,
    /// Strength (offset 44).
    pub(crate) f44: u16,
    /// Target yaw (offset 34, 11-bit engine angle; high byte = pitch
    /// for fliers) and its offset-36 companion (zeroed at spawn).
    pub(crate) f34: u16,
    pub(crate) f36: u16,
    /// Multipart chain links (offsets 52/54): +52 = toward the head
    /// (the segment's leader), +54 = toward the tail. 0 = end.
    pub(crate) f52: u16,
    pub(crate) f54: u16,
    /// Segment follow distance (offset 56, engine units).
    pub(crate) f56: u16,
    /// Awake countdown (offset 58): >0 = the creature acts (damage
    /// intake, hostile scans, segment follow); decremented by the
    /// pre-pass, re-armed to 16 (segments 18) while the player is
    /// within 24 tiles. Spawn staggers the initial value by the spawn
    /// ordinal. NewEvent default 0xFA.
    pub(crate) f58: i16,
    /// Awake re-probe delay (offset 59).
    pub(crate) f59: u8,
    /// Slot index at alloc (offset 63); the RUNTIME loop increments it
    /// per tick (:52417) — gates digger radius growth (`% 3`) and the
    /// trigger probe throttle (`& 7`). The load-time fixpoint loop
    /// never increments, so there it stays the alloc slot. Creature
    /// spawns overwrite it with the per-model spawn ordinal.
    pub(crate) f63: u8,
    pub(crate) class64: u8,
    pub(crate) model65: u8,
    /// Team/owner (offset 66; creatures spawn as 3 = wild) and its
    /// offset-67 companion. NewEvent defaults both to 0xFF.
    pub(crate) f66: u8,
    pub(crate) f67: u8,
    /// Tick-handler index (offset 70).
    pub(crate) tick70: u8,
    /// Building-table index (offset 71).
    pub(crate) f71: u8,
    /// Position, 8.8 fixed point (offsets 72/74/76).
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) z: i16,
    /// Sprite half-height (offset 78, set with the extents by
    /// `sub_36FA0` from the stats row).
    pub(crate) f78: u16,
    /// Extents (offsets 80/82/84); high byte of f80 = dig radius in tiles.
    pub(crate) f80: u16,
    pub(crate) f82: u16,
    pub(crate) f84: u16,
    /// Sprite-stats type index (offset 86), animation frame (88) and
    /// frame count (89) — what the billboard layer draws.
    pub(crate) type86: u16,
    pub(crate) frame88: u8,
    pub(crate) frames89: u8,
    /// Advance per tick (offset 126); building area>>4 (offset 128).
    /// For creatures +126 is the actual speed toward max speed +128
    /// with acceleration +130 (engine units per tick, 8.8).
    pub(crate) f126: i16,
    pub(crate) f128: i16,
    pub(crate) f130: i16,
    /// Mana pool / per-tick mana (offsets 136/140; the mana track
    /// consumes these — carried for faithful spawn state).
    pub(crate) f136: i32,
    pub(crate) f140: i32,
    /// Chase target (offset 146): pool slot of the hunted entity;
    /// [`crate::mc1::mobs::PLAYER_TARGET`] = the player's carpet.
    pub(crate) f146: u16,
    /// Behavior row index into [`crate::mc1::behavior::BEHAVIOR`]
    /// (offset 156 holds `&unk_98F38[N]` in the original).
    pub(crate) row156: u8,
    /// Source THING table index (1-based; ours, not original layout) —
    /// lets the app resolve spawned drawables through the per-slot
    /// spawn-RNG approximation. 0 = not from a THING.
    pub(crate) thing_slot: u16,
    /// Teleport destination (offsets 150/152, 8.8 fixed) — the portal's
    /// target; defaults to its own position, overwritten by the THING
    /// post-init (child/parent fields).
    pub(crate) dest_x: u16,
    pub(crate) dest_y: u16,
    /// Build-site z (offset 154): the castle's painter/leveler datum
    /// — distinct from the live entity z (+76), which tracks the
    /// ground under the flag every tick.
    pub(crate) site_z: i16,
}

/// Pending MC2 player debuff-stamp hits (slow webs, paralyze webs).
/// Manual Hash: contributes to the state hash ONLY while hits are
/// pending, so goldens pinned before the channel existed are
/// untouched (the Planes ceiling / Rec par3 discipline).
#[derive(Default)]
pub(crate) struct Mc2PlayerDebuffs {
    pub(crate) slow: u8,
    pub(crate) stun: u8,
}

impl std::hash::Hash for Mc2PlayerDebuffs {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if self.slow != 0 || self.stun != 0 {
            // Field tag: keeps this pair from aliasing a neighboring
            // conditional contribution of the same width (review J2).
            state.write_u8(6);
            (self.slow, self.stun).hash(state);
        }
    }
}

/// The event-pool engine: terrain planes + the original's 1000-slot
/// event pool and PRNG streams. Serves both the load-time feature pass
/// (fixpoint loop, this module) and the runtime world tick
/// (`mgc_sim::world`, one pass per turn) — in the original these are
/// the same pool and the same handlers.
#[derive(Hash)]
pub(crate) struct Gen {
    pub(crate) t: Planes,
    pub(crate) assets: FeatureAssets,
    /// `byte_B5D40`: 2401 x {texture, orientation bits} retile table.
    pub(crate) retile: Vec<[u8; 2]>,
    /// Per-tile head of the event intrusive list (`mapEntityIndex`).
    pub(crate) map_entity: Vec<u16>,
    pub(crate) ent: Vec<Ent>,
    /// Per-slot spawn generation (see [`SlotGens`]) — presentation
    /// identity across snapshots, hash-silent always.
    pub(crate) slot_gen: SlotGens,
    /// Free stack; built 999→1 so allocation pops 1, 2, 3, …
    pub(crate) free: Vec<u16>,
    /// Global LCG (`rand_4`), = the level seed at scan time.
    pub(crate) rand: u32,
    /// Terrain-retile LCG (`pseudoRand`), u16 stream.
    pub(crate) pseudo: u16,
    /// Per-model spawn ordinals (`str_AE400+12+model`, Type_AE400_20
    /// str_12): creature spawns record the old value into +63 and
    /// increment; model-7 sprite alternation keys off its parity.
    pub(crate) spawn_count: [u8; 20],
    /// The human player's damage inbox — the player lives outside the
    /// pool ([`crate::mc1::mobs::PLAYER_TARGET`]), so writers land here.
    /// The invincible-player dev mode discards it every tick like the
    /// original's spawn grace (:55367-71), accumulating the totals.
    pub(crate) player_mail: [(u32, u16); 6],
    /// Total ch0 damage the (invincible) player has absorbed.
    pub(crate) player_damage: u64,
    /// `gamedata+36` / `gamedata+38` (sub_25EC0): the currently
    /// erupting volcano's pool slot and its (10,19) plume's slot —
    /// 0 = none. One volcano erupts at a time; a driver that dies
    /// unclean leaves the register pointing at itself (authentic
    /// quirk: no volcano can re-arm until a clean death clears it).
    pub(crate) erupting: u16,
    pub(crate) plume: u16,
    /// The player's knock/buffet fields (Type_160 v_24 direction /
    /// v_22 magnitude, :23225-28 kraken writer, :55204-218 consumer):
    /// per-tick horizontal displacement forced onto the carpet.
    /// DIRECT struct writes in the original — spawn grace does NOT
    /// wipe them, so even the invincible dev player gets dragged.
    pub(crate) player_knock: (u16, i16),
    /// Pending MC2 debuff-stamp hits on the player — (10,65) slow
    /// web / (10,66) paralyze web (`sub_38E70`/`sub_38F70`
    /// EF:28407/28442) — drained into the flight `Mc2Ext` channels
    /// by the sim boundary each tick (docs/traces/mc2-flight-model.md
    /// §5c/5d). Hash-only-when-pending (the Planes pattern): the
    /// zero state contributes nothing, so every golden pinned before
    /// the channel existed stands.
    pub(crate) mc2_debuffs: Mc2PlayerDebuffs,
    /// Rival wizard entity by player slot (0 = none; slot 0 = the
    /// human, unused) — the sprite-family team resolver for owner
    /// recolors (mana balls 105+8·team, balloons 169+team, castle
    /// flags 177+team). Maintained by the rival spawn/respawn path;
    /// claims of an eliminated wizard keep their color (property
    /// persists).
    pub(crate) rival_ents: [u16; 8],
    /// Per-color MC2 Life scalar for the castle-HP ladder (see
    /// [`Mc2LifeScale`]); written by the MC2 rival spawn.
    pub(crate) mc2_life_scale: Mc2LifeScale,
    /// The human player's village-aggro timer (the wizard struct's
    /// +528): set to 200 by offenses against village property or
    /// population (building hits, villager-family hits and kills),
    /// decremented once per world tick (:55405-06). m4 militia only
    /// hunt a wizard whose timer is live — the hostility gate.
    pub(crate) player_aggro: i16,
    /// The player's Invisible cloak (spell 12; the wizard's +16 0x20
    /// bit, :65689-90) mirrored in for the mob-side target gates.
    pub(crate) player_invisible: bool,
    /// The player's Rebound deflection bit (spell 14; +17 0x80,
    /// :65774) — incoming class-9 projectiles bounce back.
    pub(crate) player_rebound: bool,
    /// Player stat counters: creatures killed (`Type_160+359`), shots
    /// resolved (+343), shots that struck the aimed target (+347).
    pub(crate) kills: u32,
    pub(crate) shots: u32,
    pub(crate) hits: u32,
    /// The wizard's danger-music countdown (Type_160 v_46): armed to
    /// 100 by processed hits (sub_46540's blocks call sub_46520) and
    /// by a projectile acquiring the player as target (:64013); the
    /// player tick decrements it and switches the music mode on
    /// v_46 > 0 (:55282-92 → sub_20D00).
    pub(crate) player_danger: i16,
    /// The claimed-house mana tally (wizext u32_308), stashed by the
    /// per-tick census — the castle overflow ejector's trigger reads
    /// houses + stored vs capacity (sub_47130 :56185-89).
    pub(crate) banked_houses: i32,
    /// "Castle under attack" HUD flash (Type_160+391 = 4, :56698) —
    /// armed by every processed castle hit, decremented per tick.
    pub(crate) castle_alert: u8,
    /// "You are being attacked" HUD flash (Type_160+392 = 4,
    /// :55679/:55692/:55723) — armed by every processed player hit /
    /// steal / grip, decremented per tick. The SELF sub-panel's alert.
    pub(crate) player_alert: u8,
    /// Balloon-under-attack HUD flash (Type_160+393 = 4, :56826) —
    /// armed by a processed hit on an own balloon, decremented per
    /// tick. The balloon sub-panel's alert.
    pub(crate) balloon_alert: u8,
    /// Allocations dropped on pool exhaustion (the limit-removing
    /// register's telemetry; the app logs increases). The original
    /// keeps no such count — it is observability, not behavior.
    pub(crate) exhausted: u32,
    /// The per-game chassis constant set ([`crate::chassis`]); fixed
    /// at construction, never rebranched on.
    pub(crate) chassis: ChassisParams,
    /// The per-game tier-5 verb column ([`crate::verbs`]); fixed at
    /// construction. Branched on ONLY at the dispatch seams — never
    /// inside a handler.
    pub(crate) verbs: VerbSet,
    /// Bitmask of [`crate::verbs::VerbKind`]s whose requested arm is
    /// pending and fell back to MC1 (seam telemetry, noted once each;
    /// the app/tests read it via `World::verb_fallbacks`).
    pub(crate) verb_fallbacks: u8,
    /// Unknown `(class, model, count)` things the spawn seam refused
    /// (graceful degradation's ledger; the original has no analogue —
    /// observability, not behavior).
    pub(crate) misfits: Vec<(u16, u16, u32)>,
    /// Sound requests emitted this tick at the original's
    /// sub_55370_558A0 call sites; drained by the app into the audio
    /// mixer (which reimplements that routine's attenuation/slot
    /// policy). Position/tag mirror the entity the original passed.
    pub(crate) sounds: Vec<SoundEvent>,
    /// Terrain changed inside a Gen-internal path with no dirty-
    /// returning dispatch arm (the castle downgrade's synchronous
    /// un-stamp collapse); World::tick merges + clears per turn.
    /// Playtest-8: the final destruction left the tower ON SCREEN —
    /// the sim flattened it but nothing re-uploaded the terrain.
    pub(crate) terrain_dirty: bool,
    /// MC2 non-day shading: `sub_462A0` inverts the relief shade on
    /// Night/Cave maps (remc2 Terrain.cpp:2030-2033). Per-LEVEL, set
    /// by the app from the level's environment. Hash-transparent when
    /// off so the MC1 golden hash stream is unchanged by the field.
    pub(crate) mc2_night_shade: NightShade,
    /// MC2 per-model spawn ordinals (`D41A0_0.array_0x10[model]++`,
    /// remc2 EventsFunctions.cpp per-ctor) — the per-instance phase
    /// stagger every MC2 class-5 ctor stores into byte_0x3E_62 (our
    /// f63). Separate from MC1's `spawn_count` (its own column) and
    /// hash-transparent while untouched so the MC1 golden stream is
    /// unchanged by the field.
    pub(crate) mc2_spawn_ord: Mc2Ord,
    /// m26's mana leech against the HUMAN accumulates here (remc2
    /// EF:19331-34 drains the target wizard's mana; the MC2
    /// wizard-mana ledger consumes this when it lands). Pool wizards
    /// are debited directly. Hash-transparent at zero.
    pub(crate) mc2_player_drain: Mc2Quiet<1>,
    /// Class-14 scroll pickups banked for the Phase-4.2 spell-XP
    /// system (retail grants 4 XP each in single-player,
    /// UpdateScroll_59C80 EF:41180-83). Hash-transparent at zero.
    pub(crate) mc2_scrolls: Mc2Quiet<2>,
    /// The human's collected MC2 spell tokens, a bitmask by spell
    /// model 0..25 (retail: `SpellEnabled[model]` on the wizard,
    /// sub_68FF0 EF:55726) — banked for the Phase-4.2 spell system
    /// like the scrolls. Hash-transparent at zero.
    pub(crate) mc2_spell_tokens: Mc2Quiet<3>,
    /// MC2 spell-XP mail (owner id, spell index): projectile impacts
    /// award from inside the pool tick (`sub_6D8B0` call sites,
    /// EF:63189 etc.); the world tick drains it into the wizard's
    /// book the same turn — empty at hash time like a read mailbox
    /// (and hash-transparent when empty, so every pinned stream
    /// holds across the field addition).
    pub(crate) mc2_cast_xp: Mc2XpMail,
    /// m26 spell-steal requests (`sub_28FF0` EF:19348-71 → the
    /// `sub_69300` effect): the wraith's roll lands pool-side but the
    /// human book is world-side — the world tick drains this the
    /// same turn. Hash-transparent while empty.
    pub(crate) mc2_steal_mail: Mc2StealMail,
    /// The mana-magnet aura CLAIM handshake (`word_0x7A_122` on the
    /// ball, EF:28364/28383): ball slot → claiming aura slot. An aura
    /// claims an unclaimed ball for one pull; the ball's own tick
    /// consumes and clears the claim — first-in-list keeps the ball
    /// when auras overlap. Hash-quiet while empty (E25 2026-07-15).
    pub(crate) mc2_aura_claim: Mc2SlotMap<4>,
    /// Pool wizards' WANTED timers (`word_0x248_584`): wizard slot →
    /// remaining hostility ticks. The human's lives in
    /// [`Gen::player_aggro`]; rivals had no `Ent` home. Armed by
    /// [`Gen::mc2_arm_wanted`], run down with the aggro cadence,
    /// read by the archer Scan-A post-reject. Hash-quiet while
    /// empty (E12 2026-07-15).
    pub(crate) mc2_wanted: Mc2SlotMap<5>,
    /// The human's REBOUND tier bit (`sub_6AA00` EF:56721-51: tier
    /// `life==1` stamps PRECISE — byte0xc[0]|=0x10, exact return +
    /// doubled payload; `life==0` scatter — byte[1]|=0x80). Rides
    /// beside the [`Gen::player_rebound`] mirror; 0 = scatter.
    /// Hash-transparent at zero.
    pub(crate) mc2_rebound_precise: Mc2Quiet<6>,
    /// ALLIANCE charms (spell 24): charmed creature slot → the
    /// caster's owner id (retail keeps `parentId` ON the entity,
    /// EF:29688; the port's creatures never modeled parentId — the
    /// charm must NOT clobber `id24`, the authored disposition the
    /// stage census keys on). The tier duration counts down in the
    /// creature's `f26` (`word_0x2E_46`; its `word_0x30_48` companion
    /// has no port home — f28 is the MC2 damage-contract flag).
    /// Hash-quiet while empty.
    pub(crate) mc2_allied: Mc2SlotMap<8>,
    /// Per-wizard castle research (`array_0x24E_590`, player struct
    /// +0x24E): `[stage-1]` in `.1` = the stage's HP factor
    /// (`subSpellIndex_2`), `[stage-1]` in `.2` = the stage's
    /// PART-TYPE (`life_0x1A` — 1 = fire tower, 2 = lightning),
    /// keyed by owner id. Retail fills it via the research child
    /// (`sub_69AB0` EF:56120-21) for stage `castleLevel+1`; the
    /// port stamps at cast/upgrade time from the castle-spell tier
    /// (the A.5 shortcut, castle-and-cost.md) until the research
    /// production chain lands. Hash-quiet while empty.
    pub(crate) mc2_castle_research: Mc2CastleResearch,
}

/// See [`Gen::mc2_castle_research`] — hashes to NOTHING while empty
/// (the [`Mc2Ord`] pattern; tag 7 disambiguates adjacent quiet
/// fields, review J2). Entries are `(owner, hp_factor[stage-1],
/// part_type[stage-1])` for stages 1..=7 (retail slots 1..7 / 10..16
/// of the 19-byte array — slots 0/8/9/17/18 are never addressed).
#[derive(Default)]
pub(crate) struct Mc2CastleResearch(pub Vec<(u16, [u8; 7], [u8; 7])>);

impl std::hash::Hash for Mc2CastleResearch {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if !self.0.is_empty() {
            state.write_u8(7);
            for (own, hp, part) in &self.0 {
                state.write_u16(*own);
                state.write(hp);
                state.write(part);
            }
        }
    }
}

/// See [`Gen::mc2_cast_xp`] — hashes to NOTHING while empty (the
/// [`Mc2Ord`] pattern). Entries are `(owner, spell, amount)`: the
/// area-spell effect ticks award BATCH counts (retail's single
/// `sub_6D8B0(id, spell, hits)` call per pass — one award, one
/// level-up notification), so the mail carries the amount (F3).
#[derive(Default)]
pub(crate) struct Mc2XpMail(pub Vec<(u16, u16, i32)>);

impl std::hash::Hash for Mc2XpMail {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if !self.0.is_empty() {
            self.0.hash(state);
        }
    }
}

/// See [`Gen::mc2_steal_mail`] — (wraith slot, hand: 1 = right,
/// 2 = left) requests from the m26 steal roll, drained by the world
/// tick the same turn (the book lives world-side). Empty at hash
/// time like a read mailbox; tagged against adjacent-mail aliasing
/// (review J2).
#[derive(Default)]
pub(crate) struct Mc2StealMail(pub Vec<(u16, u8)>);

impl std::hash::Hash for Mc2StealMail {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if !self.0.is_empty() {
            state.write_u8(5);
            self.0.hash(state);
        }
    }
}

/// See [`Gen::mc2_spawn_ord`] — hashes to NOTHING while all-zero
/// (golden-stream compatibility across the field addition).
#[derive(Default)]
pub(crate) struct Mc2Ord(pub [u8; 32]);

impl std::hash::Hash for Mc2Ord {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if self.0.iter().any(|&v| v != 0) {
            state.write(&self.0);
        }
    }
}

/// Per-color MC2 Life scalar (`word_0x24A_586` — the wizard-HP AND
/// castle-HP factor, EF:43768/61695). Default 256 = 1.0x for every
/// color; hashes to NOTHING while all-default (the [`Mc2Ord`]
/// pattern — goldens pinned before the field existed stand).
pub(crate) struct Mc2LifeScale(pub [u16; 8]);

impl Default for Mc2LifeScale {
    fn default() -> Self {
        Mc2LifeScale([256; 8])
    }
}

impl std::hash::Hash for Mc2LifeScale {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if self.0 != [256; 8] {
            self.0.hash(state);
        }
    }
}

/// A counter that hashes to NOTHING at zero (see [`Mc2Ord`]). The
/// const TAG (unique per field) disambiguates ADJACENT quiet fields:
/// without it, (drain=5, scrolls=0) and (drain=0, scrolls=5) fed
/// identical byte streams (the conditional-hash aliasing class,
/// review J2). Written INSIDE the condition, so zero fields — every
/// pinned golden — contribute nothing, exactly as before.
#[derive(Default)]
pub(crate) struct Mc2Quiet<const TAG: u8>(pub i32);

impl<const TAG: u8> std::hash::Hash for Mc2Quiet<TAG> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if self.0 != 0 {
            state.write_u8(TAG);
            state.write_i32(self.0);
        }
    }
}

/// A slot-keyed side-channel that hashes to NOTHING while empty
/// (golden-stream compatibility across the field addition) and
/// contributes deterministically (BTreeMap order) once entries
/// exist. Carries per-entity words that have no `Ent` field home —
/// adding a field to `Ent` would move EVERY golden's hash stream.
/// The const TAG (unique per field) keeps adjacent slot-maps from
/// aliasing (aura_claim={a} + wanted={} vs its mirror — review J2);
/// written only when non-empty, so empty maps stay transparent.
#[derive(Default)]
pub(crate) struct Mc2SlotMap<const TAG: u8>(pub std::collections::BTreeMap<u16, u16>);

impl<const TAG: u8> std::hash::Hash for Mc2SlotMap<TAG> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if !self.0.is_empty() {
            state.write_u8(TAG);
        }
        for (k, v) in &self.0 {
            state.write_u16(*k);
            state.write_u16(*v);
        }
    }
}

/// See [`Gen::mc2_night_shade`] — a bool that hashes to NOTHING when
/// false (golden-stream compatibility across the field addition).
#[derive(Default)]
pub(crate) struct NightShade(pub bool);

impl std::hash::Hash for NightShade {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if self.0 {
            state.write_u8(1);
        }
    }
}

/// Per-slot spawn generations ([`Gen::slot_gen`]) — bumped every time
/// `new_event` hands the slot out, so presentation can tell two
/// occupants of the same slot apart across tick snapshots (the render
/// interpolation identity guard; the balloon stale-slot class).
/// PRESENTATION-ONLY: never read by any sim rule, so the Hash is a
/// no-op UNCONDITIONALLY — unlike the quiet counters above it stays
/// silent even when populated.
#[derive(Default)]
pub(crate) struct SlotGens(pub Vec<u32>);

impl std::hash::Hash for SlotGens {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}

/// One sound request: engine sound id (the SNDS bank-0 index), the
/// emitter's position on the u16 torus, and its slot as the instance
/// tag (the original's entity+24). `player` marks requests the
/// original issued against the player's own entity (full volume,
/// center pan, and the gate for the player-only ids 4/14/17/29).
#[derive(Debug, Clone, Copy, Hash)]
pub struct SoundEvent {
    pub id: u8,
    pub pos: (u16, u16, i16),
    pub tag: u16,
    pub player: bool,
}

/// Rebuild the original 1-based record table from level things.
/// Build the 1-based runtime THING table. `base` maps the package's
/// 0-based `slot` export into engine slots: MC1's 1999-record file
/// is engine slots 1..=1999 (base 1); MC2's 1200-record file IS the
/// engine table including the unused slot 0 (base 0) — its stage
/// checkpoints reference these slots directly (remc2
/// entity_0x30311[stage_1]).
pub(crate) fn build_table(things: &[Thing], slots: usize, base: usize) -> Vec<Rec> {
    let mut table = vec![Rec::default(); slots];
    for th in things {
        let i = th.slot as usize + base;
        if i < table.len() {
            table[i] = Rec {
                class: th.class,
                model: th.model,
                x: th.x,
                y: th.y,
                dis_id: th.dis_id,
                swi_sz: th.swi_sz,
                swi_id: th.swi_id,
                parent: th.parent,
                child: th.child,
                par3: th.par3.unwrap_or(0),
            };
        }
    }
    table
}

impl Gen {
    /// A fresh engine over owned planes. `seed` = the level's GEN_MAP
    /// seed (`rand_4`); the retile `pseudoRand` stream is replayed from
    /// the pristine height plane.
    pub(crate) fn new(
        t: Planes,
        assets: FeatureAssets,
        seed: u32,
        chassis: ChassisParams,
        verbs: VerbSet,
    ) -> Self {
        let pseudo = post_generation_pseudo_rand(&t.height);
        Gen {
            t,
            assets,
            retile: corners::retile_table(),
            map_entity: vec![0; GRID],
            ent: vec![Ent::default(); chassis.pool_slots],
            slot_gen: SlotGens(vec![0; chassis.pool_slots]),
            free: (1..chassis.pool_slots as u16).rev().collect(),
            rand: seed,
            pseudo,
            spawn_count: [0; 20],
            player_mail: [(0, 0); 6],
            player_damage: 0,
            erupting: 0,
            plume: 0,
            player_knock: (0, 0),
            mc2_debuffs: Mc2PlayerDebuffs::default(),
            rival_ents: [0; 8],
            mc2_life_scale: Mc2LifeScale::default(),
            player_aggro: 0,
            player_invisible: false,
            player_rebound: false,
            kills: 0,
            shots: 0,
            hits: 0,
            player_danger: 0,
            banked_houses: 0,
            castle_alert: 0,
            player_alert: 0,
            balloon_alert: 0,
            exhausted: 0,
            sounds: Vec::new(),
            terrain_dirty: false,
            chassis,
            verbs,
            verb_fallbacks: 0,
            misfits: Vec::new(),
            mc2_night_shade: NightShade(false),
            mc2_spawn_ord: Mc2Ord::default(),
            mc2_player_drain: Mc2Quiet::default(),
            mc2_scrolls: Mc2Quiet::default(),
            mc2_spell_tokens: Mc2Quiet::default(),
            mc2_cast_xp: Mc2XpMail::default(),
            mc2_steal_mail: Mc2StealMail::default(),
            mc2_aura_claim: Mc2SlotMap::default(),
            mc2_wanted: Mc2SlotMap::default(),
            mc2_allied: Mc2SlotMap::default(),
            mc2_rebound_precise: Mc2Quiet::default(),
            mc2_castle_research: Mc2CastleResearch::default(),
        }
    }

    /// Note that `kind`'s requested arm is pending and the MC1
    /// implementation served instead (once per verb per world).
    pub(crate) fn note_verb_fallback(&mut self, kind: VerbKind) {
        self.verb_fallbacks |= 1 << kind as u8;
    }

    /// The spawn seam refused an unknown `(class, model)` — count it.
    pub(crate) fn note_misfit(&mut self, class: u16, model: u16) {
        if let Some(m) = self
            .misfits
            .iter_mut()
            .find(|m| m.0 == class && m.1 == model)
        {
            m.2 += 1;
        } else {
            self.misfits.push((class, model, 1));
        }
    }

    /// Emit a sound request from entity `i` (its position and slot
    /// become the request's position and instance tag).
    pub(crate) fn snd(&mut self, id: u8, i: usize) {
        let e = &self.ent[i];
        self.sounds.push(SoundEvent {
            id,
            pos: (e.x, e.y, e.z),
            tag: i as u16,
            player: false,
        });
    }

    /// Emit a player-entity sound request (the original's calls
    /// against the wizard's own entity — full volume, center pan).
    pub(crate) fn snd_player(&mut self, id: u8) {
        self.sounds.push(SoundEvent {
            id,
            pos: (0, 0, 0),
            tag: crate::mc1::mobs::PLAYER_TARGET,
            player: true,
        });
    }

    /// GenerateFeatures_36430: consume the class-10 load-time features
    /// (dis_id 0xFFFF) in slot order and run the fixpoint event loop.
    pub(crate) fn load_time_pass(&mut self, table: &mut [Rec]) {
        for i in 1..table.len() {
            if table[i].dis_id == 0xFFFF && table[i].class == 10 {
                self.dispatch(table, i);
                table[i].class = 0;
            }
        }
        self.event_loop();
    }
}

/// Apply MC1's load-time terrain features.
///
/// `seed` is the level's GEN_MAP seed (`rand_4` is loaded from it and
/// nothing before GenerateFeatures advances it); pass 0 if unknown —
/// only dither variety is affected, not feature placement.
pub fn generate_features_mc1(
    planes: TerrainPlanes<'_>,
    things: &[Thing],
    seed: u32,
    assets: &FeatureAssets,
) {
    let mut table = build_table(things, ChassisParams::MC1.level_table_slots, 1);
    let owned = Planes {
        height: planes.height.to_vec(),
        tile_type: planes.tile_type.to_vec(),
        shading: planes.shading.to_vec(),
        angle: planes.angle.to_vec(),
        ceiling: Vec::new(),
    };
    let mut g = Gen::new(
        owned,
        assets.clone(),
        seed,
        ChassisParams::MC1,
        VerbSet::MC1,
    );
    g.load_time_pass(&mut table);
    planes.height.copy_from_slice(&g.t.height);
    planes.tile_type.copy_from_slice(&g.t.tile_type);
    planes.shading.copy_from_slice(&g.t.shading);
    planes.angle.copy_from_slice(&g.t.angle);
}

impl Gen {
    // ---- pool primitives ------------------------------------------------

    /// NewEvent_372C0 (:43865). Seeds the per-entity LCG from the
    /// global stream WITHOUT advancing it. Defaults per the original:
    /// life 300, flags 8, +126 = 16, +44 = 100, +24 = own slot,
    /// +58 = 0xFA, +66 = +67 = 0xFF, +68 = 10 (:43879), +156 = row 0.
    pub(crate) fn new_event(&mut self) -> Option<usize> {
        let Some(idx) = self.free.pop() else {
            // Fail-open like the original (alloc returns null, the
            // spawn silently vanishes — map 032's starved trigger),
            // but COUNTED: the limit-removing register (ROADMAP
            // "MULTI-GAME ARCHITECTURE") wants a playtest catalogue
            // of the levels that hit the pool ceiling before any
            // bumped-pool option exists.
            self.exhausted = self.exhausted.saturating_add(1);
            return None;
        };
        let idx = idx as usize;
        // New occupant → new presentation generation (hash-silent).
        self.slot_gen.0[idx] = self.slot_gen.0[idx].wrapping_add(1);
        // The aura claim lives ON the entity in retail — slot reuse
        // resets it with every other field (no stale claim may greet
        // the slot's next occupant).
        self.mc2_aura_claim.0.remove(&(idx as u16));
        let e = &mut self.ent[idx];
        *e = Ent::default();
        e.max_life = 300;
        e.flags = 8;
        e.f126 = 16;
        e.f44 = 100;
        e.f68 = 10;
        e.id24 = idx as u16;
        e.f58 = 0xFA;
        e.f66 = 0xFF;
        e.f67 = 0xFF;
        e.rand = match self.chassis.ent_rand_width {
            RandWidth::U32 => (idx as u32).wrapping_add(self.rand),
            RandWidth::U16 => (idx as u32).wrapping_add(self.rand) & 0xFFFF,
        };
        e.f63 = idx as u8;
        Some(idx)
    }

    /// One draw of this event's own LCG (`rand_29799_4`, the stream
    /// every spawn/behavior handler rolls).
    pub(crate) fn ent_rand(&mut self, i: usize) -> u32 {
        match self.chassis.ent_rand_width {
            RandWidth::U32 => lcg32(&mut self.ent[i].rand),
            RandWidth::U16 => {
                let r = self.ent[i].rand.wrapping_mul(9377).wrapping_add(9439) & 0xFFFF;
                self.ent[i].rand = r;
                r
            }
        }
    }

    /// sub_41CF0 (:52468): link into the per-tile list and set position.
    pub(crate) fn link(&mut self, i: usize, x: u16, y: u16, z: i16) {
        if self.ent[i].flags & 4 != 0 {
            return;
        }
        let t = tile((x >> 8) as u8, (y >> 8) as u8);
        self.ent[i].prev22 = 0;
        self.ent[i].next20 = self.map_entity[t];
        let head = self.map_entity[t] as usize;
        if head != 0 {
            self.ent[head].prev22 = i as u16;
        }
        self.map_entity[t] = i as u16;
        let e = &mut self.ent[i];
        e.x = x;
        e.y = y;
        e.z = z;
        e.flags |= 4;
    }

    /// sub_41DD0 (:52486).
    fn unlink(&mut self, i: usize) {
        if self.ent[i].flags & 4 == 0 {
            return;
        }
        let (next, prev) = (self.ent[i].next20, self.ent[i].prev22);
        if prev != 0 {
            self.ent[prev as usize].next20 = next;
        } else {
            let t = tile((self.ent[i].x >> 8) as u8, (self.ent[i].y >> 8) as u8);
            self.map_entity[t] = next;
        }
        if next != 0 {
            self.ent[next as usize].prev22 = prev;
        }
        self.ent[i].flags &= !4;
    }

    /// sub_41C70 (:52442): move, relinking only across tiles.
    pub(crate) fn move_relink(&mut self, i: usize, x: u16, y: u16, z: i16) {
        let e = &self.ent[i];
        if e.x >> 8 == x >> 8 && e.y >> 8 == y >> 8 {
            let e = &mut self.ent[i];
            e.x = x;
            e.y = y;
            e.z = z;
        } else {
            self.unlink(i);
            self.link(i, x, y, z);
        }
    }

    /// sub_41E90 (:52514): unlink, clear, return the slot (LIFO).
    pub(crate) fn free_entity(&mut self, i: usize) {
        self.unlink(i);
        self.ent[i].class64 = 0;
        self.free.push(i as u16);
    }

    // ---- terrain helpers ------------------------------------------------

    /// sub_724C0 (:81516): ground height at an 8.8 position,
    /// interpolated across the tile's two triangles, in engine units
    /// (one height byte = 32).
    pub(crate) fn ground_z(&self, x: u16, y: u16) -> i32 {
        Self::interp_plane(&self.t.height, x, y)
    }

    /// `sub_10C60` → `sub_B5D68` (remc2 Terrain.cpp:2158-2164): the
    /// CAVE CEILING altitude — the exact same bilinear ×32 sampler as
    /// the floor's, reading the second heightmap. Callers must be
    /// cave-gated (the plane is empty off-cave; retail's array is
    /// all-zeros there and every retail call site is cave-gated too).
    pub(crate) fn ceiling_z(&self, x: u16, y: u16) -> i32 {
        Self::interp_plane(&self.t.ceiling, x, y)
    }

    fn interp_plane(plane: &[u8], x: u16, y: u16) -> i32 {
        let h = |dx: u8, dy: u8| plane[tile(dx, dy)] as i32;
        let (cx, cy) = ((x >> 8) as u8, (y >> 8) as u8);
        let (fx, fy) = ((x & 0xFF) as i32, (y & 0xFF) as i32);
        let (p1, comp);
        if cx.wrapping_add(cy) & 1 == 1 {
            if fx + fy > 255 {
                p1 = h(cx, cy.wrapping_add(1));
                let p2 = h(cx.wrapping_add(1), cy.wrapping_add(1));
                comp = (255 - fy) * (h(cx.wrapping_add(1), cy) - p2) + fx * (p2 - p1);
            } else {
                p1 = h(cx, cy);
                let p2 = h(cx.wrapping_add(1), cy);
                comp = fy * (h(cx, cy.wrapping_add(1)) - p1) + fx * (p2 - p1);
            }
        } else if fx <= fy {
            p1 = h(cx, cy);
            let p2 = h(cx, cy.wrapping_add(1));
            comp = fy * (p2 - p1) + fx * (h(cx.wrapping_add(1), cy.wrapping_add(1)) - p2);
        } else {
            p1 = h(cx, cy);
            let p2 = h(cx.wrapping_add(1), cy);
            comp = fy * (h(cx.wrapping_add(1), cy.wrapping_add(1)) - p2) + fx * (p2 - p1);
        }
        (comp >> 3) + 32 * p1
    }

    /// sub_361C0 (:42956): average of the four footprint corners
    /// (x, y), (x+w, y), (x+w, y+h), (x, y+h), u8-wrapping.
    pub(crate) fn avg4(&self, x: u8, y: u8, h: u8, w: u8) -> u16 {
        let p1 = self.t.height[tile(x, y)] as u16;
        let p2 = self.t.height[tile(x.wrapping_add(w), y)] as u16;
        let p3 = self.t.height[tile(x.wrapping_add(w), y.wrapping_add(h))] as u16;
        let p4 = self.t.height[tile(x, y.wrapping_add(h))] as u16;
        (p1 + p2 + p3 + p4) >> 2
    }

    /// The shared passes 2+3 of the retexture helpers (sub_33B90 /
    /// sub_33E10, :41165/:41288): retile every type-1 cell of the rect
    /// grown by one on the -x/-y side through the `byte_B5D40` table
    /// (drawing pseudoRand for types < 8), then recompute shading over
    /// the rect grown once more.
    pub(crate) fn retile_and_shade(&mut self, ax: u8, ay: u8, bx: u8, by: u8) {
        let x_add = bx.wrapping_sub(ax).wrapping_add(2);
        let y_add = by.wrapping_sub(ay).wrapping_add(2);
        let (sx, sy) = (ax.wrapping_sub(1), ay.wrapping_sub(1));
        let mut cy = sy;
        for _ in 0..y_add {
            let mut cx = sx;
            for _ in 0..x_add {
                let t = tile(cx, cy);
                if self.t.tile_type[t] == 1 {
                    let p1 = self.t.angle[t] & 7;
                    let p2 = self.t.angle[tile(cx.wrapping_add(1), cy)] & 7;
                    let p3 = self.t.angle[tile(cx.wrapping_add(1), cy.wrapping_add(1))] & 7;
                    let p4 = self.t.angle[tile(cx, cy.wrapping_add(1))] & 7;
                    let idx = p4 as usize + 7 * p3 as usize + 49 * p2 as usize + 343 * p1 as usize;
                    let [new_type, orient] = self.retile[idx];
                    self.t.tile_type[t] = new_type;
                    self.t.angle[t] = if new_type >= 8 {
                        orient.wrapping_add(self.t.angle[t] & 0x87)
                    } else {
                        self.pseudo = self.pseudo.wrapping_mul(9377).wrapping_add(9439);
                        (self.t.angle[t] & 0x87).wrapping_add(16 * (self.pseudo % 7) as u8)
                    };
                }
                cx = cx.wrapping_add(1);
            }
            cy = cy.wrapping_add(1);
        }
        // Pass 3: shading over the rect grown once more (3x3 for a
        // single cell). shade = NW height - SE height + 32, as signed
        // char; clamp <28 → (s&3)+28, >40 → (s&7)+40; clear angle bit 3.
        // MC2's twin (`sub_462A0`/`46570`) adds two DATA-variant arms,
        // both no-ops on MC1 worlds: the non-Day shade inversion
        // (Terrain.cpp:2030-2033, [`Gen::mc2_night_shade`]) and the
        // cave floor↔ceiling invariant instead of the blind bit3
        // clear (Terrain.cpp:2034-2042).
        let mut cy = sy;
        for _ in 0..y_add.wrapping_add(1) {
            let mut cx = sx;
            for _ in 0..x_add.wrapping_add(1) {
                let t = tile(cx, cy);
                let se = self.t.height[tile(cx.wrapping_add(1), cy.wrapping_add(1))];
                let nw = self.t.height[tile(cx.wrapping_sub(1), cy.wrapping_sub(1))];
                let mut s = nw.wrapping_sub(se).wrapping_add(32);
                if (s as i8) < 28 {
                    s = (s & 3) + 28;
                } else if (s as i8) > 40 {
                    s = (s & 7) + 40;
                }
                self.t.shading[t] = if self.mc2_night_shade.0 {
                    64u8.wrapping_sub(s)
                } else {
                    s
                };
                if self.is_cave() {
                    self.cave_seal_fixup(t);
                } else {
                    self.t.angle[t] &= 0xF7;
                }
                cx = cx.wrapping_add(1);
            }
            cy = cy.wrapping_add(1);
        }
    }

    /// sub_33B90 (:41165), "flag mode": stencil type 1 onto each rect
    /// cell + its W/NW/N neighbors where not building-protected (bit 7),
    /// then retile + shade.
    fn recompute_protected(&mut self, ax: u8, ay: u8, bx: u8, by: u8) {
        let (w, h) = (
            bx.wrapping_sub(ax).wrapping_add(1),
            by.wrapping_sub(ay).wrapping_add(1),
        );
        let mut cy = ay;
        for _ in 0..h {
            let mut cx = ax;
            for _ in 0..w {
                for t in [
                    tile(cx, cy),
                    tile(cx.wrapping_sub(1), cy),
                    tile(cx.wrapping_sub(1), cy.wrapping_sub(1)),
                    tile(cx, cy.wrapping_sub(1)),
                ] {
                    if self.t.angle[t] & 0x80 == 0 {
                        self.t.tile_type[t] = 1;
                    }
                }
                cx = cx.wrapping_add(1);
            }
            cy = cy.wrapping_add(1);
        }
        self.retile_and_shade(ax, ay, bx, by);
    }

    /// sub_33E10 (:41288), "dig mode": same but the stencil ignores the
    /// protection bit.
    fn recompute_unprotected(&mut self, ax: u8, ay: u8, bx: u8, by: u8) {
        let (w, h) = (
            bx.wrapping_sub(ax).wrapping_add(1),
            by.wrapping_sub(ay).wrapping_add(1),
        );
        let mut cy = ay;
        for _ in 0..h {
            let mut cx = ax;
            for _ in 0..w {
                for t in [
                    tile(cx, cy),
                    tile(cx.wrapping_sub(1), cy),
                    tile(cx.wrapping_sub(1), cy.wrapping_sub(1)),
                    tile(cx, cy.wrapping_sub(1)),
                ] {
                    self.t.tile_type[t] = 1;
                }
                cx = cx.wrapping_add(1);
            }
            cy = cy.wrapping_add(1);
        }
        self.retile_and_shade(ax, ay, bx, by);
    }

    /// sub_33AE0 (:41094), wall variant: write `ty` onto the cell and
    /// its W/NW/N neighbors unconditionally, then 3x3 shading with a
    /// hard floor of 32 (no retile, no PRNG).
    fn set_type_2x2(&mut self, t: usize, ty_val: u8) {
        let (cx, cy) = (tx(t), ty(t));
        self.t.tile_type[t] = ty_val;
        self.t.tile_type[tile(cx.wrapping_sub(1), cy)] = ty_val;
        self.t.tile_type[tile(cx.wrapping_sub(1), cy.wrapping_sub(1))] = ty_val;
        self.t.tile_type[tile(cx, cy.wrapping_sub(1))] = ty_val;
        let mut yy = cy.wrapping_sub(1);
        for _ in 0..3 {
            let mut xx = cx.wrapping_sub(1);
            for _ in 0..3 {
                let se = self.t.height[tile(xx.wrapping_add(1), yy.wrapping_add(1))];
                let nw = self.t.height[tile(xx.wrapping_sub(1), yy.wrapping_sub(1))];
                let mut s = nw.wrapping_sub(se).wrapping_add(32);
                if (s as i8) < 32 {
                    s = 32;
                } else if (s as i8) > 40 {
                    s = (s & 7) + 40;
                }
                let c = tile(xx, yy);
                self.t.shading[c] = s;
                self.t.angle[c] &= 0xF7;
                xx = xx.wrapping_add(1);
            }
            yy = yy.wrapping_add(1);
        }
    }

    /// sub_40A10 (:51621): adjust one cell's height by `delta` (clamped
    /// 0..200), update its slope nibble (1 = land; 0 = water when the
    /// floor is reached and no neighbor blocks conversion), then
    /// recompute the 1-cell neighborhood. `protect` mode aborts on
    /// building-protected cells and honors protection in the stencil.
    /// Returns true only via the literal `(0,0)` clamp latch (dead in
    /// practice; kept faithful).
    fn dig_cell(&mut self, ax: i16, ay: i16, delta: i16, protect: bool) -> bool {
        let t = tile(ax as u8, ay as u8);
        let mut saturated = false;
        let mut v = delta as i32 + self.t.height[t] as i32;
        if v > 200 {
            v = 200;
            if ax == 0 && ay == 0 {
                saturated = true;
            }
        }
        if v < 0 {
            v = 0;
            if ax == 0 && ay == 0 {
                saturated = true;
            }
        }
        if protect && self.t.angle[t] & 0x80 != 0 {
            return true;
        }
        self.t.height[t] = v as u8;
        // MC2's twin `sub_56F10` (EF:39534-39543): on a cave the
        // ceiling counter-shifts by the RAW delta (dig down = roof
        // up), saturating high at 255 and u8-truncating below zero
        // exactly like retail's char write; the invariant is then
        // re-asserted by the tail recompute's shading pass.
        if self.is_cave() {
            let c = self.t.ceiling[t] as i32 - delta as i32;
            self.t.ceiling[t] = if c >= 255 { 255 } else { c as u8 };
        }
        if v != 0 {
            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
        } else {
            // Water conversion: all 8 neighbors must not carry slope
            // codes 2, 3 or 5 (sub_409E0), else leave the angle alone.
            let clear = [
                (-1, -1),
                (0, -1),
                (1, -1),
                (1, 0),
                (-1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ]
            .iter()
            .all(|&(dx, dy)| {
                let n = self.t.angle[step(t, dx, dy)] & 7;
                n != 5 && n != 2 && n != 3
            });
            if clear {
                self.t.angle[t] &= 0xF0;
            }
        }
        if protect {
            self.recompute_protected(tx(t), ty(t), tx(t), ty(t));
        } else {
            self.recompute_unprotected(tx(t), ty(t), tx(t), ty(t));
        }
        saturated
    }

    /// The ring iterator of sub_11410/sub_114B0 (:16697/:16732): yields
    /// every (dx, dy) of rings `lo..=hi` EXCEPT the last entry of ring
    /// `hi`, which the original fetches together with the stop code and
    /// drops — a faithful off-by-one.
    /// Combat-effect access to the single-cell dig (the fire's scorch,
    /// sub_40D30(expl, 0, 0, -depth, 1)).
    pub(crate) fn dig_cell_pub(&mut self, ax: i16, ay: i16, delta: i16, protect: bool) -> bool {
        self.dig_cell(ax, ay, delta, protect)
    }

    pub(crate) fn ring_cells(&self, lo: i32, hi: i32) -> Vec<(u8, u8)> {
        let mut out = Vec::new();
        if lo < 0 || lo > 31 {
            return out;
        }
        let hi_c = hi.min(31);
        let mut ring = lo;
        loop {
            let cells = &self.assets.rings[ring as usize];
            for (k, &d) in cells.iter().enumerate() {
                let last_of_ring = k + 1 == cells.len();
                if last_of_ring && ring >= hi_c {
                    return out; // fetched with stop code, dropped
                }
                out.push(d);
                if last_of_ring {
                    break;
                }
            }
            ring += 1;
            if ring > hi_c || ring > 31 {
                return out;
            }
        }
    }

    /// sub_40D30 (:51693): dig a disc of rings `lo..=hi` (clamped to
    /// the event's radius) around the event, height delta `delta`.
    fn dig_disc(&mut self, i: usize, lo: i32, hi: i32, delta: i16, protect: bool) -> bool {
        let e = self.ent[i];
        let cx = ((e.x as u32 + 128) >> 8) as i32;
        let cy = ((e.y as u32 + 128) >> 8) as i32;
        let hi = hi.min((e.f80 >> 8) as i32);
        for (dx, dy) in self.ring_cells(lo, hi) {
            if self.dig_cell(
                (cx + dx as i32) as i16,
                (cy + dy as i32) as i16,
                delta,
                protect,
            ) && protect
            {
                return true;
            }
        }
        false
    }

    /// sub_255D0 (:28353): the -3 disc variant that never aborts.
    /// (Also ≡ MC2's `sub_31F00` EF:23460 — the (10,11) scorch
    /// ring's stamper: same template walk, same −3 dig, same
    /// f80>>8 radius clamp.)
    pub(crate) fn dig_disc_minus3(&mut self, i: usize, lo: i32, hi: i32) {
        let e = self.ent[i];
        let cx = ((e.x as u32 + 128) >> 8) as i32;
        let cy = ((e.y as u32 + 128) >> 8) as i32;
        let hi = hi.min((e.f80 >> 8) as i32);
        for (dx, dy) in self.ring_cells(lo, hi) {
            self.dig_cell((cx + dx as i32) as i16, (cy + dy as i32) as i16, -3, false);
        }
    }

    /// sub_11760 (:16869): true when the tile under the position (plain
    /// >>8, no rounding) is water (angle nibble 0) — the walker/digger
    /// > > stop probe.
    pub(crate) fn on_water(&self, x: u16, y: u16) -> bool {
        self.t.angle[tile((x >> 8) as u8, (y >> 8) as u8)] & 0xF == 0
    }

    // ---- math helpers ---------------------------------------------------

    /// sub_358D0 (:42470): shortest wrapped tile delta in -128..=128.
    pub(crate) fn wrap_delta(a: i16, b: i16) -> i32 {
        let d = b.wrapping_sub(a);
        if d > 128 {
            (d as i32) - 256
        } else if d < -128 {
            (d as i32) + 256
        } else {
            d as i32
        }
    }

    /// sub_40F87 (:51818): angle from delta in 1/2048 turns (0 = -y).
    pub(crate) fn angle_of(dx: i16, dy: i16) -> u16 {
        let lut = |n: i32, d: i32| ATAN[((n << 8) / d) as usize] as i32;
        let (a1, a2) = (dx as i32, dy as i32);
        let r = if a1 == 0 && a2 == 0 {
            0
        } else if a1 < 0 {
            if a2 < 0 {
                if -a1 < -a2 {
                    2048 - lut(-a1, -a2)
                } else {
                    1536 + lut(-a2, -a1)
                }
            } else if -a1 < a2 {
                1024 + lut(-a1, a2)
            } else {
                1536 - lut(a2, -a1)
            }
        } else if a2 < 0 {
            if a1 < -a2 {
                lut(a1, -a2)
            } else {
                512 - lut(-a2, a1)
            }
        } else if a1 < a2 {
            1024 - lut(a1, a2)
        } else {
            512 + lut(a2, a1)
        };
        r as u16
    }

    /// Distance_410CE (:51874): Newton integer sqrt with seed table.
    pub(crate) fn isqrt(square: u32) -> u32 {
        if square == 0 {
            return 0;
        }
        let bit = 31 - square.leading_zeros();
        let mut i = BIT_SQRT[bit as usize];
        while square / i < i {
            i = (square / i + i) >> 1;
        }
        i
    }

    /// sub_42150/sub_423D0 (:52638/:52739) on two 8.8 positions.
    pub(crate) fn angle_between(ax: u16, ay: u16, bx: u16, by: u16) -> u16 {
        Self::angle_of(
            (bx as i16).wrapping_sub(ax as i16),
            (by as i16).wrapping_sub(ay as i16),
        )
    }
    fn dist_between(ax: u16, ay: u16, bx: u16, by: u16) -> u16 {
        let dx = (bx as i16).wrapping_sub(ax as i16) as i32;
        let dy = (by as i16).wrapping_sub(ay as i16) as i32;
        Self::isqrt((dx * dx + dy * dy) as u32) as u16
    }

    /// sub_41EC0 (:52523), pitch-0 path: advance a position `speed`
    /// units along `angle` (16.16 trig, wrapping i16/u16 adds).
    fn advance(x: &mut u16, y: &mut u16, angle: u16, speed: i16) {
        if speed == 0 {
            return;
        }
        let a = (angle & 0x7FF) as usize;
        *x = x.wrapping_add(((speed as i32 * SIN[a]) >> 16) as u16);
        *y = y.wrapping_sub(((COS[a] * speed as i32) >> 16) as u16);
    }

    // ---- the spawn scan -------------------------------------------------

    /// sub_36480 (:43065): dispatch one feature entity.
    fn dispatch(&mut self, table: &mut [Rec], slot: usize) {
        let rec = table[slot];
        let model = rec.model;
        let chained = matches!(model, 28 | 29 | 31 | 50) && rec.swi_id != 0;
        if chained {
            self.walk_chain(table, slot);
            return;
        }
        let x = rec.x << 8;
        let y = rec.y << 8;
        let z = self.ground_z(x, y) as i16;
        if let Some(i) = self.spawn_creator(model, x, y, z) {
            if model == 45 {
                self.building_fixup(i, rec.parent.wrapping_add(16));
            }
        }
    }

    /// sub_362C0 (:42972): walk a feature chain root-first, clearing
    /// each node's pending flag and running the per-model segment
    /// function on every parent→child coordinate pair.
    fn walk_chain(&mut self, table: &mut [Rec], slot: usize) {
        let class = table[slot].class;
        let model = table[slot].model;
        // A valid chain is shorter than the table; the caps below are
        // unreachable on well-formed data and break the CYCLE livelock
        // on garbage links (frankenstein bycatch: MC2 reuses the
        // parent/child fields as context params, and a malformed
        // community MC1 level could hang retail the same way).
        let mut cur = slot;
        let mut hops = table.len();
        while table[cur].parent != 0 {
            cur = table[cur].parent as usize % table.len();
            hops -= 1;
            if hops == 0 {
                self.note_misfit(class, model);
                return;
            }
        }
        let mut hops = table.len();
        loop {
            if table[cur].class != class || table[cur].model != model {
                return;
            }
            hops -= 1;
            if hops == 0 {
                self.note_misfit(class, model);
                return;
            }
            let child = table[cur].child as usize % table.len();
            table[cur].swi_id = 0;
            if child == 0 {
                return;
            }
            let (x1, y1) = (table[cur].x, table[cur].y);
            let (x2, y2) = (table[child].x, table[child].y);
            match model {
                28 => self.segment_wall(x1 as i16, y1, x2 as i16, y2 as i16),
                29 => self.segment_track(x1 as i16, y1 as i16, x2 as i16, y2 as i16),
                31 => self.segment_canyon(x1, y1, x2, y2),
                50 => self.segment_ridge(x1, y1, x2, y2),
                _ => unreachable!(),
            }
            cur = child;
        }
    }

    /// Creators (`off_97D12`, :5075). Models absent from retail data or
    /// with null/stub creators spawn nothing. Non-ticking models spawn
    /// an event that the loop purges unticked — only its pool-slot
    /// churn is observable, so their creator bodies reduce to alloc +
    /// identity fields (positions kept for completeness).
    pub(crate) fn spawn_creator(&mut self, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        // Null/stub creator entries: model 24 (stub returning 0),
        // 37, 46..49 (null). Everything else allocates one event.
        if matches!(model, 24 | 37 | 46..=49) || model > 61 {
            return None;
        }
        // Combat-effect models get their real inits (crate::mc1::combat) —
        // in the original one init table serves load AND runtime; at
        // load time the fixpoint loop purges them unticked either way.
        // Model 17 matters in the wild: level 032 authors c10m17
        // fire-trap records behind dispositions (they erupt as the
        // 10-tick blast ring when fired).
        match model {
            0 | 1 | 5 | 17 | 23 | 25 => return self.spawn_effect(model as u8, x, y, z),
            39 => return self.spawn_mana_ball(x, y, z),
            _ => {}
        }
        let i = self.new_event()?;
        let e = &mut self.ent[i];
        e.class64 = 10;
        e.model65 = model as u8;
        e.x = x;
        e.y = y;
        e.z = z;
        match model {
            // sub_3A8D0: growing hill / volcano.
            9 => {
                e.tick70 = 9;
                e.max_life = 17;
                e.act_life = 17;
                e.f44 = 2000;
                e.flags = 0;
                e.f80 = 768;
                e.f82 = 768;
                e.f84 = 0x2000;
            }
            // sub_3A930: one-shot shallow dish.
            10 => {
                e.tick70 = 10;
                e.max_life = 1;
                e.act_life = 1;
                e.f44 = 100;
                e.flags = 0x20000;
                e.f80 = 128;
                e.f82 = 128;
                e.f84 = 128;
            }
            // sub_3A9A0: expanding crater (also the canyon digger ctor).
            11 => {
                e.tick70 = 11;
                e.max_life = 40;
                e.act_life = 40;
                e.f44 = 200;
                e.flags = 0;
                e.f80 = 2304;
                e.f82 = 2304;
                e.f84 = 0x2000;
            }
            // sub_3B060/3B120/3B1D0/3B2A0: unchained wall/track/canyon/
            // ridge nodes; their events tick straight into the self-kill
            // handler (byte70 30/31/33/54 → sub_253E0).
            28 => {
                e.tick70 = 30;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            29 => {
                e.tick70 = 31;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            30 => {
                e.tick70 = 32;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            31 => {
                e.tick70 = 33;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            50 => {
                e.tick70 = 54;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            // sub_3B180: canyon head (only reached via segment spawns
            // in practice; unchained model-32 level entities are absent
            // from retail data).
            32 => {
                e.tick70 = 34;
                e.max_life = 0;
                e.act_life = 0;
                e.f126 = 256;
                e.flags = 0;
            }
            // sub_3B230: ridge head.
            51 => {
                e.tick70 = 55;
                e.max_life = 0;
                e.act_life = 0;
                e.f26 = 256;
                e.f126 = 1024;
                e.flags = 0;
                e.f80 = 768;
                e.f82 = 768;
                e.f84 = 768;
            }
            // sub_3B690: building/castle spawner (fix-up follows).
            45 => {
                e.tick70 = 51;
                e.max_life = 30;
                e.f44 = 100;
                e.f26 = 4;
                e.flags = 9;
                e.f28 = 33;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            // sub_3ABE0 (:46946): the earthquake crevice walker —
            // life 128, step 256, RANDOM initial heading off its own
            // LCG, extents 1024/1024/0x4000, NOT map-linked (its
            // craters are the visible/audible part).
            15 => {
                e.tick70 = 15;
                e.max_life = 128;
                e.act_life = 128;
                e.f126 = 256;
                e.flags &= !8;
                e.f44 = 100;
                e.f26 = 0;
                let d = lcg32(&mut e.rand);
                e.f30 = (d & 0x7FF) as u16;
                e.f80 = 1024;
                e.f82 = 1024;
                e.f84 = 0x4000;
            }
            // sub_3ADB0 (:47008): the volcano eruption driver the
            // finished cone spawns. maxLife 10000 is NEVER counted
            // down — lifetime is the driver's own state machine
            // (sub_25EC0; see combat::eruption_tick).
            18 => {
                e.tick70 = 18;
                e.max_life = 10000;
                e.act_life = 10000;
                e.f44 = 200;
                e.f26 = 0;
                e.flags &= !8;
            }
            // sub_3B760 (:47545): the castle ground-leveling pass
            // (state 43); counter armed by its first tick.
            41 => {
                e.tick70 = 43;
                e.max_life = 10;
                e.act_life = 10;
                e.flags &= !8;
            }
            // sub_3B7B0 (:47567): the CASTLE painter (state 44,
            // sub_285C0) — the caller stamps level (+71) and the
            // castle link.
            42 => {
                e.tick70 = 44;
                e.max_life = 30;
                e.act_life = 30;
                e.flags &= !8;
            }
            // sub_3B6F0 (:47526): the castle UPGRADE token — state
            // 45, life 8, +44 = -1536 (inert dead weight, same
            // family as the possess flash), sprite row 41, 512
            // extents. The caller stamps owner + castle link.
            43 => {
                e.tick70 = 45;
                e.max_life = 8;
                e.act_life = 8;
                e.f44 = (-1536i16) as u16;
                e.flags &= !8;
                self.set_sprite(i, 41);
                self.ent[i].f80 = 512;
                self.ent[i].f82 = 512;
            }
            // sub_3B300 (model 34): the PORTAL vortex — sprite row 223,
            // 1-tile extents, spawned 640 alt units above ground (its
            // tick re-grounds it from the second turn), destination
            // defaulting to its own position (a THING post-init
            // overwrites it with the authored target). The LCG draw is
            // the original's random scatter of that default. Purged
            // unticked at LOAD time; persistent + drawable at runtime.
            34 => {
                e.tick70 = 36;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                e.dest_x = e.x;
                e.dest_y = e.y;
                lcg32(&mut e.rand);
                let (x, y, z) = (e.x, e.y, e.z);
                self.set_sprite(i, 223);
                self.ent[i].f80 = 256;
                self.ent[i].f82 = 256;
                self.ent[i].f84 = 256;
                self.link(i, x, y, z.wrapping_add(640));
            }
            // All remaining retail models (0, 1, 5, 6, 8, 13, 14, 15,
            // 17, 23, 25, 33, 38, 39, 44, 52, …): purged unticked, no
            // terrain writes, no global PRNG — slot churn only. Models
            // 13/14/15 draw from their (doomed) entity LCG; unobservable.
            _ => {
                e.tick70 = model as u8; // never dispatched
            }
        }
        Some(i)
    }

    /// sub_36DF0 (:43707): building placement fix-up. `bt` = the level
    /// entity's parent + 16, an index into the build table.
    pub(crate) fn building_fixup(&mut self, i: usize, bt: u16) {
        let def = self.assets.build_tab[bt as usize % self.assets.build_tab.len()];
        let (bw, bh) = (def.w as u16, def.h as u16);
        self.ent[i].f26 = 2;
        self.ent[i].f128 = ((bw * bh) >> 4) as i16;
        // Snap to the tile origin.
        let (px, py, pz) = (
            self.ent[i].x & 0xFF00,
            self.ent[i].y & 0xFF00,
            self.ent[i].z,
        );
        self.move_relink(i, px, py, pz);
        let e = &self.ent[i];
        let mut cx = ((e.x >> 8) as u8).wrapping_sub((bw >> 1) as u8);
        let cy = ((e.y >> 8) as u8).wrapping_sub((bh >> 1) as u8);
        if (cx as u16 + cy as u16) % 2 == 1 {
            // Odd corner parity: shift one tile east (relinks).
            let (nx, ny, nz) = (
                self.ent[i].x.wrapping_add(0x100),
                self.ent[i].y,
                self.ent[i].z,
            );
            self.move_relink(i, nx, ny, nz);
            cx = cx.wrapping_add(1);
        }
        let z = 32 * self.avg4(cx, cy, bh as u8, bw as u8) as i32;
        let e = &mut self.ent[i];
        e.f80 = ((bw << 8).wrapping_add(1280)) >> 1;
        e.f82 = ((bh << 8).wrapping_add(1280)) >> 1;
        e.f84 = 0x4000;
        e.act_life = 30;
        e.f44 = 2000;
        e.z = z as i16;
        e.f28 |= 2;
        e.f71 = bt as u8;
    }

    // ---- segment functions ----------------------------------------------

    /// sub_35900 (:42487): the spawn z both wall segments use.
    fn seg_z(&self, x1: i16, y1: u16, x2lo: u8, y2lo: u8) -> i16 {
        let h1 = self.t.height[tile(x1 as u8, y1 as u8)];
        let h2 = self.t.height[tile(x2lo, y2lo)];
        32 * h1.max(h2) as i16
    }

    /// Spawn one wall piece (ctor model 27, sub_3B000 :47142).
    fn spawn_wall_piece(&mut self, x: i16, y: u16, z: i16, tick: u8, run: u16) {
        if let Some(i) = self.new_event() {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 27;
            e.tick70 = tick;
            e.max_life = 2;
            e.act_life = 2;
            e.f44 = ((z >> 5) + 48) as u16;
            e.f26 = run as i16;
            e.flags = 0;
            let (px, py) = ((x as u16) << 8, y << 8);
            self.link(i, px, py, z);
        }
    }

    /// sub_35960 (:42513), model 28: decompose the wrapped delta into a
    /// staircase of `|major|/10 + 1` alternating axis-aligned pieces
    /// (remainders folded into the first step) and spawn a wall-strip
    /// event per piece.
    fn segment_wall(&mut self, x1: i16, y1: u16, x2: i16, y2: i16) {
        let mut dx = Self::wrap_delta(x1, x2);
        let mut dy = Self::wrap_delta(y1 as i16, y2);
        if dx == 0 && dy == 0 {
            return;
        }
        let (mut cx, mut cy) = (x1, y1);
        let (mut ex, mut ey) = (x2 as u8, y2 as u8);
        if dx < 0 {
            dy = -dy;
            dx = -dx;
            // Swap endpoints (only the low bytes of the far end are used).
            let (sx, sy) = (cx as u8, cy as u8);
            cx = ex as i16;
            cy = ey as u16;
            ex = sx;
            ey = sy;
        }
        if dy.abs() >= dx {
            let steps = (dy / 10).abs() + 1;
            let (qy, mut ry) = (dy / steps, dy % steps);
            let (qx, mut rx) = (dx / steps, dx % steps);
            for _ in 0..steps {
                let z = self.seg_z(cx, cy, ex, ey as u8);
                if qy >= 0 {
                    self.spawn_wall_piece(cx, cy, z, 28, (ry + qy) as u16);
                } else {
                    self.spawn_wall_piece(cx, cy, z, 27, (-qy - ry) as u16);
                }
                cy = cy.wrapping_add((qy + ry) as u16);
                let z = self.seg_z(cx, cy, ex, ey as u8);
                self.spawn_wall_piece(cx, cy, z, 29, (rx + qx) as u16);
                cx = cx.wrapping_add((rx + qx) as i16);
                ry = 0;
                rx = 0;
            }
        } else {
            let steps = dx / 10 + 1;
            let (qx, mut rx) = (dx / steps, dx % steps);
            let (qy, mut ry) = (dy / steps, dy % steps);
            for _ in 0..steps {
                let z = self.seg_z(cx, cy, ex, ey as u8);
                self.spawn_wall_piece(cx, cy, z, 29, (rx + qx) as u16);
                cx = cx.wrapping_add((rx + qx) as i16);
                let z = self.seg_z(cx, cy, ex, ey as u8);
                if qy >= 0 {
                    self.spawn_wall_piece(cx, cy, z, 28, (ry + qy) as u16);
                } else {
                    self.spawn_wall_piece(cx, cy, z, 27, (-qy - ry) as u16);
                }
                cy = cy.wrapping_add((qy + ry) as u16);
                rx = 0;
                ry = 0;
            }
        }
    }

    /// sub_35BF0 (:42629), model 29: split the delta into a diagonal
    /// run and an axis-aligned run; spawn a track-painter event (ctor
    /// model 30, byte70 32) for each.
    fn segment_track(&mut self, x1: i16, y1: i16, x2: i16, y2: i16) {
        let dx = Self::wrap_delta(x1, x2);
        let dy = Self::wrap_delta(y1, y2);
        let sdx = dx.signum();
        let sdy = dy.signum();
        let adx = dx.abs();
        let ady = dy.abs();
        let diag = adx.min(ady);
        let rest = (ady - adx).abs();
        let (rest_dx, rest_dy) = if adx <= ady { (0, sdy) } else { (sdx, 0) };
        let spawn_track = |g: &mut Self, x: i16, y: i16, count: i32, stx: i32, sty: i32| {
            if let Some(i) = g.new_event() {
                let e = &mut g.ent[i];
                e.class64 = 10;
                e.model65 = 30;
                e.tick70 = 32;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                e.f26 = count as i16;
                e.f30 = stx as u16;
                e.f32 = sty as u16;
                let (px, py) = ((x as u16) << 8, (y as u16) << 8);
                g.link(i, px, py, 0);
            }
        };
        spawn_track(self, x1, y1, diag, sdx, sdy);
        spawn_track(
            self,
            x1.wrapping_add((diag * sdx) as i16),
            y1.wrapping_add((diag * sdy) as i16),
            rest,
            rest_dx,
            rest_dy,
        );
    }

    /// sub_35D30 (:42697), model 31: spawn a canyon head aimed at the
    /// child, with a life of `distance >> 8` tiles.
    fn segment_canyon(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) {
        let (ax, ay) = (x1 << 8, y1 << 8);
        let (bx, by) = (x2 << 8, y2 << 8);
        let ang = Self::angle_between(ax, ay, bx, by);
        let dist = Self::dist_between(ax, ay, bx, by);
        if let Some(i) = self.new_event() {
            let z = 32 * self.t.height[tile(x1 as u8, y1 as u8)] as i16;
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 32;
            e.tick70 = 34;
            e.max_life = 0;
            e.f126 = 256;
            e.flags = 0;
            e.x = ax;
            e.y = ay;
            e.z = z;
            e.f30 = ang;
            e.act_life = (dist >> 8) as i32;
        }
    }

    /// sub_35DE0 (:42722), model 50: spawn a ridge head, life =
    /// `distance / 1024` (one raise every 4 tiles).
    fn segment_ridge(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) {
        let (ax, ay) = (x1 << 8, y1 << 8);
        let (bx, by) = (x2 << 8, y2 << 8);
        let ang = Self::angle_between(ax, ay, bx, by);
        let dist = Self::dist_between(ax, ay, bx, by);
        if let Some(i) = self.new_event() {
            let z = 16 * self.t.height[tile(x1 as u8, y1 as u8)] as i16;
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 51;
            e.tick70 = 55;
            e.max_life = 0;
            e.f26 = 256;
            e.f126 = 1024;
            e.flags = 0;
            e.f80 = 768;
            e.f82 = 768;
            e.f84 = 768;
            e.x = ax;
            e.y = ay;
            e.z = z;
            e.f30 = ang;
            e.act_life = dist as i32 / 1024;
        }
    }

    // ---- the event loop -------------------------------------------------

    /// sub_36620 (:43181): one global PRNG step, then sweep the pool to
    /// fixpoint. Eligibility is tested on the MODEL; the handler is
    /// selected by byte 70.
    fn event_loop(&mut self) {
        lcg32(&mut self.rand);
        loop {
            let mut run_again = false;
            for i in 1..self.ent.len() {
                if self.ent[i].class64 == 0 {
                    continue;
                }
                if self.ent[i].class64 != 10 {
                    self.ent[i].flags |= 0x400;
                } else {
                    let model = self.ent[i].model65;
                    let eligible = match model {
                        0..=0x1A => matches!(model, 9..=0xB),
                        0x1B..=0x20 => true,
                        0x21..=0x2C => false,
                        0x2D => self.ent[i].tick70 == 51,
                        0x2E..=0x31 => false,
                        0x32 | 0x33 => true,
                        _ => false,
                    };
                    if eligible {
                        run_again = true;
                        self.tick(i, None);
                    } else if model != 0x2D {
                        self.ent[i].flags |= 0x400;
                    }
                }
                if self.ent[i].flags & 0x400 != 0 {
                    self.free_entity(i);
                }
            }
            if !run_again {
                break;
            }
        }
    }

    /// str_255998 (:4856) dispatch by byte 70. `ctx` = the player
    /// context at RUNTIME (None during the load fixpoint): the
    /// terrain deformers broadcast ch0 damage + the loop-10 rumble,
    /// which only matter — and only have a listener — once the world
    /// runs (deviation: the original's load pass broadcasts into the
    /// half-built pool too; nothing observable survives it).
    pub(crate) fn tick(&mut self, i: usize, ctx: Option<&crate::mc1::mobs::MobCtx>) {
        match self.ent[i].tick70 {
            9 => self.tick_hill(i, ctx),
            10 => self.tick_dish(i),
            11 => self.tick_digger(i, ctx),
            15 => self.tick_quake_walker(i),
            27 => self.tick_wall_neg_y(i),
            28 => self.tick_wall_pos_y(i),
            29 => self.tick_wall_pos_x(i),
            32 => self.tick_track(i),
            34 => self.tick_canyon_head(i),
            43 => self.tick_castle_leveler(i),
            44 => self.tick_castle_painter(i),
            45 => self.tick_upgrade_token(i),
            51 => self.tick_building(i),
            55 => self.tick_ridge_head(i, ctx),
            // sub_253E0 rows (30, 31, 33, 54, …): pure self-kill.
            _ => self.ent[i].flags |= 0x400,
        }
    }

    /// sub_25470 (:28302), byte70 9: growing hill; finish punches a
    /// -40 pit at the center and spawns a transient model-18 marker
    /// (owner passed on — the eruption driver inherits immunity).
    /// Every growth tick is a KILL ZONE: full +44 (2000) on ch0 over
    /// the live extents (:28327, via the sub_127E0 writer — its
    /// wizard +50=30 ground-ride stamp is the mortality track) plus
    /// the loop-10 rumble (:28328).
    fn tick_hill(&mut self, i: usize, ctx: Option<&crate::mc1::mobs::MobCtx>) {
        let life = self.ent[i].act_life;
        self.ent[i].f26 = self.ent[i].f26.wrapping_add(1);
        self.ent[i].act_life = life - 1;
        let finish = if life < 0 {
            true
        } else {
            let r = lcg32(&mut self.ent[i].rand);
            let hi = self.ent[i].f26 as i32 / 6;
            self.dig_disc(i, 0, hi, (r % 9) as i16, false)
        };
        if finish {
            self.dig_disc(i, 0, 0, -40, false);
            let (x, y, own) = (self.ent[i].x, self.ent[i].y, self.ent[i].id24);
            let z = self.ground_z(x, y) as i16;
            if let Some(m) = self.spawn_creator(18, x, y, z) {
                self.ent[m].id24 = own; // :28322
            }
            self.ent[i].flags |= 0x400;
        } else if let Some(ctx) = ctx {
            let amt = self.ent[i].f44 as u32;
            self.area_write(i, 0, amt, ctx, false, true);
            self.snd(10, i);
        }
    }

    /// sub_25570 (:28333), byte70 10: one-shot shallow dish, honoring
    /// building protection.
    fn tick_dish(&mut self, i: usize) {
        let e = self.ent[i];
        if !self.on_water(e.x, e.y) {
            let r = lcg32(&mut self.ent[i].rand);
            let hi = (self.ent[i].f80 >> 8) as i32;
            self.dig_disc(i, 0, hi, -((r % 7) as i16), true);
        }
        self.ent[i].flags |= 0x400;
    }

    /// sub_25670 (:28379), byte70 11: expanding -3 crater; radius grows
    /// only when the event's pool slot is divisible by 3. Every
    /// surviving tick: ch0 damage — full +44 before the phase-2 flag
    /// sets, +44/25 after (:28396-400) — and the loop-10 rumble
    /// (:28421).
    fn tick_digger(&mut self, i: usize, ctx: Option<&crate::mc1::mobs::MobCtx>) {
        if self.ent[i].f63 % 3 == 0 {
            self.ent[i].f26 = self.ent[i].f26.wrapping_add(1);
        }
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        let e = self.ent[i];
        if life < 0 || self.on_water(e.x, e.y) {
            self.ent[i].flags |= 0x400;
            return;
        }
        if let Some(ctx) = ctx {
            let amt = if self.ent[i].flags & 2 != 0 {
                self.ent[i].f44 as u32 / 25
            } else {
                self.ent[i].f44 as u32
            };
            self.area_write(i, 0, amt, ctx, false, true);
        }
        let radius = (e.f80 >> 8) as i16;
        let mut upto = e.f26;
        if upto > radius - 1 {
            upto = radius - 1;
            if e.flags & 2 == 0 {
                self.dig_disc_minus3(i, radius as i32, radius as i32);
            }
        }
        self.ent[i].flags |= 2;
        self.dig_disc_minus3(i, 0, upto as i32);
        if ctx.is_some() {
            self.snd(10, i); // :28421
        }
    }

    /// sub_26670 (:29030), byte70 27: wall strip toward -Y.
    fn tick_wall_neg_y(&mut self, i: usize) {
        let e = self.ent[i];
        let x = ((e.x as u32 + 128) >> 8) as u8;
        let mut y = (((e.y as u32 + 128) >> 8) as u8).wrapping_add(2);
        let w = e.act_life as u16; // strip thickness (2)
        for _ in 0..w.wrapping_add(e.f26 as u16) {
            self.t.angle[tile(x.wrapping_sub(1), y)] |= 0x80;
            let mut t = tile(x, y);
            for _ in 0..w {
                self.wall_raise(t);
                t = (t + 1) & 0xFFFF;
            }
            self.t.angle[t] |= 0x80;
            y = y.wrapping_sub(1);
        }
        self.ent[i].flags |= 0x400;
    }

    /// sub_26560 (:28999), byte70 28: wall strip toward +Y, x aligned
    /// even then shifted -1.
    fn tick_wall_pos_y(&mut self, i: usize) {
        let e = self.ent[i];
        let mut x = ((e.x as u32 + 128) >> 8) as u8;
        let mut y = ((e.y as u32 + 128) >> 8) as u8;
        if x & 1 == 1 {
            x = x.wrapping_add(1);
        }
        let w = e.act_life as u16;
        x = x.wrapping_sub(w as u8).wrapping_add(1);
        for _ in 0..w.wrapping_add(e.f26 as u16) {
            self.t.angle[tile(x.wrapping_sub(1), y)] |= 0x80;
            let mut t = tile(x, y);
            for _ in 0..w {
                self.wall_raise(t);
                t = (t + 1) & 0xFFFF;
            }
            self.t.angle[t] |= 0x80;
            y = y.wrapping_add(1);
        }
        self.ent[i].flags |= 0x400;
    }

    /// sub_26760 (:29059), byte70 29: wall strip toward +X, aligned on
    /// (x+y) parity; border rows above and below.
    fn tick_wall_pos_x(&mut self, i: usize) {
        let e = self.ent[i];
        let mut x = ((e.x as u32 + 128) >> 8) as u8;
        let y = ((e.y as u32 + 128) >> 8) as u8;
        if (x as u16 + y as u16) % 2 == 1 {
            x = x.wrapping_add(1);
        }
        let run = e.f26 as u16;
        let mut t = tile(x, y).wrapping_sub(256) & 0xFFFF; // row y-1
        for _ in 0..run {
            self.t.angle[t] |= 0x80;
            t = (t + 1) & 0xFFFF;
        }
        let mut yy = y;
        for _ in 0..e.act_life as u16 {
            let mut t = tile(x, yy);
            for _ in 0..run {
                self.wall_raise(t);
                t = (t + 1) & 0xFFFF;
            }
            yy = yy.wrapping_add(1);
        }
        let mut t = tile(x, yy);
        for _ in 0..run {
            self.t.angle[t] |= 0x80;
            t = (t + 1) & 0xFFFF;
        }
        self.ent[i].flags |= 0x400;
    }

    /// The shared wall raise op: +48 height (u8 wrap, no clamp) unless
    /// the tile is already wall (type 8) with a type-8 west neighbor
    /// and no 4-neighbor towering ≥ 31 above (sub_264D0, :28966), then
    /// stamp type 8 on the 2x2 and reshade.
    fn wall_raise(&mut self, t: usize) {
        let raise = if self.t.tile_type[t] != 8 {
            true
        } else {
            let (cx, cy) = (tx(t), ty(t));
            let lim = self.t.height[t] as i32 + 30;
            self.t.tile_type[tile(cx.wrapping_sub(1), cy)] != 8
                || self.t.height[tile(cx.wrapping_sub(1), cy)] as i32 > lim
                || self.t.height[tile(cx.wrapping_add(1), cy)] as i32 > lim
                || self.t.height[tile(cx, cy.wrapping_add(1))] as i32 > lim
                || self.t.height[tile(cx, cy.wrapping_sub(1))] as i32 > lim
        };
        if raise {
            self.t.height[t] = self.t.height[t].wrapping_add(48);
        }
        self.set_type_2x2(t, 8);
    }

    /// sub_26890 (:29106), byte70 32: track painter — walk f26 tiles
    /// stepping (f30, f32), stamping slope 1 + protected retexture.
    fn tick_track(&mut self, i: usize) {
        let e = self.ent[i];
        let mut x = ((e.x as u32 + 128) >> 8) as u8;
        let mut y = ((e.y as u32 + 128) >> 8) as u8;
        let mut n = e.f26 as i32;
        while n != 0 {
            let t = tile(x, y);
            self.t.angle[t] = (self.t.angle[t] & 0xF0) | 1;
            self.recompute_protected(x, y, x, y);
            x = x.wrapping_add(e.f30 as u8);
            y = y.wrapping_add(e.f32 as u8);
            n -= 1;
        }
        self.ent[i].flags |= 0x400;
    }

    /// sub_25990 (:28534), byte70 15: the EARTHQUAKE crevice walker
    /// (spell 6's authentic payload — direct import). Water under it
    /// counts a ledger up (dry ticks count it back down); dies when
    /// the ledger passes 8 or life runs out. Each tick: wander the
    /// heading ±45, step 256 units, and drop a 10-tick m11 digger at
    /// the new spot with the walker's extents + owner. The rumble is
    /// the diggers' own loop-10.
    fn tick_quake_walker(&mut self, i: usize) {
        let (x0, y0) = (self.ent[i].x, self.ent[i].y);
        if self.on_water(x0, y0) {
            self.ent[i].f26 += 1;
        } else if self.ent[i].f26 > 0 {
            self.ent[i].f26 -= 1;
        }
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 || self.ent[i].f26 > 8 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let d = lcg32(&mut self.ent[i].rand);
        self.ent[i].f30 = ((d % 0x5B) as u16)
            .wrapping_add(self.ent[i].f30)
            .wrapping_sub(45)
            & 0x7FF;
        let (mut x, mut y) = (self.ent[i].x, self.ent[i].y);
        Self::advance(&mut x, &mut y, self.ent[i].f30, 256);
        self.ent[i].x = x;
        self.ent[i].y = y;
        let e = self.ent[i];
        if let Some(dg) = self.spawn_creator(11, x, y, e.z) {
            let g = &mut self.ent[dg];
            g.f80 = e.f80; // dword copy +80 covers both axes (:28564)
            g.f82 = e.f82;
            g.f84 = e.f84;
            g.act_life = 10;
            g.id24 = e.id24;
        }
    }

    /// sub_26920 (:29122), byte70 34: canyon head — spawn a 3-tick
    /// digger at the current position, advance one tile along the
    /// heading; stop on distance or water.
    fn tick_canyon_head(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        let e = self.ent[i];
        if life < 0 || self.on_water(e.x, e.y) {
            self.ent[i].flags |= 0x400;
            return;
        }
        if let Some(d) = self.spawn_creator(11, e.x, e.y, e.z) {
            self.ent[d].act_life = 2;
            self.ent[d].f84 = e.f84;
            self.ent[d].id24 = e.id24; // :29141 — owner immunity chains
        }
        let (mut x, mut y) = (self.ent[i].x, self.ent[i].y);
        Self::advance(&mut x, &mut y, self.ent[i].f30, self.ent[i].f126);
        self.ent[i].x = x;
        self.ent[i].y = y;
    }

    /// sub_269A0 (:29147), byte70 55: ridge head — raise a radius-3
    /// disc by rand%15+10, advance 4 tiles. Each successful raise:
    /// full +44 on ch0 + the loop-10 rumble (:29163-64).
    fn tick_ridge_head(&mut self, i: usize, ctx: Option<&crate::mc1::mobs::MobCtx>) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        let e = self.ent[i];
        if life < 0 || self.on_water(e.x, e.y) {
            self.ent[i].flags |= 0x400;
            return;
        }
        let r = lcg32(&mut self.ent[i].rand);
        self.dig_disc(i, 0, 1024, (r % 0xF + 10) as i16, false);
        if let Some(ctx) = ctx {
            let amt = self.ent[i].f44 as u32;
            self.area_write(i, 0, amt, ctx, false, false);
            self.snd(10, i);
        }
        let (mut x, mut y) = (self.ent[i].x, self.ent[i].y);
        Self::advance(&mut x, &mut y, self.ent[i].f30, self.ent[i].f126);
        self.ent[i].x = x;
        self.ent[i].y = y;
    }

    /// sub_27D30 (:29993), byte70 51: building construction — flatten
    /// the RLE footprint toward the placement height each tick, paint
    /// every 5th tick and at life 1; on the final tick retile the full
    /// rect and become a persistent (inert) castle entity.
    fn tick_building(&mut self, i: usize) {
        let e = self.ent[i];
        let cx = ((e.x as u32 + 128) >> 8) as u8;
        let cy = ((e.y as u32 + 128) >> 8) as u8;
        let target = (e.z >> 5) as i32;
        let def = self.assets.build_tab[e.f71 as usize % self.assets.build_tab.len()];
        let (w, h) = (def.w as u16, def.h as u16);
        let (half_w, half_h) = ((w >> 1) as u8, (h >> 1) as u8);
        self.ent[i].act_life -= 1;
        let life = self.ent[i].act_life;
        let x0 = cx.wrapping_sub(half_w);
        let y0 = cy.wrapping_sub(half_h);
        if life != 0 {
            self.flatten_build_row(e.f71 as usize, cx, cy, target, life);
            if life % 5 == 0 || life == 1 {
                self.paint_build_row(e.f71 as usize, cx, cy);
            }
        } else {
            // Final tick: retile the whole rect, become a castle.
            self.recompute_protected(x0, y0, cx.wrapping_add(half_w), cy.wrapping_add(half_h));
            // byte70 == 51 (the only load-time case): persist as an
            // inert entity (byte70 52) with perimeter smoothing.
            self.ent[i].act_life = self.ent[i].f44 as i32;
            self.ent[i].flags |= 1;
            self.ent[i].tick70 = 52;
            let (x, y) = (self.ent[i].x, self.ent[i].y);
            self.ent[i].z = self.ground_z(x, y) as i16;
            self.smooth_perimeter(cx, cy, half_h as u16, half_w as u16, 2);
            self.smooth_perimeter(cx, cy, half_h as u16, half_w as u16, 5);
        }
    }

    /// One flatten pass over build-table row `bt` centered on tile
    /// (cx, cy): the shared cell-code goal decode of sub_27D30
    /// (:30040-70) / sub_285C0 (:30541-94), stepping each tile's
    /// height toward its goal by /divisor.
    fn flatten_build_row(&mut self, bt: usize, cx: u8, cy: u8, target: i32, divisor: i32) {
        let def = self.assets.build_tab[bt % self.assets.build_tab.len()];
        let (w, h) = (def.w as u16, def.h as u16);
        let x0 = cx.wrapping_sub((w >> 1) as u8);
        let y0 = cy.wrapping_sub((h >> 1) as u8);
        let mut rows = h;
        let (mut x, mut y) = (x0, y0);
        let mut c = def.offset as usize;
        while rows != 0 {
            let ctl = self.assets.build_dat[c] as i8;
            c += 1;
            if ctl == 0 {
                y = y.wrapping_add(1);
                rows -= 1;
                x = x0;
                continue;
            }
            if ctl < 0 {
                x = x.wrapping_add((-(ctl as i32)) as u8);
                continue;
            }
            for _ in 0..ctl {
                let b = self.assets.build_dat[c];
                c += 1;
                let t = tile(x, y);
                let goal = if b < 0xF {
                    if b > 6 { Some(target) } else { None }
                } else if b >> 4 == 3 {
                    match (b % 16) % 3 {
                        1 => Some(target + 12),
                        2 => Some(target + 16),
                        _ => None,
                    }
                } else {
                    let lo = b % 16;
                    if lo != 0 {
                        Some(4 * (lo as i32 - 1) + target)
                    } else {
                        None
                    }
                };
                if let Some(goal) = goal {
                    let angle_before = self.t.angle[t];
                    let hh = self.t.height[t] as i32;
                    self.t.height[t] = self.t.height[t].wrapping_add(((goal - hh) / divisor) as u8);
                    if angle_before & 7 == 0 {
                        self.t.angle[t] = (angle_before & 0xF0) | 1;
                        self.recompute_protected(x, y, x, y);
                    }
                }
                x = x.wrapping_add(1);
            }
        }
    }

    /// sub_40E20 (:51729): the castle-transformation kill, one pass
    /// over the NEW level's RLE footprint. Per occupied cell, walking
    /// the tile's entity chain: anything owned by the castle owner is
    /// SPARED (:51744 — broader than the caster: your skeletons
    /// survive your own castle); class-2 scenery is deleted outright
    /// (:51749); class-5 creatures die instantly at any HP (life =
    /// −1, killer = the owner → kill credit + normal corpse drops)
    /// EXCEPT models 6/8/16 (:51753 — boss-tier exemptions). Every
    /// other class (wizards, balloons, castles, projectiles,
    /// effects) is structurally immune (:51760 default: break).
    fn build_footprint_kill(&mut self, bt: usize, cx: u8, cy: u8, owner: u16) {
        let def = self.assets.build_tab[bt % self.assets.build_tab.len()];
        let (w, h) = (def.w as u16, def.h as u16);
        let x0 = cx.wrapping_sub((w >> 1) as u8);
        let y0 = cy.wrapping_sub((h >> 1) as u8);
        let mut rows = h;
        let (mut x, mut y) = (x0, y0);
        let mut c = def.offset as usize;
        while rows != 0 {
            let ctl = self.assets.build_dat[c] as i8;
            c += 1;
            if ctl == 0 {
                y = y.wrapping_add(1);
                rows -= 1;
                x = x0;
                continue;
            }
            if ctl < 0 {
                x = x.wrapping_add((-(ctl as i32)) as u8);
                continue;
            }
            for _ in 0..ctl {
                let b = self.assets.build_dat[c];
                c += 1;
                if b != 0 {
                    let mut j = self.map_entity[tile(x, y)] as usize;
                    while j != 0 {
                        let next = self.ent[j].next20 as usize;
                        if self.ent[j].id24 != owner && self.ent[j].flags & 0x400 == 0 {
                            match self.ent[j].class64 {
                                2 => self.free_entity(j),
                                5 if !matches!(self.ent[j].model65, 6 | 8 | 16) => {
                                    self.ent[j].act_life = -1;
                                    self.ent[j].f38 = owner;
                                    self.ent[j].f40 = owner;
                                }
                                _ => {}
                            }
                        }
                        j = next;
                    }
                }
                x = x.wrapping_add(1);
            }
        }
    }

    /// One paint pass over build-table row `bt` (the shared tile-type
    /// decode of sub_27D30/sub_285C0 via sub_33800).
    fn paint_build_row(&mut self, bt: usize, cx: u8, cy: u8) {
        let def = self.assets.build_tab[bt % self.assets.build_tab.len()];
        let (w, h) = (def.w as u16, def.h as u16);
        let x0 = cx.wrapping_sub((w >> 1) as u8);
        let y0 = cy.wrapping_sub((h >> 1) as u8);
        let mut rows = h;
        let (mut x, mut y) = (x0, y0);
        let mut c = def.offset as usize;
        while rows != 0 {
            let ctl = self.assets.build_dat[c] as i8;
            c += 1;
            if ctl == 0 {
                y = y.wrapping_add(1);
                rows -= 1;
                x = x0;
                continue;
            }
            if ctl < 0 {
                x = x.wrapping_add((-(ctl as i32)) as u8);
                continue;
            }
            for _ in 0..ctl {
                let b = self.assets.build_dat[c];
                c += 1;
                let t = tile(x, y);
                match b >> 4 {
                    0 => {
                        let k = b % 7;
                        if k != 0 {
                            self.paint(k as i8, 7, t, k - 1);
                        }
                    }
                    hi @ 1..=2 => self.paint(0, b as i8, t, hi + 7),
                    3 => {
                        let lo = b % 16;
                        self.paint((lo % 3) as i8, (lo / 3 + 10) as i8, t, lo / 3 + 10)
                    }
                    hi => self.paint(0, b as i8, t, hi + 11),
                }
                x = x.wrapping_add(1);
            }
        }
    }

    /// sub_285C0 (:30445), byte70 44: the CASTLE painter — the m42
    /// event a castle level-up spawns. 20 ticks (counter +26 armed
    /// to 19 on the first tick); each tick flattens the CUMULATIVE
    /// footprints of build rows 1..=level toward the event z, paints
    /// on every 7th counter value and the last, and the finish
    /// stamps the protection bit over the level footprint and hands
    /// the castle (f146) to sub-state 5 (:30703-08).
    fn tick_castle_painter(&mut self, i: usize) {
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            self.ent[i].f26 = 19;
        }
        let e = self.ent[i];
        let cx = ((e.x as u32 + 128) >> 8) as u8;
        let cy = ((e.y as u32 + 128) >> 8) as u8;
        let target = (e.z >> 5) as i32;
        let level = e.f71.clamp(1, 8) as usize;
        let divisor = (e.f26 as i32 + 1).max(1);
        for r in 1..=level {
            self.flatten_build_row(r, cx, cy, target, divisor);
        }
        // THE CASTLE WEAPON (sub_40E20 :51729, called per footprint
        // tile per paint tick :30631-34): the rising transformation
        // EXECUTES what stands on it — but only under the upgrade-
        // commit painter (the +18&1 kill bit, :56492); the damage
        // repaint kills nothing.
        if e.flags & 0x10000 != 0 {
            self.build_footprint_kill(level, cx, cy, e.id24);
        }
        if e.f26 % 7 == 0 || e.f26 == 0 {
            for r in 1..=level {
                self.paint_build_row(r, cx, cy);
            }
        }
        self.ent[i].f26 -= 1;
        if self.ent[i].f26 < 0 {
            // Finish (:30697-707): PROMOTE pending protection — only
            // tiles carrying bit 0x08 flip to 0x80; unpainted cells of
            // the RLE footprint stay unprotected.
            let def = self.assets.build_tab[level % self.assets.build_tab.len()];
            let x0 = cx.wrapping_sub((def.w >> 1) as u8);
            let y0 = cy.wrapping_sub((def.h >> 1) as u8);
            for dy in 0..def.h {
                for dx in 0..def.w {
                    let t = tile(x0.wrapping_add(dx), y0.wrapping_add(dy));
                    if self.t.angle[t] & 8 != 0 {
                        self.t.angle[t] = (self.t.angle[t] & 0x77) | 0x80;
                    }
                }
            }
            let c = self.ent[i].f146 as usize;
            if c != 0 && self.ent[c].class64 == 3 && self.ent[c].model65 == 2 {
                self.ent[c].f59 = 5;
            }
            self.ent[i].flags |= 0x400;
        }
    }

    /// sub_28200 (:30284), byte70 43: the castle ground LEVELER — a
    /// uniform vertical TRANSLATION of the whole sculpted footprint,
    /// never a flatten: each tick every w*h tile gets the SAME signed
    /// step, so the painted tower rides along with the base. Init
    /// (:30429-41): counter (+26) = 10, current (+48, ours f28) =
    /// event z>>5, target (+44) = the OUTSIDE 4-corner average
    /// sub_361C0(x0-1, y0-1, h+2, w+2) clamped 220; already equal →
    /// straight to finish. Stepping (:30333-36): step = (target -
    /// current) / counter (signed truncating div), current += step;
    /// counter 10..2 add step to all tiles (:30386-416); counter 1
    /// adds + downgrades protection 0x80→0x08 (:30337-62) then
    /// counter = -10; -10..-2 idle; -1 restores 0x08→0x80
    /// (:30363-85). Finish (counter 0, :30419-27): castle sub-state
    /// 2, castle site z = 32*current, perimeter smooth depth 3,
    /// despawn. (The original also aborts to finish when castle +50
    /// [rebuild-pending] goes nonzero — field unported, always 0.)
    fn tick_castle_leveler(&mut self, i: usize) {
        let e = self.ent[i];
        let cx = ((e.x as u32 + 128) >> 8) as u8;
        let cy = ((e.y as u32 + 128) >> 8) as u8;
        let def = self.assets.build_tab[e.f71 as usize % self.assets.build_tab.len()];
        let x0 = cx.wrapping_sub((def.w >> 1) as u8);
        let y0 = cy.wrapping_sub((def.h >> 1) as u8);
        if e.flags & 2 == 0 {
            self.ent[i].flags |= 2;
            self.ent[i].f26 = 10;
            let cur = e.z >> 5;
            self.ent[i].f28 = cur as u16;
            let mut tgt = self.avg4(
                x0.wrapping_sub(1),
                y0.wrapping_sub(1),
                def.h.wrapping_add(2),
                def.w.wrapping_add(2),
            );
            if tgt > 220 {
                tgt = 220;
            }
            self.ent[i].f44 = tgt;
            if cur == tgt as i16 {
                self.ent[i].f26 = 0;
            }
            return;
        }
        let counter = self.ent[i].f26;
        if counter != 0 {
            let step = (self.ent[i].f44 as i32 - self.ent[i].f28 as i16 as i32) / counter as i32;
            self.ent[i].f28 = (self.ent[i].f28 as i16 as i32 + step) as i16 as u16;
            let add = |g: &mut Self, unstamp: bool| {
                for gy in 0..def.h {
                    for gx in 0..def.w {
                        let t = tile(x0.wrapping_add(gx), y0.wrapping_add(gy));
                        if unstamp && g.t.angle[t] & 0x80 != 0 {
                            g.t.angle[t] = (g.t.angle[t] & 0x77) | 8;
                        }
                        g.t.height[t] = (g.t.height[t] as i32 + step) as u8;
                    }
                }
            };
            if counter == 1 {
                add(self, true);
                self.ent[i].f26 = -10;
            } else if counter == -1 {
                for gy in 0..def.h {
                    for gx in 0..def.w {
                        let t = tile(x0.wrapping_add(gx), y0.wrapping_add(gy));
                        if self.t.angle[t] & 8 != 0 {
                            self.t.angle[t] = (self.t.angle[t] & 0x77) | 0x80;
                        }
                    }
                }
                self.ent[i].f26 += 1;
            } else if counter < 0 {
                self.ent[i].f26 += 1;
            } else {
                add(self, false);
                self.ent[i].f26 -= 1;
            }
        } else {
            let c = self.ent[i].f146 as usize;
            if c != 0 && self.ent[c].class64 == 3 && self.ent[c].model65 == 2 {
                self.ent[c].f59 = 2;
                // Castle SITE z (+154) = 32 * final — the next
                // build's datum (:30424); the entity z refreshes
                // from live ground on its own tick.
                self.ent[c].site_z = 32 * self.ent[i].f28 as i16;
            }
            self.smooth_perimeter(cx, cy, (def.h >> 1) as u16, (def.w >> 1) as u16, 3);
            self.ent[i].flags |= 0x400;
        }
    }

    /// The level-init starting-castle terrain replay (the sub_279D0
    /// loop :54982-93): the cumulative build-row footprints stamped
    /// INSTANTLY (divisor-1 flatten + paint per row), protection
    /// promoted like the painter finish (:30697-707). Rival wizards
    /// with a nonzero level-tail castle level spawn on this.
    pub(crate) fn stamp_castle_terrain(&mut self, rows: usize, cx: u8, cy: u8, target: i32) {
        let rows = rows.clamp(1, 8);
        for r in 1..=rows {
            self.flatten_build_row(r, cx, cy, target, 1);
            self.paint_build_row(r, cx, cy);
        }
        let def = self.assets.build_tab[rows % self.assets.build_tab.len()];
        let x0 = cx.wrapping_sub((def.w >> 1) as u8);
        let y0 = cy.wrapping_sub((def.h >> 1) as u8);
        for dy in 0..def.h {
            for dx in 0..def.w {
                let t = tile(x0.wrapping_add(dx), y0.wrapping_add(dy));
                if self.t.angle[t] & 8 != 0 {
                    self.t.angle[t] = (self.t.angle[t] & 0x77) | 0x80;
                }
            }
        }
    }

    /// sub_37150 (:43798) + the HP ladder: size a castle entity's
    /// extents and life to its level (level 0 keeps the ctor shell).
    pub(crate) fn castle_extents(&mut self, i: usize, lvl: u8) {
        if lvl >= 1 {
            let def = self.assets.build_tab[lvl as usize % self.assets.build_tab.len()];
            let e = &mut self.ent[i];
            e.f80 = (((def.w as u16) << 8).wrapping_add(1280)) >> 1;
            e.f82 = (((def.h as u16) << 8).wrapping_add(1280)) >> 1;
            e.f84 = 0x4000;
        }
        let hp = Self::CASTLE_HP[(lvl as usize).min(7)];
        self.ent[i].max_life = hp;
        self.ent[i].act_life = hp as i32;
        self.ent[i].site_z = self.ent[i].z;
    }

    /// sub_293D0 (:31009), byte70 45: the castle UPGRADE token — the
    /// delivery receipt the upgrade ball morphs into at the castle.
    /// One armed tick: touching the linked castle (the original
    /// resolves it through wizext +50 — same castle) → ch5 mail
    /// {10, owner} (:31033-34) and despawn; the fall-through deletes
    /// it on the next tick regardless (the ball already carried it
    /// to the castle — the token is not a traveler).
    fn tick_upgrade_token(&mut self, i: usize) {
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            let c = self.ent[i].f146 as usize;
            if c != 0
                && self.ent[c].class64 == 3
                && self.ent[c].model65 == 2
                && self.ent[c].flags & 0x400 == 0
                && self.ent_overlap(i, c)
            {
                self.ent[c].mail[5] = (10, self.ent[i].id24);
                self.ent[i].flags |= 0x400;
            }
            return;
        }
        self.ent[i].flags |= 0x400;
    }

    /// sub_47DD0 (:56617): castle mana capacity by level (level 0 =
    /// the pre-tower shell; player castles occupy 1..=7).
    pub(crate) const CASTLE_CAP: [i32; 8] =
        [5000, 10000, 20000, 40000, 80000, 160000, 320000, 30_000_000];

    /// sub_12C50 (:17616): the upgrade pre-clear — every house whose
    /// AABB overlaps the NEXT level's footprint grown by 256 is
    /// killed outright (life = -1 → the collapse walker evacuates).
    fn castle_upgrade_preclear(&mut self, i: usize) {
        let next = (self.ent[i].f26 + 1).clamp(1, 8) as usize;
        let def = self.assets.build_tab[next % self.assets.build_tab.len()];
        let half_w = ((((def.w as u16) << 8).wrapping_add(1280)) >> 1) as i32 + 256;
        let half_h = ((((def.h as u16) << 8).wrapping_add(1280)) >> 1) as i32 + 256;
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
        for j in 1..self.ent.len() {
            let e = &self.ent[j];
            if e.class64 == 10
                && e.model65 == 45
                && e.flags & 0x400 == 0
                && wd(e.x, x) < e.f80 as i32 + half_w
                && wd(e.y, y) < e.f82 as i32 + half_h
            {
                self.ent[j].act_life = -1;
            }
        }
    }

    /// sub_12D10 (:17643): the upgrade space gate — FAIL when
    /// another castle overlaps the next level's extents, or any
    /// tile on the four edges of the new footprint carries the
    /// protection bit (blocked/steep ground).
    pub(crate) fn castle_upgrade_space_ok(&self, i: usize) -> bool {
        let next = (self.ent[i].f26 + 1).clamp(1, 8) as usize;
        let def = self.assets.build_tab[next % self.assets.build_tab.len()];
        let half_w = ((((def.w as u16) << 8).wrapping_add(1280)) >> 1) as i32;
        let half_h = ((((def.h as u16) << 8).wrapping_add(1280)) >> 1) as i32;
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
        for j in 1..self.ent.len() {
            let e = &self.ent[j];
            if j != i
                && e.class64 == 3
                && e.model65 == 2
                && e.flags & 0x400 == 0
                && wd(e.x, x) < e.f80 as i32 + half_w
                && wd(e.y, y) < e.f82 as i32 + half_h
            {
                return false;
            }
        }
        let cx = ((x as u32 + 128) >> 8) as u8;
        let cy = ((y as u32 + 128) >> 8) as u8;
        let (htx, hty) = ((half_w >> 8) as i32, (half_h >> 8) as i32);
        let blocked = |gx: i32, gy: i32| {
            self.t.angle[tile((cx as i32 + gx) as u8, (cy as i32 + gy) as u8)] & 0x80 != 0
        };
        for gx in -htx..=htx {
            if blocked(gx, -hty) || blocked(gx, hty) {
                return false;
            }
        }
        for gy in -hty..=hty {
            if blocked(-htx, gy) || blocked(htx, gy) {
                return false;
            }
        }
        true
    }

    /// sub_46DB0 (:57023-32): direct ball absorption — an OWNED m39
    /// ball touching the castle empties into the store while the
    /// store sits below capacity (the whole ball lands; overflow is
    /// the ejector's business).
    fn castle_absorb(&mut self, i: usize) {
        if self.ent[i].f140 >= self.ent[i].f136 {
            return;
        }
        let own = self.ent[i].id24;
        for j in 1..self.ent.len() {
            if self.ent[j].class64 == 10
                && self.ent[j].model65 == 39
                && self.ent[j].flags & 0x400 == 0
                && self.ent[j].f144 == own
                && self.ent_overlap(i, j)
            {
                self.ent[i].f140 += self.ent[j].f140;
                self.ent[j].flags |= 0x400;
            }
        }
    }

    /// A wizard owner tag's team slot: PLAYER_TARGET = 0, a rival's
    /// entity slot = its player slot (wizext var_48 in the original).
    pub(crate) fn owner_team(&self, owner: u16) -> Option<u8> {
        if owner == crate::mc1::mobs::PLAYER_TARGET {
            return Some(0);
        }
        (owner != 0)
            .then(|| self.rival_ents.iter().position(|&e| e == owner))
            .flatten()
            .map(|s| s as u8)
    }

    /// sub_37A00 (:44266): the mana BALLOON entity (class 3 m3) —
    /// life 10000, speed 48, cargo capacity 10000, behavior row 9,
    /// sprite 169. The castle dispatcher overwrites the ctor's
    /// state 7 with the working state 9 (:56355).
    fn spawn_balloon(&mut self, x: u16, y: u16, z: i16, own: u16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 3;
            e.model65 = 3;
            e.tick70 = 9;
            e.max_life = 10000;
            e.act_life = 10000;
            e.f126 = 48;
            e.f136 = 10000;
            e.f140 = 0;
            // The ch0 vulnerability bit (+28 = 1, :44283) — without
            // it area writes skip the balloon entirely (the
            // playtest-9 "balloons are invulnerable" report; the
            // authored-placement ctor had it, this spawner didn't).
            e.f28 = 1;
            e.row156 = 9;
            e.id24 = own;
            e.f144 = own;
        }
        // Linked at spawn like the ctor (sub_41CF0 :44284) — an
        // unlinked balloon hovering its home tile was also invisible
        // to the direct-hit cell scans.
        self.link(i, x, y, z);
        self.refill_life(i);
        // Balloon sprite = 169 + team (the castle dispatcher's
        // `+86 += var_48`, :56347).
        let team = self.owner_team(own).unwrap_or(0) as u16;
        self.set_sprite(i, 169 + team);
        Some(i)
    }

    /// sub_47400 (:56264): the balloon/guard dispatcher, run from
    /// the established castle every other tick (:56016-20). Fleet
    /// quota by level: (balloons, guards) = L1(1,0) L2(1,0) L3(1,4)
    /// L4(2,6) L5(2,14) L6(3,18) L7(3,34); shortfalls respawn at the
    /// castle (guards = class-5 m15, HP 512). Targeting (:56358-95):
    /// the state-9 balloon's target DEFAULTS to the castle every
    /// pass (:56376 — the return/offload/hover-home behavior), then
    /// is overridden to the nearest own claimed ball no sibling is
    /// on, ONLY when the balloon still has cargo room and the castle
    /// census (house tally + stored) is below capacity. No free ball
    /// → the castle stays the target: balloons come home and wait
    /// there (playtest-8 fix — the old none→idle parked them at the
    /// last pickup). Untraced nicety: retail staggers retargeting by
    /// castle+63 % fleet, keeping a stale ball target between slots'
    /// turns; we re-pick every pass.
    fn castle_balloons(&mut self, i: usize) {
        const FLEET: [(usize, usize); 8] = [
            (0, 0),
            (1, 0),
            (1, 0),
            (1, 4),
            (2, 6),
            (2, 14),
            (3, 18),
            (3, 34),
        ];
        let own = self.ent[i].id24;
        let (bq, gq) = FLEET[self.ent[i].f26.clamp(0, 7) as usize];
        let mut balloons: Vec<usize> = Vec::new();
        let mut guards = 0usize;
        let mut house_tally = 0i64;
        for j in 1..self.ent.len() {
            let e = &self.ent[j];
            if e.flags & 0x400 != 0 {
                continue;
            }
            match (e.class64, e.model65) {
                (3, 3) if e.id24 == own => balloons.push(j),
                (5, 15) if e.id24 == own => guards += 1,
                (10, 45) if e.f144 == own => house_tally += e.f140.max(0) as i64,
                _ => {}
            }
        }
        let (cx, cy, cz) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        while balloons.len() < bq {
            let Some(b) = self.spawn_balloon(cx, cy, cz, own) else {
                break;
            };
            balloons.push(b);
        }
        // Guard respawn (:56412-47): throttled by the castle's +46
        // cooldown (ours f46) — at most ONE guard per dispatch pass,
        // 16 passes between spawns — placed at (x+128, y+640) on the
        // GROUND (the courtyard, off the tower slopes), facing 512.
        if self.ent[i].f46 > 0 {
            self.ent[i].f46 -= 1;
        }
        if guards < gq && self.ent[i].f46 == 0 {
            let gx = cx.wrapping_add(128);
            let gy = cy.wrapping_add(640);
            let gz = self.ground_z(gx, gy) as i16;
            // Both games park a (5,15) archer in the courtyard; the
            // guard itself is per-column (MC2: mc2_spawn_m15, retail
            // EF:61488 — spawning the MC1 creature under the MC2
            // dispatch was the class-5-model-15 misfit despawn).
            let guard = match self.verbs.movement {
                crate::verbs::MovementVerb::Mc2 => self.mc2_spawn_m15(gx, gy, gz),
                _ => self.spawn_creature(15, gx, gy, gz),
            };
            if let Some(g) = guard {
                self.ent[g].id24 = own;
                self.ent[g].f144 = own;
                self.ent[g].f30 = 512;
                self.ent[g].f34 = 512;
                self.ent[i].f46 = 16;
            }
        }
        let full = house_tally + self.ent[i].f140.max(0) as i64 >= self.ent[i].f136.max(0) as i64;
        for k in 0..balloons.len() {
            let b = balloons[k];
            if full || self.ent[b].f140 >= self.ent[b].f136 {
                self.ent[b].f146 = i as u16;
                continue;
            }
            // Nearest own claimed ball a sibling isn't already on
            // (sub_46CA0 :55922).
            let (bx, by) = (self.ent[b].x, self.ent[b].y);
            let mut best = 0usize;
            let mut best_d = i32::MAX;
            for j in 1..self.ent.len() {
                let e = &self.ent[j];
                if e.class64 != 10 || e.model65 != 39 || e.flags & 0x400 != 0 || e.f144 != own {
                    continue;
                }
                if balloons
                    .iter()
                    .any(|&s| s != b && self.ent[s].f146 as usize == j)
                {
                    continue;
                }
                let d = Self::dist2_sq(bx, by, e.x, e.y);
                if d < best_d {
                    best_d = d;
                    best = j;
                }
            }
            // No free ball → the castle default stands (:56376).
            self.ent[b].f146 = if best != 0 { best as u16 } else { i as u16 };
        }
    }

    /// sub_47F90 (:56716): the BALLOON tick (class-3 m3 state 9).
    /// Ball target: >1024 away clears the ball's tether bit, near
    /// sets it (+ ball homes the balloon); touching absorbs the
    /// cargo and refreshes life; within one speed-step the balloon
    /// snaps over the ball. Castle target: within level·speed and
    /// low enough, the cargo empties into the castle store. All
    /// paths finish through the row-9 altitude servo (sub_42000
    /// params from the behavior row). Death drops the cargo as a
    /// claimed ball (the dispatcher's slot cleanup, :56368-72).
    pub(crate) fn balloon_tick(&mut self, i: usize) {
        self.balloon_move(i);
        // ch0 damage inbox at the tick's END (sub_481D0, reached via
        // LABEL_17 :56755-58 — movement/delivery FIRST, so the dock
        // pass's full heal precedes the damage: a balloon parked in
        // its castle ring is authentically near-invulnerable to chip
        // damage; they die in flight, or to a single lethal burst).
        if self.ent[i].mail[0].1 != 0 {
            let amt = self.ent[i].mail[0].0;
            self.ent[i].mail[0].1 = 0;
            self.ent[i].act_life -= amt as i32;
            // Balloon-under-attack flash (Type_160+393 = 4, :56826).
            if self.ent[i].id24 == crate::mc1::mobs::PLAYER_TARGET {
                self.balloon_alert = 4;
            }
        }
        if self.ent[i].act_life < 0 {
            self.corpse_drop(i);
            self.ent[i].flags |= 0x400;
        }
    }

    fn balloon_move(&mut self, i: usize) {
        use crate::mc1::behavior::BEHAVIOR;
        let t = self.ent[i].f146 as usize;
        if t == 0 || self.ent[t].flags & 0x400 != 0 {
            return; // idle (:56814)
        }
        // Stale-slot guard (2026-07-15): the claim ticket is a RAW
        // slot index. A collected ball's slot LIFO-recycled by
        // another class-10 (a dwelling) passed the class check and
        // got "absorbed" — the instant-death bug. Retail sub_47F90
        // (:56742-73) has the same latent bug; the dispatcher only
        // ever assigns (10,39), so this blocks nothing legitimate.
        if self.ent[t].class64 == 10 && self.ent[t].model65 != 39 {
            self.ent[i].f146 = 0; // ball is gone: back to idle
            return;
        }
        let mut pos = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let (tx, ty) = (self.ent[t].x, self.ent[t].y);
        let yaw = Self::angle_between(pos.0, pos.1, tx, ty);
        self.ent[i].f30 = yaw;
        let speed = self.ent[i].f126;
        let own = self.ent[i].id24;
        let mut step = true;
        if self.ent[t].class64 == 10 {
            if self.ent[t].f144 != own {
                step = false; // stale claim: hover (:56744)
            } else {
                let d = Self::isqrt(Self::dist2_sq(pos.0, pos.1, tx, ty) as u32) as i32;
                if d > 1024 {
                    self.ent[t].flags &= !0x40;
                } else {
                    self.ent[t].flags |= 0x40;
                    self.ent[t].f146 = i as u16;
                    if self.ent_overlap(i, t) {
                        let cargo = self.ent[t].f140;
                        let ball_owner = self.ent[t].f144;
                        self.ent[i].f140 += cargo;
                        self.ent[i].f144 = ball_owner;
                        self.ent[i].f146 = 0;
                        self.ent[i].act_life = self.ent[i].max_life as i32;
                        self.ent[t].flags |= 0x400;
                    }
                }
                if d <= speed as i32 {
                    pos.0 = tx;
                    pos.1 = ty;
                    step = false;
                }
            }
        } else {
            // Castle target: delivery ring = level * speed.
            let d = Self::isqrt(Self::dist2_sq(pos.0, pos.1, tx, ty) as u32) as i32;
            if d <= self.ent[t].f26 as i32 * speed as i32 {
                let ground = self.ground_z(pos.0, pos.1) as i16;
                if pos.2 <= ground.wrapping_add(BEHAVIOR[9].v_12) && self.ent[t].f26 > 0 {
                    pos.0 = tx;
                    pos.1 = ty;
                    let cargo = self.ent[i].f140;
                    self.ent[t].f140 += cargo;
                    self.ent[i].f140 = 0;
                    self.ent[i].f144 = own;
                    self.ent[i].act_life = self.ent[i].max_life as i32;
                }
                step = false;
            }
        }
        if step {
            Self::polar_step(&mut pos, yaw, self.ent[i].f32, speed);
        }
        // The row-9 altitude servo + writeback (LABEL_17).
        let ground = self.ground_z(pos.0, pos.1) as i16;
        let mut z = pos.2;
        Self::alt_clamp(&mut z, ground, &BEHAVIOR[9]);
        self.move_relink(i, pos.0, pos.1, z);
    }

    /// sub_47C60 (:56572): castle max health by level (level 0 = 0 =
    /// keep the ctor's 40000). Levels 6/7 use the decompiler-mangled
    /// const `loc_13880` = 0x13880 = 80000 (decoded 2026-07-07 —
    /// corrects the earlier 60000 carry). The carry-over rule on any
    /// level change (sub_47BD0 :56552-60): a NEGATIVE old life
    /// (overkill) is re-deducted from the new max, capped at half of
    /// it; positive life just resets to full.
    const CASTLE_HP: [u32; 8] = [40000, 20000, 40000, 40000, 60000, 60000, 80000, 80000];

    /// sub_46F10 (:56043): the class-3 m2 CASTLE state machine
    /// (sub-state f59 = the original's +48). Remaining housekeeping:
    /// the overflow ejector, downgrade/respawn. The entity z (+76)
    /// refreshes to live ground every tick (idle :56014 + wait
    /// cases 1/4/6 :56073-78) — the flag rides the painted tower;
    /// the build-site datum lives in f28 (+154).
    pub(crate) fn castle_tick(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
        match self.ent[i].f59 {
            // Level-up (sub_47960 :56461, case 0 :56053-72): the
            // house pre-clear + (for standing castles) the space
            // gate — a reject bounces back to established with no
            // sound (the cast-time fizzle was the only failure
            // audio). Extents from build row = level (sub_37150
            // :43798; its +78=0xE000 marker skipped — it would
            // z-orphan our AABB overlaps), the loop-10 build gong,
            // the m42 painter, and the capacity ladder (sub_47C60 →
            // sub_47DD0 :56617).
            0 => {
                self.castle_upgrade_preclear(i);
                if self.ent[i].f26 > 0 && !self.castle_upgrade_space_ok(i) {
                    self.ent[i].f59 = 2;
                    return;
                }
                let (x, y, own, site_z) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.id24, e.site_z)
                };
                // The painter targets the build-site datum (+154),
                // not the live tower-top ground (sub_47020 spawns at
                // the site triple). The WHOLE level-up commit lives
                // inside sub_47960's `if (v1)` on this spawn
                // (:56471-93): a pool-full failure changes nothing
                // and case 0 retries next tick. Committing (or
                // advancing to the wait) without a painter was the
                // stuck-castle deadlock under meteor pool exhaustion
                // (fixed 2026-07-15).
                let Some(p) = self.spawn_creator(42, x, y, site_z) else {
                    return;
                };
                let lvl = (self.ent[i].f26 + 1).clamp(1, 8);
                self.ent[i].f26 = lvl;
                self.ent[i].f136 = Self::CASTLE_CAP[(lvl as usize).min(7)];
                let hp = Self::CASTLE_HP[(lvl as usize).min(7)];
                self.ent[i].max_life = hp;
                self.ent[i].act_life = hp as i32;
                let def = self.assets.build_tab[lvl as usize % self.assets.build_tab.len()];
                {
                    let e = &mut self.ent[i];
                    e.f80 = (((def.w as u16) << 8).wrapping_add(1280)) >> 1;
                    e.f82 = (((def.h as u16) << 8).wrapping_add(1280)) >> 1;
                    e.f84 = 0x4000;
                }
                self.snd(10, i);
                {
                    let e = &mut self.ent[p];
                    e.f146 = i as u16;
                    e.f71 = lvl as u8;
                    e.id24 = own;
                    e.flags |= 0x10000; // +18 |= 1 (:56492)
                }
                // WAIT in sub-state 1 (the original's pure-wait
                // :56073) — NOT established. Damage/demolish/upgrade
                // mail accrue untouched until the leveler hands back
                // state 4: the original's standing tick is the ONLY
                // damage processor (sub_47EC0 runs from +70=4 alone).
                // Processing lethals mid-transformation was the
                // playtest-6 orphaned-tower bug (a downgrade collapse
                // under a still-running painter), and it erased the
                // authentic between-transformations upgrade window
                // (the dragon-squat survival trick).
                self.ent[i].f59 = 1;
            }
            // Painter done → the m41 ground leveler (case 5,
            // sub_47080 :56119-35), then wait in sub-state 6 — the
            // original's real flow (:56132; cases 1/4/6 are pure
            // waits, :56073-78).
            5 => {
                let (x, y, z, own, lvl) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.site_z, e.id24, e.f26)
                };
                // sub_47080 advances only inside `if (result)`
                // (:56126-33) — a failed leveler spawn leaves the
                // case to retry next tick.
                if let Some(l) = self.spawn_creator(41, x, y, z) {
                    {
                        let e = &mut self.ent[l];
                        e.f146 = i as u16;
                        e.f71 = lvl as u8;
                        e.id24 = own;
                    }
                    self.ent[i].f59 = 6; // authentic wait state (:56132)
                }
            }
            // Leveler done → established (case 2 → sub_46DB0).
            2 => self.ent[i].f59 = 4,
            // Blast-shake expiry → the damage REPAINT (sub_47020
            // :56100-15): a painter at the CURRENT level with the
            // kill bit CLEAR — it re-stamps the tower and kills
            // nothing (:56492 sets the bit only on the upgrade
            // commit).
            3 => {
                let (x, y, own, site_z, lvl) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.id24, e.site_z, e.f26)
                };
                // sub_47020 advances only inside `if (result)`
                // (:56107-13) — a failed repaint spawn retries.
                if let Some(p) = self.spawn_creator(42, x, y, site_z) {
                    {
                        let e = &mut self.ent[p];
                        e.f146 = i as u16;
                        e.f71 = lvl.clamp(1, 8) as u8;
                        e.id24 = own;
                    }
                    self.ent[i].f59 = 1; // wait for the repaint painter
                }
            }
            // Established (sub_46DB0 :55978): the blast-shake
            // countdown FREEZES everything else while it runs
            // (:55981-93 — the mailbox accrues, processing waits),
            // then the ch0 damage intake (sub_47EC0 :56678), the ch5
            // upgrade intake (:56690-95 — sender must be the owner,
            // max level 7), and the every-other-tick block
            // (:56016-37): overflow ejector, balloons, absorption.
            4 => {
                if self.ent[i].f50 > 0 {
                    self.ent[i].f50 -= 1;
                    if self.ent[i].f50 == 1 {
                        self.ent[i].f50 = 0;
                        self.ent[i].f59 = 3;
                    }
                    return;
                }
                // sub_47EC0's first line (:56683): already below
                // zero → straight to the downgrade. This is also
                // the demolish path — Shift+L writes life = −1 with
                // no mail at all (:55846-50).
                if self.ent[i].act_life < 0 {
                    self.castle_downgrade(i);
                    return;
                }
                // sub_47EC0: HP -= pending ch0; lethal → the
                // one-level downgrade (state 6 → sub_47A70).
                if self.ent[i].mail[0].1 != 0 {
                    let amt = self.ent[i].mail[0].0;
                    self.ent[i].mail[0] = (0, 0);
                    self.ent[i].act_life -= amt as i32;
                    if self.ent[i].act_life < 0 {
                        self.castle_downgrade(i);
                        return;
                    }
                    // "Castle under attack" flash (Type_160+391=4).
                    if self.ent[i].id24 == crate::mc1::mobs::PLAYER_TARGET {
                        self.castle_alert = 4;
                    }
                }
                if self.ent[i].mail[5].1 != 0 {
                    let sender = self.ent[i].mail[5].1;
                    self.ent[i].mail[5] = (0, 0);
                    if sender == self.ent[i].id24 && self.ent[i].f26 < 7 {
                        self.ent[i].f59 = 0;
                    }
                }
                if self.ent[i].f63 & 1 == 0 {
                    // The overflow ejector (sub_47130, called :56016):
                    // banked houses + stored over capacity spill out
                    // as owner-tagged wild-flying balls.
                    self.castle_eject(i);
                    self.castle_balloons(i);
                    // Absorption sits inside the every-other-tick
                    // block in the original too (:57023-32).
                    self.castle_absorb(i);
                }
            }
            // 1 = waiting for a painter, 6 = waiting for the
            // leveler (the original's pure waits, :56073-78): the
            // mailbox and any pending lethal accrue untouched.
            _ => {}
        }
    }

    /// sub_47A70 (:56498) + the state-6 wrapper (sub_470E0 :56138):
    /// lethal damage knocks the castle DOWN one level — collapse
    /// rumble (sound 30), ~10% of capacity ejected as mana balls,
    /// the footprint un-stamped to rough ground (the collapse
    /// walker's zeroed fake event, :56515-24), then the ladder reset
    /// with the overkill carry and a 5-tick timer into the repaint.
    /// At level 1 the whole castle dies instead (:56531-37): the
    /// balloon is released, the ENTIRE bank scatters, the entity is
    /// freed — the player is castle-less (die now = restart).
    fn castle_downgrade(&mut self, i: usize) {
        self.terrain_dirty = true; // the synchronous un-stamp below
        self.snd(30, i);
        let lvl = self.ent[i].f26;
        // 10% capacity haircut before the ejector (:56507-09) — the
        // ejector spills everything above the reduced ceiling.
        let cut = 10 * self.ent[i].f136 / 100;
        self.ent[i].f136 -= cut;
        self.castle_eject(i);
        // The footprint un-stamp: a zeroed fake collapse event over
        // the CURRENT level's build row, run synchronously
        // (sub_28FE0 direct call, :56524).
        let (x, y, site_z, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.site_z, e.id24)
        };
        if let Some(f) = self.new_event() {
            {
                let e = &mut self.ent[f];
                e.class64 = 10;
                e.model65 = 0; // zeroed model → z>>5 datum fallback
                e.f71 = lvl.clamp(1, 8) as u8;
                e.f26 = 0; // no evacuees on a castle (:56521)
                e.x = x;
                e.y = y;
                e.z = site_z;
            }
            self.tick_building_collapse(f);
            self.free_entity(f);
        }
        let lvl = lvl - 1;
        self.ent[i].f26 = lvl;
        if lvl <= 0 {
            // Total destruction (:56531-37): release the balloons,
            // scatter the whole remaining bank (the level-0 ejector
            // rule spills ALL stored, :56172), free the castle.
            self.ent[i].f136 = 0;
            self.castle_eject(i);
            for j in 1..self.ent.len() {
                if self.ent[j].class64 == 3
                    && self.ent[j].model65 == 3
                    && self.ent[j].id24 == own
                    && self.ent[j].flags & 0x400 == 0
                {
                    self.ent[j].flags |= 0x400; // release ≈ despawn
                }
            }
            self.ent[i].flags |= 0x400;
            return;
        }
        // Ladder reset at the new level (sub_47C60 → sub_47BD0): the
        // overkill deficit carries, capped at half the new max.
        let new_max = Self::CASTLE_HP[(lvl as usize).min(7)];
        let deficit = (-self.ent[i].act_life).clamp(0, new_max as i32 / 2);
        self.ent[i].max_life = new_max;
        self.ent[i].act_life = new_max as i32 - deficit;
        self.ent[i].f136 = Self::CASTLE_CAP[(lvl as usize).min(7)];
        let def = self.assets.build_tab[lvl as usize % self.assets.build_tab.len()];
        {
            let e = &mut self.ent[i];
            e.f80 = (((def.w as u16) << 8).wrapping_add(1280)) >> 1;
            e.f82 = (((def.h as u16) << 8).wrapping_add(1280)) >> 1;
        }
        // 5 ticks, then the repaint re-stamps the smaller castle
        // (:56158 +48=0/+50=5 → the state-4 countdown → sub-state 3).
        self.ent[i].f50 = 5;
        self.ent[i].f59 = 4;
    }

    /// sub_47130 (:56162): the castle mana EJECTOR. Spill = stored −
    /// capacity when houses + stored exceed capacity (ALL stored for
    /// a level-0/dying castle), thrown as 1..=32 owner-tagged balls
    /// of spill/count each, teleported 15-35 tiles out at random
    /// yaws with an upward pop, plus 4 (10,54) mana magnets at 25
    /// tiles (their ball-pull is the banked magnet chain; the ch4
    /// writes land but the pull is inert until it's ported).
    fn castle_eject(&mut self, i: usize) {
        let stored = self.ent[i].f140;
        let cap = self.ent[i].f136;
        let mut spill = if self.banked_houses.saturating_add(stored) > cap {
            stored - cap
        } else {
            0
        };
        if self.ent[i].f26 == 0 {
            spill = stored;
        }
        if spill <= 0 {
            return;
        }
        let count = (spill / 1000).clamp(1, 32);
        let mut share = spill / count;
        let (cx, cy, cz, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24)
        };
        let ground = self.ground_z(cx, cy) as i16;
        for _ in 0..count {
            let Some(b) = self.spawn_mana_ball(cx, cy, cz) else {
                break;
            };
            self.ent[b].f140 = share;
            self.ent[b].f144 = own;
            // Ball-seed draw → +126 (vestigial speed, kept for
            // stream parity); +150/152 velocity zeroed (:56221-23).
            let d = self.ent_rand(b);
            self.ent[b].f126 = (d % 0x30 + 16) as i16;
            self.ent[b].dest_x = 0;
            self.ent[b].dest_y = 0;
            // Upward pop scaled by how low the flag sits (:56227).
            self.ent[b].f46 = ((1024 - (cz.wrapping_sub(ground)) as i32) / 8) as i16;
            // Castle-seed draws: distance then yaw (:56231-37).
            let dist = (lcg32(&mut self.ent[i].rand) % 0x1400 + 3840) as i16;
            let yaw = (lcg32(&mut self.ent[i].rand) & 0x7FF) as u16;
            let mut pos = (cx, cy, cz);
            Self::polar_step(&mut pos, yaw, 0, dist);
            self.move_relink(b, pos.0, pos.1, pos.2);
            let taken = self.ent[b].f140;
            spill -= taken;
            self.ent[i].f140 -= taken;
            if spill < share {
                share = spill;
            }
            if spill <= 0 {
                break;
            }
        }
        for _ in 0..4 {
            let dist = 6400i16;
            let yaw = (lcg32(&mut self.ent[i].rand) & 0x7FF) as u16;
            let mut pos = (cx, cy, cz);
            Self::polar_step(&mut pos, yaw, 0, dist);
            self.spawn_castle_magnet(pos.0, pos.1, pos.2, own);
        }
    }

    /// sub_3B970 (:47672): the (10,54) mana MAGNET — invisible,
    /// 128 ticks, not damageable. Its tick (sub_29920 :31234) stamps
    /// ch4 attract mail on every mana ball within ~14 tiles.
    fn spawn_castle_magnet(&mut self, x: u16, y: u16, z: i16, own: u16) -> Option<usize> {
        let s = self.new_event()?;
        {
            let e = &mut self.ent[s];
            e.class64 = 10;
            e.model65 = 54;
            e.tick70 = 59;
            e.max_life = 128;
            e.f126 = 256;
            e.f44 = 100;
            e.f26 = 0;
            e.flags &= !8;
            e.id24 = own;
            let d = lcg32(&mut e.rand);
            e.f30 = (d & 0x7FF) as u16;
        }
        self.link(s, x, y, z);
        self.refill_life(s);
        {
            let e = &mut self.ent[s];
            e.f80 = 1024;
            e.f82 = 1024;
            e.f84 = 0x4000;
        }
        Some(s)
    }

    /// sub_29920 (:31234), byte70 59: the (10,54) magnet tick — life
    /// runs down, and every m39 ball within dist² < 12845056 (~14
    /// tiles) gets ch4 mail {100, self} (a direct overwrite,
    /// :31255-57). The ball-side ch4 consumer is the banked magnet
    /// chain — the writes land but the pull is inert today (the
    /// spell-side state-21 APPROX puller is separate; unify when the
    /// (9,17)→(10,54) chain is traced).
    pub(crate) fn mana_magnet_tick(&mut self, i: usize) {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i64).abs();
        for j in 1..self.ent.len() {
            if self.ent[j].class64 == 10
                && self.ent[j].model65 == 39
                && self.ent[j].flags & 0x400 == 0
            {
                let (dx, dy) = (wd(self.ent[j].x, x), wd(self.ent[j].y, y));
                if dx * dx + dy * dy < 12_845_056 {
                    self.ent[j].mail[4] = (100, i as u16);
                }
            }
        }
    }

    /// sub_3B620 (:47477): the (10,40) GRAVE a dying wizard leaves —
    /// sprite 65, ch1 (possession) mask only, f26 = slot % 11.
    pub(crate) fn spawn_grave(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let s = self.new_event()?;
        {
            let e = &mut self.ent[s];
            e.class64 = 10;
            e.model65 = 40;
            e.tick70 = 42;
            e.f26 = (s % 11) as i16;
            e.f28 = 2;
        }
        self.link(s, x, y, z);
        self.refill_life(s);
        self.set_sprite(s, 65);
        Some(s)
    }

    /// sub_275C0 (:29636), byte70 42: the grave tick — ground-snap,
    /// and a wizard-family possession claim (ch1) inherits EVERYTHING
    /// the grave owns (+144 == grave slot → claimant), then the grave
    /// vanishes. Reclaiming your own scattered bank after a death is
    /// exactly this possess.
    pub(crate) fn grave_tick(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
        if self.ent[i].mail[1].1 != 0 {
            let claimant = self.ent[i].mail[1].1;
            self.ent[i].mail[1] = (0, 0);
            if self.attacker_is_wizard(claimant) && self.ent[i].f144 == 0 {
                for j in 1..self.ent.len() {
                    if self.ent[j].f144 == i as u16 && self.ent[j].class64 != 0 {
                        self.ent[j].f144 = claimant;
                    }
                }
            }
            self.free_entity(i);
        }
    }

    /// sub_28DC0 (:30767), byte70 52: the LIVE village building.
    /// Damage intake sub_29640 (ch0; the decompile's u16 amount read
    /// is union slicing — writers store u32): non-lethal hits pop one
    /// militiaman (m4) out at (x+f80, y) while occupants +26 > 2, and
    /// put a wizard attacker on the village's wanted list (+528 =
    /// 200); death latches the killer and moves to state 53. Every 40
    /// ticks the mana pool +140 tracks occupants<<8, and a FULL house
    /// with capacity > 5 has a ~1/16 chance to emit a villager.
    /// The ch1 possession re-owner (:30801-14): claim the sender,
    /// chime 4, clear the active bit, swap to the claimed sprite
    /// (177). Deviation: the immediate mana credit off the claimer's
    /// +48 is the mana-economy track.
    pub(crate) fn tick_building_live(&mut self, i: usize) {
        if self.ent[i].act_life < 0 {
            // Killed directly (castle crush life = -1, :17638).
            self.ent[i].tick70 = 53;
            return;
        }
        if self.ent[i].mail[1].1 != 0 {
            let src = self.ent[i].mail[1].1;
            self.ent[i].mail[1] = (0, 0);
            if src != self.ent[i].f144 {
                self.ent[i].f144 = src;
                self.ent[i].flags &= !1;
                // Anchored at the CLAIMANT (:30806) — the player-
                // gated id 4 sounds exactly when YOU capture.
                if src == crate::mc1::mobs::PLAYER_TARGET {
                    self.snd_player(4);
                }
                self.set_sprite(i, 177);
            }
        }
        if self.ent[i].mail[0].1 != 0 {
            let (amt, src) = self.ent[i].mail[0];
            self.ent[i].mail[0].1 = 0;
            // Captured buildings are immune to their OWNER's damage
            // ("as if they were your castle" — PLAYER GROUND TRUTH;
            // no substrate found in the decompile's ch0 writer or
            // intake, sub_120B0/:31070 — DOSBox verification owed).
            if src != 0 && src == self.ent[i].f144 {
                return;
            }
            self.ent[i].act_life -= amt as i32;
            if self.ent[i].act_life < 0 {
                self.ent[i].f38 = src;
                self.ent[i].tick70 = 53;
                return;
            }
            self.ent[i].f40 = src;
            if self.ent[i].f26 > 2 {
                self.ent[i].f26 -= 1;
                let (x, y, f80) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.f80)
                };
                let sx = x.wrapping_add(f80);
                let z = self.ground_z(sx, y) as i16;
                self.spawn_creature(4, sx, y, z);
            }
            if src == crate::mc1::mobs::PLAYER_TARGET {
                self.player_aggro = 200;
            }
        }
        if self.ent[i].f63 % 40 == 0 {
            self.ent[i].f140 = (self.ent[i].f26 as i32) << 8;
            let cap = self.ent[i].f128;
            // EXACT equality (:30819) — occupancy only rises (militia
            // walk-ins have no cap check, emission never decrements),
            // so retail's gate self-extinguishes once a house
            // overshoots. `>=` here was the level-001 runaway: every
            // full house emitted forever, flooding the level with
            // villagers + loose mana until the pool saturated
            // (traced + runtime-reproduced 2026-07-16).
            if cap > 5 && self.ent[i].f26 == cap {
                let d = self.ent_rand(i) % cap as u32;
                if d > (cap - cap / 16 - 2) as u32 {
                    self.building_emit(i);
                }
            }
        }
    }

    /// sub_28D10 (:30715): one villager emitted at (x+f80, y) —
    /// LCG%12: 0-1 militia m4, 2-3 migrant m14, 4-8 villager m13,
    /// 9-11 settler m12 (their natural spawn states 25/85/79/73).
    fn building_emit(&mut self, i: usize) {
        let d = self.ent_rand(i) % 12;
        let model = match d {
            0 | 1 => 4,
            2 | 3 => 14,
            4..=8 => 13,
            _ => 12,
        };
        let (x, y) = {
            let e = &self.ent[i];
            (e.x.wrapping_add(e.f80), e.y)
        };
        let z = self.ground_z(x, y) as i16;
        self.spawn_creature(model, x, y, z);
    }

    /// sub_28FE0 (:30835), byte70 53: the one-shot collapse. Walks
    /// the BUILD footprint once: per occupied cell an occupant
    /// evacuates (the LAST one is a settler m12, ≥4 remaining draw
    /// from the emit mix, otherwise a militiaman m4 — village defense
    /// IS the evacuation; spawn z drops 10 tiles every 8th STREAM
    /// byte, :30913-17). Per cell code hi nibble (:30940-93):
    /// 0 = unprotect only; 3 = unprotect + tower knock-down (-12
    /// AND -16 for sub-code 1, -16 for 2) + single-tile retexture;
    /// walls (1/2/4..7) = corner code forced to 1, single-tile
    /// retexture BEFORE the height drop (LCG%50 ≤ 20 → the full
    /// 4·(lo-1), else minus LCG%20 of it; at or below the wall
    /// height → 0). Finish = the full-rect 3x3 height smoother
    /// sub_36080 (:31004) and despawn. No mana spill. Base z =
    /// avg4 of the footprint corners when the event carries a model
    /// (:30879-81); the castle demolish path's zeroed fake event
    /// falls back to z>>5.
    pub(crate) fn tick_building_collapse(&mut self, i: usize) {
        let e = self.ent[i];
        let cx = ((e.x as u32 + 128) >> 8) as u8;
        let cy = ((e.y as u32 + 128) >> 8) as u8;
        let def = self.assets.build_tab[e.f71 as usize % self.assets.build_tab.len()];
        let (w, h) = (def.w as u16, def.h as u16);
        let (half_w, half_h) = ((w >> 1) as u8, (h >> 1) as u8);
        let x0 = cx.wrapping_sub(half_w);
        let y0 = cy.wrapping_sub(half_h);
        let base_h = if e.model65 != 0 {
            self.avg4(x0, y0, h as u8, w as u8) as i32
        } else {
            (e.z >> 5) as i32
        };
        let (z_hi, z_lo) = ((32 * base_h) as i16, (32 * (base_h - 10)) as i16);
        let mut rows = h;
        let (mut x, mut y) = (x0, y0);
        let mut c = def.offset as usize;
        // Stream position (the original's v2) — control bytes count.
        let mut pos = 0u32;
        while rows != 0 {
            let ctl = self.assets.build_dat[c] as i8;
            c += 1;
            pos += 1;
            if ctl == 0 {
                y = y.wrapping_add(1);
                rows -= 1;
                x = x0;
                continue;
            }
            if ctl < 0 {
                x = x.wrapping_add((-(ctl as i32)) as u8);
                continue;
            }
            for _ in 0..ctl {
                let b = self.assets.build_dat[c];
                c += 1;
                pos += 1;
                if b != 0 {
                    let t = tile(x, y);
                    // Evacuation (:30907-35): tile-corner position,
                    // low z every 8th stream byte.
                    let occ = self.ent[i].f26;
                    if occ > 0 {
                        self.ent[i].f26 = occ - 1;
                        let ez = if pos & 7 == 0 { z_lo } else { z_hi };
                        let wx = (x as u16) << 8;
                        let wy = (y as u16) << 8;
                        if occ == 1 {
                            self.spawn_creature(12, wx, wy, ez);
                        } else if occ - 1 >= 4 {
                            self.building_emit(i);
                        } else {
                            self.spawn_creature(4, wx, wy, ez);
                        }
                    }
                    // Rubble (:30940-93).
                    let hi = b >> 4;
                    let lo = b % 16;
                    if hi == 0 {
                        // Floors: unprotect, texture kept (:30994-95).
                        self.t.angle[t] &= !0x80;
                    } else if hi == 3 {
                        // Towers (:30974-93): unprotect, knock down,
                        // re-infer the tile. Sub-code 1 drops BOTH
                        // steps (decompile fall-through, verbatim).
                        self.t.angle[t] &= !0x80;
                        let sub = (lo % 16) % 3;
                        if sub == 1 && self.t.height[t] > 12 {
                            self.t.height[t] -= 12;
                        }
                        if (sub == 1 || sub == 2) && self.t.height[t] > 16 {
                            self.t.height[t] -= 16;
                        }
                        self.recompute_unprotected(x, y, x, y);
                    } else {
                        // Walls (:30944-71): corner code 1, retile
                        // BEFORE the height drop.
                        self.t.angle[t] = (self.t.angle[t] & 0x70) | 1;
                        self.recompute_unprotected(x, y, x, y);
                        if lo != 0 {
                            let full = 4 * (lo as i32 - 1);
                            if (self.t.height[t] as i32) <= full {
                                self.t.height[t] = 0;
                            } else {
                                let d = lcg32(&mut self.ent[i].rand);
                                let drop = if (d % 50) as i32 <= 20 {
                                    full
                                } else {
                                    full - (lcg32(&mut self.ent[i].rand) % 20) as i32
                                };
                                let hh = self.t.height[t] as i32;
                                self.t.height[t] = (hh - drop) as u8;
                            }
                        }
                    }
                }
                x = x.wrapping_add(1);
            }
        }
        // Finish (:31004): the full-rect vertex smoother over the
        // footprint (rows/cols exactly w x h, per-vertex sub_360C0 —
        // building-typed quads are self-excluding).
        for gy in 0..h {
            for gx in 0..w {
                self.smooth_cell(tile(x0.wrapping_add(gx as u8), y0.wrapping_add(gy as u8)));
            }
        }
        self.ent[i].flags |= 0x400;
    }

    /// sub_33800 (:40980): paint one building tile. `a4 < 8` writes a
    /// terrain class + retexture; higher codes select {type,
    /// orientation} pairs from the paint tables and set the protection
    /// bit (plus clear bit 3 on the E/SE/S neighbors). Codes
    /// 0x14/0x15/0x16 are the white-wall DAMAGE stages (types
    /// 10/11/12 via PAINT_BC) — the fire cell's burn ladder.
    pub(crate) fn paint(&mut self, a1: i8, a2: i8, t: usize, a4: u8) {
        if a4 < 8 {
            self.t.angle[t] = a4 | (self.t.angle[t] & 0xF0);
            self.recompute_protected(tx(t), ty(t), tx(t), ty(t));
            return;
        }
        let checker = ((tx(t).wrapping_add(ty(t))) & 1) as usize;
        let pair: Option<[u8; 2]> = match a4 {
            8 => {
                self.t.tile_type[t] = 8;
                None
            }
            9 => {
                self.t.tile_type[t] = 9;
                None
            }
            10..=14 => {
                let (v, flat) = self.corner_orient(a1, a2, t);
                let idx = v as usize + if flat { 8 } else { 0 } + 16 * (a4 as usize - 10);
                Some(PAINT_FC[3 + idx / 8][idx % 8])
            }
            15 => {
                self.t.tile_type[t] = 11;
                None
            }
            16 => {
                let cur = self.t.tile_type[t];
                if matches!(cur, 10 | 11 | 12) {
                    None
                } else {
                    let (v, _) = self.corner_orient(cur as i8, a2, t);
                    Some(PAINT_AC[0][v as usize])
                }
            }
            17 => {
                let (v, _) = self.corner_orient(a1, a2, t);
                Some(PAINT_EC[0][v as usize])
            }
            18 => {
                let (v, _) = self.corner_orient(a1, a2, t);
                Some(PAINT_FC[checker][v as usize])
            }
            19 => {
                let (v, _) = self.corner_orient(a1, a2, t);
                Some(PAINT_FC[1 + checker][v as usize])
            }
            20..=22 => {
                let (v, _) = self.corner_orient(a1, a2, t);
                Some(PAINT_BC[a4 as usize - 20][v as usize])
            }
            _ => None,
        };
        if let Some([ty_val, ang]) = pair {
            self.t.tile_type[t] = ty_val;
            self.t.angle[t] = (self.t.angle[t] & 0x8F) | ang;
        }
        // Protection marks: claim this tile, clear bit 3 on E/SE/S.
        self.t.angle[t] = (self.t.angle[t] & 0x77) | 0x80;
        let (cx, cy) = (tx(t), ty(t));
        self.t.angle[tile(cx.wrapping_add(1), cy)] &= 0xF7;
        self.t.angle[tile(cx.wrapping_add(1), cy.wrapping_add(1))] &= 0xF7;
        self.t.angle[tile(cx, cy.wrapping_add(1))] &= 0xF7;
    }

    /// sub_33640 (:40870): corner orientation of a tile's height quad.
    /// `a1`/`a2` act as caller defaults for the max / runner-up corner
    /// indices. Returns (code 0..7, flat) where flat = max-min <= 8.
    fn corner_orient(&self, mut a1: i8, mut a2: i8, t: usize) -> (u8, bool) {
        let (cx, cy) = (tx(t), ty(t));
        let c = [
            self.t.height[t],
            self.t.height[tile(cx.wrapping_add(1), cy)],
            self.t.height[tile(cx.wrapping_add(1), cy.wrapping_add(1))],
            self.t.height[tile(cx, cy.wrapping_add(1))],
        ];
        let mut vmax = 0u8;
        if c[0] != 0 {
            vmax = c[0];
            a1 = 0;
        }
        let mut vmin = 0xFFu8;
        if c[0] != 0xFF {
            vmin = c[0];
        }
        for k in 1..4 {
            if c[k] > vmax {
                vmax = c[k];
                a1 = k as i8;
            }
            if c[k] < vmin {
                vmin = c[k];
            }
        }
        let mut v2nd = 0u8;
        if a1 != 0 && c[0] != 0 {
            v2nd = c[0];
            a2 = 0;
        }
        for k in 1..4 {
            if a1 != k as i8 && c[k] > v2nd {
                v2nd = c[k];
                a2 = k as i8;
            }
        }
        let flat = vmax.wrapping_sub(vmin) as i32 <= 8;
        if vmax as i32 - v2nd as i32 >= 8 {
            return ((a1 as u8) & 7, flat);
        }
        let code = match a1 {
            0 => {
                if a2 == 1 {
                    4
                } else {
                    7
                }
            }
            1 => {
                if a2 == 2 {
                    5
                } else {
                    4
                }
            }
            2 => {
                if a2 == 3 {
                    6
                } else {
                    5
                }
            }
            3 => {
                if a2 != 0 {
                    6
                } else {
                    7
                }
            }
            _ => 0,
        };
        (code, flat)
    }

    /// sub_35F30 (:42799): smooth a ring of thickness `thick`+1 around
    /// the footprint (left+right column strips interleaved, then
    /// top+bottom row strips interleaved), each cell via sub_360C0.
    fn smooth_perimeter(&mut self, cx: u8, cy: u8, half_h: u16, half_w: u16, thick: u8) {
        let left_x = cx.wrapping_sub(half_w as u8).wrapping_sub(thick);
        let right_x = cx.wrapping_add(half_w as u8);
        let top_y = cy.wrapping_sub(half_h as u8);
        for row in 0..(2 * half_h) {
            let y = top_y.wrapping_add(row as u8);
            for k in 0..=thick {
                self.smooth_cell(tile(left_x.wrapping_add(k), y));
                self.smooth_cell(tile(right_x.wrapping_add(k), y));
            }
        }
        let strip_x = cx.wrapping_sub(half_w as u8).wrapping_sub(thick);
        let top_strip_y = cy.wrapping_sub(half_h as u8).wrapping_sub(thick);
        let bot_strip_y = cy.wrapping_add(half_h as u8);
        for col in 0..(2 * thick as u16 + 2 * half_w) {
            let x = strip_x.wrapping_add(col as u8);
            for k in 0..=thick {
                self.smooth_cell(tile(x, top_strip_y.wrapping_add(k)));
                self.smooth_cell(tile(x, bot_strip_y.wrapping_add(k)));
            }
        }
    }

    /// sub_360C0 (:42892): if the cell is land and its NW 2x2 quad has
    /// no building/wall texture (types 6..=0x22), replace its height by
    /// the 3x3 average over similarly-plain cells. Index arithmetic is
    /// linear u16 (rows wrap into each other) — faithful.
    fn smooth_cell(&mut self, t: usize) {
        if self.t.angle[t] & 7 == 0 || self.t.height[t] == 0 {
            return;
        }
        let plain = |ty_val: u8| ty_val <= 5 || ty_val > 0x22;
        let quad = [
            (t.wrapping_sub(257)) & 0xFFFF,
            (t.wrapping_sub(256)) & 0xFFFF,
            (t.wrapping_sub(1)) & 0xFFFF,
            t,
        ];
        if !quad.iter().all(|&q| plain(self.t.tile_type[q])) {
            return;
        }
        let mut sum = 0u32;
        let mut n = 0u32;
        let mut idx = (t.wrapping_sub(257)) & 0xFFFF;
        for _ in 0..3 {
            for _ in 0..3 {
                if plain(self.t.tile_type[idx]) {
                    n += 1;
                    sum += self.t.height[idx] as u32;
                }
                idx = (idx + 1) & 0xFFFF;
            }
            idx = (idx + 253) & 0xFFFF;
        }
        if n != 0 {
            self.t.height[t] = (sum / n) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_assets() -> FeatureAssets {
        // A tiny diamond ring grid centered at (15,15) mimicking
        // SEARCH.DAT's shape: ring = max(|dx|,|dy|) but with a 2x2 ring 0.
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        // One 4x4 building: plain floor (code 7) with a wall ring (0x10).
        let mut dat = Vec::new();
        for row in 0..4 {
            let inner = row == 1 || row == 2;
            dat.push(4u8);
            if inner {
                dat.extend_from_slice(&[0x10, 7, 7, 0x10]);
            } else {
                dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            }
            dat.push(0);
        }
        let tab: Vec<u8> = (0..24u32)
            .flat_map(|_| {
                let mut e = 0u32.to_le_bytes().to_vec();
                e.push(4);
                e.push(4);
                e
            })
            .collect();
        FeatureAssets::parse(&grid, &tab, &dat).unwrap()
    }

    fn thing(slot: u32, class: u16, model: u16, x: u16, y: u16) -> Thing {
        Thing {
            slot,
            kind: mgc_formats::ThingKind::Entity,
            class,
            model,
            x,
            y,
            dis_id: 0xFFFF,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }
    }

    fn flat_land(h: u8) -> Planes {
        Planes {
            height: vec![h; GRID],
            tile_type: vec![5; GRID],
            shading: vec![32; GRID],
            angle: vec![5; GRID], // class 5 land
            ceiling: Vec::new(),
        }
    }

    fn run(p: &mut Planes, things: &[Thing], seed: u32, assets: &FeatureAssets) {
        generate_features_mc1(
            TerrainPlanes {
                height: &mut p.height,
                tile_type: &mut p.tile_type,
                shading: &mut p.shading,
                angle: &mut p.angle,
            },
            things,
            seed,
            assets,
        );
    }

    #[test]
    fn ring_iterator_drops_last_cell_of_end_ring() {
        let assets = synthetic_assets();
        let (r0, r1) = (assets.rings[0].len(), assets.rings[1].len());
        let g = Gen::new(
            Planes {
                height: vec![0; GRID],
                tile_type: vec![0; GRID],
                shading: vec![0; GRID],
                angle: vec![0; GRID],
                ceiling: Vec::new(),
            },
            assets,
            0,
            ChassisParams::MC1,
            VerbSet::MC1,
        );
        assert_eq!(g.ring_cells(0, 0).len(), r0 - 1);
        assert_eq!(g.ring_cells(0, 1).len(), r0 + r1 - 1);
    }

    #[test]
    fn crater_digs_a_bowl() {
        let assets = synthetic_assets();
        let mut p = flat_land(100);
        let things = vec![thing(0, 10, 11, 128, 128)];
        run(&mut p, &things, 1234, &assets);
        let center = p.height[128 * 256 + 128];
        assert!(center < 100, "crater lowers the center, got {center}");
        // Far away untouched.
        assert_eq!(p.height[10 * 256 + 10], 100);
    }

    #[test]
    fn canyon_chain_carves_a_channel() {
        let assets = synthetic_assets();
        let mut p = flat_land(100);
        // Two chained canyon nodes: slots 0 and 1 (engine 1 and 2).
        let mut a = thing(0, 10, 31, 100, 100);
        a.swi_id = 1;
        a.child = 2;
        let mut b = thing(1, 10, 31, 120, 100);
        b.swi_id = 1;
        b.parent = 1;
        run(&mut p, &[a, b], 99, &assets);
        // Sampled along the line: meaningfully dug.
        let dug = (100..120)
            .filter(|&x| p.height[100 * 256 + x as usize] < 95)
            .count();
        assert!(dug > 10, "canyon digs along the segment, {dug} tiles dug");
        assert_eq!(p.height[10 * 256 + 200], 100, "far tiles untouched");
    }

    #[test]
    fn building_flattens_and_paints() {
        let assets = synthetic_assets();
        let mut p = flat_land(100);
        // Slope under the building so flattening is observable.
        for y in 0..256 {
            for x in 0..256 {
                p.height[y * 256 + x] = (60 + (x / 8) as i32).min(200) as u8;
            }
        }
        let mut b = thing(0, 10, 45, 128, 128);
        b.parent = 0; // build type 16
        run(&mut p, &[b], 7, &assets);
        // The 4x4 footprint centered near (128,128) got wall paint
        // (types 8/9 or table pairs) and the protection bit.
        let protected = (125..132)
            .flat_map(|y| (125..132).map(move |x| (x, y)))
            .filter(|&(x, y)| p.angle[y * 256 + x] & 0x80 != 0)
            .count();
        assert!(
            protected >= 8,
            "building marks protected tiles, got {protected}"
        );
    }

    /// Stuck-castle regression (2026-07-15): the transform must RETRY
    /// a failed painter/leveler spawn — retail keeps each commit
    /// inside the spawn-success arm (sub_47960 :56471, sub_47020
    /// :56107, sub_47080 :56126). Advancing to a pure-wait state with
    /// no helper spawned froze the castle forever (neither upgradable
    /// nor destroyable) under meteor pool exhaustion.
    #[test]
    fn castle_transform_retries_failed_spawns() {
        let mut g = Gen::new(
            flat_land(8),
            synthetic_assets(),
            1,
            ChassisParams::MC1,
            VerbSet::MC1,
        );
        let i = g.new_event().unwrap();
        {
            let e = &mut g.ent[i];
            e.class64 = 3;
            e.model65 = 2;
            e.x = 0x8000;
            e.y = 0x8000;
            e.f26 = 0; // fresh: awaiting the first level-up
            e.f59 = 0;
        }
        // Drain the pool, keeping three slots to hand back one at a
        // time (one per transform stage under test).
        let spares = [
            g.new_event().unwrap(),
            g.new_event().unwrap(),
            g.new_event().unwrap(),
        ];
        while g.new_event().is_some() {}

        // Case 0: exhausted pool → no commit, no wait state.
        g.castle_tick(i);
        assert_eq!(
            g.ent[i].f59, 0,
            "level-up retries instead of parking in wait"
        );
        assert_eq!(g.ent[i].f26, 0, "no level commit without a painter");
        g.free.push(spares[0] as u16);
        g.castle_tick(i);
        assert_eq!(g.ent[i].f59, 1, "freed slot: the painter spawned");
        assert_eq!(g.ent[i].f26, 1, "the level-up committed with it");
        assert!(
            g.ent
                .iter()
                .any(|e| e.class64 == 10 && e.model65 == 42 && e.flags & 0x400 == 0),
            "the m42 painter exists"
        );

        // Case 5 (leveler) and case 3 (repaint) hold their state too.
        g.ent[i].f59 = 5;
        g.castle_tick(i);
        assert_eq!(g.ent[i].f59, 5, "leveler spawn failure holds state 5");
        g.free.push(spares[1] as u16);
        g.castle_tick(i);
        assert_eq!(g.ent[i].f59, 6, "freed slot: the leveler handoff");
        g.ent[i].f59 = 3;
        g.castle_tick(i);
        assert_eq!(g.ent[i].f59, 3, "repaint spawn failure holds state 3");
        g.free.push(spares[2] as u16);
        g.castle_tick(i);
        assert_eq!(g.ent[i].f59, 1, "freed slot: the repaint painter wait");
    }

    /// Instant-death regression (2026-07-15): the balloon claim
    /// ticket is a RAW slot index — a collected ball's slot recycled
    /// by another class-10 entity (a dwelling) must not be devoured
    /// as if it were still the claimed (10,39) ball. Retail sub_47F90
    /// (:56742-73) shares the latent bug; the dispatcher only ever
    /// assigns (10,39), so the guard blocks nothing legitimate.
    #[test]
    fn balloon_ignores_recycled_claim_slots() {
        let mut g = Gen::new(
            flat_land(8),
            synthetic_assets(),
            1,
            ChassisParams::MC1,
            VerbSet::MC1,
        );
        let b = g.new_event().unwrap();
        {
            let e = &mut g.ent[b];
            e.class64 = 3;
            e.model65 = 4;
            e.x = 0x4000;
            e.y = 0x4000;
            e.z = 300;
            e.f126 = 8;
        }
        let own = g.ent[b].id24;
        // The claimed slot, recycled as a DWELLING (10,45) overlapping
        // the balloon (the LIFO-reuse shape).
        let t = g.new_event().unwrap();
        {
            let e = &mut g.ent[t];
            e.class64 = 10;
            e.model65 = 45;
            e.x = 0x4000;
            e.y = 0x4000;
            e.z = 300;
            e.f80 = 64;
            e.f82 = 64;
            e.f84 = 64;
            e.f144 = own;
            e.f140 = 500;
        }
        g.ent[b].f146 = t as u16;
        g.balloon_move(b);
        assert_eq!(g.ent[t].flags & 0x400, 0, "the dwelling survives");
        assert_eq!(g.ent[b].f146, 0, "the stale claim is dropped");

        // Control: the same slot as a real (10,39) ball IS collected.
        g.ent[t].model65 = 39;
        g.ent[b].f146 = t as u16;
        g.balloon_move(b);
        assert_ne!(g.ent[t].flags & 0x400, 0, "the real ball is absorbed");
    }

    fn mc2_gen() -> Gen {
        Gen::new(
            flat_land(8),
            synthetic_assets(),
            1,
            ChassisParams::MC1,
            crate::verbs::VerbSet::MC2,
        )
    }

    fn ctx_at(px: u16, py: u16, pz: i16) -> crate::mc1::mobs::MobCtx {
        crate::mc1::mobs::MobCtx {
            px,
            py,
            pz,
            pyaw: 0,
            pmana: 1000,
        }
    }

    /// The creature awake gate is a chassis parameter (the
    /// `--awake-range` G-class override): the faithful 0x240_0000
    /// (24 tiles, both retail engines) leaves a distant creature
    /// asleep; `i32::MAX` = the always-awake override arms it.
    #[test]
    fn awake_gate_is_a_chassis_parameter() {
        let run = |gate: i32| {
            let mut ch = ChassisParams::MC1;
            ch.awake_gate_sq = gate;
            let mut g = Gen::new(
                flat_land(8),
                synthetic_assets(),
                1,
                ch,
                crate::verbs::VerbSet::MC1,
            );
            // A bare class-5 creature 40 tiles from the player —
            // outside the retail gate, inside an infinite one.
            g.ent[5].class64 = 5;
            g.ent[5].act_life = 10;
            g.ent[5].x = 40 * 256;
            g.ent[5].y = 0;
            g.mob_awake_pass(&ctx_at(0, 0, 0));
            g.ent[5].f58
        };
        assert_eq!(run(0x240_0000), 0, "40 tiles out stays asleep (faithful)");
        assert_eq!(run(i32::MAX), 16, "always-awake override arms f58");
    }

    /// E3: only the %-forms of the m18 timer table draw the
    /// per-entity LCG; the flat forms are draw-free (the old
    /// unconditional pre-draw desynced the tank's rand stream) and
    /// (0,1)/(2,1) carry the pinned retail values.
    #[test]
    fn m18_timer_values_and_rng_parity() {
        let mut g = mc2_gen();
        let i = g.mc2_spawn_m18(0x4000, 0x4000, 300).unwrap();
        for (role, sub, flat) in [(2u8, 1u8, Some(10i16)), (2, 2, Some(12)), (2, 3, Some(14))] {
            let r0 = g.ent[i].rand;
            g.m18_timer(i, role, sub);
            assert_eq!(g.ent[i].f26, flat.unwrap(), "flat value ({role},{sub})");
            assert_eq!(g.ent[i].rand, r0, "flat forms draw NOTHING ({role},{sub})");
        }
        let r0 = g.ent[i].rand;
        g.m18_timer(i, 0, 1);
        assert!(
            (60..120).contains(&g.ent[i].f26),
            "(0,1) = 60 + rand%60, got {}",
            g.ent[i].f26
        );
        assert_ne!(g.ent[i].rand, r0, "(0,1) draws exactly its one roll");
    }

    /// E1: every in-range drain path STAYS in state 210 — only a
    /// target beyond the row range exits to 209.
    #[test]
    fn m26_leech_stays_draining_in_range() {
        let mut g = mc2_gen();
        let i = g.mc2_spawn_m26(0x4000, 0x4000, 300).unwrap();
        g.ent[i].tick70 = 210; // M26_BASE + 2, the drain state
        g.ent[i].f146 = crate::mc1::mobs::PLAYER_TARGET;
        g.ent[i].f63 = 0;
        let near = ctx_at(0x4100, 0x4000, 300); // 256 away, avatar
        let drained0 = g.mc2_player_drain.0;
        g.m26_tick(i, &near);
        assert_eq!(g.ent[i].tick70, 210, "in-range avatar: stay draining");
        assert!(g.mc2_player_drain.0 > drained0, "the drain landed");
        // Far target: the one authentic exit.
        let far = ctx_at(0x4000u16.wrapping_add(0x7000), 0x4000, 300);
        g.ent[i].f63 = 0;
        g.m26_tick(i, &far);
        assert_eq!(g.ent[i].tick70, 209, "out of range: back to approach");
    }

    /// E25: the aura claim handshake — the first aura in slot order
    /// keeps an overlapped ball; the second must not overwrite the
    /// pull (the old unconditional write was last-writer-wins).
    #[test]
    fn mc2_aura_first_claim_wins() {
        let mut g = mc2_gen();
        let mk_aura = |g: &mut Gen, x: u16| {
            let a = g.new_event().unwrap();
            let e = &mut g.ent[a];
            e.x = x;
            e.y = 0x4000;
            e.f26 = 14; // tile range
            e.act_life = 100;
            a
        };
        let a1 = mk_aura(&mut g, 0x4000);
        let a2 = mk_aura(&mut g, 0x4600);
        let b = g.new_event().unwrap();
        {
            let e = &mut g.ent[b];
            e.class64 = 10;
            e.model65 = 39;
            e.x = 0x4200;
            e.y = 0x4000;
        }
        g.mc2_aura_tick(a1);
        let claimed = (g.ent[b].dest_x, g.ent[b].dest_y);
        assert_eq!(
            g.mc2_aura_claim.0.get(&(b as u16)),
            Some(&(a1 as u16)),
            "aura 1 claims the ball"
        );
        g.mc2_aura_tick(a2);
        assert_eq!(
            (g.ent[b].dest_x, g.ent[b].dest_y),
            claimed,
            "the second aura must not steal the claimed ball's pull"
        );
    }

    /// E19: the m12 template walk falls back to 17 on exhaustion
    /// (empty bldgprm) — the old walk returned a failure and wrapped
    /// at the wrong boundary.
    #[test]
    fn m12_template_pick_falls_back_to_17() {
        let mut g = mc2_gen();
        assert_eq!(g.m12_pick_template(), 17, "exhaustion returns 17");
    }

    /// E23: the m25 death split under pool exhaustion still FALLS
    /// THROUGH to the (10,1) burst + the state advance — the old
    /// early return skipped both.
    #[test]
    fn m25_split_exhausted_pool_still_bursts() {
        let mut g = mc2_gen();
        let i = g.mc2_spawn_m25(0x4000, 0x4000, 300).unwrap();
        g.ent[i].tick70 = 204; // M25_BASE + 4, the split state
        g.ent[i].f71 = 0;
        g.ent[i].f140 = 0; // no mana: the sphere dump spawns nothing
        let spare = g.new_event().unwrap();
        while g.new_event().is_some() {}
        g.free.push(spare as u16); // exactly one slot: <= 1 = exhausted
        let ctx = ctx_at(0x1000, 0x1000, 300);
        g.m25_tick(i, &ctx);
        assert_eq!(g.ent[i].tick70, 205, "the split advanced past itself");
        assert!(
            g.ent
                .iter()
                .any(|e| e.class64 == 10 && e.model65 == 1 && e.flags & 0x400 == 0),
            "the (10,1) burst fired on the exhaustion path"
        );
    }

    /// F8: the Summon Army ring — a firefly (model 19) cast raises
    /// EIGHT allied nodes (weak-swarm size), every one carrying the
    /// caster's id24, the allied StageVar2=13 marker, the 8·M+7
    /// action and the 250-tick lifespan.
    #[test]
    fn summon_army_ring_is_eight_allied_fireflies() {
        let mut g = mc2_gen();
        g.mc2_spawn_summon_ring(0x4000, 0x4000, 19, 0x77);
        let nodes: Vec<&Ent> = g
            .ent
            .iter()
            .filter(|e| e.class64 == 5 && e.model65 == 19 && e.flags & 0x400 == 0)
            .collect();
        assert_eq!(nodes.len(), 8, "firefly army size");
        for e in nodes {
            assert_eq!(e.id24, 0x77, "allied to the caster");
            assert_eq!(e.site_z, 13, "the summon-army StageVar2 marker");
            assert_eq!(e.tick70, 19u8.wrapping_mul(8).wrapping_add(7));
            assert_eq!(e.f26, 250, "the 250-tick lifespan");
        }
    }

    /// E21: falling-prop gravity is position-THEN-decrement — the
    /// position takes the OLD velocity before the −24 applies.
    #[test]
    fn falling_prop_position_takes_old_velocity() {
        let mut g = mc2_gen();
        let i = g.new_event().unwrap();
        let ground = g.ground_z(0x4000, 0x4000) as i16;
        {
            let e = &mut g.ent[i];
            e.class64 = 2;
            e.model65 = 7;
            e.x = 0x4000;
            e.y = 0x4000;
            e.z = ground + 400;
            e.f44 = 100u16; // upward velocity
            e.f126 = 0;
            e.act_life = 100;
        }
        let z0 = g.ent[i].z;
        g.mc2_falling_tick(i);
        assert_eq!(g.ent[i].z, z0 + 100, "position moved by the OLD velocity");
        assert_eq!(g.ent[i].f44 as i16, 76, "then the velocity decremented");
    }

    #[test]
    fn deterministic() {
        let assets = synthetic_assets();
        let things = vec![
            thing(0, 10, 9, 50, 50),
            thing(1, 10, 11, 60, 60),
            thing(2, 10, 45, 80, 80),
        ];
        let mut p1 = flat_land(90);
        let mut p2 = flat_land(90);
        run(&mut p1, &things, 4242, &assets);
        run(&mut p2, &things, 4242, &assets);
        assert_eq!(p1.height, p2.height);
        assert_eq!(p1.tile_type, p2.tile_type);
        assert_eq!(p1.angle, p2.angle);
        assert_eq!(p1.shading, p2.shading);
    }
}
