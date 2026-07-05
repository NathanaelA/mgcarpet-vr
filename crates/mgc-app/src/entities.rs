//! Level entities -> billboards.
//!
//! Two paths: with a live [`mgc_sim::world::World`] (MC1/HW),
//! [`billboards_from_poses`] consumes the sim's pose snapshot — sprite
//! types, spawn facing and jitter come from the ported spawn handlers'
//! per-event LCG (byte-faithful), and positions move with the mob
//! tick. The static [`billboards`] path resolves THING records through
//! the (class, model) -> type-index mapping with a per-slot LCG
//! approximation — kept for MC2 (its runtime is unported) and
//! `--no-terrain-features` comparison renders.

use mgc_formats::{Thing, ThingKind};
use mgc_render::{Billboard, HealthBar};
use mgc_sim::mc1_entities::{Mc1TypePick, SpawnRng, mc1_entity_parts, mc1_entity_type};
use mgc_sim::mc1_sprite_stats::SPRITE_STATS;
use mgc_sim::world::LivePose;
use mgc_sim::{HEIGHT_SCALE, MAP_TILES};

/// Engine fixed-point units per tile.
const UNITS_PER_TILE: f32 = 256.0;

/// Resolve all drawable entities of a level against the post-feature
/// height plane. `sprite_dims(id)` returns a sprite's pixel size (for
/// the load-time aspect fixup when the stats row has height 0).
pub fn billboards(
    things: &[Thing],
    height: &[u8],
    sprite_dims: impl Fn(u16) -> Option<(u16, u16)>,
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
    poses: &[LivePose],
    sprite_dims: impl Fn(u16) -> Option<(u16, u16)>,
) -> Vec<Billboard> {
    let mut out = Vec::new();
    for p in poses {
        let Some(stats) = SPRITE_STATS.get(p.type_index as usize) else {
            continue;
        };
        let world_h = if stats.height != 0 {
            stats.height as f32 / UNITS_PER_TILE
        } else {
            let Some((sw, sh)) = sprite_dims(stats.sprite_base) else {
                continue;
            };
            if sw == 0 || stats.width == 0 {
                continue;
            }
            stats.width as f32 * sh as f32 / sw as f32 / UNITS_PER_TILE
        };
        out.push(Billboard {
            x: p.x,
            y: p.alt,
            z: p.z,
            yaw: p.yaw,
            sprite_base: stats.sprite_base,
            draw_type: stats.draw_type,
            frame: p.frame,
            world_h,
        });
    }
    out
}

/// Monster health bars from the live pose set (unfaithful debug
/// overlay, `enhancements.health_bars` / H): one classic red-on-black
/// bar hovering above each class-5 chain head, width tied to the
/// sprite's world width, life fraction sim-owned.
pub fn health_bars_from_poses(
    poses: &[LivePose],
    sprite_dims: impl Fn(u16) -> Option<(u16, u16)>,
) -> Vec<HealthBar> {
    let mut out = Vec::new();
    for p in poses {
        let Some(frac) = p.life_frac else {
            continue;
        };
        let Some(stats) = SPRITE_STATS.get(p.type_index as usize) else {
            continue;
        };
        let world_h = if stats.height != 0 {
            stats.height as f32 / UNITS_PER_TILE
        } else {
            let Some((sw, sh)) = sprite_dims(stats.sprite_base) else {
                continue;
            };
            if sw == 0 || stats.width == 0 {
                continue;
            }
            stats.width as f32 * sh as f32 / sw as f32 / UNITS_PER_TILE
        };
        out.push(HealthBar {
            x: p.x,
            y: p.alt + world_h + 0.15,
            z: p.z,
            w: (stats.width as f32 / UNITS_PER_TILE).clamp(0.6, 2.0),
            frac,
        });
    }
    out
}

/// Map dots from the live pose set — same color switch as [`map_dots`]
/// (remc1 sub_48710_48A50); body segments are hidden from the map like
/// the original's state-120 exclusion.
pub fn map_dots_from_poses(
    poses: &[LivePose],
    palette: &[[u8; 4]; 256],
) -> Vec<mgc_render::MapDot> {
    let near_black = nearest_palette_index(palette, vga(3, 3, 7));
    let dark_green = nearest_palette_index(palette, vga(3, 7, 3));
    let red = nearest_palette_index(palette, vga(63, 3, 7));
    const SCENERY: u8 = 28;
    const PLAYER_TEAM_BLUE: u8 = 0x71;

    let mut out = Vec::new();
    for p in poses {
        if p.segment {
            continue;
        }
        let color = match (p.class, p.model) {
            (2, _) => SCENERY,
            (3, 2) => PLAYER_TEAM_BLUE,
            (5, 12..=14) => dark_green,
            (5, _) => near_black,
            (12, _) => red,
            _ => continue,
        };
        out.push(mgc_render::MapDot {
            x: p.x,
            z: p.z,
            color,
        });
    }
    out
}

/// Resolve one type index to a billboard at a world position; skips
/// rows whose size cannot be resolved (missing sprite dims).
fn push_billboard(
    out: &mut Vec<Billboard>,
    height: &[u8],
    sprite_dims: &impl Fn(u16) -> Option<(u16, u16)>,
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
        let Some((sw, sh)) = sprite_dims(stats.sprite_base) else {
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
    let near_black = nearest_palette_index(palette, vga(3, 3, 7));
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

/// The single-player start: the class-3 model-4 marker (player start
/// #0 of 8; the original's marker spawner copies its position into the
/// per-player start table, sub_37720 :44068). Returns tile-center
/// coordinates. The spawn stores NO orientation — the wizard entity is
/// zero-initialized, i.e. the original starts facing engine yaw 0
/// (north; our yaw 0) — and altitude is re-derived at spawn as ground
/// height plus a hover offset.
pub fn player_start(things: &[Thing]) -> Option<(f32, f32)> {
    things
        .iter()
        .find(|t| t.kind == ThingKind::Entity && t.class == 3 && t.model == 4)
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
