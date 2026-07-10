//! Level entities -> billboards.
//!
//! Two paths: with a live [`mgc_sim::mc1::world::World`] (all games
//! since Phase 3.5), [`billboards_from_poses`] consumes the sim's
//! pose snapshot — sprite types, spawn facing and jitter come from
//! the ported spawn handlers' per-event LCG (byte-faithful), and
//! positions move with the mob tick; each pose resolves through its
//! game's own sprite table ([`resolve_pose_sprite`]). The static
//! [`billboards`] path resolves THING records through the MC1
//! (class, model) -> type-index mapping with a per-slot LCG
//! approximation — kept for `--no-terrain-features` comparison
//! renders.

use mgc_formats::{Thing, ThingKind};
use mgc_render::{Billboard, HealthBar};
use mgc_sim::ids::GameId;
use mgc_sim::mc1::entities::{Mc1TypePick, SpawnRng, mc1_entity_parts, mc1_entity_type};
use mgc_sim::mc1::sprite_stats::SPRITE_STATS;
use mgc_sim::mc1::world::LivePose;
use mgc_sim::{HEIGHT_SCALE, MAP_TILES};

/// Engine fixed-point units per tile.
const UNITS_PER_TILE: f32 = 256.0;

/// A live pose's sprite row resolved under the game's own table.
struct PoseSprite {
    sprite_base: u16,
    draw_type: u8,
    /// Billboard height in tiles.
    world_h: f32,
    /// Billboard width in tiles (health-bar sizing; the renderer
    /// derives its own draw width from the frame's pixel aspect).
    world_w: f32,
}

/// MC1/HW: `type_index` = the sprite-STATS row — explicit engine-unit
/// width/height (a 0 height derives from width and the base sprite's
/// pixel aspect, the original's load-time fixup).
///
/// MC2: `type_index` = the sprite-PARAM row (`particlesParameters`,
/// the entity's +90). `word_0` is the TMAPS sprite base;
/// `rot_speed_8` IS the world height in engine units — the renderer
/// draws `projScale * rotSpeed_8 / depth` px tall and re-derives
/// width from the frame's pixel aspect (remc2
/// GameRenderOriginal.cpp:2192-98; our renderer does the same with
/// `world_h`). A 0 height derives from `speed_6` and the pixel
/// aspect, mirroring the load-time cross-fill
/// (EventsFunctions.cpp:44895-903). The draw type is NOT the table's
/// `byte_12` — the loader overwrites it from the TMAPS entry header
/// byte (payload[1] = the flags high byte, :44906), which the bake
/// preserves in `SpriteEntry.flags`.
fn resolve_pose_sprite(
    game: GameId,
    type_index: u16,
    sprite_dims: &impl Fn(u16) -> Option<(u16, u16, u16)>,
) -> Option<PoseSprite> {
    match game {
        GameId::Mc1 | GameId::Mc1Hw => {
            let stats = SPRITE_STATS.get(type_index as usize)?;
            let world_h = if stats.height != 0 {
                stats.height as f32 / UNITS_PER_TILE
            } else {
                let (sw, sh, _) = sprite_dims(stats.sprite_base)?;
                if sw == 0 || stats.width == 0 {
                    return None;
                }
                stats.width as f32 * sh as f32 / sw as f32 / UNITS_PER_TILE
            };
            Some(PoseSprite {
                sprite_base: stats.sprite_base,
                draw_type: stats.draw_type,
                world_h,
                world_w: stats.width as f32 / UNITS_PER_TILE,
            })
        }
        GameId::Mc2 => {
            let param = mgc_sim::mc2::sprite_params::SPRITE_PARAMS.get(type_index as usize)?;
            let (sw, sh, flags) = sprite_dims(param.word_0)?;
            if sw == 0 || sh == 0 {
                return None;
            }
            let world_h = if param.rot_speed_8 != 0 {
                param.rot_speed_8 as f32 / UNITS_PER_TILE
            } else if param.speed_6 != 0 {
                param.speed_6 as f32 * sh as f32 / sw as f32 / UNITS_PER_TILE
            } else {
                return None;
            };
            Some(PoseSprite {
                sprite_base: param.word_0,
                draw_type: (flags >> 8) as u8,
                world_h,
                world_w: world_h * sw as f32 / sh as f32,
            })
        }
    }
}

