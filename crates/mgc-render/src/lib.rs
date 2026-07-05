//! The wgpu renderer.
//!
//! Reads simulation state, never mutates it; interpolates between fixed
//! ticks for smooth motion at any display rate.
//!
//! Design commitments (see project README):
//! - Terrain, billboarded sprites, and water from baked packages.
//! - Palette-index data kept all the way to the fragment shader
//!   (palette-as-LUT) so the authentic 8-bit look is the baseline and
//!   enhanced rendering is a toggle, not a rewrite.
//!
//! Current scope: the terrain pass — a 256x256 tile mesh (one vertex
//! per grid point, engine-authentic alternating diagonals), tiles
//! textured in the fragment shader from the baked terrain atlas (the
//! terrain-type byte is the atlas cell index), texels resolved through
//! the engine's shade LUT and palette; flat map colors as the fallback
//! when no atlas is baked. Per-vertex hillshade, distance fog.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use mgc_sim::{HEIGHT_SCALE, MAP_TILES};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Number of light levels in the engine's shade-remap table.
pub const SHADE_LEVELS: usize = 64;

/// Width in pixels of a baked terrain-texture atlas (`terrain-atlas-N.bin`).
pub const ATLAS_WIDTH: usize = 256;
/// Edge length of one atlas cell (one terrain texture).
pub const ATLAS_CELL: usize = 32;

/// Everything the renderer needs from a loaded level: terrain arrays
/// from the package, color tables from the baked assets. Pixels resolve
/// exactly like the original engine: a base palette index — an atlas
/// texel where a terrain atlas is available, else the tile's flat map
/// color `tile_colors[type]` — through the shade remap and palette:
/// `palette[shade_lut[shade][index]]`.
pub struct LevelView {
    /// 256x256 terrain-type bytes, row-major `y * 256 + x`.
    pub tile_type: Vec<u8>,
    /// 256x256 height bytes, same layout.
    pub height: Vec<u8>,
    /// 256x256 light levels (the generator's shading array); None for
    /// packages baked without it (a synthetic hillshade fills in).
    pub shading: Option<Vec<u8>>,
    /// 256 RGB triplets (sRGB bytes, as baked).
    pub palette: [[u8; 3]; 256],
    /// Terrain type -> base palette index (`tile-colors-N.bin`).
    pub tile_colors: [u8; 256],
    /// Shade level x base index -> final palette index
    /// (`shade-lut-N.bin`, [`SHADE_LEVELS`] rows of 256).
    pub shade_lut: Vec<u8>,
    /// Terrain-texture atlas (`terrain-atlas-N.bin`): 8-bit palette
    /// indices, [`ATLAS_WIDTH`] wide, [`ATLAS_CELL`]-square cells, the
    /// terrain-type byte indexing cells row-major. None renders every
    /// tile with its flat map color.
    pub atlas: Option<Vec<u8>>,
    /// 256x256 angle/flags bytes (`terrain/angle.bin`): bits 4-6 pick
    /// the tile's texture UV orientation. None renders orientation 0
    /// everywhere (transition tiles like shorelines will misalign).
    pub angle: Option<Vec<u8>>,
}

/// Flat-color overhead map: one RGBA pixel per tile (256x256, row-major
/// like the terrain grids), each resolved through the engine's map-view
/// color path `palette[shade_lut[shade][tile_colors[type]]]` — the
/// exact lookup the original's fullscreen map uses (remc2 GameUI).
pub fn map_pixels(level: &LevelView) -> Vec<u8> {
    let n = MAP_TILES;
    let mut out = vec![0u8; n * n * 4];
    for i in 0..n * n {
        let ty = level.tile_type[i] as usize;
        let shade = level
            .shading
            .as_ref()
            .map(|s| (s[i] as usize).min(SHADE_LEVELS - 1))
            .unwrap_or(32);
        let base = level.tile_colors[ty] as usize;
        let idx = level.shade_lut[shade * 256 + base] as usize;
        out[i * 4..i * 4 + 3].copy_from_slice(&level.palette[idx]);
        out[i * 4 + 3] = 255;
    }
    out
}

