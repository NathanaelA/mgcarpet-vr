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
use mgc_sim::spells::{DISPLAY_ORDER, SPELL_COUNT, SPELLS, SpellId};
use mgc_sim::world::{LifeState, LoadoutView, PlayerVitals};

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

        // Base atlas = RAW palette colors, no blend. The original draws
        // the panel BACKGROUNDS (sub_23940) blended over the live
        // framebuffer (the bright sky) — NOT over black — and the icons/
        // glyphs (DrawBitmap_60CE0) raw with no blend at all. Compositing
        // the base atlas through `blend[src|0]` (over black) was
        // darkening every panel sprite (~30%: [41] (109,109,117) →
        // (81,73,69)); raw palette restores their true brightness. The
        // luminous spell-icon ramps that genuinely need the blend read
        // over the stone slab in the slot tiles below, not here.
        for (i, &src) in pixels.iter().enumerate() {
            if src == 0 {
                continue; // transparent
            }
            let c = palette[src as usize];
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
                slot_uv[spell][variant] = [tx as f32, ty as f32, tile_w as f32, tile_h as f32];
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

    /// The UI-atlas UV rect (texels) for one HSPR sprite, for map
    /// stamping (castle 58+team, balloon 66+team — remc1 sub_48710
    /// :57230/:57234). The renderer projects the world position and
    /// blits it upright over the rotated map; position is filled by the
    /// caller per entity.
    pub fn map_stamp(&self, id: usize) -> Option<mgc_render::MapStamp> {
        let (x, y, w, h) = self.sprite_rects.get(id).copied().flatten()?;
        if w == 0 || h == 0 {
            return None;
        }
        // Per-range anchor (remc1 sub_48710 :57344-64): castle sprites
        // 58-65 pin at bottom-LEFT (the flagpole foot in the lower-left
        // of the rectangular flag icon); balloon sprites 66-73 pin at
        // bottom-CENTER (the balloon base). Others default center-bottom.
        let anchor = match id {
            58..=65 => [0.0, 1.0], // castle: bottom-left
            66..=73 => [0.5, 1.0], // balloon: bottom-center
            _ => [0.5, 1.0],
        };
        Some(mgc_render::MapStamp {
            x: 0.0,
            z: 0.0,
            w,
            h,
            uv: [x as f32, y as f32, w as f32, h as f32],
            anchor,
        })
    }

    /// The pre-composited icon-on-slab tile for a spell; `variant`
    /// 0 = plain, 1 = left-equipped, 2 = right-equipped highlight. Kept
    /// for the composited luminous-ramp look (the icon blended over the
    /// slab); the book now draws slab + native-uniform icon separately to
    /// avoid the non-4:3 stretch, and the equipped-hand variants are the
    /// parked unfaithful binding indicator.
    #[allow(dead_code)]
    fn slot_quad(&self, spell: SpellId, variant: usize, rect: [f32; 4], tint: [f32; 4]) -> UiQuad {
        UiQuad {
            rect,
            uv: self.slot_uv[spell.0 as usize][variant],
            tint,
        }
    }

    /// Pixel dimensions of one `begSprTab[id]` UI sprite, or None if the
    /// sprite is empty/absent.
    pub fn sprite_dims(&self, id: usize) -> Option<(f32, f32)> {
        let (_, _, w, h) = self.sprite_rects.get(id).copied().flatten()?;
        (w != 0 && h != 0).then_some((w as f32, h as f32))
    }

    /// Blit `begSprTab[id]` at screen pixel (x, y), opaque. For the
    /// icons/glyphs the original draws raw (DrawBitmap, no blend).
    fn sprite_quad(&self, id: usize, x: f32, y: f32, scale: f32) -> Option<UiQuad> {
        self.sprite_quad_tint(id, x, y, scale, WHITE)
    }

    /// Blit `begSprTab[id]` with an explicit tint (for the translucent
    /// panel BACKGROUNDS — the original's sub_23940 blends them over the
    /// live framebuffer, so HUD transparency is always on; we approximate
    /// with an alpha over the sky, which the UI pass already blends).
    fn sprite_quad_tint(
        &self,
        id: usize,
        x: f32,
        y: f32,
        scale: f32,
        tint: [f32; 4],
    ) -> Option<UiQuad> {
        let (sx, sy, w, h) = self.sprite_rects.get(id).copied().flatten()?;
        if w == 0 || h == 0 {
            return None;
        }
        Some(UiQuad {
            rect: snap([x, y, w as f32 * scale, h as f32 * scale]),
            uv: [sx as f32, sy as f32, w as f32, h as f32],
            tint,
        })
    }

    /// Blit `begSprTab[id]` into an explicit destination rect (for the
    /// spellbook: the slab stretches to the cell, the icon draws at a
    /// uniform-scaled centered rect so it never distorts).
    fn sprite_quad_rect_tint(&self, id: usize, rect: [f32; 4], tint: [f32; 4]) -> Option<UiQuad> {
        let (sx, sy, w, h) = self.sprite_rects.get(id).copied().flatten()?;
        if w == 0 || h == 0 {
            return None;
        }
        Some(UiQuad {
            rect: snap(rect),
            uv: [sx as f32, sy as f32, w as f32, h as f32],
            tint,
        })
    }

    /// Like [`Self::sprite_quad_rect_tint`] but MASK-DARKEN: the sprite is
    /// a coverage mask, and the shader fills it with the (translucent)
    /// tint so the destination beneath (the slab) shows through DARKENED —
    /// the dark-relief look of UNOWNED spellbook icons cut into the stone
    /// texture (the original's sub_23AE0 blend[0xA6 | dest]). A NEGATIVE
    /// uv width is the mode flag. player 2026-07-07.
    fn sprite_quad_rect_mask(&self, id: usize, rect: [f32; 4], tint: [f32; 4]) -> Option<UiQuad> {
        let (sx, sy, w, h) = self.sprite_rects.get(id).copied().flatten()?;
        if w == 0 || h == 0 {
            return None;
        }
        Some(UiQuad {
            rect: snap(rect),
            uv: [sx as f32, sy as f32, -(w as f32), h as f32],
            tint,
        })
    }
}