/// Resolve all drawable entities of a level against the post-feature
/// height plane. `sprite_dims(id)` returns a sprite's pixel size (for
/// the load-time aspect fixup when the stats row has height 0).
pub fn billboards(
    things: &[Thing],
    height: &[u8],
    sprite_dims: impl Fn(u16) -> Option<(u16, u16, u16)>,
) -> Vec<Billboard> {
    let mut out = Vec::new();
    // Per-(class, model) spawn counters (AlternateByCount picks).
    let mut counts = std::collections::HashMap::<(u16, u16), u32>::new();

    for t in things {
        if t.kind != ThingKind::Entity {
            continue;
        }
        let Some(pick) = mc1_entity_type(t.class, t.model) else {
            continue;
        };
        let n = counts.entry((t.class, t.model)).or_default();
        let count = *n;
        *n += 1;

        // Position: tile center (the original spawns at `<<8 | +128`
        // engine units); trees additionally jitter by the LCG.
        let mut rng = SpawnRng(t.slot);
        let mut x = t.x as f32 + 0.5;
        let mut z = t.y as f32 + 0.5;
        let type_index = match pick {
            Mc1TypePick::Const(i) => i,
            Mc1TypePick::RandomBit(even, odd) => {
                // The tree spawner's draw order (sub_37BC0): actLife,
                // x jitter, y jitter, then the variant bit.
                rng.draw(); // actLife
                x += ((rng.draw() & 0x3F) as f32 - 32.0) / UNITS_PER_TILE;
                z += ((rng.draw() & 0x3F) as f32 - 32.0) / UNITS_PER_TILE;
                if rng.draw() & 1 != 0 { odd } else { even }
            }
            Mc1TypePick::RandomSevenSplit(major, minor) => {
                if rng.draw() % 7 < 4 {
                    major
                } else {
                    minor
                }
            }
            Mc1TypePick::AlternateByCount(first, second) => {
                if count.is_multiple_of(2) {
                    first
                } else {
                    second
                }
            }
            Mc1TypePick::Mana => {
                if t.swi_id >= 3 {
                    280
                } else {
                    77
                }
            }
        };

        let yaw = (rng.draw() & 0x7FF) as f32 * std::f32::consts::TAU / 2048.0;
        push_billboard(&mut out, height, &sprite_dims, type_index, x, z, yaw);

        // Multi-part creatures: the original spawns the body segments
        // stacked on the head (state 120) and its movement strings
        // them out from the first tick — a state the player never
        // sees. Until mobs move, settle the body in a trailing line
        // behind the head (approximation; movement will own segment
        // positions).
        const PART_SPACING: f32 = 0.35; // tiles between segments
        let (fx, fz) = (yaw.sin(), -yaw.cos()); // facing (yaw 0 = -Z)
        for (i, &part) in mc1_entity_parts(t.class, t.model).iter().enumerate() {
            let d = PART_SPACING * (i + 1) as f32;
            push_billboard(
                &mut out,
                height,
                &sprite_dims,
                part,
                x - fx * d,
                z - fz * d,
                yaw,
            );
        }
    }
    out
}

/// The live-world path: billboards straight from the sim's pose
/// snapshot — position, altitude, yaw, sprite type and animation frame
/// are all sim-owned (the spawn handlers ran the original's per-event
/// LCG), so nothing is re-derived here. The static `billboards` path
/// above remains for MC2 / `--no-terrain-features` comparison renders.
pub fn billboards_from_poses(
    game: GameId,
    poses: &[LivePose],
    sprite_dims: impl Fn(u16) -> Option<(u16, u16, u16)>,
) -> Vec<Billboard> {
    let mut out = Vec::new();
    for p in poses {
        if p.map_only {
            continue; // map presence only (unclaimed MC2 buildings)
        }
        let Some(s) = resolve_pose_sprite(game, p.type_index, &sprite_dims) else {
            continue;
        };
        out.push(Billboard {
            x: p.x,
            y: p.alt,
            z: p.z,
            yaw: p.yaw,
            sprite_base: s.sprite_base,
            draw_type: s.draw_type,
            frame: p.frame,
            world_h: s.world_h,
            blend: p.blend,
        });
    }
    out
}

/// Monster health bars from the live pose set (unfaithful debug
/// overlay, `enhancements.health_bars` / H): one classic red-on-black
/// bar hovering above each class-5 chain head, width tied to the
/// sprite's world width, life fraction sim-owned.
pub fn health_bars_from_poses(
    game: GameId,
    poses: &[LivePose],
    sprite_dims: impl Fn(u16) -> Option<(u16, u16, u16)>,
) -> Vec<HealthBar> {
    let mut out = Vec::new();
    for p in poses {
        let Some(frac) = p.life_frac else {
            continue;
        };
        let Some(s) = resolve_pose_sprite(game, p.type_index, &sprite_dims) else {
            continue;
        };
        out.push(HealthBar {
            x: p.x,
            y: p.alt + s.world_h + 0.15,
            z: p.z,
            w: s.world_w.clamp(0.6, 2.0),
            frac,
        });
    }
    out
}

/// The team color pairs `byte_99B58[16]` (remc1 :5740): per team,
/// even entry = the violet-family projectile/blink-A color, odd =
/// the blue-family creature/blink-B color. Raw palette indices,
/// exactly as plotted.
const TEAM_COLORS: [(u8, u8); 8] = [
    (0xB7, 0x71),
    (0x7D, 0x7A),
    (0x9D, 0x9A),
    (0x07, 0x5A),
    (0x1D, 0x1B),
    (0xDD, 0xDA),
    (0x3C, 0x39),
    (0x10, 0x0E),
];
#[cfg(test)]
const TEAM0_VIOLET: u8 = TEAM_COLORS[0].0;
#[cfg(test)]
const TEAM0_BLUE: u8 = TEAM_COLORS[0].1;

