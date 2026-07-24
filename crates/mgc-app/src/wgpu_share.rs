//! Bridges the ash Vulkan instance/device already owned by [`XrContext`]
//! into a wgpu `Device`/`Queue` backed by the SAME Vulkan objects.
//!
//! OpenXR must own the Vulkan device the compositor renders into, so we
//! cannot let wgpu create its own instance/device the way the desktop path
//! (`Renderer::for_window`) does. Instead we hand wgpu-hal the raw ash
//! handles [`XrContext`] already created and had OpenXR's session built on.
//!
//! `drop_callback: None` is passed at every hal interop point so wgpu never
//! destroys the raw VkInstance/VkDevice itself — [`XrContext`]'s `Drop` impl
//! remains the sole owner of that teardown. Because of this, [`WgpuContext`]
//! must be dropped BEFORE the [`XrContext`] it was built from (wgpu's own
//! per-object cleanup, e.g. waiting for the device to go idle, still needs
//! the Vulkan handles to be valid).

use std::ffi::CStr;

use wgpu::hal;

use crate::xr_init::XrContext;

type Vulkan = hal::api::Vulkan;

pub struct WgpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl WgpuContext {
    pub fn from_xr(ctx: &XrContext) -> Result<Self, Box<dyn std::error::Error>> {
        // No instance extensions requested: we don't rely on any of the
        // optional loaders wgpu-hal gates on them (debug utils, physical
        // device properties2 are both fine to go without here).
        let instance_extensions: Vec<&'static CStr> = Vec::new();

        let hal_instance = unsafe {
            hal::vulkan::Instance::from_raw(
                ctx.vk_entry.clone(),
                ctx.vk_instance.clone(),
                ash::vk::API_VERSION_1_1,
                ctx.android_sdk_version,
                None, // no debug-utils messenger
                instance_extensions,
                wgpu::InstanceFlags::default(),
                false, // has_nv_optimus
                None,  // drop_callback — XrContext::drop owns destroy_instance
            )?
        };

        let exposed_adapter = hal_instance
            .expose_adapter(ctx.vk_physical_device)
            .ok_or("wgpu-hal could not expose the XR-selected Vulkan physical device")?;

        let instance = unsafe { wgpu::Instance::from_hal::<Vulkan>(hal_instance) };
        let adapter = unsafe { instance.create_adapter_from_hal::<Vulkan>(exposed_adapter) };

        log::info!("wgpu adapter over XR device: {:?}", adapter.get_info());

        // enabled_extensions is empty because we only enabled OpenXR's
        // required device extensions on ctx.vk_device (xr_init.rs), none of
        // which are among wgpu-hal's optional feature-detection extensions
        // (draw_indirect_count, timeline_semaphore, ray tracing, debug
        // utils) — an honest reflection of what's actually enabled.
        let open_device = unsafe {
            adapter.as_hal::<Vulkan, _, _>(|hal_adapter| {
                hal_adapter
                    .expect("adapter created via create_adapter_from_hal::<Vulkan> is Vulkan")
                    .device_from_raw(
                        ctx.vk_device.clone(),
                        None, // drop_callback — XrContext::drop owns destroy_device
                        &[],
                        wgpu::Features::empty(),
                        &wgpu::MemoryHints::default(),
                        ctx.vk_queue_family_index,
                        0,
                    )
            })
        }?;

        let (device, queue) = unsafe {
            adapter.create_device_from_hal::<Vulkan>(
                open_device,
                &wgpu::DeviceDescriptor {
                    label: Some("mgcarpet-xr"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None, // trace_path
            )?
        };

        log::info!("wgpu device/queue created against the XR Vulkan session");

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}