/// Push an optional quad (from the sprite-blit helpers) if present.
fn push_opt(quads: &mut Vec<UiQuad>, q: Option<UiQuad>) {
    if let Some(u) = q {
        quads.push(u);
    }
}

/// Snap a rect to the integer pixel grid, EDGE-consistently: left/top
/// and right/bottom round independently, so adjacent cells that share
/// an edge in native coordinates still share it after snapping (no
/// gaps, no overlaps). Without this, fractional scale factors (e.g.
/// 1.5 at 720p) rasterize identical native sources into visibly
/// different rows/columns — the jagged icons and the meter-dot rows
/// that didn't match (player 2026-07-08). The structural fix for the
/// remaining in-sprite aliasing at fractional scales is a native
/// 640×480 UI layer upscaled once with a real filter — banked.
fn snap(rect: [f32; 4]) -> [f32; 4] {
    let x0 = rect[0].round();
    let y0 = rect[1].round();
    let x1 = (rect[0] + rect[2]).round();
    let y1 = (rect[1] + rect[3]).round();
    [x0, y0, x1 - x0, y1 - y0]
}

fn solid(rect: [f32; 4], tint: [f32; 4]) -> UiQuad {
    UiQuad {
        rect: snap(rect),
        uv: [0.0; 4],
        tint,
    }
}

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Unowned spells: icon ghosted way down (the original greys via the
/// blend table's dim row; a tint is our stand-in).
/// Unowned spell icons: the icon's outer SHAPE used as a mask, filled
/// with a dark TRANSLUCENT ink so the stone-slab texture shows through,
/// DARKENED — a dark relief cut into the tile (player 2026-07-07: "a
/// silhouette … it follows the outer shape of the sprite, but the
/// texture of the tile exactly"). The original's sub_23AE0 writes
/// blend[0xA6 | dest]; rgb = the dark ink, a = darkening strength.
const UNOWNED_MASK: [f32; 4] = [0.05, 0.04, 0.03, 0.74];
/// The book slab tint. Our raw [3] sprite is a cool blue-grey
/// (~158,165,198); retail's slab reads a WARM DARK BROWN (the original
/// blends [3] through the LUT over the book background, warming +
/// darkening it). A neutral darkening kept it blue-grey, so this tint
/// warms toward brown (boosts red-relative, cuts blue) AND darkens.
/// player 2026-07-07 side-by-side.
const SLAB_DIM: [f32; 4] = [0.58, 0.46, 0.32, 1.0];
/// Quick-select digit ink: the original blends the glyph toward
/// `byte_AD167_AD157[1]` (black); a black multiplicative tint blackens
/// the sprite's yellow ink while keeping its coverage/alpha.
const DIGIT_INK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

// The spellbook grid (remc1 :26915-70), native 640×480 scaled by w/640,
// h/480. 24 spells iterate in DISPLAY_ORDER packed with NO gaps: cell =
// the slot-slab sprite [3] = 64×37, 4 cols × 6 rows from (384,194). The
// origin was measured from the player's hi-res retail screenshot
// (2026-07-07) and lives in mgc-render (which places the world
// viewport against the same edges) — consumed here so the two crates
// cannot drift; the grid bottom (194 + 6·37 = 416) is the map-pane
// base and the black-bar top.
const BOOK_GRID_X: f32 = mgc_render::BOOK_SPELL_X;
const BOOK_GRID_Y: f32 = mgc_render::BOOK_SPELL_Y;
const BOOK_CELL_W: f32 = 64.0;
const BOOK_CELL_H: f32 = 37.0;
const BOOK_GRID_COLS: usize = 4;
/// Quick-select digit glyphs: `[30 + slot]` (slot 0 = "1" … slot 9 =
/// "0"), 10×14 badges — the number retail stamps in a hotkeyed spell's
/// book cell (sub_24230 :27857).
const SPR_QUICK_DIGIT: usize = 30;
/// The slot slab's ACTIVE variant — retail swaps [3]→[4] while the
/// spell's burst counter runs (sub_24230 :27810).
const SPR_SLOT_BG_ACTIVE: usize = 4;

/// One grid cell's slab rect in screen pixels (the icon is pre-composited
/// onto the 64×37 slot slab). `k` = display index 0..24.
fn book_cell(w: f32, h: f32, k: usize) -> [f32; 4] {
    let sx = w / 640.0;
    let sy = h / 480.0;
    let col = (k % BOOK_GRID_COLS) as f32;
    let row = (k / BOOK_GRID_COLS) as f32;
    [
        (BOOK_GRID_X + col * BOOK_CELL_W) * sx,
        (BOOK_GRID_Y + row * BOOK_CELL_H) * sy,
        BOOK_CELL_W * sx,
        BOOK_CELL_H * sy,
    ]
}

/// Book screen quads + the display slot under the cursor (if any).
pub fn book_quads(
    assets: &UiAssets,
    loadout: &LoadoutView,
    quick_binds: &[Option<u8>; 10],
    w: f32,
    h: f32,
    cursor: (f32, f32),
) -> (Vec<UiQuad>, Option<SpellId>) {
    let mut quads = Vec::with_capacity(SPELL_COUNT * 3 + 2);
    let mut hovered = None;
    for (k, &spell) in DISPLAY_ORDER.iter().enumerate() {
        let spell_id = SpellId(spell);
        let cell = book_cell(w, h, k);
        // Two spellbook states, per the actual draw split (remc1
        // :26932/:26972):
        //   OWNED   → sub_24230: slab + the icon drawn in FULL color
        //             (DrawBitmap, raw). Affordability is shown by the
        //             separate diagonal-line marks (sub_247C0), NOT by
        //             dimming the icon.
        //   NOT owned → sub_23CF0: slab + the icon as a coverage mask
        //             DIM-TINTED toward color 0xA6 (sub_23AE0) — the full
        //             icon SHAPE stays visible, just darkened (player
        //             2026-07-07: "unowned drawn in full, not silhouettes").
        // The slab itself is drawn via sub_23940 = a BLEND over the book's
        // black background, so it reads DARKER than the raw sprite (player:
        // "background should be the darker sprite"). We approximate the
        // blend-over-black with a darkening tint.
        let owned = loadout.owned[spell as usize];
        // The :26926 BIND gate: the spell's castle_req (+132, the
        // castle-stored unlock ladder) vs the castle's stored mana —
        // NOT a player-mana affordability test. Computed sim-side.
        let bindable = owned && loadout.bindable[spell as usize];
        let over = cursor.0 >= cell[0]
            && cursor.0 < cell[0] + cell[2]
            && cursor.1 >= cell[1]
            && cursor.1 < cell[1] + cell[3];
        // Every OWNED hovered spell becomes the bind target — the
        // castle-req gate does NOT block assignment (player retail
        // memory, 2026-07-09: quickselect keys were campaign state
        // routinely bound to not-yet-castable spells; the equip
        // command :48717-31 checks ownership only). The :26926
        // castle gate stays purely visual (the LOCKED wash + the
        // equipped-panel wash) and the CAST keeps fizzling sim-side
        // until the castle stores enough.
        if over && owned {
            hovered = Some(spell_id);
        }

        // The stone slab fills the cell, drawn DARKER (the original's
        // sub_23940 blends it over the black book background) —
        // stretching the slab texture is invisible. While the spell's
        // burst counter runs, retail swaps the slab to the ACTIVE
        // variant [4] (sub_24230 :27810; no cooldown veil exists —
        // ours was an invention).
        let slab = if owned && loadout.cooldown[spell as usize] > 0.0 {
            SPR_SLOT_BG_ACTIVE
        } else {
            SPR_SLOT_BG as usize
        };
        push_opt(
            &mut quads,
            assets
                .sprite_quad_rect_tint(slab, cell, SLAB_DIM)
                .or_else(|| assets.sprite_quad_rect_tint(SPR_SLOT_BG as usize, cell, SLAB_DIM)),
        );
        // The ICON at native 62×34 × the UNIFORM art scale, anchored at
        // the CELL ORIGIN — retail's sub_24230 draws it with
        // `DrawBitmap(a1, a2, icon)`: top-left at the cell corner, so
        // the 62×34 art leaves 2px right + 3px bottom slack (player
        // 2026-07-08: centering + fit-to-cell sat it too low and made
        // e.g. the castle icon touch the cell bottom). Uniform scale =
        // min(w/640, h/480) so the art never stretches at non-4:3.
        // OWNED = full colour (raw). NOT owned = the icon's SHAPE cut
        // into the slab as a dark relief (the original's
        // blend[0xA6|dest], sub_23AE0).
        let su = (w / 640.0).min(h / 480.0);
        let icon_id = SPR_SPELL_ICON + spell as usize;
        if let Some((iw, ih)) = assets.sprite_dims(icon_id) {
            let irect = [cell[0], cell[1], iw * su, ih * su];
            push_opt(
                &mut quads,
                if owned {
                    assets.sprite_quad_rect_tint(icon_id, irect, WHITE)
                } else {
                    assets.sprite_quad_rect_mask(icon_id, irect, UNOWNED_MASK)
                },
            );
        }
        if owned {
            // Quick-select number badge (sub_24230 :27857): a spell
            // bound to a number key shows its digit glyph [30+slot]
            // at the CELL ORIGIN (retail `sub_23AE0(a1, a2, ...)` —
            // the glyph's own margins do the placement), blended
            // toward `byte_AD167[1]` = BLACK ink (a coverage-mask
            // blend). Retail actually gates this on a per-spell
            // countdown (+844, decremented per draw — the badge
            // FLASHES after assignment) or a book-wide flag (+14421);
            // we keep it always-on in the book as the readable
            // interpretation.
            if let Some(slot) = quick_binds.iter().position(|&b| b == Some(spell)) {
                push_opt(
                    &mut quads,
                    assets.sprite_quad_tint(
                        SPR_QUICK_DIGIT + slot,
                        cell[0],
                        cell[1],
                        su,
                        DIGIT_INK,
                    ),
                );
            }
            // LOCKED overlay (sub_24230 :27860): when castle_req
            // exceeds the castle's stored mana (or no castle stands),
            // retail remaps the WHOLE cell through fog row 0x30
            // (sub_247C0) — a uniform wash over slab + icon + badge.
            // This is the visual for the unlock ladder ("owned but
            // can't select" is faithful; the wash tells you why).
            if !bindable {
                quads.push(solid(cell, LOCKED_WASH));
            }
        }
        if over && !owned {
            // Hover ring (sub_24DA0/sub_24D20, ink byte_AE167): retail
            // rings EVERY hovered cell — unowned included — only the
            // bind-candidate recording is gated (on ownership). A
            // hovered OWNED cell gets the panel redraw below instead
            // of a ring. (Ring colour = a text-table ink; hand-tuned
            // until the LUT bake.)
            let f = cell;
            let t = [0.9, 0.85, 0.5, 0.9];
            quads.push(solid([f[0], f[1], f[2], 2.0], t));
            quads.push(solid([f[0], f[1] + f[3] - 2.0, f[2], 2.0], t));
            quads.push(solid([f[0], f[1], 2.0, f[3]], t));
            quads.push(solid([f[0] + f[2] - 2.0, f[1], 2.0, f[3]], t));
        }
    }
    // The hovered OWNED cell is redrawn as a full equipped-spell
    // panel at the cell origin — retail calls `sub_23D40(x, y, spell,
    // 1)` AFTER the grid loop (a4=1 = raw opaque DrawBitmap frame, not
    // the translucent sub_23940 blend), overdrawing its neighbours
    // with the 64×44 frame. Frame [1]/[2] by the burst counter, icon
    // raw, availability meter at (+4,+36).
    if let Some(spell_id) = hovered {
        let sp = spell_id.0;
        let k = DISPLAY_ORDER.iter().position(|&d| d == sp).unwrap_or(0);
        let cell = book_cell(w, h, k);
        let (sx, sy) = (w / 640.0, h / 480.0);
        let frame = if loadout.cooldown[sp as usize] > 0.0 {
            SPR_SLOT_HELD
        } else {
            SPR_SLOT_IDLE
        };
        if let Some((fw, fh)) = assets.sprite_dims(frame) {
            push_opt(
                &mut quads,
                assets.sprite_quad_rect_tint(frame, [cell[0], cell[1], fw * sx, fh * sy], WHITE),
            );
        }
        let su = sx.min(sy);
        let icon_id = SPR_SPELL_ICON + sp as usize;
        if let Some((iw, ih)) = assets.sprite_dims(icon_id) {
            // Retail draws the icon at the frame origin (DrawBitmap
            // (a1, a2)), uniform art scale.
            push_opt(
                &mut quads,
                assets.sprite_quad_rect_tint(icon_id, [cell[0], cell[1], iw * su, ih * su], WHITE),
            );
        }
        // Availability meter (sub_23D40 :27703-34): partial-cast
        // progress bar + one shaded dot per whole affordable cast.
        let cost = SPELLS[sp as usize].possess_mana.max(1);
        let mana = loadout.mana;
        let (mx, my) = (cell[0] + 4.0 * sx, cell[1] + 36.0 * sy);
        let partial = (56.0 * (mana % cost) as f32 / cost as f32).floor();
        quads.push(solid([mx, my, partial * sx, 4.0 * sy], METER_GREY));
        meter_dots(&mut quads, mx, my, sx, sy, (mana / cost).min(54) as usize);
        // Retail's sub_23D40 re-stamps the quickselect digit inside
        // the redraw (:27749-67) — without it the badge vanishes
        // exactly while hovering the cell you're assigning (player
        // 2026-07-08).
        if let Some(slot) = quick_binds.iter().position(|&b| b == Some(sp)) {
            push_opt(
                &mut quads,
                assets.sprite_quad_tint(SPR_QUICK_DIGIT + slot, cell[0], cell[1], su, DIGIT_INK),
            );
        }
    }
    // The whole screen bottom (below the map + spellbook) is simply
    // BLACK and empty in retail — the multiplayer message log draws
    // there (via the DrawText path, not built yet), but with no panel
    // fill or tint. The renderer's black clear shows through; nothing to
    // draw here.
    (quads, hovered)
}

// Panel sprite ids (remc1 begSprTab; ROADMAP "HUD parity"). The panel
// strip is laid out at the original's 640-wide coordinates, scaled to
// the live resolution.
const SPR_SLOT_IDLE: usize = 1; // equipped-spell frame, idle
const SPR_SLOT_HELD: usize = 2; // equipped-spell frame, active/held
const SPR_PANEL_BG: usize = 40; // wizard-strip left cap
const SPR_WIZ_BG: usize = 41; // a wizard sub-panel background
const SPR_DIVIDER: usize = 42; // between the level digit and the bars
const SPR_CASTLE_LVL: usize = 43; // +level 0..7 = the castle-level glyph
const SPR_BALLOON_GLYPH: usize = 50; // +count 1..3 = the balloon-roster glyph
const SPR_WIZ_EMPTY: usize = 54; // no-wizard slot
const SPR_WIZ_ALERT: usize = 55; // castle-under-attack flash
const SPR_SPELL_ICON: usize = 6; // spell icon base: [spell + 6]
/// HUD panel background translucency — the original's panels blend over
/// the framebuffer (transparency is ALWAYS on, not a toggle; player
/// 2026-07-07). We approximate with an alpha over the sky; the icons/
/// glyphs/bars stay opaque (drawn raw in retail).
const PANEL_TINT: [f32; 4] = [1.0, 1.0, 1.0, mgc_render::HUD_PANEL_ALPHA];
/// Life-bar color (remc1 uses palette index 0x7B, a team red).
const LIFE_RED: [f32; 4] = [0.85, 0.15, 0.12, 1.0];
/// Collected/banked mana bar (sub_22E50 :27377, color v29 =
/// byte_99B58[2*owner]) — WHITE, not blue (player 2026-07-07).
const MANA_WHITE: [f32; 4] = [0.95, 0.95, 0.95, 1.0];
/// Spell availability progress bar (sub_23D40 :27705, color v26 =
/// byte_99B58[1+2*owner]) — GREY, not blue (player 2026-07-07); the
/// partial mana toward the next cast, under the equipped-spell icon.
const METER_GREY: [f32; 4] = [0.55, 0.55, 0.55, 1.0];
/// Locked-spell overlay (sub_24230 :27860 + sub_23D40 :27767): when a
/// spell's castle_req (+132) exceeds the linked castle's STORED mana
/// (+140) — or no castle stands — retail remaps the whole cell/panel
/// rect through fog row 0x30 (sub_247C0), a uniform wash over
/// everything beneath. DARKENS, per the player's retail book
/// screenshot (2026-07-08) — which also means the fog rows run
/// dark-high (flagging the map marker cross's white fade for a
/// polarity re-check); exact shade lands with the LUT bake.
const LOCKED_WASH: [f32; 4] = [0.0, 0.0, 0.0, 0.5];
/// Bar geometry (sub_22810 draws a 64-wide fill; sub_22E50 offsets).
const BAR_W: f32 = 64.0;
const BAR_X: f32 = 58.0; // bars start +58 from the sub-panel origin
/// One HUD section = 640/5 = 128 native px (the 5×20% top strip).
const HUD_SECTION: f32 = 128.0;

/// A solid bar fill at panel-space (x,y) scaled to screen — the
/// original's `sub_22810(x,y,64,h,(val<<6)/max,color)`: `fill` is the
/// value/max fraction of the 64-px ruler. Faithful sub_22810 (:26991)
/// draws ONLY the clamped colored fill, straight on the panel marble —
/// no background track — and skips fills under 2px. (The track we used
/// to draw also covered the amber capacity fill wherever a white
/// stored-mana bar overlaid it.)
fn bar(quads: &mut Vec<UiQuad>, s: f32, x: f32, y: f32, h: f32, frac: f32, color: [f32; 4]) {
    let fill = (BAR_W * frac.clamp(0.0, 1.0)).max(0.0);
    if fill >= 2.0 {
        quads.push(solid([x * s, y * s, fill * s, h * s], color));
    }
}

/// A thin (2-px) balloon bar with no dark track — the original stacks
/// these per balloon (sub_22E50 :27338-39, `sub_22810(x, y, 64, 2,
/// frac, color)`). Just the colored fill; the panel marble shows
/// between them.
fn thin_bar(quads: &mut Vec<UiQuad>, s: f32, x: f32, y: f32, frac: f32, color: [f32; 4]) {
    let fill = (BAR_W * frac.clamp(0.0, 1.0)).max(0.0);
    if fill >= 1.0 {
        quads.push(solid([x * s, y * s, fill * s, 2.0 * s], color));
    }
}

/// The availability dots (sub_23D40 :27713-33): one dot per whole
/// affordable cast, filled column-major (2 rows at +0/+2, 27 columns
/// at +4,+6,…). Each dot is EXACTLY ONE native pixel — both screen
/// writers (sub_615D4 hi-res 640w, sub_61594 lo-res 320w) plot a
/// single byte; the "shaded 2×2" look in DOSBox captures is its
/// upscaler smearing that pixel across the 2-px spacing grid
/// (decompile-verified vs player screenshot, 2026-07-08). `mx/my` in
/// screen px; `sx/sy` = native→screen scale (snap() keeps every dot
/// rasterizing alike).
fn meter_dots(quads: &mut Vec<UiQuad>, mx: f32, my: f32, sx: f32, sy: f32, casts: usize) {
    for d in 0..casts {
        let (col, row) = ((d / 2) as f32, (d % 2) as f32);
        let (x, y) = (mx + col * 2.0 * sx, my + row * 2.0 * sy);
        quads.push(solid([x, y, sx.max(1.0), sy.max(1.0)], MANA_WHITE));
    }
}

/// In-game HUD — the faithful top strip (remc1 sub_22E50 wizard panel +
/// sub_23D40 equipped-spell panels), laid out at the original's 640-wide
/// coordinates scaled by `w/640`. The rotating round minimap is drawn by
/// the renderer; here we place the wizard stat panel (left) and the two
/// equipped-spell panels (right, x=510/574).
pub fn hud_quads(
    assets: &UiAssets,
    loadout: &LoadoutView,
    vitals: &PlayerVitals,
    transparent: bool,
    alert_blink: bool,
    w: f32,
    _h: f32,
) -> Vec<UiQuad> {
    let mut quads = Vec::new();
    let s = w / 640.0;
    // Panel-background tint: translucent (faithful MC1, always-on
    // transparency) or opaque (the MC2 readability toggle).
    let panel_tint = if transparent { PANEL_TINT } else { WHITE };

    // --- Wizard stat strip (sub_22E50): three 128-wide sub-panels. ---
    // Tiles pack from x=2: [40] radar frame (124w), then sub-panels at
    // v22 = 2 + [40].w, then +128 each. The three panels are, in order
    // (player retail ground truth + the trace :27214/:27334/:27374):
    //   A (v22, `var_50`)  = the player's LINKED CASTLE — castle HP +
    //                        castle mana capacity/banked, level glyph.
    //   B (v23, `var_52[]`)= the player's MANA BALLOONS — 1..3 by castle
    //                        level, each a thin stacked HP + cargo bar.
    //   C (v24, `a1x`)     = the player's OWN wizard — self life + mana
    //                        capacity/banked, drawn UNCONDITIONALLY.
    let cap_w = assets.sprite_dims(SPR_PANEL_BG).map_or(124.0, |(w, _)| w);
    push_opt(
        &mut quads,
        assets.sprite_quad_tint(SPR_PANEL_BG, 2.0 * s, 2.0 * s, s, panel_tint),
    );
    let v22 = 2.0 + cap_w; // slot A = castle panel
    let v23 = v22 + HUD_SECTION; // slot B = balloons
    let v24 = v22 + 2.0 * HUD_SECTION; // slot C = self

    let world = loadout.world_mana.max(1) as f32;
    // The level-goal marks (:27267-74 slot A / :27380-87 slot C): TWO
    // 2×2 ticks at y=26 and y=38 bracketing the mana ruler at win_pct%
    // of its 64px width (`v20 + (pct<<6)/100`), colour alternating
    // between the two team-ramp entries per blink frame (v28 =
    // byte_99B58[2·owner + phase]; white/grey stand-ins until the LUT
    // bake). The green completed state is our labeled helper — retail
    // has no completion recolour here.
    let win_tick = |quads: &mut Vec<UiQuad>, ox: f32| {
        if loadout.win_pct > 0 {
            let tx = (ox + BAR_X) * s + BAR_W * s * (loadout.win_pct as f32 / 100.0).min(1.0);
            let tc = if loadout.completed {
                [0.3, 0.95, 0.3, 1.0]
            } else if alert_blink {
                MANA_WHITE
            } else {
                METER_GREY
            };
            for y in [26.0, 38.0] {
                quads.push(solid([tx, y * s, 2.0 * s, 2.0 * s], tc));
            }
        }
    };

    // === Slot A: the linked castle (:27215). Gated on the castle
    // existing AND level > 0 (else the bare marble [54]). player. ===
    // Alert marbles: retail flickers [55] on alternate blink frames
    // while the per-source hit counter runs (u8_391 castle / u8_393
    // balloons / u8_392 self, each decremented per flash) — the
    // `alert_blink` gate approximates that frame flicker.
    let castle = loadout.castle.filter(|(_, _, lvl)| *lvl > 0);
    let slot_a_bg = if castle.is_none() {
        SPR_WIZ_EMPTY
    } else if vitals.castle_alert && alert_blink {
        SPR_WIZ_ALERT
    } else {
        SPR_WIZ_BG
    };
    push_opt(
        &mut quads,
        assets.sprite_quad_tint(slot_a_bg, v22 * s, 2.0 * s, s, panel_tint),
    );
    if let Some((_stored, capacity, level)) = castle {
        let ox = v22;
        // Castle-level glyph [43+level] (emblem/heart/orb/digit baked
        // in) then the divider [42].
        push_opt(
            &mut quads,
            assets.sprite_quad(SPR_CASTLE_LVL + level as usize, (ox + 2.0) * s, 2.0 * s, s),
        );
        push_opt(
            &mut quads,
            assets.sprite_quad(SPR_DIVIDER, (ox + 38.0) * s, 2.0 * s, s),
        );
        // Life bar (+58, y=10) = the CASTLE's HP (v4x->actLife/maxLife,
        // palette 0x7B) — NOT the player's life (:27237). castle_hp is
        // the downgrade meter.
        let hp = loadout
            .castle_hp
            .map_or(1.0, |(cur, max)| cur.max(0) as f32 / max.max(1) as f32);
        bar(&mut quads, s, ox + BAR_X, 10.0, 10.0, hp, LIFE_RED);
        // Mana capacity + banked, world-relative (y=28), overlaid
        // (:27240-66 verbatim): capacity (castle +136) in v27 =
        // byte_99B58[1+2·team] — the GREY family, same index as the
        // spell meter (player-certified; our amber was an invention)
        // — then the BANKED total (houses u32_308 + castle stored
        // +140 = loadout.banked; adding `stored` again was the
        // double-count that pinned the bar full). banked == capacity
        // blinks the single full bar between the pair (:27242-53).
        if loadout.banked >= capacity && capacity > 0 {
            let c = if alert_blink { METER_GREY } else { MANA_WHITE };
            bar(
                &mut quads,
                s,
                ox + BAR_X,
                28.0,
                10.0,
                capacity as f32 / world,
                c,
            );
        } else {
            bar(
                &mut quads,
                s,
                ox + BAR_X,
                28.0,
                10.0,
                capacity as f32 / world,
                METER_GREY,
            );
            bar(
                &mut quads,
                s,
                ox + BAR_X,
                28.0,
                10.0,
                loadout.banked as f32 / world,
                MANA_WHITE,
            );
        }
        win_tick(&mut quads, ox);
    }

    // === Slot B: the mana balloons (:27278-344). The marble [54]
    // ONLY when no castle stands (:27281); otherwise the glyph is
    // [50+roster] where the roster WIDTH comes from castle level —
    // it does NOT shrink when balloons die (dead slots simply draw no
    // bars, :27335-40). Thin HP + cargo bars per live balloon. ===
    let balloons = &loadout.balloons;
    let slot_b_bg = if balloons.is_empty() {
        SPR_WIZ_EMPTY
    } else if vitals.balloon_alert && alert_blink {
        SPR_WIZ_ALERT
    } else {
        SPR_WIZ_BG
    };
    push_opt(
        &mut quads,
        assets.sprite_quad_tint(slot_b_bg, v23 * s, 2.0 * s, s, panel_tint),
    );
    if !balloons.is_empty() {
        let ox = v23;
        let roster = balloons.len().min(3);
        push_opt(
            &mut quads,
            assets.sprite_quad(SPR_BALLOON_GLYPH + roster, (ox + 2.0) * s, 2.0 * s, s),
        );
        push_opt(
            &mut quads,
            assets.sprite_quad(SPR_DIVIDER, (ox + 38.0) * s, 2.0 * s, s),
        );
        // Per LIVE balloon: HP bar at y=12+2i (red), cargo bar at
        // y=30+2i (banked-mana white) — the thin stacked lines
        // (:27338-39); dead/unspawned roster slots stay bar-less.
        for (i, slot) in balloons.iter().enumerate().take(3) {
            let Some((hp, cargo)) = *slot else { continue };
            let y = 2.0 * i as f32;
            thin_bar(&mut quads, s, ox + BAR_X, 12.0 + y, hp, LIFE_RED);
            thin_bar(&mut quads, s, ox + BAR_X, 30.0 + y, cargo, MANA_WHITE);
        }
    }

    // === Slot C: the player's OWN wizard (:27346-388). Always drawn
    // (no gate) — the wizard is always present. The alert marble here
    // is the PLAYER-hit flash (u8_392, :27347), independent of the
    // castle's u8_391. ===
    let slot_c_bg = if vitals.player_alert && alert_blink {
        SPR_WIZ_ALERT
    } else {
        SPR_WIZ_BG
    };
    push_opt(
        &mut quads,
        assets.sprite_quad_tint(slot_c_bg, v24 * s, 2.0 * s, s, panel_tint),
    );
    {
        let ox = v24;
        // Base wizard glyph [43] + divider [42] (:27358-72; the alert
        // /grace variant swaps a blended copy — we keep the plain draw).
        push_opt(
            &mut quads,
            assets.sprite_quad(SPR_CASTLE_LVL, (ox + 2.0) * s, 2.0 * s, s),
        );
        push_opt(
            &mut quads,
            assets.sprite_quad(SPR_DIVIDER, (ox + 38.0) * s, 2.0 * s, s),
        );
        // Self life bar (+58, y=10) = the PLAYER's health (a1x->actLife,
        // 0x7B red) — this is where player life belongs (:27375).
        bar(
            &mut quads,
            s,
            ox + BAR_X,
            10.0,
            10.0,
            vitals.life as f32 / vitals.life_max.max(1) as f32,
            LIFE_RED,
        );
        // Self mana: capacity (var_136 = mana_max, the v27 grey) +
        // current (var_140 = mana, white) over the world total
        // (:27376-77).
        bar(
            &mut quads,
            s,
            ox + BAR_X,
            28.0,
            10.0,
            loadout.mana_max as f32 / world,
            METER_GREY,
        );
        bar(
            &mut quads,
            s,
            ox + BAR_X,
            28.0,
            10.0,
            loadout.mana as f32 / world,
            MANA_WHITE,
        );
        win_tick(&mut quads, ox);
    }
    // --- Equipped-spell panels (sub_23D40) at x=510 and x=574. ---
    // Frame [1]/[2] (64x44), then the icon [spell+6] at its NATIVE 62x34
    // (top-aligned, NOT stretched to the frame), then the availability
    // meter at y=+36: a progress bar (partial mana toward the next cast)
    // plus a row of dots (whole casts currently affordable) — sub_23D40
    // :27700-34.
    for (spell, px) in [(loadout.left, 510.0), (loadout.right, 574.0)] {
        // Frame [2] = the CAST-IN-PROGRESS highlight (sub_23D40 :27675:
        // `a3x->var_48` = the burst counter, nonzero only while firing/
        // channeling), else the idle frame [1]. Equipped ≠ casting — the
        // highlight flashes on projectile casts and stays lit for
        // duration effects (speed etc.), driven by the burst counter.
        let active = spell.is_some_and(|sp| loadout.cooldown[sp as usize] > 0.0);
        let frame = if active { SPR_SLOT_HELD } else { SPR_SLOT_IDLE };
        push_opt(
            &mut quads,
            assets.sprite_quad_tint(frame, px * s, 2.0 * s, s, panel_tint),
        );
        if let Some(sp) = spell {
            // Icon drawn raw at the FRAME ORIGIN (sub_23D40's
            // `DrawBitmap(a1, a2, icon)` — the art's own margins do
            // the placement), native size × the HUD scale.
            push_opt(
                &mut quads,
                assets.sprite_quad(SPR_SPELL_ICON + sp as usize, px * s, 2.0 * s, s),
            );
            // Availability meter at (frame+4, frame+36) — sub_23D40
            // :27703-34: the grey partial-cast progress bar, then one
            // 2×2 SHADED dot per whole affordable cast over it.
            let cost = SPELLS[sp as usize].possess_mana.max(1);
            let mana = loadout.mana;
            let mx = (px + 4.0) * s; // sub_23D40 a1+4
            let my = (2.0 + 36.0) * s; // a2+36
            let partial = (56.0 * (mana % cost) as f32 / cost as f32).floor();
            quads.push(solid([mx, my, partial * s, 4.0 * s], METER_GREY));
            meter_dots(&mut quads, mx, my, s, s, (mana / cost).min(54) as usize);
            // Locked equipped spell (sub_23D40 :27767): the same fog
            // wash as the book cell covers the whole panel while the
            // castle_req isn't met — the equipped hand tells you it
            // can't fire.
            if !loadout.bindable[sp as usize] {
                let (fw, fh) = assets.sprite_dims(frame).unwrap_or((64.0, 44.0));
                quads.push(solid([px * s, 2.0 * s, fw * s, fh * s], LOCKED_WASH));
            }
        }
    }
    quads
}

/// Pause indicator. Retail draws the text "PAUSED" at native (132,50)
/// (banked with the DrawText path — ROADMAP Tier 3); until the font
/// lands, a ‖ pause glyph at the same spot marks the frozen sim so a
/// still screen doesn't read as a hang.
pub fn pause_quads(w: f32, _h: f32) -> Vec<UiQuad> {
    let s = (w / 640.0).max(1.0);
    let (x, y) = (132.0 * s, 50.0 * s);
    let ink = [0.95, 0.95, 0.95, 0.95];
    vec![
        solid([x, y, 4.0 * s, 14.0 * s], ink),
        solid([x + 8.0 * s, y, 4.0 * s, 14.0 * s], ink),
    ]
}

/// Mortality overlays + the life bar (functional-first placement;
/// the faithful HUD layout is the banked UI/UX track). `blink`
/// drives the dead-screen respawn prompt.
pub fn vitals_quads(v: &PlayerVitals, w: f32, h: f32, blink: bool) -> Vec<UiQuad> {
    let mut quads = Vec::new();
    let scale = (w / 640.0).max(1.0);
    // (The old bottom-center life bar is GONE — player health lives in
    // the wizard strip's slot C now, where retail draws it; the bar
    // was redundant, player 2026-07-08.)
    let bw = w * 0.25;
    let y = h - 26.0 * scale;
    // Spawn-grace shimmer: a thin white strip draining bottom-center
    // (no faithful equivalent — retail shows nothing for grace; kept
    // as the readable invulnerability cue).
    if v.grace > 0 && v.state == LifeState::Alive {
        quads.push(solid(
            [
                (w - bw) / 2.0,
                y - 3.0 * scale,
                bw * (v.grace as f32 / 100.0).min(1.0),
                2.0 * scale,
            ],
            [1.0, 1.0, 1.0, 0.8],
        ));
    }
    // Castle under attack: the faithful cue is the wizard strip's
    // alert marble [55] (hud_quads slot A) — the amber strip that used
    // to flash here anchored to the old bottom-right castle panel,
    // which moved to the top strip.
    // The red hit flash (sub_44BE0(2) — palette row 2 in retail).
    if v.hit_flash > 0 && v.state == LifeState::Alive {
        let a = 0.08 * v.hit_flash as f32;
        quads.push(solid([0.0, 0.0, w, h], [0.8, 0.05, 0.05, a]));
    }
    match v.state {
        // The death fall: a deepening red-out.
        LifeState::Falling => {
            quads.push(solid([0.0, 0.0, w, h], [0.45, 0.03, 0.03, 0.35]));
        }
        // Dead: the grey screen (palette row 7) + a blinking center
        // strip as the Space prompt (no text renderer yet).
        LifeState::Dead => {
            quads.push(solid([0.0, 0.0, w, h], [0.22, 0.22, 0.25, 0.55]));
            if blink {
                let pw = w * 0.30;
                quads.push(solid(
                    [(w - pw) / 2.0, h * 0.62, pw, 4.0 * scale],
                    [0.95, 0.95, 0.95, 0.9],
                ));
            }
        }
        LifeState::Alive => {}
    }
    quads
}

/// The autoaim crosshair instrument (`enhancements.crosshair` / C —
/// P-class, playtest predictor, not a combat aid): a black,
/// white-edged cross at the TRUE aim point (the faithful camera
/// pitches at HALF the aim pitch, so the aim is never screen
/// center), plus per-hand lock markers on the target each hand's
/// equipped spell would acquire this instant (`World::aim_preview`):
/// left hand = an upright `+`, right hand = a diagonal `×`, cores
/// blinking gently red while locked (both shapes compose when the
/// hands lock the same target). Acquisition ≠ hit — homing yaw is
/// authentically capped at 5/tick, so the marker shows what the shot
/// will CHASE, not what it will catch.
pub fn crosshair_quads(
    quads: &mut Vec<UiQuad>,
    w: f32,
    neutral: Option<(f32, f32)>,
    locks: [Option<(f32, f32)>; 2],
    blink: f32,
) {
    let s = w / 640.0;
    let red = [0.30 + 0.70 * blink.clamp(0.0, 1.0), 0.02, 0.02, 1.0];
    if let Some((cx, cy)) = neutral {
        plus_glyph(quads, cx, cy, s, [0.0, 0.0, 0.0, 1.0]);
    }
    if let Some((cx, cy)) = locks[0] {
        plus_glyph(quads, cx, cy, s, red);
    }
    if let Some((cx, cy)) = locks[1] {
        diag_glyph(quads, cx, cy, s, red);
    }
}

/// White edge under a colored core, both crosshair glyph shapes.
const GLYPH_EDGE: [f32; 4] = [1.0, 1.0, 1.0, 0.85];

/// Upright `+`: 16-native-px arms, white-edged.
fn plus_glyph(quads: &mut Vec<UiQuad>, cx: f32, cy: f32, s: f32, core: [f32; 4]) {
    quads.push(solid(
        [cx - 8.0 * s, cy - 2.0 * s, 16.0 * s, 4.0 * s],
        GLYPH_EDGE,
    ));
    quads.push(solid(
        [cx - 2.0 * s, cy - 8.0 * s, 4.0 * s, 16.0 * s],
        GLYPH_EDGE,
    ));
    quads.push(solid([cx - 7.0 * s, cy - 1.0 * s, 14.0 * s, 2.0 * s], core));
    quads.push(solid([cx - 1.0 * s, cy - 7.0 * s, 2.0 * s, 14.0 * s], core));
}

/// Diagonal `×`: chunky pixel diagonals (axis-aligned quads only),
/// white-edged; edges first so no core is covered by a neighbor.
fn diag_glyph(quads: &mut Vec<UiQuad>, cx: f32, cy: f32, s: f32, core: [f32; 4]) {
    const ARM: [f32; 7] = [-6.0, -4.0, -2.0, 0.0, 2.0, 4.0, 6.0];
    for i in ARM {
        quads.push(solid(
            [cx + (i - 2.0) * s, cy + (i - 2.0) * s, 4.0 * s, 4.0 * s],
            GLYPH_EDGE,
        ));
        quads.push(solid(
            [cx + (i - 2.0) * s, cy - (i + 2.0) * s, 4.0 * s, 4.0 * s],
            GLYPH_EDGE,
        ));
    }
    for i in ARM {
        quads.push(solid(
            [cx + (i - 1.0) * s, cy + (i - 1.0) * s, 2.0 * s, 2.0 * s],
            core,
        ));
        quads.push(solid(
            [cx + (i - 1.0) * s, cy - (i + 1.0) * s, 2.0 * s, 2.0 * s],
            core,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellbook_grid_is_tightly_packed_at_native_coords() {
        // At native 640×480 the 24 cells sit at (384,194)+(col·64,row·37),
        // 4 cols × 6 rows, with NO gaps — the faithful spellbook packing
        // (measured from the player's hi-res retail shot). Anchors + step.
        let (w, h) = (640.0, 480.0);
        // First cell at the grid origin.
        assert_eq!(book_cell(w, h, 0), [384.0, 194.0, 64.0, 37.0]);
        // End of row 0 (col 3): x = 384 + 3·64 = 576, right edge = 640.
        let c3 = book_cell(w, h, 3);
        assert_eq!(c3, [576.0, 194.0, 64.0, 37.0]);
        assert_eq!(c3[0] + c3[2], 640.0, "row fills to the screen edge");
        // Wraps to the next row at col 0 (k=4): x back to 384, y += 37.
        assert_eq!(book_cell(w, h, 4), [384.0, 231.0, 64.0, 37.0]);
        // Last cell (k=23 = col 3, row 5): bottom edge = 194 + 6·37 = 416.
        let last = book_cell(w, h, 23);
        assert_eq!(last, [576.0, 379.0, 64.0, 37.0]);
        assert_eq!(last[1] + last[3], 416.0, "grid bottom = spellbook base");
        // Tightly packed: adjacent cells share an edge (no gap).
        let a = book_cell(w, h, 0);
        let b = book_cell(w, h, 1);
        assert_eq!(a[0] + a[2], b[0], "columns are gapless");
    }

    #[test]
    fn spellbook_grid_scales_with_resolution() {
        // Cells scale by w/640, h/480 so the layout is resolution-parametric.
        let cell = book_cell(1280.0, 960.0, 0);
        assert_eq!(cell, [768.0, 388.0, 128.0, 74.0], "2× native");
    }
}
