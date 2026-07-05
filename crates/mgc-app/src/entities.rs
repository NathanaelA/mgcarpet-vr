//! Level entities -> billboards: resolve each THING record through the
//! original's (class, model) -> type-index mapping and sprite stats
//! into what the renderer draws.
//!
//! Static placement only (the "inhabited world" slice): every drawable
//! entity stands at its spawn position with its authentic sprite,
//! rotation-view behavior and world size; no behavior, no animation
//! ticking yet.
//!
//! Fidelity notes (deliberate approximations until entity spawning is
//! ported 1:1, tracked in docs/ROADMAP.md):
//! - The original draws its spawn LCG in strict slot order across the
//!   whole load; we seed a fresh LCG per slot instead, so random picks
//!   (tree variants) and jitter are stable but not byte-identical.
//! - Entity facing: level records carry no yaw; the original assigns
//!   it at spawn (mostly LCG). We use the per-slot LCG too.

use mgc_formats::{Thing, ThingKind};
use mgc_render::Billboard;
use mgc_sim::mc1_entities::{Mc1TypePick, SpawnRng, mc1_entity_type};
use mgc_sim::mc1_sprite_stats::SPRITE_STATS;
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

        let Some(stats) = SPRITE_STATS.get(type_index as usize) else {
            continue;
        };
        // World height; a 0 height derives from width and the base
        // sprite's pixel aspect (the original's load-time fixup).
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
            x: x.rem_euclid(MAP_TILES as f32),
            y: ground_height(height, x, z),
            z: z.rem_euclid(MAP_TILES as f32),
            yaw: (rng.draw() & 0x7FF) as f32 * std::f32::consts::TAU / 2048.0,
            sprite_base: stats.sprite_base,
            draw_type: stats.draw_type,
            world_h,
        });
    }
    out
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
