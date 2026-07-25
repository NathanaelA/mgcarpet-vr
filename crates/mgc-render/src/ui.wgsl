// Screen-space or world-space UI sprites (spellbook icons, HUD slots,
// mana bars). In the default screen-space mode quads live in pixel
// coordinates, origin top-left. In VR the same quad stream can be pinned
// to a world-space panel in front of the player so the HUD has
// stereoscopic depth instead of being glued to the near plane.
// The atlas is RGBA, pre-composited on the CPU through the engine's blend
// LUT (`blend[src | dest<<8]`, the original's 2D blit path) so the shader
// stays a dumb textured blit. A zero-width UV rect means "solid quad"
// (mana-bar fills, dim overlays) drawn from the tint alone.

struct UiGlobals {
    // Surface size in pixels (z/w unused).
    screen: vec4<f32>,
    // Per-eye view-projection matrix (used in world-space mode).
    view_proj: mat4x4<f32>,
    // World-space panel basis.  The panel is centered in screen pixels;
    // a pixel offset from the screen centre is multiplied by panel_scale
    // and expanded along panel_right / panel_up.
    panel_origin: vec3<f32>,
    panel_scale: f32,
    panel_right: vec3<f32>,
    panel_mode: u32,
    panel_up: vec3<f32>,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> ui: UiGlobals;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_samp: sampler;

struct Instance {
    // x, y, w, h in pixels.
    @location(0) rect: vec4<f32>,
    // u, v, w, h in texels; w == 0 -> solid tint quad.
    @location(1) uv: vec4<f32>,
    @location(2) tint: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    // UV in TEXELS (not normalized): the fragment stage clamps it to
    // the sprite's own cell before sampling.
    @location(0) uv: vec2<f32>,
    // 0 = textured, 1 = solid tint quad, 2 = MASK-DARKEN (the sprite is a
    // coverage MASK; inside it the destination — the stone slab already
    // drawn beneath — is DARKENED, so the icon's outer shape reads as a
    // dark relief cut into the slab texture. The original's sub_23AE0
    // writes blend[0xA6 | dest]; we approximate with a dark translucent
    // fill over the slab so the tile texture shows through, darkened.
    // Used for unowned spellbook icons).
    @location(1) @interpolate(flat) mode: u32,
    @location(2) @interpolate(flat) tint: vec4<f32>,
    // The sprite's cell in the atlas, in texels — the clamp bounds.
    @location(3) @interpolate(flat) uv_min: vec2<f32>,
    @location(4) @interpolate(flat) uv_max: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: Instance) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corners[vid];
    // SNAP each corner to the pixel grid. The UI is authored at a fixed
    // resolution and scaled by an arbitrary factor, so quad edges land
    // between pixels; adjacent sprites (a scroll's three pieces, the
    // panel behind a button) then leave a hairline gap or overlap along
    // their shared edge. Rounding the CORNERS rather than the origin
    // keeps neighbours welded: two quads sharing an edge round the same
    // coordinate the same way, so the seam cannot open.
    let px = round(inst.rect.xy + c * inst.rect.zw);
    var out: VsOut;

    // Solid quads with a negative uv.w are drawn screen-space even in VR,
    // so fullscreen flashes/fades cover the whole eye instead of only the
    // world-space HUD panel.
    let screen_space = (ui.panel_mode == 0u) || (inst.uv.w < 0.0);
    if screen_space {
        // SCREEN-SPACE HUD (flat screen / fullscreen VR flash): Pixel -> NDC
        // (y down in pixels, up in NDC).
        out.clip = vec4<f32>(
            px.x / ui.screen.x * 2.0 - 1.0,
            1.0 - px.y / ui.screen.y * 2.0,
            0.0,
            1.0,
        );
    } else {
        // WORLD-SPACE HUD PANEL (VR): each pixel offset from the screen
        // centre is mapped onto a rectangle floating in world space, then
        // projected through the per-eye view-projection matrix.
        let center = ui.screen.xy * 0.5;
        let offset = (px - center) * ui.panel_scale;
        let world = ui.panel_origin
                  + ui.panel_right * offset.x
                  + ui.panel_up    * offset.y;
        out.clip = ui.view_proj * vec4<f32>(world, 1.0);
    }

    // uv.z (width) encodes the draw mode: 0 = solid tint; < 0 = silhouette
    // (|w| is the real width); > 0 = normal textured.
    let uvw = abs(inst.uv.z);
    let uv_size = vec2<f32>(uvw, inst.uv.w);
    out.uv = inst.uv.xy + c * uv_size;
    out.uv_min = inst.uv.xy;
    out.uv_max = inst.uv.xy + uv_size;
    if inst.uv.z == 0.0 {
        out.mode = 1u;
    } else if inst.uv.z < 0.0 {
        out.mode = 2u;
    } else {
        out.mode = 0u;
    }
    out.tint = inst.tint;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if in.mode == 1u {
        return in.tint;
    }
    // Nearest-texel chunky sampling, like every sprite path here.
    //
    // Clamped to the sprite's own cell first. Sprites are packed into
    // the atlas with NO gutter between them, so a fragment whose
    // interpolated coordinate reaches the far edge samples the
    // NEIGHBOURING sprite instead — one stray row or column of foreign
    // pixels along the edge. Half a texel in from each side keeps every
    // sample inside the cell without shifting the interior.
    let tex_size = vec2<f32>(textureDimensions(atlas_tex));
    let t = clamp(in.uv, in.uv_min + 0.5, in.uv_max - 0.5) / tex_size;
    let c = textureSample(atlas_tex, atlas_samp, t);
    if c.a < 0.5 {
        discard;
    }
    if in.mode == 2u {
        // Mask-darken: the icon is a coverage mask. Output a dark fill
        // with the tint's alpha, which ALPHA-BLENDS over the slab already
        // drawn beneath — so the stone texture shows through, darkened,
        // inside the icon's outer shape (the original's blend[0xA6|dest]).
        // tint.rgb = the dark ink, tint.a = how strongly it darkens.
        return vec4<f32>(in.tint.rgb, in.tint.a);
    }
    return c * in.tint;
}
