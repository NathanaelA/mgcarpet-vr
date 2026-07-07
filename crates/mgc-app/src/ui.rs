//! Spellbook + HUD quad building over the bundle's HSPR UI sprites.
//!
//! Icon colors are resolved the original's way: every 2D blit runs
//! `blend[src | dest<<8]` against the pixel underneath (remc1
//! `strPal.byte_BB934_BB924`, sub_main.cpp:27444/27564 — the LUT is
//! the bundle's `blend-lut.bin`, TABLES +0x4000). We pre-composite
//! each sprite once against the book backdrop (dest 0 = black) into
//! an RGBA atlas, which reproduces the authentic icon colors (the
//! red heal heart) without palette machinery in the shader.
//!
//! Layout is functional-first (player-sanctioned): the book screen's
//! bottom-right rect gets the 24-slot grid in the original's display
//! order (`byte_99B88`); the in-game HUD gets the two equipped slots
//! bottom-left/right. Polish/reshuffle comes later with the real
//! book page art.

use mgc_formats::bundle::SpriteIndex;
use mgc_render::UiQuad;
use mgc_sim::spells::{SpellId, DISPLAY_ORDER, SPELL_COUNT};
use mgc_sim::world::LoadoutView;

/// UI sprite ids (remc1 begSprTab layout; ROADMAP "Spell repertoire").
const SPR_HILITE_LEFT: u32 = 1;
const SPR_HILITE_RIGHT: u32 = 2;
const SPR_SLOT_BG: u32 = 3;
const ICON_W: f32 = 62.0;
const ICON_H: f32 = 34.0;

pub struct UiAssets {
    pub atlas_w: u32,
    pub atlas_h: u32,
    pub atlas_rgba: Vec<u8>,
    /// Atlas uv (texels) of the pre-composited icon-on-slab tiles,
    /// indexed by internal spell id: [plain, left-equipped,
    /// right-equipped] — the equip highlights (sprites 1/2) are
    /// blend-composited over the icon like everything else, so they
    /// bake as whole-tile variants rather than overlay quads.
    slot_uv: [[[f32; 4]; 3]; SPELL_COUNT],
    /// Base-atlas frame rects (x, y, w, h) per HSPR sprite id — the
    /// map's icon-marker crops.
    sprite_rects: Vec<Option<(u32, u32, u32, u32)>>,
}