/// Icon patches for the map's UI-sprite markers (cropped from the
/// composited HSPR atlas): castle = sprite 58+team, balloon = 66+team
/// (remc1 sub_48710 :57230/:57234); the advertised-trigger X markers
/// 83/84 join when trigger markers land.
#[derive(Default)]
pub struct MapIcons {
    /// Castle stamps 58..=65 by team slot.
    pub castle: [Option<mgc_render::MapStamp>; 8],
    /// Balloon stamps 66..=73 by team slot.
    pub balloon: [Option<mgc_render::MapStamp>; 8],
    /// Spell icons by spell id (game-aware source sprite; see
    /// `ui::spell_icon_sprite`), shrunk to marker size — the
    /// expose-jar-spells debug stamps, consumed only when that
    /// option is on.
    pub spell: Vec<Option<mgc_render::MapStamp>>,
}

/// The MC2 map environment — selects the minimap's team-colour table
/// and map-type colours (`sub_48120` rebuilds `playersColors_E88E0x`
/// per `MapType`, remc2 EventsFunctions.cpp:32180-32262; the v90/v91/
/// v92 map-type colours are GameUI.cpp:1043-1063).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mc2MapEnv {
    #[default]
    Day,
    Night,
    Cave,
}

/// `playersColors_E88E0x[8][{bright, dark}]` — 8-bit palette indices
/// (EventsFunctions.cpp: day :32236-59, night :32188-32231, cave =
/// night except wizard 0 :32206-31). Column [2] (0x7B shared) is the
/// name-text colour, unused by the dot pass.
const MC2_TEAM_DAY: [(u8, u8); 8] = [
    (0x60, 0x64),
    (0x7B, 0x77),
    (0x1C, 0x18),
    (0x5B, 0x57),
    (0x9A, 0x97),
    (0xDB, 0xD8),
    (0x76, 0xA0),
    (0x3D, 0x3A),
];
const MC2_TEAM_NIGHT: [(u8, u8); 8] = [
    (0xA4, 0xAA),
    (0x77, 0x7D),
    (0xC0, 0xC6),
    (0x58, 0x5D),
    (0x97, 0x9D),
    (0xD7, 0xDD),
    (0x69, 0x62),
    (0xC9, 0xCF),
];

/// A 12-bit `0xRGB` minimap colour code (`MapColourIndexs.h`)
/// resolved through the level palette. Retail routes these through
/// `CLRD-0.DAT` — which is exactly a PRECOMPUTED nearest-palette-
/// index table for the 4096 codes (loaded Basic.cpp:330, indexed
/// GameUI.cpp:1166 etc.) — so the live quantization here is the same
/// mapping without a bake. (RGB nibble order: the enum's own names
/// align with the shared marker conventions — SPELLS 0xF00 red,
/// CIVILIANS 0x00F blue, CREATURE 0xFFF white.)
fn mc2_clrd(palette: &[[u8; 4]; 256], code: u16) -> u8 {
    let n = |v: u16| {
        let v = (v & 0xF) as u8;
        (v << 4) | v
    };
    nearest_palette_index(palette, [n(code >> 8), n(code >> 4), n(code)])
}

