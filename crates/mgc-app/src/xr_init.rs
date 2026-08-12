//! OpenXR instance + Vulkan device initialisation for Android / Quest 3.
//!
//! Uses `XR_KHR_vulkan_enable` (the "legacy" path in openxr 0.19) which gives
//! us the `vulkan_legacy_*` convenience wrappers for querying extensions and
//! the physical device, while `vulkan_graphics_device` (without the `_legacy_`
//! prefix) selects the physical device.
//!
//! We use `Entry::create_instance` (the safe API) and avoid the raw `sys::*`
//! construction.  On Android the Meta OpenXR runtime finds the JVM from the
//! process's global JNI state when launched via NativeActivity, so the
//! explicit `XR_KHR_android_create_instance` next-chain is not strictly
//! required at runtime.  If the runtime rejects the instance without it a
//! future revision will add the raw-pointer approach.

use crate::wgpu_share::WgpuContext;
use ash::vk;
use ash::vk::Handle as _;
use openxr as xr;
use wgpu::hal;
use winit::platform::android::activity::AndroidApp;

/// All XR and Vulkan state for the lifetime of the session.
#[allow(unused)]
pub struct XrContext {
    // ── OpenXR ──────────────────────────────────────────────────────────────
    pub xr_instance: xr::Instance,
    pub xr_system: xr::SystemId,
    pub xr_session: xr::Session<xr::Vulkan>,
    pub frame_waiter: xr::FrameWaiter,
    pub frame_stream: xr::FrameStream<xr::Vulkan>,
    pub stage_space: xr::Space,
    pub env_blend_mode: xr::EnvironmentBlendMode,
    // ── Vulkan (kept alive; session holds raw handles into these) ────────────
    pub vk_entry: ash::Entry,
    pub vk_instance: ash::Instance,
    pub vk_physical_device: vk::PhysicalDevice,
    pub vk_device: ash::Device,
    pub vk_queue_family_index: u32,
    pub vk_queue: vk::Queue,
    /// The device's actual Android SDK level (not the NDK API level this
    /// crate was compiled for) — wgpu-hal uses it for driver workarounds.
    pub android_sdk_version: u32,
}