impl UiAssets {
    /// Composite the 8bpp UI atlas to RGBA through the blend LUT and
    /// the world palette. Two dest treatments, both the original's
    /// blit rule `blend[src | dest<<8]`:
    /// - the base atlas composites against dest 0 (dark backdrop);
    /// - the 24 spell icons ADDITIONALLY bake as icon-on-slot-slab
    ///   tiles (appended below the base atlas), compositing each
    ///   pixel against the slab sprite's — several icon ramps
    ///   (fireball flame, the possess glow) are luminous
    ///   brighten-the-dest rows that only read correctly over the
    ///   stone slab, exactly as the original draws them.
    pub fn build(
        index: SpriteIndex,
        pixels: &[u8],
        palette: &[[u8; 4]; 256],
        blend_lut: Option<&[u8]>,
    ) -> Self {
        let resolve = |src: u8, dest: u8| -> u8 {
            match blend_lut {
                Some(lut) => lut[src as usize | (dest as usize) << 8],
                None => src,
            }
        };
        let base_w = index.atlas_width as usize;
        let base_h = index.atlas_height as usize;

        // Slab sprite (entry 3) as an 8bpp grid, for per-pixel dests.
        let sprite_px = |id: usize| -> Option<(usize, usize, Vec<u8>)> {
            let e = index.sprites.get(id)?;
            let f = e.frames.first()?;
            let (w, h) = (e.width as usize, e.height as usize);
            let mut out = vec![0u8; w * h];
            for y in 0..h {
                let row = (f.y as usize + y) * base_w + f.x as usize;
                out[y * w..(y + 1) * w].copy_from_slice(&pixels[row..row + w]);
            }
            Some((w, h, out))
        };
        let slab = sprite_px(SPR_SLOT_BG as usize);

        // Composited slot tiles appended below the base atlas: 3
        // variants per spell (plain / left-equip / right-equip
        // highlight), 8 per row.
        let hilites = [
            None,
            sprite_px(SPR_HILITE_LEFT as usize),
            sprite_px(SPR_HILITE_RIGHT as usize),
        ];
        let (tile_w, tile_h) = slab
            .as_ref()
            .map(|(w, h, _)| (*w, *h))
            .unwrap_or((ICON_W as usize + 2, ICON_H as usize + 3));
        let tiles_per_row = base_w / tile_w;
        let tile_count = SPELL_COUNT * 3;
        let tile_rows = tile_count.div_ceil(tiles_per_row);
        let total_h = base_h + tile_rows * tile_h;
        let mut rgba = vec![0u8; base_w * total_h * 4];

        for (i, &src) in pixels.iter().enumerate() {
            if src == 0 {
                continue; // transparent
            }
            let c = palette[resolve(src, 0) as usize];
            rgba[i * 4..i * 4 + 3].copy_from_slice(&c[..3]);
            rgba[i * 4 + 3] = 255;
        }

        let mut slot_uv = [[[0.0f32; 4]; 3]; SPELL_COUNT];
        for spell in 0..SPELL_COUNT {
            let icon = sprite_px(spell + 6);
            for (variant, hilite) in hilites.iter().enumerate() {
                let tile = spell * 3 + variant;
                let (tx, ty) = (
                    (tile % tiles_per_row) * tile_w,
                    base_h + (tile / tiles_per_row) * tile_h,
                );
                slot_uv[spell][variant] =
                    [tx as f32, ty as f32, tile_w as f32, tile_h as f32];
                for y in 0..tile_h {
                    for x in 0..tile_w {
                        // Layered exactly like the original's blits:
                        // slab, then icon, then the equip highlight,
                        // each `blend[src | under<<8]`.
                        let mut v = match &slab {
                            Some((w, _, px)) => px[y * w + x],
                            None => 0,
                        };
                        if let Some((iw, ih, px)) = &icon {
                            let (ox, oy) = ((tile_w - iw) / 2, (tile_h - ih) / 2);
                            if x >= ox && x < ox + iw && y >= oy && y < oy + ih {
                                let s = px[(y - oy) * iw + (x - ox)];
                                if s != 0 {
                                    v = resolve(s, v);
                                }
                            }
                        }
                        if let Some((hw, hh, px)) = hilite {
                            // Top-aligned, clipped to the slab tile
                            // (the sprite runs a few rows taller —
                            // the HUD panel's bar area).
                            let ox = (tile_w.saturating_sub(*hw)) / 2;
                            if x >= ox && x < ox + hw && y < *hh {
                                let s = px[y * hw + (x - ox)];
                                if s != 0 {
                                    v = resolve(s, v);
                                }
                            }
                        }
                        if v == 0 {
                            continue;
                        }
                        let c = palette[v as usize];
                        let o = ((ty + y) * base_w + tx + x) * 4;
                        rgba[o..o + 3].copy_from_slice(&c[..3]);
                        rgba[o + 3] = 255;
                    }
                }
            }
        }

        // Frame rects per sprite id in the base atlas region (the
        // map's icon markers crop from here).
        let sprite_rects = index
            .sprites
            .iter()
            .map(|e| {
                e.frames
                    .first()
                    .map(|f| (f.x as u32, f.y as u32, e.width as u32, e.height as u32))
            })
            .collect();

        Self {
            atlas_w: index.atlas_width,
            atlas_h: total_h as u32,
            atlas_rgba: rgba,
            slot_uv,
            sprite_rects,
        }
    }

    /// Crop one HSPR sprite from the composited base atlas as an RGBA
    /// patch for map stamping (castle 58+team, balloon 66+team —
    /// remc1 sub_48710 :57230/:57234). Position is filled by the
    /// caller per entity.
    pub fn map_stamp(&self, id: usize) -> Option<mgc_render::MapStamp> {
        let (x, y, w, h) = self.sprite_rects.get(id).copied().flatten()?;
        if w == 0 || h == 0 {
            return None;
        }
        let aw = self.atlas_w as usize;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for row in 0..h as usize {
            let src = ((y as usize + row) * aw + x as usize) * 4;
            let dst = row * w as usize * 4;
            rgba[dst..dst + w as usize * 4]
                .copy_from_slice(&self.atlas_rgba[src..src + w as usize * 4]);
        }
        Some(mgc_render::MapStamp {
            x: 0.0,
            z: 0.0,
            w,
            h,
            rgba: std::sync::Arc::new(rgba),
        })
    }

    /// The pre-composited icon-on-slab tile for a spell; `variant`
    /// 0 = plain, 1 = left-equipped, 2 = right-equipped highlight.
    fn slot_quad(&self, spell: SpellId, variant: usize, rect: [f32; 4], tint: [f32; 4]) -> UiQuad {
        UiQuad {
            rect,
            uv: self.slot_uv[spell.0 as usize][variant],
            tint,
        }
    }

}