/// The MC2 minimap entity law — `DrawMinimapEntities_B_61A00` (remc2
/// GameUI.cpp:951, entity switch :1134-1411), the dot/colour rules on
/// our full-map view (the rotating-radar projection stays MC1-shaped;
/// the player asked for the COLORS — MC2 playtest-1).
///
/// INTERIM stand-ins (banked): the MSPRD bitmap stamps — class-11
/// models 0x0C/0x1F (map X-markers 83/84), the class-3 castle flag
/// (+58) and balloon (+66) families — draw as 2x2 dots until the
/// MSPRD bank is baked; the castle "rope" line (:1089-1130) joins the
/// guide-path machinery when MC2 castles land; the Beyond-Sight
/// enemy-wizard reveal (:1492-1529) waits for MC2 rivals.
fn mc2_map_dots(
    poses: &[LivePose],
    palette: &[[u8; 4]; 256],
    env: Mc2MapEnv,
    turn: u32,
) -> Vec<mgc_render::MapDot> {
    let team_tab = match env {
        Mc2MapEnv::Day => MC2_TEAM_DAY,
        Mc2MapEnv::Night | Mc2MapEnv::Cave => {
            let mut t = MC2_TEAM_NIGHT;
            if env == Mc2MapEnv::Cave {
                t[0] = (0xE0, 0x58);
            }
            t
        }
    };
    // Map-type colours (GameUI.cpp:1043-63): v92 = the unit fill,
    // v91/v90 = building/marker fallbacks.
    let (v92, v91, v90) = match env {
        Mc2MapEnv::Day => (mc2_clrd(palette, 0), 0xE8, 0x1C),
        Mc2MapEnv::Night => (mc2_clrd(palette, 4095), 0xE8, 0x84),
        Mc2MapEnv::Cave => (mc2_clrd(palette, 4095), 0x1C, mc2_clrd(palette, 240)),
    };
    // Blink phases `colorIndex_121[k] = (Turn / k) & 1`
    // (EventsFunctions.cpp:37563-66).
    let blink3 = (turn / 3) & 1 == 1;
    let blink2 = (turn / 2) & 1 == 1;
    let team = |t: Option<u8>| t.map(|t| team_tab[(t as usize).min(7)]);
    // LABEL_56 (GameUI.cpp:1291-96): owner wizard → team bright, else
    // the UNPOSSESSED_BUILDING2 code.
    let by_owner = |t: Option<u8>| {
        team(t)
            .map(|(bright, _)| bright)
            .unwrap_or_else(|| mc2_clrd(palette, 0xF0F))
    };
    // LABEL_173 (:1303-16): linked wizard → the blink pair, else v91.
    let linked_blink = |t: Option<u8>| {
        team(t)
            .map(|(bright, dark)| if blink3 { bright } else { dark })
            .unwrap_or(v91)
    };

    let mut out = Vec::new();
    for p in poses {
        if p.segment {
            continue;
        }
        let mut size = 1u8;
        let color = match (p.class, p.model) {
            // Scenery: tree v90 (:1147-62); marker stone blinks the
            // MARKER_STONE code (:1163-70); dolmen blinks the
            // UNPOSSESSED_BUILDING code against v90 (:1171-79).
            (2, 0) => v90,
            (2, 1) => {
                if blink3 {
                    mc2_clrd(palette, 0x88)
                } else {
                    continue;
                }
            }
            (2, 2) => {
                if blink2 {
                    mc2_clrd(palette, 0x888)
                } else {
                    v90
                }
            }
            (2, _) => continue,
            // Castle: retail = the +58 MSPRD flag stamp (:1188-95);
            // 2x2 team dot until the stamp bank bakes. Wizard bodies
            // (own = the player arrow; enemies need Beyond Sight) and
            // balloons skip.
            (3, 2) => {
                size = 2;
                by_owner(p.team)
            }
            (3, _) => continue,
            // Units (:1219-53): wizard-owned → team dark; wild
            // civilians (12..=14) → CIVILIANS; every other wild
            // creature → the map-type fill.
            (5, _) if p.team.is_some() => team(p.team).unwrap().1,
            (5, 12..=14) => mc2_clrd(palette, 15),
            (5, _) => v92,
            (9, _) => by_owner(p.team),
            // Class 10 (:1256-1332): 0x12 and 0x56/0x57 skip; the
            // portal (34) grows 2x2; buildings (45) and the flag
            // models blink the owner pair; 0x4E is own-only.
            (10, 0x12) => continue,
            (10, 34) => {
                size = 2;
                by_owner(p.team)
            }
            (10, 45) => {
                if p.team.is_some() {
                    linked_blink(p.team)
                } else {
                    by_owner(p.team)
                }
            }
            (10, 0x27..=0x39) => linked_blink(p.team),
            (10, 0x4E) => {
                if !p.player_owned {
                    continue;
                }
                let (b, d) = team_tab[0];
                if blink3 { b } else { d }
            }
            (10, 0x56 | 0x57) => continue,
            (10, _) => by_owner(p.team),
            // Switch X-markers (models 0x0C/0x1F → MSPRD stamps
            // 83/84, :1385-92): 2x2 white until the stamps bake.
            // Every other switch is undrawn (:1341-84).
            (11, 0x0C | 0x1F) => {
                size = 2;
                mc2_clrd(palette, 4095)
            }
            (11, _) => continue,
            // Spells + class-15 (:1396-1402).
            (12 | 15, _) => mc2_clrd(palette, 3840),
            // The class-14 model 5 blinker (:1403-09).
            (14, 5) => {
                if blink3 {
                    mc2_clrd(palette, 3840)
                } else {
                    mc2_clrd(palette, 4095)
                }
            }
            _ => continue,
        };
        out.push(mgc_render::MapDot {
            x: p.x,
            z: p.z,
            color,
            size,
        });
    }
    out
}

