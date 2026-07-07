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
use mgc_sim::spells::{SpellId, DISPLAY_ORDER, SPELLS, SPELL_COUNT};
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
    fn sprite_quad_tint(&self, id: usize, x: f32, y: f32, scale: f32, tint: [f32; 4]) -> Option<UiQuad> {
        let (sx, sy, w, h) = self.sprite_rects.get(id).copied().flatten()?;
        if w == 0 || h == 0 {
            return None;
        }
        Some(UiQuad {
            rect: [x, y, w as f32 * scale, h as f32 * scale],
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
            rect,
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
            rect,
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
const COOLDOWN_SHADE: [f32; 4] = [0.0, 0.0, 0.0, 0.55];
const BAR_BG: [f32; 4] = [0.05, 0.05, 0.05, 0.9];

// The spellbook grid (remc1 :26915-70), native 640×480 scaled by w/640,
// h/480. 24 spells iterate in DISPLAY_ORDER packed with NO gaps: cell =
// the slot-slab sprite [3] = 64×37, 4 cols × 6 rows from (384,194). The
// origin was measured from the player's hi-res retail screenshot
// (2026-07-07) — the world viewport ends and the spellbook begins at
// y=194 (must agree with the renderer's BOOK_SPELL_Y); the grid bottom
// (194 + 6·37 = 416) is the map-pane base and the black-bar top.
const BOOK_GRID_X: f32 = 384.0;
const BOOK_GRID_Y: f32 = 194.0;
const BOOK_CELL_W: f32 = 64.0;
const BOOK_CELL_H: f32 = 37.0;
const BOOK_GRID_COLS: usize = 4;
/// Quick-select digit glyphs: `[30 + slot]` (slot 0 = "1" … slot 9 =
/// "0"), 10×14 badges — the number retail stamps in a hotkeyed spell's
/// book cell (sub_24230 :27857).
const SPR_QUICK_DIGIT: usize = 30;

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
        let cost = SPELLS[spell as usize].possess_mana;
        let castable = owned && cost <= loadout.mana;
        let over = cursor.0 >= cell[0]
            && cursor.0 < cell[0] + cell[2]
            && cursor.1 >= cell[1]
            && cursor.1 < cell[1] + cell[3];
        // Only owned+affordable spells are hoverable/bindable (the click
        // gate checks the mana cost at :26926).
        if over && castable {
            hovered = Some(spell_id);
        }

        // The stone slab [3] fills the cell, drawn DARKER (the original's
        // sub_23940 blends it over the black book background). Stretching
        // the slab texture to the cell is invisible.
        push_opt(&mut quads, assets.sprite_quad_rect_tint(SPR_SLOT_BG as usize, cell, SLAB_DIM));
        // The ICON at its NATIVE 62×34, UNIFORM-scaled + centered (NOT
        // baked into the stretched slab — that distorted at non-4:3).
        // OWNED = full colour (raw). NOT owned = the icon's SHAPE cut into
        // the slab as a dark relief: a translucent dark fill over the
        // stone so the tile texture shows through, darkened (player
        // 2026-07-07: "a silhouette … outer shape of the sprite, but the
        // texture of the tile exactly" — the original's blend[0xA6|dest]).
        let icon_id = SPR_SPELL_ICON + spell as usize;
        if let Some((iw, ih)) = assets.sprite_dims(icon_id) {
            let s = (cell[2] / iw).min(cell[3] / ih);
            let (dw, dh) = (iw * s, ih * s);
            let ix = cell[0] + (cell[2] - dw) / 2.0;
            let iy = cell[1] + (cell[3] - dh) / 2.0;
            let irect = [ix, iy, dw, dh];
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
            // Cooldown veil sweeps down as the burst counter runs.
            let cd = loadout.cooldown[spell as usize];
            if cd > 0.0 {
                quads.push(solid([cell[0], cell[1], cell[2], cell[3] * cd], COOLDOWN_SHADE));
            }
            // Quick-select number badge (sub_24230 :27857): a spell
            // bound to a number key shows its digit glyph [30+slot] in
            // the cell corner (slot 0 = key "1" = glyph [30], … slot 9 =
            // key "0" = [39]). The original blends the glyph toward color
            // `byte_AD167_AD157[1]` (sub_23AE0 = a coverage-mask blend,
            // NOT a raw copy) — which renders BLACK, not the sprite's own
            // yellow (shared with the castle-level digits). We reproduce
            // that with a black multiplicative tint over the glyph, so
            // the digit shape stays but the ink is black (player
            // 2026-07-07: retail digits are black).
            if let Some(slot) = quick_binds.iter().position(|&b| b == Some(spell)) {
                let s = (w / 640.0).min(h / 480.0);
                push_opt(
                    &mut quads,
                    assets.sprite_quad_tint(
                        SPR_QUICK_DIGIT + slot,
                        cell[0] + 2.0,
                        cell[1] + 2.0,
                        s,
                        DIGIT_INK,
                    ),
                );
            }
        }
        if over && castable {
            // Hover highlight (sub_24DA0): a bright frame on the cell.
            let f = cell;
            let t = [0.9, 0.85, 0.5, 0.9];
            quads.push(solid([f[0], f[1], f[2], 2.0], t));
            quads.push(solid([f[0], f[1] + f[3] - 2.0, f[2], 2.0], t));
            quads.push(solid([f[0], f[1], 2.0, f[3]], t));
            quads.push(solid([f[0] + f[2] - 2.0, f[1], 2.0, f[3]], t));
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
const CAP_AMBER: [f32; 4] = [0.85, 0.7, 0.2, 1.0];
/// Collected/banked mana bar (sub_22E50 :27377, color v29 =
/// byte_99B58[2*owner]) — WHITE, not blue (player 2026-07-07). The
/// castle-capacity bar under it (v27) stays the amber team tint.
const MANA_WHITE: [f32; 4] = [0.95, 0.95, 0.95, 1.0];
/// Spell availability progress bar (sub_23D40 :27705, color v26 =
/// byte_99B58[1+2*owner]) — GREY, not blue (player 2026-07-07); the
/// partial mana toward the next cast, under the equipped-spell icon.
const METER_GREY: [f32; 4] = [0.55, 0.55, 0.55, 1.0];
/// Bar geometry (sub_22810 draws a 64-wide fill; sub_22E50 offsets).
const BAR_W: f32 = 64.0;
const BAR_X: f32 = 58.0; // bars start +58 from the sub-panel origin
/// One HUD section = 640/5 = 128 native px (the 5×20% top strip).
const HUD_SECTION: f32 = 128.0;

/// A solid bar fill at panel-space (x,y) scaled to screen — the
/// original's `sub_22810(x,y,64,h,(val<<6)/max,color)`: `fill` is the
/// value/max fraction of the 64-px ruler. `bg` draws the dark track.
fn bar(quads: &mut Vec<UiQuad>, s: f32, x: f32, y: f32, h: f32, frac: f32, color: [f32; 4]) {
    quads.push(solid([x * s, y * s, BAR_W * s, h * s], BAR_BG));
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
    w: f32,
    _h: f32,
) -> Vec<UiQuad> {
    let mut quads = Vec::new();
    let s = w / 640.0;
    // Panel-background tint: translucent (faithful MC1, always-on
    // transparency) or opaque (the MC2 readability toggle).
    let panel_tint = if transparent { PANEL_TINT } else { WHITE };
    let push = |q: &mut Vec<UiQuad>, o: Option<UiQuad>| {
        if let Some(u) = o {
            q.push(u);
        }
    };

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
    push(&mut quads, assets.sprite_quad_tint(SPR_PANEL_BG, 2.0 * s, 2.0 * s, s, panel_tint));
    let v22 = 2.0 + cap_w; // slot A = castle panel
    let v23 = v22 + HUD_SECTION; // slot B = balloons
    let v24 = v22 + 2.0 * HUD_SECTION; // slot C = self

    let world = loadout.world_mana.max(1) as f32;
    // The level-goal tick on a mana ruler at panel-origin `ox` (the
    // win_pct mark; green once completed) — sub_22E50 :27268.
    let win_tick = |quads: &mut Vec<UiQuad>, ox: f32| {
        if loadout.win_pct > 0 {
            let tx = (ox + BAR_X) * s + BAR_W * s * (loadout.win_pct as f32 / 100.0).min(1.0);
            let tc = if loadout.completed {
                [0.3, 0.95, 0.3, 1.0]
            } else {
                [0.95, 0.95, 0.95, 1.0]
            };
            quads.push(solid([tx - 1.0, 26.0 * s, 2.0, 12.0 * s], tc));
        }
    };

    // === Slot A: the linked castle (:27215). Gated on the castle
    // existing AND level > 0 (else the bare marble [54]). player. ===
    let castle = loadout.castle.filter(|(_, _, lvl)| *lvl > 0);
    let slot_a_bg = if castle.is_none() {
        SPR_WIZ_EMPTY
    } else if vitals.castle_alert {
        SPR_WIZ_ALERT
    } else {
        SPR_WIZ_BG
    };
    push(&mut quads, assets.sprite_quad_tint(slot_a_bg, v22 * s, 2.0 * s, s, panel_tint));
    if let Some((stored, capacity, level)) = castle {
        let ox = v22;
        // Castle-level glyph [43+level] (emblem/heart/orb/digit baked
        // in) then the divider [42].
        push(
            &mut quads,
            assets.sprite_quad(SPR_CASTLE_LVL + level as usize, (ox + 2.0) * s, 2.0 * s, s),
        );
        push(&mut quads, assets.sprite_quad(SPR_DIVIDER, (ox + 38.0) * s, 2.0 * s, s));
        // Life bar (+58, y=10) = the CASTLE's HP (v4x->actLife/maxLife,
        // palette 0x7B) — NOT the player's life (:27237). castle_hp is
        // the downgrade meter.
        let hp = loadout
            .castle_hp
            .map_or(1.0, |(cur, max)| cur.max(0) as f32 / max.max(1) as f32);
        bar(&mut quads, s, ox + BAR_X, 10.0, 10.0, hp, LIFE_RED);
        // Mana capacity + banked, world-relative (y=28), overlaid.
        bar(&mut quads, s, ox + BAR_X, 28.0, 10.0, capacity as f32 / world, CAP_AMBER);
        bar(
            &mut quads,
            s,
            ox + BAR_X,
            28.0,
            10.0,
            (stored + loadout.banked) as f32 / world,
            MANA_WHITE,
        );
        win_tick(&mut quads, ox);
    }

    // === Slot B: the mana balloons (:27278-344). Empty [54] with no
    // roster; otherwise the balloon glyph [50+count] + divider, then a
    // thin HP + cargo bar per balloon, stacked 2px apart. ===
    let balloons = &loadout.balloons;
    let slot_b_bg = if balloons.is_empty() { SPR_WIZ_EMPTY } else { SPR_WIZ_BG };
    push(&mut quads, assets.sprite_quad_tint(slot_b_bg, v23 * s, 2.0 * s, s, panel_tint));
    if !balloons.is_empty() {
        let ox = v23;
        let count = balloons.len().min(3);
        push(
            &mut quads,
            assets.sprite_quad(SPR_BALLOON_GLYPH + count, (ox + 2.0) * s, 2.0 * s, s),
        );
        push(&mut quads, assets.sprite_quad(SPR_DIVIDER, (ox + 38.0) * s, 2.0 * s, s));
        // Per balloon: HP bar at y=12+2i (red), cargo bar at y=30+2i
        // (banked-mana white) — the thin stacked lines (:27338-39).
        for (i, &(hp, cargo)) in balloons.iter().enumerate().take(3) {
            let y = 2.0 * i as f32;
            thin_bar(&mut quads, s, ox + BAR_X, 12.0 + y, hp, LIFE_RED);
            thin_bar(&mut quads, s, ox + BAR_X, 30.0 + y, cargo, MANA_WHITE);
        }
    }

    // === Slot C: the player's OWN wizard (:27346-388). Always drawn
    // (no gate) — the wizard is always present. ===
    let slot_c_bg = if vitals.castle_alert { SPR_WIZ_ALERT } else { SPR_WIZ_BG };
    push(&mut quads, assets.sprite_quad_tint(slot_c_bg, v24 * s, 2.0 * s, s, panel_tint));
    {
        let ox = v24;
        // Base wizard glyph [43] + divider [42] (:27358-72; the alert
        // /grace variant swaps a blended copy — we keep the plain draw).
        push(&mut quads, assets.sprite_quad(SPR_CASTLE_LVL, (ox + 2.0) * s, 2.0 * s, s));
        push(&mut quads, assets.sprite_quad(SPR_DIVIDER, (ox + 38.0) * s, 2.0 * s, s));
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
        // Self mana: capacity (var_136 = mana_max, amber) + current
        // (var_140 = mana, white) over the world total (:27376-77).
        bar(&mut quads, s, ox + BAR_X, 28.0, 10.0, loadout.mana_max as f32 / world, CAP_AMBER);
        bar(&mut quads, s, ox + BAR_X, 28.0, 10.0, loadout.mana as f32 / world, MANA_WHITE);
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
        push(&mut quads, assets.sprite_quad_tint(frame, px * s, 2.0 * s, s, panel_tint));
        if let Some(sp) = spell {
            // Icon at native size, drawn raw on top of the frame (the
            // original's DrawBitmap, no stretch) — a couple px in.
            push(
                &mut quads,
                assets.sprite_quad(SPR_SPELL_ICON + sp as usize, (px + 1.0) * s, 3.0 * s, s),
            );
            // Availability meter at (frame+4, frame+36) — sub_23D40
            // :27703-34. A progress bar (partial mana toward the next
            // cast) with a row-pair of SINGLE-PIXEL dots over it, one dot
            // per whole cast currently affordable (sub_61594 writes one
            // pixel each; 2 rows, columns step by 2, up to 27 wide).
            let cost = SPELLS[sp as usize].possess_mana.max(1);
            let mana = loadout.mana;
            let mx = px + 4.0; // sub_23D40 a1+4
            let my = 2.0 + 36.0; // a2+36
            // Progress bar: (56 * (mana % cost)) / cost px wide, 4 tall.
            let partial = (56.0 * (mana % cost) as f32 / cost as f32).floor();
            quads.push(solid([mx * s, my * s, partial * s, 4.0 * s], METER_GREY));
            // Dots: one single pixel per whole cast, filled column-major
            // (2 rows), columns +2 apart — up to 27 columns (54 casts).
            let casts = (mana / cost).min(54) as usize;
            for d in 0..casts {
                let col = (d / 2) as f32;
                let row = (d % 2) as f32;
                quads.push(solid(
                    [(mx + col * 2.0) * s, (my + row * 2.0) * s, s.max(1.0), s.max(1.0)],
                    WHITE,
                ));
            }
        }
    }
    quads
}

/// Mortality overlays + the life bar (functional-first placement;
/// the faithful HUD layout is the banked UI/UX track). `blink`
/// drives the dead-screen respawn prompt.
pub fn vitals_quads(v: &PlayerVitals, w: f32, h: f32, blink: bool) -> Vec<UiQuad> {
    let mut quads = Vec::new();
    let scale = (w / 640.0).max(1.0);
    // Life bar just above the center mana bar, traffic-light tinted.
    let frac = (v.life as f32 / v.life_max.max(1) as f32).clamp(0.0, 1.0);
    let bw = w * 0.25;
    let y = h - 26.0 * scale;
    quads.push(solid([(w - bw) / 2.0, y, bw, 7.0 * scale], BAR_BG));
    let color = if frac > 0.5 {
        [0.25, 0.8, 0.3, 1.0]
    } else if frac > 0.25 {
        [0.9, 0.75, 0.2, 1.0]
    } else {
        [0.9, 0.2, 0.15, 1.0]
    };
    quads.push(solid(
        [(w - bw) / 2.0 + 1.0, y + 1.0, (bw - 2.0) * frac, 7.0 * scale - 2.0],
        color,
    ));
    // Spawn-grace shimmer: a thin white strip draining over the bar.
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
    // Castle under attack (the original flashes the castle panel,
    // Type_160+391): an amber strip over the castle-panel bars.
    if v.castle_alert {
        quads.push(solid(
            [w - w * 0.18 - 12.0 * scale, h - 66.0 * scale, w * 0.18, 3.0 * scale],
            [1.0, 0.4, 0.1, 0.9],
        ));
    }
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