impl XrContext {
    pub fn new(app: &AndroidApp) -> Result<Self, Box<dyn std::error::Error>> {
        log::info!("XR Init getting Android activity and JVM pointers");

        let activity_ptr: *mut std::ffi::c_void = app.activity_as_ptr();

        let vm_ptr: *mut std::ffi::c_void = app.vm_as_ptr();

        let platform_info = unsafe { openxr::AndroidPlatformInfo::new(vm_ptr, activity_ptr) };

        // ── 1. Load OpenXR runtime ───────────────────────────────────────────
        log::info!("XR Init Loading OpenXR runtime");
        let xr_entry = unsafe { xr::Entry::load(&platform_info)? };

        // ── 2. Create OpenXR instance ────────────────────────────────────────
        log::info!("XR Init Creating OpenXR instance");
        let mut ext_set = xr::ExtensionSet::default();

        ext_set.khr_vulkan_enable2 = true;

        ext_set.khr_android_create_instance = true;

        let xr_instance = xr_entry.create_instance(
            &xr::ApplicationInfo {
                application_name: "mgcarpet-vr",
                application_version: 1,
                engine_name: "mgcarpet",
                engine_version: 1,
                api_version: xr::Version::new(1, 0, 0),
            },
            &ext_set,
            &[],
            &platform_info,
        )?;

        // ── 3. Get the HMD system ────────────────────────────────────────────
        let xr_system = xr_instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;

       /* let environment_blend_mode = xr_instance
            .enumerate_environment_blend_modes(xr_system, xr::ViewConfigurationType::PRIMARY_STEREO)
            .unwrap()[0]; */


        // ── 4. Validate minimum Vulkan version ───────────────────────────────
        let _vk_reqs = xr_instance.graphics_requirements::<xr::Vulkan>(xr_system)?;


        // ── 5. Create Vulkan instance ────────────────────────────────────────
        let vk_entry = unsafe { ash::Entry::load()? };

        let vk_app_info = vk::ApplicationInfo::default()
            .application_name(c"mgcarpet-vr")
            .application_version(vk::make_api_version(0, 0, 1, 0))
            .api_version(vk::API_VERSION_1_1);

        let vk_instance = unsafe {
            let vk_instance = xr_instance
                .create_vulkan_instance(
                    xr_system,
                    std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                    &vk::InstanceCreateInfo::default().application_info(&vk_app_info) as *const _
                        as *const _,
                )
                .expect("XR error creating Vulkan instance")
                .map_err(vk::Result::from_raw)
                .expect("Vulkan error creating Vulkan instance");
            ash::Instance::load(
                vk_entry.static_fn(),
                vk::Instance::from_raw(vk_instance as _),
            )
        };

        // ── 6. Ask OpenXR which physical device to use ───────────────────────
        let vk_phys_raw = unsafe {
            xr_instance.vulkan_graphics_device(xr_system, vk_instance.handle().as_raw() as _)?
        };
        let vk_physical_device = vk::PhysicalDevice::from_raw(vk_phys_raw as u64);

        // ── 7. Find a graphics queue family ──────────────────────────────────
        let queue_family_index = unsafe {
            vk_instance
                .get_physical_device_queue_family_properties(vk_physical_device)
                .into_iter()
                .enumerate()
                .find(|(_, p)| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .map(|(i, _)| i as u32)
                .ok_or("no graphics queue family on XR physical device")?
        };

        // ── 8. Create Vulkan device ───────────────────────────────────────────
        let vk_device = unsafe {
            let vk_device = xr_instance
                .create_vulkan_device(
                    xr_system,
                    std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                    vk_physical_device.as_raw() as _,
                    &vk::DeviceCreateInfo::default()
                        .queue_create_infos(&[vk::DeviceQueueCreateInfo::default()
                            .queue_family_index(queue_family_index)
                            .queue_priorities(&[1.0])])
                        .push_next(&mut vk::PhysicalDeviceMultiviewFeatures {
                            multiview: vk::TRUE,
                            ..Default::default()
                        }) as *const _ as *const _,
                )
                .expect("XR error creating Vulkan device")
                .map_err(vk::Result::from_raw)
                .expect("Vulkan error creating Vulkan device");

            ash::Device::load(vk_instance.fp_v1_0(), vk::Device::from_raw(vk_device as _))
        };

        let vk_queue = unsafe { vk_device.get_device_queue(queue_family_index, 0) };

        // ── 9. Create OpenXR Vulkan session ─────────────────────────────────
        let (xr_session, frame_waiter, frame_stream) = unsafe {
            xr_instance.create_session::<xr::Vulkan>(
                xr_system,
                &xr::vulkan::SessionCreateInfo {
                    instance: vk_instance.handle().as_raw() as _,
                    physical_device: vk_physical_device.as_raw() as _,
                    device: vk_device.handle().as_raw() as _,
                    queue_family_index,
                    queue_index: 0,
                },
            )?
        };

        // ── 10. Reference space (floor level) ────────────────────────────────
        let stage_space = xr_session
            .create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)?;

        // ── 11. Environment blend mode ────────────────────────────────────────
        let env_blend_mode = xr_instance
            .enumerate_environment_blend_modes(
                xr_system,
                xr::ViewConfigurationType::PRIMARY_STEREO,
            )?
            .into_iter()
            .find(|&m| m == xr::EnvironmentBlendMode::OPAQUE)
            .unwrap_or(xr::EnvironmentBlendMode::OPAQUE);

        let android_sdk_version = app.config().sdk_version() as u32;

        log::info!("XrContext ready, blend={env_blend_mode:?}, sdk={android_sdk_version}");

        Ok(Self {
            xr_instance,
            xr_system,
            xr_session,
            frame_waiter,
            frame_stream,
            stage_space,
            env_blend_mode,
            vk_entry,
            vk_instance,
            vk_physical_device,
            vk_device,
            vk_queue_family_index: queue_family_index,
            vk_queue,
            android_sdk_version,
        })
    }
}