/// Map dots from the live pose set — the verbatim color switch of
/// remc1 sub_48710_48A50 (:57184-:57292); body segments hidden like
/// the original's state-120 exclusion. `turn` = the sim tick
/// (MC1's claimed-ball blink derives its ~4 Hz phase from it; MC2's
/// `colorIndex_121` divides it directly). `owned_buildings` = our
/// MC2-style enhancement: owned dwellings get a 2x2 grown dot instead
/// of the original's barely-distinct 1px.
///
/// MC2 worlds dispatch to [`mc2_map_dots`] — the real
/// DrawMinimapEntities_B_61A00 law.
pub fn map_dots_from_poses(
    game: GameId,
    poses: &[LivePose],
    palette: &[[u8; 4]; 256],
    owned_buildings: bool,
    env: Mc2MapEnv,
    turn: u32,
) -> Vec<mgc_render::MapDot> {
    if game == GameId::Mc2 {
        return mc2_map_dots(poses, palette, env, turn);
    }
    let blink = (turn >> 3) & 1 == 0;
    // The engine's computed colors go through its 16x16x16 RGB LUT
    // (byte_AD167_AD157, BLUE-major per the retail map's blue-violet
    // village dots): [1] = near-black (wild creatures), [16] = dark
    // green (villagers), [3856] = the vivid blue-violet (wild
    // class-9/10 things — houses, projectiles). The earlier "settlers
    // are purple" report resolves as these HOUSE dots (2026-07-07,
    // player screenshot).
    let near_black = nearest_palette_index(palette, vga(7, 3, 3));
    // Villager green: retail's LUT[16] decodes to (r0, g1, b0) — a
    // green so dark the nearest-palette match lands on black (the
    // playtest-6 "humans show black" report). The player's ground
    // truth is VISIBLY green dots and the map is gameplay-critical,
    // so aim at a legible mid-green instead of the literal cube
    // color (map-marker legibility ruling, 2026-07-05).
    let dark_green = nearest_palette_index(palette, vga(8, 32, 8));
    let wild_blue = nearest_palette_index(palette, vga(3, 7, 63));
    let red = nearest_palette_index(palette, vga(63, 3, 7));
    const SCENERY: u8 = 28;
    const WILD_BALL: u8 = 232; // v74 = -24 (:57291)

    let mut out = Vec::new();
    for p in poses {
        if p.segment {
            continue;
        }
        // LABEL_32 (:57272-76): owner class-3 → the team color;
        // wild → the LUT[3856] blue-violet.
        let team = p.team.map(|t| TEAM_COLORS[(t as usize).min(7)]);
        let owner_color = team.map(|(v, _)| v).unwrap_or(wild_blue);
        let mut size = 1u8;
        let color = match (p.class, p.model) {
            // Charred trees leave the map (v29 stays 0, :57219).
            (2, 0) if matches!(p.type_index, 226 | 227) => continue,
            // Models 1/3 = the settings-gated near-black family
            // (:57195-57210); the rest plain scenery 28.
            (2, 1 | 3) => near_black,
            (2, _) => SCENERY,
            // Castle/balloon draw as icon STAMPS, not dots.
            (3, _) => continue,
            (5, 12..=14) if team.is_none() => dark_green,
            // :57252 (the team pair's odd entry).
            (5, _) if team.is_some() => team.unwrap().1,
            (5, _) => near_black,
            (9, _) => owner_color,
            // Portal vortex: the 2x2 grown dot (v60 = 2, :57270).
            (10, 34) => {
                size = 2;
                owner_color
            }
            // Mana balls: wild = 232; claimed BLINK the team pair
            // on the global phase (:57282-91).
            (10, 39 | 40) => {
                if let Some((v, b)) = team {
                    if blink { v } else { b }
                } else {
                    WILD_BALL
                }
            }
            // Houses and every other class-10 effect: the owner rule
            // (yes, the original dots houses — the blue-violet
            // village speckles on the retail map). The enhancement
            // grows OWNED dwellings to 2x2 for legibility.
            (10, 45) => {
                if owned_buildings && team.is_some() {
                    size = 2;
                }
                owner_color
            }
            (10, _) => owner_color,
            (12, _) => red,
            _ => continue,
        };
        out.push(mgc_render::MapDot {
            x: p.x,
            z: p.z,
            color,
            size,
        });
    }
    out
}

/// Icon stamps from the live pose set — remc1 :57224-37 draws these
/// as UI sprites instead of dots. Retail rule (sub_48710): EVERY
/// castle stamps unconditionally with its team's sprite [58+team];
/// balloons [66+team] only when own or Beyond Sight is live (v59,
/// :57232-35). `beyond_sight` also reveals rival WIZARD positions —
/// retail draws their NAME there in team color (:57413-48); until
/// the DrawText path lands, a 2x2 team-color marker dot stands in
/// (banked with the font track).
pub fn map_stamps_from_poses(
    poses: &[LivePose],
    icons: &MapIcons,
    beyond_sight: bool,
    expose_jar_spells: bool,
) -> Vec<mgc_render::MapStamp> {
    let mut out = Vec::new();
    for p in poses {
        let team = p.team.map(|t| (t as usize).min(7)).unwrap_or(0);
        let icon = match (p.class, p.model) {
            (3, 2) => icons.castle[team].as_ref(),
            (3, 3) if p.team == Some(0) || beyond_sight => icons.balloon[team].as_ref(),
            // expose-jar-spells: pickable jars (MC1 class 12, MC2
            // class-15 tokens; owned manifestations never reach the
            // pose list) tag with their spell's icon.
            (12 | 15, m) if expose_jar_spells => {
                icons.spell.get(m as usize).and_then(Option::as_ref)
            }
            _ => None,
        };
        if let Some(i) = icon {
            let mut s = *i;
            s.x = p.x;
            s.z = p.z;
            out.push(s);
        }
    }
    out
}

/// The expose-jar-spells world markers: every pickable spell jar's
/// `(x, alt, z, spell id)` — MC1 class 12 (pre-placed, red or blue,
/// and death-scattered) plus MC2's class-15 tokens. model65 = spell
/// id (off_987DE dispatch, docs/traces/mc1-blue-jars.md).
pub fn jar_markers_from_poses(poses: &[LivePose]) -> Vec<(f32, f32, f32, u8)> {
    poses
        .iter()
        .filter(|p| matches!(p.class, 12 | 15))
        .map(|p| (p.x, p.alt, p.z, p.model))
        .collect()
}