fn solid(rect: [f32; 4], tint: [f32; 4]) -> UiQuad {
    UiQuad {
        rect,
        uv: [0.0; 4],
        tint,
    }
}

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Unowned spells: icon ghosted way down (the original greys via the
/// blend table's dim row; a tint is our stand-in).
const GHOST: [f32; 4] = [0.28, 0.28, 0.28, 1.0];
const MANA_BLUE: [f32; 4] = [0.15, 0.35, 0.9, 1.0];
const COOLDOWN_SHADE: [f32; 4] = [0.0, 0.0, 0.0, 0.55];
const BAR_BG: [f32; 4] = [0.05, 0.05, 0.05, 0.9];

/// The book screen's spell-grid rect (must match the renderer's book
/// layout fractions: map pane 0.6 left, viewport 0.42 top-right).
fn book_rect(w: f32, h: f32) -> (f32, f32, f32, f32) {
    let x = w * 0.6;
    let y = h * 0.42;
    (x, y, w - x, h - y)
}

/// One grid cell: outer rect of display slot `k` (4 cols x 6 rows).
fn book_cell(w: f32, h: f32, k: usize) -> [f32; 4] {
    let (bx, by, bw, bh) = book_rect(w, h);
    let (cols, rows) = (4.0, 6.0);
    let (cw, ch) = (bw / cols, bh / rows);
    let (col, row) = ((k % 4) as f32, (k / 4) as f32);
    // Icon area keeps the 62x34 aspect inside the cell with padding.
    let pad = 0.08;
    let (iw, ih) = fit(ICON_W, ICON_H, cw * (1.0 - 2.0 * pad), ch * (1.0 - 2.0 * pad) - 6.0);
    [
        bx + col * cw + (cw - iw) / 2.0,
        by + row * ch + (ch - 6.0 - ih) / 2.0,
        iw,
        ih,
    ]
}

fn fit(sw: f32, sh: f32, mw: f32, mh: f32) -> (f32, f32) {
    let s = (mw / sw).min(mh / sh);
    (sw * s, sh * s)
}

/// Book screen quads + the display slot under the cursor (if any).
pub fn book_quads(
    assets: &UiAssets,
    loadout: &LoadoutView,
    w: f32,
    h: f32,
    cursor: (f32, f32),
) -> (Vec<UiQuad>, Option<SpellId>) {
    let mut quads = Vec::with_capacity(SPELL_COUNT * 4 + 4);
    let mut hovered = None;
    for (k, &spell) in DISPLAY_ORDER.iter().enumerate() {
        let spell_id = SpellId(spell);
        let cell = book_cell(w, h, k);
        let owned = loadout.owned[spell as usize];
        let over = cursor.0 >= cell[0]
            && cursor.0 < cell[0] + cell[2]
            && cursor.1 >= cell[1]
            && cursor.1 < cell[1] + cell[3] + 6.0;
        if over {
            hovered = Some(spell_id);
        }

        // Icon pre-composited on its slot slab (one quad); the tile
        // variant carries the equipped-hand highlight.
        let bg_rect = [cell[0] - 3.0, cell[1] - 3.0, cell[2] + 6.0, cell[3] + 6.0];
        let variant = if loadout.left == Some(spell) {
            1
        } else if loadout.right == Some(spell) {
            2
        } else {
            0
        };
        quads.push(assets.slot_quad(
            spell_id,
            variant,
            bg_rect,
            if owned { WHITE } else { GHOST },
        ));
        if owned {
            // Cooldown veil sweeps down as the burst counter runs.
            let cd = loadout.cooldown[spell as usize];
            if cd > 0.0 {
                quads.push(solid(
                    [cell[0], cell[1], cell[2], cell[3] * cd],
                    COOLDOWN_SHADE,
                ));
            }
        }
        if over && owned {
            // Hover ring: thin bright frame.
            let f = [cell[0] - 4.0, cell[1] - 4.0, cell[2] + 8.0, cell[3] + 8.0];
            let t = [0.9, 0.85, 0.5, 0.9];
            quads.push(solid([f[0], f[1], f[2], 2.0], t));
            quads.push(solid([f[0], f[1] + f[3] - 2.0, f[2], 2.0], t));
            quads.push(solid([f[0], f[1], 2.0, f[3]], t));
            quads.push(solid([f[0] + f[2] - 2.0, f[1], 2.0, f[3]], t));
        }
    }
    // Player mana bar along the very bottom of the spell pane.
    let (bx, _, bw, _) = book_rect(w, h);
    let frac = loadout.mana as f32 / loadout.mana_max.max(1) as f32;
    quads.push(solid([bx + 8.0, h - 14.0, bw - 16.0, 8.0], BAR_BG));
    quads.push(solid(
        [bx + 9.0, h - 13.0, (bw - 18.0) * frac.clamp(0.0, 1.0), 6.0],
        MANA_BLUE,
    ));
    (quads, hovered)
}

/// In-game HUD: the two equipped slots + the mana bar.
pub fn hud_quads(assets: &UiAssets, loadout: &LoadoutView, w: f32, h: f32) -> Vec<UiQuad> {
    let mut quads = Vec::new();
    let scale = (w / 640.0).max(1.0);
    let (iw, ih) = (ICON_W * scale, ICON_H * scale);
    let margin = 12.0 * scale;
    let slots = [
        (loadout.left, margin, 1usize),
        (loadout.right, w - margin - iw, 2usize),
    ];
    for (spell, x, variant) in slots {
        let rect = [x, h - ih - margin - 10.0 * scale, iw, ih];
        let Some(spell) = spell else {
            quads.push(solid(rect, BAR_BG));
            continue;
        };
        quads.push(assets.slot_quad(SpellId(spell), variant, rect, WHITE));
        let cd = loadout.cooldown[spell as usize];
        if cd > 0.0 {
            quads.push(solid([rect[0], rect[1], rect[2], rect[3] * cd], COOLDOWN_SHADE));
        }
        // Per-slot readiness strip under the icon.
        let bar = [rect[0], rect[1] + rect[3] + 2.0, rect[2], 5.0 * scale];
        quads.push(solid(bar, BAR_BG));
        quads.push(solid(
            [bar[0], bar[1], bar[2] * (1.0 - cd), bar[3]],
            [0.85, 0.7, 0.2, 1.0],
        ));
    }
    // Center mana bar (the castable pool vs the claimed ceiling).
    let frac = loadout.mana as f32 / loadout.mana_max.max(1) as f32;
    let bw = w * 0.25;
    quads.push(solid([(w - bw) / 2.0, h - 16.0 * scale, bw, 7.0 * scale], BAR_BG));
    quads.push(solid(
        [
            (w - bw) / 2.0 + 1.0,
            h - 16.0 * scale + 1.0,
            (bw - 2.0) * frac.clamp(0.0, 1.0),
            7.0 * scale - 2.0,
        ],
        MANA_BLUE,
    ));
    // The castle panel's WORLD-RELATIVE pair (sub_22E50 :27172-290):
    // the original never shows absolute mana — everything scales
    // against the level's total. Row 1 = castle capacity / world
    // (amber), row 2 = banked / world (blue), and the level-goal
    // tick at win_pct% on both rows (`(value<<6)/world` fills +
    // `(pct<<6)/100` tick on a shared ruler). The tick turns green
    // once the win latches (16 sustained ticks over the goal).
    if loadout.world_mana > 0 {
        let world = loadout.world_mana as f32;
        let pw = w * 0.18;
        let px = w - pw - 12.0 * scale;
        let mut py = h - 60.0 * scale;
        let rows: [(f32, [f32; 4]); 2] = [
            (
                loadout.castle.map_or(0.0, |(_, cap, _)| cap as f32) / world,
                [0.85, 0.7, 0.2, 1.0],
            ),
            (loadout.banked as f32 / world, MANA_BLUE),
        ];
        for (row_frac, color) in rows {
            quads.push(solid([px, py, pw, 5.0 * scale], BAR_BG));
            quads.push(solid(
                [px + 1.0, py + 1.0, (pw - 2.0) * row_frac.clamp(0.0, 1.0), 5.0 * scale - 2.0],
                color,
            ));
            if loadout.win_pct > 0 {
                let tick = px + pw * (loadout.win_pct as f32 / 100.0).clamp(0.0, 1.0);
                let tick_color = if loadout.completed {
                    [0.3, 0.95, 0.3, 1.0]
                } else {
                    [0.95, 0.95, 0.95, 1.0]
                };
                quads.push(solid([tick - 1.0, py - 1.0, 2.0, 7.0 * scale], tick_color));
            }
            py += 8.0 * scale;
        }
    }
    quads
}
