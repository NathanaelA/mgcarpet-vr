# Plan: Meta Quest 3 VR Support

## Context

The user wants to add full stereoscopic VR support for Meta Quest 3. The Quest runs Android (aarch64), uses OpenXR 1.0 for XR session/rendering, and Touch controllers for input. The existing desktop path (winit + wgpu Surface) is left completely untouched. A new `mgc-vr` crate acts as the Android entry point. `mgc-render` gains stereo rendering support behind an `openxr` feature flag.

The sim (`mgc-sim`), formats, importer, and audio crates require **zero changes** — the sim is already platform-agnostic. The desktop `mgc-app` is not modified.

---

## New crate: `crates/mgc-vr/`

The Android VR shell. Replaces `mgc-app` for the Quest target.

```
crates/mgc-vr/
  Cargo.toml              — crate-type = ["cdylib"]; deps below
  android/
    AndroidManifest.xml   — Quest VR manifest
    build.gradle          — minimal APK packager referencing the .so
  src/
    lib.rs                — #[no_mangle] android_main entry point
    xr_init.rs            — OpenXR instance/session, wgpu Vulkan sharing
    xr_loop.rs            — OpenXR frame loop (replaces winit RedrawRequested)
    xr_input.rs           — Action sets, Touch controller bindings → FlightInput
    data_path.rs          — /sdcard/mgcarpet/baked/ resolution
```

### `Cargo.toml` for `mgc-vr`

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
mgc-formats  = { path = "../mgc-formats" }
mgc-sim      = { path = "../mgc-sim" }
mgc-render   = { path = "../mgc-render", features = ["openxr"] }
mgc-audio    = { path = "../mgc-audio" }
android-activity = { version = "0.6", features = ["game-activity"] }
openxr       = "0.19"
ash          = "=0.38.0+1.3.281"   # must match wgpu-hal 24's exact ash (see below)
wgpu         = { version = "24", features = ["vulkan"] }
ndk          = "0.9"
log          = "0.4"
android_logger = "0.13"
```

**Critical ash version pinning:** wgpu-hal 24 uses `ash = "=0.38.0+1.3.281"` internally. The `openxr` crate 0.19 also uses ash 0.38. Both must resolve to the exact same crate version — otherwise `ash::Device` handles passed between wgpu's hal and openxr will be incompatible types. Pin `ash` to that exact version in both `mgc-vr/Cargo.toml` and the workspace `Cargo.toml`.

---

## Changes to `crates/mgc-render/`

Add a new `openxr` feature flag. The existing `for_window()` / `offscreen()` constructors and the mono `render()` path are unchanged.

### `Cargo.toml` additions

```toml
[features]
openxr = ["dep:openxr", "dep:ash", "wgpu/vulkan"]

[dependencies]
openxr = { version = "0.19", optional = true }
ash    = { version = "=0.38.0+1.3.281", optional = true }
```

### New types in `lib.rs`

```rust
/// Per-eye view for stereoscopic rendering.
pub struct EyeView {
    pub x: f32, pub y: f32, pub z: f32,   // world position (includes IPD offset)
    pub yaw: f32, pub pitch: f32, pub roll: f32,
    /// Column-major projection matrix from OpenXR (handles asymmetric frustum).
    pub proj: [[f32; 4]; 4],
}