/// Camera state for one rendered frame (already interpolated).
#[derive(Debug, Clone, Copy)]
pub struct CameraView {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    /// Vertical field of view in radians.
    pub fov_y: f32,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Vertex {
    pos: [f32; 3],
    light: f32,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    camera: [f32; 4],
    fog_color: [f32; 4],
    /// x = atlas cell count (0 = untextured), y/z/w reserved.
    atlas: [u32; 4],
    /// Camera basis for billboard expansion (screen-aligned quads).
    cam_right: [f32; 4],
    cam_up: [f32; 4],
}

/// One world sprite to draw, resolved from a level entity. Static data;
/// the view-dependent part (which rotation view, mirroring) is computed
/// per frame from `yaw` and the camera.
#[derive(Debug, Clone, Copy)]
pub struct Billboard {
    /// Feet-center position, world units (x/z tile coords, y altitude).
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Facing, radians (same convention as [`CameraView::yaw`]).
    pub yaw: f32,
    /// First sprite id of the entity's view/animation family.
    pub sprite_base: u16,
    /// The original's view-selection mode (sprite flags high byte /
    /// stats-table draw type): 0/1/21 single view, 2..=16 animation,
    /// 17 = 8 views + mirrored back half, 18 = 16 views, 19/20 =
    /// 5-/3-view folds.
    pub draw_type: u8,
    /// World height of the quad (engine `var_8 / 256`).
    pub world_h: f32,
}

/// 16 view sectors folded to 5 sprites (draw type 19, `byte_906E8`).
const VIEW_FOLD_5: [u8; 16] = [0, 1, 1, 2, 2, 3, 3, 4, 4, 3, 3, 2, 2, 1, 1, 0];
/// 16 view sectors folded to 3 sprites (draw type 20, `byte_906F8`).
const VIEW_FOLD_3: [u8; 16] = [0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, 0, 0];

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct BillboardInstance {
    pos: [f32; 3],
    size: [f32; 2],
    uv_pos: [f32; 2],
    uv_size: [f32; 2],
    /// x = mirror, y = shade LUT row.
    flags: [u32; 2],
    _pad: [u32; 1],
}

/// Sky/fog color, the classic hazy horizon. sRGB values converted to
/// linear where uploaded.
const SKY_SRGB: [f32; 3] = [0.42, 0.55, 0.75];
const FOG_DENSITY: f32 = 0.006;

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn sky_color_linear() -> [f64; 3] {
    [
        srgb_to_linear(SKY_SRGB[0]) as f64,
        srgb_to_linear(SKY_SRGB[1]) as f64,
        srgb_to_linear(SKY_SRGB[2]) as f64,
    ]
}

enum Target {
    Window {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    Offscreen {
        color: wgpu::Texture,
        width: u32,
        height: u32,
    },
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: Target,
    depth: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: Option<wgpu::BindGroup>,
    vertex_buf: Option<wgpu::Buffer>,
    index_buf: Option<wgpu::Buffer>,
    index_count: u32,
    /// Cell count of the loaded terrain atlas (0 = render flat colors).
    atlas_cells: u32,
    /// Interpolate per-tile shade across tile centers (enhancement,
    /// off = the original's per-tile shade snap).
    smooth_shading: bool,
    /// The book screen (the original's Enter view): overhead map on the
    /// right half, left half reserved for the spell list.
    map_view: bool,
    map_pipeline: wgpu::RenderPipeline,
    map_globals_buf: wgpu::Buffer,
    map_bind_group_layout: wgpu::BindGroupLayout,
    map_bind_group: Option<wgpu::BindGroup>,
    fill_pipeline: wgpu::RenderPipeline,
    // Billboard (world sprite) pass.
    billboard_pipeline: wgpu::RenderPipeline,
    billboard_bind_group_layout: wgpu::BindGroupLayout,
    billboard_bind_group: Option<wgpu::BindGroup>,
    billboard_buf: Option<wgpu::Buffer>,
    billboard_capacity: usize,
    /// CPU copy of the sprite index for per-frame view selection.
    sprite_index: Option<mgc_formats::bundle::SpriteIndex>,
    sprite_tex: Option<wgpu::Texture>,
    colormap_tex: Option<wgpu::Texture>,
    billboards: Vec<Billboard>,
}

#[derive(Debug)]
pub enum RenderError {
    NoAdapter,
    Device(String),
    Surface(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAdapter => write!(f, "no compatible GPU adapter found"),
            Self::Device(e) => write!(f, "device: {e}"),
            Self::Surface(e) => write!(f, "surface: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

impl Renderer {
    /// Renderer presenting to a winit window.
    pub fn for_window(window: Arc<winit::window::Window>) -> Result<Self, RenderError> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(window)
            .map_err(|e| RenderError::Surface(e.to_string()))?;
        let (adapter, device, queue) = request_device(&instance, Some(&surface))?;
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or(RenderError::NoAdapter)?;
        // Prefer an sRGB format so shader output is linear color.
        let caps = surface.get_capabilities(&adapter);
        if let Some(srgb) = caps.formats.iter().find(|f| f.is_srgb()) {
            config.format = *srgb;
        }
        surface.configure(&device, &config);
        let format = config.format;
        let (width, height) = (config.width, config.height);
        Ok(Self::finish_init(
            device,
            queue,
            Target::Window { surface, config },
            format,
            width,
            height,
        ))
    }

    /// Renderer drawing into an offscreen texture (screenshot mode,
    /// used for autonomous end-to-end verification).
    pub fn offscreen(width: u32, height: u32) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let (_adapter, device, queue) = request_device(&instance, None)?;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Ok(Self::finish_init(
            device,
            queue,
            Target::Offscreen {
                color,
                width,
                height,
            },
            format,
            width,
            height,
        ))
    }

    fn finish_init(
        device: wgpu::Device,
        queue: wgpu::Queue,
        target: Target,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terrain.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // The map (book screen) pass: fullscreen-quad pipeline over the
        // CPU-composed map texture.
        let map_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("map"),
            source: wgpu::ShaderSource::Wgsl(include_str!("map.wgsl").into()),
        });
        let map_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("map"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let map_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("map"),
            bind_group_layouts: &[&map_bind_group_layout],
            push_constant_ranges: &[],
        });
        let map_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("map"),
            layout: Some(&map_layout),
            vertex: wgpu::VertexState {
                module: &map_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &map_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let map_globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map globals"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Solid sky fill behind the book screen's world viewport.
        let fill_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fill"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fill.wgsl").into()),
        });
        let fill_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fill"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fill"),
            layout: Some(&fill_layout),
            vertex: wgpu::VertexState {
                module: &fill_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fill_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // Billboard pass: instanced screen-aligned quads over the
        // sprite atlas, same colormap as terrain.
        let billboard_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("billboard"),
            source: wgpu::ShaderSource::Wgsl(include_str!("billboard.wgsl").into()),
        });
        let billboard_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("billboard"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let billboard_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("billboard"),
            bind_group_layouts: &[&billboard_bind_group_layout],
            push_constant_ranges: &[],
        });
        let billboard_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("billboard"),
            layout: Some(&billboard_layout),
            vertex: wgpu::VertexState {
                module: &billboard_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BillboardInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32x2, 2 => Float32x2,
                        3 => Float32x2, 4 => Uint32x2,
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &billboard_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let depth = create_depth(&device, width, height);

        Self {
            device,
            queue,
            target,
            depth,
            pipeline,
            globals_buf,
            bind_group_layout,
            bind_group: None,
            vertex_buf: None,
            index_buf: None,
            index_count: 0,
            atlas_cells: 0,
            smooth_shading: false,
            map_view: false,
            map_pipeline,
            map_globals_buf,
            map_bind_group_layout,
            map_bind_group: None,
            fill_pipeline,
            billboard_pipeline,
            billboard_bind_group_layout,
            billboard_bind_group: None,
            billboard_buf: None,
            billboard_capacity: 0,
            sprite_index: None,
            sprite_tex: None,
            colormap_tex: None,
            billboards: Vec::new(),
        }
    }

    /// Toggle the book screen (overhead map + reserved spell half).
    pub fn set_map_view(&mut self, on: bool) {
        self.map_view = on;
    }

    pub fn map_view(&self) -> bool {
        self.map_view
    }

    /// Toggle smooth (tile-interpolated) shading; off is the original's
    /// per-tile shade snap. Takes effect on the next frame.
    pub fn set_smooth_shading(&mut self, on: bool) {
        self.smooth_shading = on;
    }

    pub fn smooth_shading(&self) -> bool {
        self.smooth_shading
    }

    /// Upload a level: build the terrain mesh and the color/type LUTs.
    pub fn load_level(&mut self, level: &LevelView) {
        let n = MAP_TILES;
        assert_eq!(level.height.len(), n * n);
        assert_eq!(level.tile_type.len(), n * n);

        // Height at a wrapped grid point.
        let h = |x: usize, z: usize| -> f32 {
            level.height[(z % n) * n + (x % n)] as f32 * HEIGHT_SCALE
        };

        // One vertex per grid point, plus a duplicated wrap row/column so
        // the last tile closes the seam with the first.
        let verts_per_side = n + 1;
        let mut vertices = Vec::with_capacity(verts_per_side * verts_per_side);
        // When the package carries the generator's shading array, it is
        // the light source (vertex light stays 1.0). Otherwise fall back
        // to a synthetic hillshade: fixed sun from the north-west.
        let synthetic = level.shading.is_none();
        let sun = {
            let v: [f32; 3] = [-0.45, 0.8, -0.4];
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / len, v[1] / len, v[2] / len]
        };
        for z in 0..verts_per_side {
            for x in 0..verts_per_side {
                let y = h(x, z);
                let light = if synthetic {
                    // Central-difference normal with wraparound neighbors.
                    let dx = h(x + 1, z) - h(x + n - 1, z);
                    let dz = h(x, z + 1) - h(x, z + n - 1);
                    let inv = 1.0 / (dx * dx + dz * dz + 4.0).sqrt();
                    let normal = [-dx * inv, 2.0 * inv, -dz * inv];
                    let ndotl = normal[0] * sun[0] + normal[1] * sun[1] + normal[2] * sun[2];
                    0.55 + 0.55 * ndotl.max(0.0)
                } else {
                    1.0
                };
                vertices.push(Vertex {
                    pos: [x as f32, y, z as f32],
                    light,
                });
            }
        }

        // Two triangles per tile; diagonal orientation alternates in a
        // checkerboard exactly like the engine's altitude interpolation
        // (sub_B5C60: `(tile_x + tile_z) & 1` picks the split).
        let mut indices: Vec<u32> = Vec::with_capacity(n * n * 6);
        let at = |x: usize, z: usize| (z * verts_per_side + x) as u32;
        for z in 0..n {
            for x in 0..n {
                let (a, b, c, d) = (at(x, z), at(x + 1, z), at(x + 1, z + 1), at(x, z + 1));
                if (x + z) & 1 == 0 {
                    // Split along the a-c diagonal.
                    indices.extend_from_slice(&[a, c, b, a, d, c]);
                } else {
                    // Split along the b-d diagonal.
                    indices.extend_from_slice(&[a, d, b, b, d, c]);
                }
            }
        }

        use wgpu::util::DeviceExt;
        self.vertex_buf = Some(
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("terrain vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
        );
        self.index_buf = Some(
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("terrain indices"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
        );
        self.index_count = indices.len() as u32;

        // A small helper: 2D R8Uint texture from a byte grid.
        let byte_tex = |label: &str, bytes: &[u8], width: u32, height: u32| {
            let extent = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                tex.as_image_copy(),
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width),
                    rows_per_image: None,
                },
                extent,
            );
            tex
        };

        let type_tex = byte_tex("tile types", &level.tile_type, n as u32, n as u32);
        // Without a baked shading array, a constant mid level keeps the
        // colormap row selection stable (vertex light shades instead).
        let flat_shading;
        let shading: &[u8] = match &level.shading {
            Some(s) => s,
            None => {
                flat_shading = vec![32u8; n * n];
                &flat_shading
            }
        };
        let shade_tex = byte_tex("tile shading", shading, n as u32, n as u32);

        // Type -> flat base palette index, for tiles rendered without a
        // texture (no atlas, or type beyond the atlas).
        let tile_colors_tex = byte_tex("tile colors", &level.tile_colors, 256, 1);

        // Terrain-texture atlas (a 1x1 dummy keeps the bind group layout
        // uniform when the level has none; the shader gates on the cell
        // count in Globals).
        let (atlas_data, atlas_w, atlas_h): (&[u8], u32, u32) = match &level.atlas {
            Some(a) => {
                assert_eq!(a.len() % (ATLAS_WIDTH * ATLAS_CELL), 0, "ragged atlas");
                (a, ATLAS_WIDTH as u32, (a.len() / ATLAS_WIDTH) as u32)
            }
            None => (&[0], 1, 1),
        };
        self.atlas_cells = level
            .atlas
            .as_ref()
            .map(|a| (a.len() / (ATLAS_WIDTH * ATLAS_CELL)) * (ATLAS_WIDTH / ATLAS_CELL))
            .unwrap_or(0) as u32;
        let atlas_tex = byte_tex("terrain atlas", atlas_data, atlas_w, atlas_h);

        // Per-tile texture orientation (angle bits 4-6); orientation 0
        // for packages baked before the angle member existed.
        let flat_angle;
        let angle: &[u8] = match &level.angle {
            Some(a) => {
                assert_eq!(a.len(), n * n);
                a
            }
            None => {
                flat_angle = vec![0u8; n * n];
                &flat_angle
            }
        };
        let angle_tex = byte_tex("tile angles", angle, n as u32, n as u32);

        // Colormap (x = palette index, y = shade): the engine's shade
        // remap composed with the palette on the CPU. sRGB format so
        // sampling yields linear color. Texture texels and flat tile
        // colors both resolve through this one table, exactly like the
        // original's textured inner loop `shade_lut[shade*256 + texel]`.
        assert_eq!(level.shade_lut.len(), SHADE_LEVELS * 256);
        let mut colormap = vec![0u8; SHADE_LEVELS * 256 * 4];
        for shade in 0..SHADE_LEVELS {
            for index in 0..256 {
                let final_idx = level.shade_lut[shade * 256 + index] as usize;
                let rgb = level.palette[final_idx];
                let o = (shade * 256 + index) * 4;
                colormap[o..o + 3].copy_from_slice(&rgb);
                colormap[o + 3] = 255;
            }
        }
        let colormap_extent = wgpu::Extent3d {
            width: 256,
            height: SHADE_LEVELS as u32,
            depth_or_array_layers: 1,
        };
        let colormap_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("type/shade colormap"),
            size: colormap_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            colormap_tex.as_image_copy(),
            &colormap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4),
                rows_per_image: None,
            },
            colormap_extent,
        );

        self.colormap_tex = Some(colormap_tex.clone());
        self.rebuild_billboard_bind_group();

        self.bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.globals_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &type_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &shade_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        &colormap_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &tile_colors_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        &atlas_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        &angle_tex.create_view(&Default::default()),
                    ),
                },
            ],
        }));

        // Overhead map for the book screen, composed on the CPU through
        // the engine's map color path.
        let map_rgba = map_pixels(level);
        let map_extent = wgpu::Extent3d {
            width: n as u32,
            height: n as u32,
            depth_or_array_layers: 1,
        };
        let map_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("overhead map"),
            size: map_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            map_tex.as_image_copy(),
            &map_rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(n as u32 * 4),
                rows_per_image: None,
            },
            map_extent,
        );
        self.map_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("map"),
            layout: &self.map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.map_globals_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &map_tex.create_view(&Default::default()),
                    ),
                },
            ],
        }));
    }

    /// Upload the bundle's sprite atlas + index for billboard drawing.
    pub fn load_sprites(&mut self, index: mgc_formats::bundle::SpriteIndex, atlas: &[u8]) {
        assert_eq!(
            atlas.len(),
            index.atlas_width as usize * index.atlas_height as usize
        );
        let extent = wgpu::Extent3d {
            width: index.atlas_width,
            height: index.atlas_height,
            depth_or_array_layers: 1,
        };
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite atlas"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            tex.as_image_copy(),
            atlas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(index.atlas_width),
                rows_per_image: None,
            },
            extent,
        );
        self.sprite_tex = Some(tex);
        self.sprite_index = Some(index);
        self.rebuild_billboard_bind_group();
    }

    /// Replace the set of world sprites drawn each frame.
    pub fn set_billboards(&mut self, billboards: Vec<Billboard>) {
        self.billboards = billboards;
    }

    fn rebuild_billboard_bind_group(&mut self) {
        let (Some(sprites), Some(colormap)) = (&self.sprite_tex, &self.colormap_tex) else {
            return;
        };
        self.billboard_bind_group =
            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("billboard"),
                layout: &self.billboard_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.globals_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &sprites.create_view(&Default::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &colormap.create_view(&Default::default()),
                        ),
                    },
                ],
            }));
    }

    /// Resolve each billboard against the camera (rotation view,
    /// mirroring, wrap-nearest position) into instance data — the
    /// original's per-sprite draw dispatch (remc1 DrawSprite3D_2F170),
    /// with the yaw quantization done in engine angle units.
    fn billboard_instances(&self, cam: &CameraView) -> Vec<BillboardInstance> {
        let Some(index) = &self.sprite_index else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(self.billboards.len());
        let full = MAP_TILES as f32;
        for b in &self.billboards {
            // 16 view sectors from relative yaw, exactly the engine's
            // `(((entityYaw - camYaw) >> 3) & 0xF0) >> 4` on 11-bit
            // angles: floor(rel / 128) of 2048 steps.
            let rel = (b.yaw - cam.yaw).rem_euclid(std::f32::consts::TAU);
            let view = ((rel * (2048.0 / std::f32::consts::TAU)) as i32 >> 7).clamp(0, 15) as u16;
            let (offset, mirror) = match b.draw_type {
                17 => {
                    if view < 8 {
                        (view, false)
                    } else {
                        (15 - view, true)
                    }
                }
                18 => (view, false),
                19 => (VIEW_FOLD_5[view as usize] as u16, view >= 8),
                20 => (VIEW_FOLD_3[view as usize] as u16, view >= 8),
                // 0/1/21, the 2..=16 animation modes (frame 0 until
                // entity ticking lands), and anything unknown: base.
                _ => (0, false),
            };
            let id = (b.sprite_base + offset) as usize;
            let Some(entry) = index.sprites.get(id) else {
                continue;
            };
            let Some(frame) = entry.frames.first() else {
                continue; // known-corrupt source entry
            };
            let (w, h) = (entry.width as f32, entry.height as f32);
            let world_w = b.world_h * w / h;
            // Nearest torus copy relative to the camera.
            let wrap = |p: f32, c: f32| {
                let mut d = p - c;
                if d > full / 2.0 {
                    d -= full;
                }
                if d < -full / 2.0 {
                    d += full;
                }
                c + d
            };
            out.push(BillboardInstance {
                pos: [wrap(b.x, cam.x), b.y, wrap(b.z, cam.z)],
                size: [world_w, b.world_h],
                uv_pos: [frame.x as f32, frame.y as f32],
                uv_size: [w, h],
                flags: [mirror as u32, 32],
                _pad: [0],
            });
        }
        out
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if let Target::Window { surface, config } = &mut self.target {
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
        }
        self.depth = create_depth(&self.device, width, height);
    }

    fn size(&self) -> (u32, u32) {
        match &self.target {
            Target::Window { config, .. } => (config.width, config.height),
            Target::Offscreen { width, height, .. } => (*width, *height),
        }
    }

    /// Render one frame.
    pub fn render(&mut self, cam: &CameraView) -> Result<(), wgpu::SurfaceError> {
        let (w, hpx) = self.size();

        // Book-screen layout fractions (the original's Enter view): map
        // pane left, world viewport top-right, spell list bottom-right.
        let map_pane_frac = 0.6f32;
        let viewport_h_frac = 0.42f32;
        let view_rect = (
            (w as f32 * map_pane_frac) as u32,
            0u32,
            w - (w as f32 * map_pane_frac) as u32,
            (hpx as f32 * viewport_h_frac) as u32,
        );

        let aspect = if self.map_view {
            view_rect.2 as f32 / view_rect.3.max(1) as f32
        } else {
            w as f32 / hpx as f32
        };
        let view_proj = camera_matrix(cam, aspect);
        let sky = sky_color_linear();
        // Camera right/up for billboard expansion (matches
        // `camera_matrix`'s basis).
        let (sy, cy) = cam.yaw.sin_cos();
        let (sp, cp) = cam.pitch.sin_cos();
        let fwd = [sy * cp, sp, -cy * cp];
        let right = [cy, 0.0, sy];
        let up = [
            right[1] * fwd[2] - right[2] * fwd[1],
            right[2] * fwd[0] - right[0] * fwd[2],
            right[0] * fwd[1] - right[1] * fwd[0],
        ];
        let globals = Globals {
            view_proj,
            camera: [cam.x, cam.y, cam.z, FOG_DENSITY],
            fog_color: [sky[0] as f32, sky[1] as f32, sky[2] as f32, 1.0],
            atlas: [self.atlas_cells, self.smooth_shading as u32, 0, 0],
            cam_right: [right[0], right[1], right[2], 0.0],
            cam_up: [up[0], up[1], up[2], 0.0],
        };
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        // Billboard instances for this camera (empty when no sprites
        // are loaded).
        let instances = self.billboard_instances(cam);
        let instance_count = instances.len() as u32;
        if !instances.is_empty() {
            let bytes: &[u8] = bytemuck::cast_slice(&instances);
            let need = bytes.len();
            if self.billboard_buf.is_none() || self.billboard_capacity < need {
                self.billboard_buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("billboard instances"),
                    size: need.next_power_of_two() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.billboard_capacity = need.next_power_of_two();
            }
            self.queue
                .write_buffer(self.billboard_buf.as_ref().unwrap(), 0, bytes);
        }

        let frame = match &self.target {
            Target::Window { surface, .. } => Some(surface.get_current_texture()?),
            Target::Offscreen { .. } => None,
        };
        let color_view = match (&frame, &self.target) {
            (Some(f), _) => f.texture.create_view(&Default::default()),
            (None, Target::Offscreen { color, .. }) => color.create_view(&Default::default()),
            _ => unreachable!(),
        };

        if self.map_view {
            // Square map letterboxed into the left pane.
            let pane_w = w as f32 * map_pane_frac;
            let side = pane_w.min(hpx as f32) * 0.98;
            let center_x = map_pane_frac - 1.0; // middle of the left pane in NDC
            let map_globals: [f32; 8] = [
                center_x,
                0.0,
                side / w as f32,
                side / hpx as f32,
                cam.x,
                cam.z,
                0.0,
                0.0,
            ];
            self.queue.write_buffer(
                &self.map_globals_buf,
                0,
                bytemuck::cast_slice(&map_globals),
            );
        }

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            // The book screen replaces the world view entirely (as in
            // the original); a dark backdrop fills the spell half.
            let clear = if self.map_view {
                wgpu::Color {
                    r: 0.02,
                    g: 0.015,
                    b: 0.01,
                    a: 1.0,
                }
            } else {
                wgpu::Color {
                    r: sky[0],
                    g: sky[1],
                    b: sky[2],
                    a: 1.0,
                }
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            let draw_world = |pass: &mut wgpu::RenderPass<'_>| {
                if let (Some(bg), Some(vb), Some(ib)) =
                    (&self.bind_group, &self.vertex_buf, &self.index_buf)
                {
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    // 3x3 wrap copies; the vertex shader offsets by instance.
                    pass.draw_indexed(0..self.index_count, 0, 0..9);
                }
                if let (1.., Some(bg), Some(buf)) = (
                    instance_count,
                    &self.billboard_bind_group,
                    &self.billboard_buf,
                ) {
                    pass.set_pipeline(&self.billboard_pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..6, 0..instance_count);
                }
            };
            if self.map_view {
                // World viewport in the top-right corner: sky fill, then
                // the terrain, clipped to the rect.
                let (vx, vy, vw, vh) = view_rect;
                if vw > 0 && vh > 0 {
                    pass.set_viewport(vx as f32, vy as f32, vw as f32, vh as f32, 0.0, 1.0);
                    pass.set_scissor_rect(vx, vy, vw, vh);
                    pass.set_pipeline(&self.fill_pipeline);
                    pass.draw(0..3, 0..1);
                    draw_world(&mut pass);
                    pass.set_viewport(0.0, 0.0, w as f32, hpx as f32, 0.0, 1.0);
                    pass.set_scissor_rect(0, 0, w, hpx);
                }
                // The map pane; the rest of the dark clear is the book
                // backdrop (spell list placeholder).
                if let Some(bg) = &self.map_bind_group {
                    pass.set_pipeline(&self.map_pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.draw(0..6, 0..1);
                }
            } else {
                draw_world(&mut pass);
            }
        }
        self.queue.submit([encoder.finish()]);
        if let Some(frame) = frame {
            frame.present();
        }
        Ok(())
    }

    /// Read back the offscreen target as tightly-packed RGBA8 rows.
    /// Panics if the renderer targets a window.
    pub fn read_offscreen(&self) -> (u32, u32, Vec<u8>) {
        let Target::Offscreen {
            color,
            width,
            height,
        } = &self.target
        else {
            panic!("read_offscreen on a windowed renderer");
        };
        let (width, height) = (*width, *height);
        let unpadded = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            tx.send(r).ok();
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async callback dropped")
            .expect("buffer map failed");
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((unpadded * height) as usize);
        for row in 0..height {
            let start = (row * padded) as usize;
            out.extend_from_slice(&data[start..start + unpadded as usize]);
        }
        (width, height, out)
    }
}