/// The Beyond-Sight rival position markers (interim for the retail
/// name labels, :57413-48): a 2x2 dot in the rival's team color at
/// each live, non-cloaked rival wizard.
pub fn rival_markers(
    rivals: &[mgc_sim::mc1::world::RivalView],
    beyond_sight: bool,
) -> Vec<mgc_render::MapDot> {
    if !beyond_sight {
        return Vec::new();
    }
    rivals
        .iter()
        .filter(|r| r.alive && !r.invisible)
        .map(|r| mgc_render::MapDot {
            x: r.x,
            z: r.z,
            color: TEAM_COLORS[(r.slot as usize).min(7)].1,
            size: 2,
        })
        .collect()
}

/// Resolve one type index to a billboard at a world position; skips
/// rows whose size cannot be resolved (missing sprite dims).
fn push_billboard(
    out: &mut Vec<Billboard>,
    height: &[u8],
    sprite_dims: &impl Fn(u16) -> Option<(u16, u16, u16)>,
    type_index: u16,
    x: f32,
    z: f32,
    yaw: f32,
) {
    let Some(stats) = SPRITE_STATS.get(type_index as usize) else {
        return;
    };
    // World height; a 0 height derives from width and the base
    // sprite's pixel aspect (the original's load-time fixup).
    let world_h = if stats.height != 0 {
        stats.height as f32 / UNITS_PER_TILE
    } else {
        let Some((sw, sh, _)) = sprite_dims(stats.sprite_base) else {
            return;
        };
        if sw == 0 || stats.width == 0 {
            return;
        }
        stats.width as f32 * sh as f32 / sw as f32 / UNITS_PER_TILE
    };

    out.push(Billboard {
        x: x.rem_euclid(MAP_TILES as f32),
        y: ground_height(height, x, z),
        z: z.rem_euclid(MAP_TILES as f32),
        yaw,
        sprite_base: stats.sprite_base,
        draw_type: stats.draw_type,
        frame: 0,
        world_h,
        blend: 0,
    });
}

/// Entity dots for the overhead map, one pixel per entity, colored as
/// the original's map overlay (remc1 sub_48710_48A50, :57050): the
/// draw switches on LIVE entity class — trees/scenery (live class 2,
/// spawned state 0) = raw palette index 28; wild creatures =
/// near-black, wizard-owned creatures = the owner's team color
/// (byte_99B58; team 0 = the player's blue family), villagers (class
/// 5 models 12-14) = dark green; pre-placed mana/spell pickup jars
/// (class 12) = bright red — the vital red dots. "Computed" colors go
/// through the engine's 16x16x16 RGB->palette LUT (`byte_AD167_AD157`,
/// nearest-palette-match of RGB(3+4r, 3+4g, 3+4b) in 6-bit VGA), which
/// [`nearest_palette_index`] reproduces against the bundle palette.
///
/// Not replicated yet:
/// - Castle markers are team-colored UI-SPRITE ICONS in the original
///   (begSprTab 58+team / 66+team; balloons 83/84) — pending the
///   HSPR/UI-sprite bake; until then castles get a team-blue dot.
/// - Runtime loose mana balls (the orange / blinking claimed dots the
///   player reports) are live-class-2 models 1/3 entities spawned at
///   runtime, not level records — they land with mana mechanics.
/// - Dot blinking, the 2x2 grown dot of one creature sub-case, rival
///   name labels (runtime state, not placement).
pub fn map_dots(things: &[Thing], palette: &[[u8; 4]; 256]) -> Vec<mgc_render::MapDot> {
    let near_black = nearest_palette_index(palette, vga(7, 3, 3));
    let dark_green = nearest_palette_index(palette, vga(3, 7, 3));
    let red = nearest_palette_index(palette, vga(63, 3, 7));
    const SCENERY: u8 = 28;
    const PLAYER_TEAM_BLUE: u8 = 0x71; // byte_99B58[1 + 2*0]

    let mut out = Vec::new();
    for t in things {
        if t.kind != ThingKind::Entity {
            continue;
        }
        let color = match (t.class, t.model) {
            (2, _) => SCENERY,
            // Castle markers only (the original draws live class 3
            // from model 2 up; models 0/1 are the player balloon —
            // the map's center cross — and 3 needs rivals revealed).
            (3, 2) => PLAYER_TEAM_BLUE,
            (5, 12..=14) => dark_green,
            (5, _) => near_black,
            (12, _) => red,
            _ => continue,
        };
        out.push(mgc_render::MapDot {
            x: t.x as f32 + 0.5,
            z: t.y as f32 + 0.5,
            color,
            size: 1,
        });
    }
    out
}

/// Expand a 6-bit VGA triple the way the bundle palette was baked.
fn vga(r: u8, g: u8, b: u8) -> [u8; 3] {
    [
        (r << 2) | (r >> 4),
        (g << 2) | (g >> 4),
        (b << 2) | (b >> 4),
    ]
}