pub struct StereoView {
    pub left:  EyeView,
    pub right: EyeView,
}
```

### New `Target` variant

Add `Target::Xr { width: u32, height: u32 }` to the existing private `Target` enum. No surface, no config — the frame textures come from the XR swapchain per-frame, passed directly to `render_stereo()`.

### New constructors and methods

```rust
/// For VR: device/queue are pre-created and owned by OpenXR's Vulkan session.
pub fn for_xr(
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> Self

/// Render both eyes into caller-provided swapchain TextureViews.
pub fn render_stereo(
    &mut self,
    stereo: &StereoView,
    left_color:  &wgpu::TextureView,
    right_color: &wgpu::TextureView,
    depth:       &wgpu::TextureView,
) -> Result<(), wgpu::SurfaceError>
```

`for_xr` calls the existing `finish_init(device, queue, Target::Xr { width, height }, format, width, height)` — identical to how `offscreen` works.

`render_stereo` factors out an internal `render_eye(&EyeView, color_view, depth_view, encoder)` that the existing mono `render()` is also refactored to call. No code is duplicated.

For the projection matrix, `render_eye` uses `EyeView.proj` directly (supplied by OpenXR) instead of calling the existing `camera_matrix()` helper — OpenXR's projection already handles the asymmetric Quest optical frustum.

---

## Build system

### `.cargo/config.toml` (workspace root, new file)

```toml
[target.aarch64-linux-android]
linker = "aarch64-linux-android34-clang"
```

In practice `cargo-ndk` injects the linker; this file is a fallback.

### `crates/mgc-vr/android/AndroidManifest.xml`

Key entries required for Quest 3:
- `uses-feature android:name="android.hardware.vr.headtracking"` (required)
- `uses-permission READ_EXTERNAL_STORAGE / MANAGE_EXTERNAL_STORAGE` (for baked data)
- NativeActivity with `android.app.lib_name = "mgc_vr"`
- Intent filter `com.oculus.intent.category.VR` — makes the OS launch it as VR
- `meta-data com.oculus.intent.category.VR value="vr_only"`

### APK build

```sh
# 1. Install toolchain
rustup target add aarch64-linux-android
cargo install cargo-ndk

# 2. Build the .so
cargo ndk -t arm64-v8a -o crates/mgc-vr/android-build/app/src/main/jniLibs \
    build --release -p mgc-vr

# 3. Package + sign via the minimal Gradle stub in crates/mgc-vr/android/
./gradlew assembleRelease

# 4. Sideload
adb install -r mgc-vr.apk
```

---

## OpenXR + wgpu Vulkan sharing (`xr_init.rs`)

The central constraint: OpenXR must own the Vulkan device; wgpu must be initialized with OpenXR's device handles, not its own. In wgpu 24 this uses `wgpu::hal::api::Vulkan`.

**Sequence:**

1. Create `openxr::Instance` with extensions: `XR_KHR_VULKAN_ENABLE2`, `XR_KHR_ANDROID_CREATE_INSTANCE`
2. Call `xr_instance.vulkan_instance_extensions_khr2(system_id)` → required VkInstance extensions
3. Call `xr_instance.vulkan_device_extensions_khr2(system_id)` → required VkDevice extensions
4. Manually create `ash::Instance` with those extensions + `ash::Device` on the physical device OpenXR selects
5. Wrap into wgpu HAL types: `unsafe { wgpu::hal::api::Vulkan::Instance::from_raw(...) }` + `Device::from_raw(...)`
6. Expose as wgpu: `wgpu::Instance::from_hal(...)` → `wgpu::Device` / `wgpu::Queue`
7. Create `openxr::Session<openxr::Vulkan>` with the same VkInstance/VkPhysicalDevice/VkDevice handles
8. Create XR swapchain: `R8G8B8A8_SRGB`, `array_size: 2` (left/right eyes in one swapchain)
9. Wrap each swapchain VkImage as `wgpu::Texture` via `Device::texture_from_raw()`
10. Call `Renderer::for_xr(device, queue, format, eye_width, eye_height)`

The `for_xr` wgpu device is backed by the same Vulkan device as the XR session — no Vk device duplication, no memory allocator conflict.

---

## Frame loop (`xr_loop.rs`)

Replaces winit's `RedrawRequested`. Runs on the Android main thread.

```
loop:
  poll_event() → handle session state (IDLE/READY/SYNCHRONIZED/VISIBLE/FOCUSED)
  frame_state = waiter.wait()
  stream.begin()
  if !frame_state.should_render → stream.end(empty) → continue

  // Fixed-timestep sim (identical to mgc-app accumulator)
  dt += wall_clock; while acc >= TICK_DT: sim.step(poll_input())

  // Head pose
  views = session.locate_views(PRIMARY_STEREO, predicted_display_time, &stage_space)

  // Build StereoView: Flyer position (lerped) + XR head orientation + IPD offsets
  stereo = build_stereo_view(alpha, &views)

  // Acquire, render, release
  img_idx = swapchain.acquire_image()
  renderer.render_stereo(&stereo, &left_views[img_idx], &right_views[img_idx], &depth)
  swapchain.release_image()

  // Submit composition layer
  stream.end(predicted_display_time, OPAQUE, &[projection_layer])
```

`build_stereo_view` converts OpenXR quaternion poses to `(yaw, pitch, roll)` Euler angles, applies the Flyer's world position (lerped between prev/cur ticks at `alpha`), and reads IPD from `views[0/1].pose.position.x`.

---

## Controller input (`xr_input.rs`)

Action set binding paths for `/interaction_profiles/oculus/touch_controller`:

| Action | Path | `FlightInput` field |
|--------|------|---------------------|
| Left thumbstick | `/user/hand/left/input/thumbstick` (Vector2f) | `thrust` (Y), `strafe` (X) |
| Right thumbstick | `/user/hand/right/input/thumbstick` (Vector2f) | `yaw_delta` (X), `pitch_delta` (-Y) |
| Left trigger | `/user/hand/left/input/trigger/value` (f32) | `fire_left` (> 0.5) |
| Right trigger | `/user/hand/right/input/trigger/value` (f32) | `fire_right` (> 0.5) |
| A button | `/user/hand/right/input/a/click` (bool) | spell cycle / equip |
| B button | `/user/hand/right/input/b/click` (bool) | respawn |
| X button | `/user/hand/left/input/x/click` (bool) | demolish |
| Y button | `/user/hand/left/input/y/click` (bool) | (free — spell select) |

The right stick's `stick_x`/`stick_y` are also written as `(right.x * 127) as i16` / `(-right.y * 127) as i16` for the MC1 faithful thrust model's virtual stick.

---

## Data loading (`data_path.rs`)

`mgcl::read()` and `Bundle::load()` accept a `Path` and use `std::fs::File` — no changes to `mgc-formats`. On Android:

1. Primary: `/sdcard/mgcarpet/baked/` (user pushes data via `adb push baked/ /sdcard/mgcarpet/baked/`)
2. Fallback: `app.external_data_path().join("baked")`
3. Runtime permission request for `READ_EXTERNAL_STORAGE` (Android 13+) via `ndk`

---

## Phase ordering

| Phase | Goal | Test |
|-------|------|------|
| 1 | Cross-compile: minimal `android_main` that logs and exits | APK installs, no crash |
| 2 | OpenXR session, black scene, no rendering | App launches as VR, Quest hands visible, logcat shows FOCUSED state |
| 3 | wgpu Vulkan device sharing with XR | No Vk validation errors in logcat |
| 4 | `Renderer::for_xr` + `render_stereo`, load level from sdcard | Terrain visible in both eyes, static scene |
| 5 | Head tracking (`locate_views` → `StereoView`) | World rotates with head movement, stereo depth correct |
| 6 | Sim + controller input + audio | Carpet flies, spells fire, audio plays |
| 7 | Polish: runtime permissions, save/load, APK signing, HUD review | Sideloadable signed APK |

---

## Workspace `Cargo.toml` change

Add `"crates/mgc-vr"` to `[workspace] members`. Pin `ash = "=0.38.0+1.3.281"` as a workspace dep to ensure wgpu-hal 24 and openxr 0.19 resolve to identical ash types.

---

## Verification

- Phase 2: `adb logcat | grep mgcarpet` shows XR state machine transitions
- Phase 4: Quest headset displays terrain (sideload + wear headset)
- Phase 5: Head tracking — turn head left/right, world follows correctly
- Phase 6: `cargo test --workspace` still passes on desktop; game playable on Quest
- Phase 7: `cargo ndk` + Gradle produce a signed `.apk` that installs and runs without ADB