impl Drop for XrContext {
    fn drop(&mut self) {
        // vk_device / vk_instance are dropped after the session (struct field order).
        unsafe {
            self.vk_device.device_wait_idle().ok();
            self.vk_device.destroy_device(None);
            self.vk_instance.destroy_instance(None);
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Picks an OpenXR swapchain format that also has a direct
/// `wgpu::TextureFormat` counterpart, so the render pipeline (built
/// against one fixed format in `Renderer::for_xr`) matches the actual
/// swapchain images bit-for-bit.
pub fn pick_swapchain_format(
    ctx: &XrContext,
) -> Result<(u32, wgpu::TextureFormat), Box<dyn std::error::Error>> {
    let supported = ctx.xr_session.enumerate_swapchain_formats()?;
    for raw in supported {
        let wgpu_format = match vk::Format::from_raw(raw as i32) {
            vk::Format::R8G8B8A8_SRGB => Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            vk::Format::B8G8R8A8_SRGB => Some(wgpu::TextureFormat::Bgra8UnormSrgb),
            _ => None,
        };
        if let Some(f) = wgpu_format {
            return Ok((raw, f));
        }
    }
    Err("XR runtime offers no R8G8B8A8_SRGB/B8G8R8A8_SRGB swapchain format".into())
}

/// Wraps one XR swapchain VkImage (a 2-layer array: left=layer 0,
/// right=layer 1) as a wgpu `Texture`, then splits it into per-eye
/// `TextureView`s. Same hal-interop pattern as `wgpu_share.rs`'s device
/// sharing: `drop_callback: None` because the XR runtime owns the image,
/// not us.
pub fn wrap_swapchain_image(
    wgpu_ctx: &WgpuContext,
    raw_image: u64,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> (wgpu::TextureView, wgpu::TextureView) {
    let vk_image = vk::Image::from_raw(raw_image);
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 2,
    };
    let hal_desc = hal::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: hal::TextureUses::COLOR_TARGET,
        memory_flags: hal::MemoryFlags::empty(),
        view_formats: vec![],
    };
    let hal_texture = unsafe { hal::vulkan::Device::texture_from_raw(vk_image, &hal_desc, None) };
    let texture = unsafe {
        wgpu_ctx.device.create_texture_from_hal::<hal::api::Vulkan>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("xr swapchain image"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
        )
    };
    let eye_view = |layer: u32| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: layer,
            array_layer_count: Some(1),
            ..Default::default()
        })
    };
    (eye_view(0), eye_view(1))
}

/// Rotates a vector by a unit quaternion (x, y, z, w).
pub fn quat_rotate(q: xr::Quaternionf, v: [f32; 3]) -> [f32; 3] {
    let (qx, qy, qz, qw) = (q.x, q.y, q.z, q.w);
    let uv = [
        qy * v[2] - qz * v[1],
        qz * v[0] - qx * v[2],
        qx * v[1] - qy * v[0],
    ];
    let uuv = [
        qy * uv[2] - qz * uv[1],
        qz * uv[0] - qx * uv[2],
        qx * uv[1] - qy * uv[0],
    ];
    std::array::from_fn(|i| v[i] + 2.0 * (qw * uv[i] + uuv[i]))
}

/// Decomposes an OpenXR head-pose quaternion into the (yaw, pitch, roll)
/// triple `mgc_render::camera_basis` expects: `fwd =
/// [sin(yaw)cos(pitch), sin(pitch), -cos(yaw)cos(pitch)]`, rolled about
/// `fwd` by `roll`. Exact for any orientation away from the (rare, for a
/// worn headset) straight-up/down + rolled combinations where this
/// particular Euler order is inherently ambiguous.
pub fn quat_to_ypr(q: xr::Quaternionf) -> (f32, f32, f32) {
    let fwd = quat_rotate(q, [0.0, 0.0, -1.0]);
    let pitch = fwd[1].clamp(-1.0, 1.0).asin();
    let yaw = fwd[0].atan2(-fwd[2]);

    let (sy, cy) = yaw.sin_cos();
    let flat_right = [cy, 0.0, sy];
    let flat_up = [
        flat_right[1] * fwd[2] - flat_right[2] * fwd[1],
        flat_right[2] * fwd[0] - flat_right[0] * fwd[2],
        flat_right[0] * fwd[1] - flat_right[1] * fwd[0],
    ];
    let actual_right = quat_rotate(q, [1.0, 0.0, 0.0]);
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let roll = (-dot(actual_right, flat_up)).atan2(dot(actual_right, flat_right));

    (yaw, pitch, roll)
}

/// Builds a (possibly asymmetric) perspective projection matrix from an
/// OpenXR `Fovf`, in the same column-major / depth-0..1 convention as
/// `mgc_render`'s own symmetric `camera_matrix` — see that function's
/// comment for the row/column layout this must match.
pub fn xr_projection_matrix(fov: xr::Fovf, near: f32, far: f32) -> [[f32; 4]; 4] {
    let tan_left = fov.angle_left.tan();
    let tan_right = fov.angle_right.tan();
    let tan_up = fov.angle_up.tan();
    let tan_down = fov.angle_down.tan();
    let tan_width = tan_right - tan_left;
    let tan_height = tan_up - tan_down;

    [
        [2.0 / tan_width, 0.0, 0.0, 0.0],
        [0.0, 2.0 / tan_height, 0.0, 0.0],
        [
            (tan_right + tan_left) / tan_width,
            (tan_up + tan_down) / tan_height,
            far / (near - far),
            -1.0,
        ],
        [0.0, 0.0, near * far / (near - far), 0.0],
    ]
}