/// Nearest palette entry by squared RGB distance (the engine's
/// `sub_5CC70_5D180` palette-match used to build its RGB LUT).
fn nearest_palette_index(palette: &[[u8; 4]; 256], rgb: [u8; 3]) -> u8 {
    let mut best = (0usize, u32::MAX);
    for (i, p) in palette.iter().enumerate() {
        let d = p[..3]
            .iter()
            .zip(rgb)
            .map(|(&a, b)| {
                let d = a as i32 - b as i32;
                (d * d) as u32
            })
            .sum();
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0 as u8
}

/// The single-player start. MC1/HW: the class-3 model-4 marker
/// (player start #0 of 8; the original's marker spawner copies its
/// position into the per-player start table, sub_37720 :44068). MC2:
/// the (10, 0x52) wizard-start record — GenerateEvents spawns the
/// class-3 m0 wizard from it FIRST, `parent` = the player number
/// (remc2 Events.cpp:162-170, AddPlayer_4A920) — with the MC1-shaped
/// (3, 4) marker as the fallback: campaign level-000 authors THAT
/// and no (10, 0x52) at all. Returns tile-center coordinates.
/// Neither game stores an orientation (both wizards spawn at engine
/// yaw 0 = our north); altitude re-derives at spawn from ground
/// height (MC2 places at terrain alt exactly — hover is flight
/// physics, not spawn state).
pub fn player_start(game: GameId, things: &[Thing]) -> Option<(f32, f32)> {
    let mc1_marker = |t: &&Thing| t.kind == ThingKind::Entity && t.class == 3 && t.model == 4;
    match game {
        GameId::Mc1 | GameId::Mc1Hw => things.iter().find(mc1_marker),
        GameId::Mc2 => things
            .iter()
            .find(|t| t.class == 10 && t.model == 0x52 && t.parent == 0)
            .or_else(|| things.iter().find(mc1_marker)),
    }
    .map(|t| (t.x as f32 + 0.5, t.y as f32 + 0.5))
}

/// Spawn altitude above ground (the original's `sub_11F50 + 1` hover;
/// exact engine scaling still to pin down in the flight-model port).
pub const START_HOVER: f32 = 1.0;

/// Ground altitude at a position (public for spawn placement).
pub fn ground_at(height: &[u8], x: f32, z: f32) -> f32 {
    ground_height(height, x, z)
}

/// Bilinear ground altitude from the corner-based height grid.
fn ground_height(height: &[u8], x: f32, z: f32) -> f32 {
    if height.len() != MAP_TILES * MAP_TILES {
        return 0.0;
    }
    let n = MAP_TILES;
    let (fx, fz) = (x.rem_euclid(n as f32), z.rem_euclid(n as f32));
    let (x0, z0) = (fx.floor() as usize % n, fz.floor() as usize % n);
    let (x1, z1) = ((x0 + 1) % n, (z0 + 1) % n);
    let (tx, tz) = (fx.fract(), fz.fract());
    let h = |xx: usize, zz: usize| height[zz * n + xx] as f32 * HEIGHT_SCALE;
    let top = h(x0, z0) * (1.0 - tx) + h(x1, z0) * tx;
    let bot = h(x0, z1) * (1.0 - tx) + h(x1, z1) * tx;
    top * (1.0 - tz) + bot * tz
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(class: u8, model: u8, owned: bool, type_index: u16) -> LivePose {
        LivePose {
            class,
            model,
            type_index,
            frame: 0,
            x: 10.0,
            z: 10.0,
            alt: 1.0,
            yaw: 0.0,
            segment: false,
            life_frac: None,
            player_owned: owned,
            team: owned.then_some(0),
            blend: 0,
            map_only: false,
        }
    }

    /// The verbatim sub_48710 color switch (:57184-:57292).
    #[test]
    fn map_dot_color_switch() {
        let pal = [[0u8; 4]; 256];
        // blink true ↔ turn 0, false ↔ turn 8 ((turn >> 3) & 1 == 0).
        let dots = |p: LivePose, blink: bool| {
            map_dots_from_poses(
                GameId::Mc1,
                &[p],
                &pal,
                false,
                Mc2MapEnv::Day,
                if blink { 0 } else { 8 },
            )
        };

        // Player projectiles = the team-0 violet; wild = LUT blue.
        assert_eq!(dots(pose(9, 0, true, 42), false)[0].color, TEAM0_VIOLET);
        // Claimed mana balls blink the team pair on the phase.
        assert_eq!(dots(pose(10, 39, true, 105), true)[0].color, TEAM0_VIOLET);
        assert_eq!(dots(pose(10, 39, true, 105), false)[0].color, TEAM0_BLUE);
        // Wild balls = the raw 232 (:57291).
        assert_eq!(dots(pose(10, 39, false, 52), false)[0].color, 232);
        // Portals draw the 2x2 grown dot (:57270).
        assert_eq!(dots(pose(10, 34, false, 223), false)[0].size, 2);
        // Charred trees leave the map (:57219).
        assert!(dots(pose(2, 0, false, 226), false).is_empty());
        assert_eq!(dots(pose(2, 0, false, 83), false).len(), 1);
        // Castles/balloons are icon stamps, never dots.
        assert!(dots(pose(3, 2, true, 0), false).is_empty());
    }

    /// The MC2 minimap law (DrawMinimapEntities_B_61A00, remc2
    /// GameUI.cpp:1134-1411): 12-bit codes through the palette,
    /// team pairs from playersColors_E88E0x, blink phases Turn/k.
    #[test]
    fn mc2_map_dot_law() {
        // A palette with distinct anchors for the 12-bit codes:
        // [1] blue (CIVILIANS 0x00F), [2] red (SPELLS 0xF00),
        // [3] white (CREATURE 0xFFF), [4] magenta (0xF0F).
        let mut pal = [[0u8; 4]; 256];
        pal[1] = [0, 0, 255, 255];
        pal[2] = [255, 0, 0, 255];
        pal[3] = [255, 255, 255, 255];
        pal[4] = [255, 0, 255, 255];
        let dots = |p: LivePose, turn: u32| {
            map_dots_from_poses(GameId::Mc2, &[p], &pal, false, Mc2MapEnv::Night, turn)
        };

        // Wild civilians (12..=14) = CIVILIANS blue (:1228-37).
        assert_eq!(dots(pose(5, 13, false, 0), 0)[0].color, 1);
        // Every other wild creature = the night map-type fill (white
        // v92, :1246-53 + :1052-55).
        assert_eq!(dots(pose(5, 1, false, 0), 0)[0].color, 3);
        // Wizard-owned units = the team DARK column (:1222-26).
        assert_eq!(dots(pose(5, 4, true, 0), 0)[0].color, MC2_TEAM_NIGHT[0].1);
        // Wild buildings = UNPOSSESSED_BUILDING2 magenta (LABEL_56,
        // :1291-96); owned ones blink the team pair (:1273-86).
        assert_eq!(dots(pose(10, 45, false, 0), 0)[0].color, 4);
        assert_eq!(dots(pose(10, 45, true, 0), 3)[0].color, MC2_TEAM_NIGHT[0].0);
        assert_eq!(dots(pose(10, 45, true, 0), 0)[0].color, MC2_TEAM_NIGHT[0].1);
        // Spells = SPELLS red (:1396-1402).
        assert_eq!(dots(pose(12, 0, false, 0), 0)[0].color, 2);
        // The marker stone blinks phase 3 on/off (:1163-70).
        assert_eq!(dots(pose(2, 1, false, 0), 3).len(), 1);
        assert!(dots(pose(2, 1, false, 0), 0).is_empty());
        // Route explosions' class-10 model 0x12 is skipped (:1262-63);
        // the portal (34) grows 2x2 (:1264-67).
        assert!(dots(pose(10, 0x12, false, 0), 0).is_empty());
        assert_eq!(dots(pose(10, 34, false, 0), 0)[0].size, 2);
        // Switches: only the X-marker models draw (:1341-92).
        assert!(dots(pose(11, 0, false, 0), 0).is_empty());
        assert_eq!(dots(pose(11, 0x0C, false, 0), 0)[0].size, 2);
    }

    /// The MC2 billboard size law (remc2 GameRenderOriginal.cpp
    /// :2192-98 + the loader cross-fill :44895-903): `rot_speed_8` =
    /// world height in engine units; width from the frame's pixel
    /// aspect; draw type from the TMAPS header byte (flags >> 8).
    #[test]
    fn mc2_billboard_size_law() {
        // Row 43: word_0 0x52, rot_speed_8 0x96 = 150 (the row
        // remc2's own table authors at Type_WORD_D951C.cpp:47;
        // indexing verified 0-based against the vendored source).
        let dims = |id: u16| (id == 0x52).then_some((32u16, 64u16, 0x1200u16));
        let s = resolve_pose_sprite(GameId::Mc2, 43, &dims).unwrap();
        assert_eq!(s.sprite_base, 0x52);
        assert_eq!(s.draw_type, 0x12, "draw type = the TMAPS header byte");
        assert!((s.world_h - 150.0 / 256.0).abs() < 1e-6);
        assert!(
            (s.world_w - s.world_h * 0.5).abs() < 1e-6,
            "width = pixel aspect"
        );
        // MC1 rows still resolve through SPRITE_STATS untouched.
        let any_dims = |_: u16| Some((32u16, 64u16, 0u16));
        let mc1 = resolve_pose_sprite(GameId::Mc1, 43, &any_dims).unwrap();
        assert_eq!(mc1.sprite_base, SPRITE_STATS[43].sprite_base);
    }

    /// Player-start resolution against the real level-000 package
    /// (self-skips without baked data): MC2 falls back to the
    /// MC1-shaped (3,4) marker — level-000 authors that one.
    #[test]
    fn mc2_player_start_level_000() {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../baked/mc2/level-000.mgcl");
        let Ok(f) = std::fs::File::open(p) else {
            eprintln!("skipped: baked mc2 data not present");
            return;
        };
        let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(f).unwrap();
        assert_eq!(
            player_start(GameId::Mc2, &pkg.things.things),
            Some((77.5, 222.5)),
            "the (3,4) fallback start marker"
        );
    }
}