fn request_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), RenderError> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: surface,
        force_fallback_adapter: false,
    }))
    .ok_or(RenderError::NoAdapter)?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("mgcarpet"),
            ..Default::default()
        },
        None,
    ))
    .map_err(|e| RenderError::Device(e.to_string()))?;
    Ok((adapter, device, queue))
}

fn create_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// Column-major view-projection matrix. Yaw 0 faces -Z, positive pitch
/// looks up; right-handed, Y-up, depth 0..1.
fn camera_matrix(cam: &CameraView, aspect: f32) -> [[f32; 4]; 4] {
    let (sy, cy) = cam.yaw.sin_cos();
    let (sp, cp) = cam.pitch.sin_cos();
    let fwd = [sy * cp, sp, -cy * cp];
    let right = [cy, 0.0, sy];
    let up = [
        right[1] * fwd[2] - right[2] * fwd[1],
        right[2] * fwd[0] - right[0] * fwd[2],
        right[0] * fwd[1] - right[1] * fwd[0],
    ];
    let eye = [cam.x, cam.y, cam.z];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

    // View matrix: camera basis rows, look direction mapped to -Z.
    let view = [
        [right[0], up[0], -fwd[0], 0.0],
        [right[1], up[1], -fwd[1], 0.0],
        [right[2], up[2], -fwd[2], 0.0],
        [-dot(right, eye), -dot(up, eye), dot(fwd, eye), 1.0],
    ];

    // Perspective, near 0.05 tiles, far 600 (a 256-tile world plus fog
    // headroom), depth 0..1.
    let (near, far) = (0.05_f32, 600.0_f32);
    let f = 1.0 / (cam.fov_y * 0.5).tan();
    let proj = [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, near * far / (near - far), 0.0],
    ];

    // proj * view, both column-major.
    let mut out = [[0.0f32; 4]; 4];
    for (c, out_col) in out.iter_mut().enumerate() {
        for (r, out_cell) in out_col.iter_mut().enumerate() {
            *out_cell = (0..4).map(|k| proj[k][r] * view[c][k]).sum();
        }
    }
    out
}
