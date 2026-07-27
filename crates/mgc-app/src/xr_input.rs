//! Quest Touch controller → `FlightInput` mapping.
//!
//! | Quest input        | `FlightInput` field       |
//! |---------------------|---------------------------|
//! | Left thumbstick Y/X | `thrust` / `strafe`       |
//! | Right thumbstick X/Y| `yaw_delta` / `pitch_delta` (turn-rate, like a traditional flight stick — independent of head look) |
//! | Left/right trigger  | `fire_left` / `fire_right`|
//! | Right A             | `respawn`                 |
//! | Right B             | `demolish`                |
//! | Left X              | `equip_left` (spell 0, dev placeholder) |
//! | Left Y              | `equip_right` (spell 0, dev placeholder) |
//!
//! `fire_left`/`fire_right`/`equip_*`/`respawn`/`demolish` only take
//! effect once a [`mgc_sim::engine::world::World`] is attached to the
//! sim (spellcasting, triggers, dispositions all live there);
//! `Simulation::with_terrain` alone is a flight-only sandbox, so today
//! these are wired but inert. Left in place so the mapping doesn't need
//! revisiting when a `World` lands.

use mgc_sim::{FlightInput, Flyer};
use openxr as xr;

use crate::xr_init;

/// Turn rate at full stick deflection, radians/tick (24 Hz sim) — a
/// traditional flight-stick feel, independent of head orientation.
const YAW_RATE_PER_TICK: f32 = 1.5 / mgc_sim::TICK_RATE_HZ as f32; // was 1.2
const PITCH_RATE_PER_TICK: f32 = 0.8 / mgc_sim::TICK_RATE_HZ as f32;

/// Distance from the head at which the virtual UI panel is placed
/// (world units).  The pointer ray is intersected against this panel.
const POINTER_PANEL_DISTANCE: f32 = 0.5;
/// World units per UI pixel.  This is tuned so the panel is readable
/// at `POINTER_PANEL_DISTANCE`.
const POINTER_PANEL_SCALE: f32 = POINTER_PANEL_DISTANCE * 0.0015;

/// Controller-pointer state produced by `InputActions::poll` when
/// `grabbed == false`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PointerState {
    /// Pointer position in the same pixel space the UI shader uses
    /// (top-left origin, physical render-target pixels).
    pub screen_pos: Option<(f32, f32)>,
    /// World-space line segment from the controller to the panel hit.
    pub beam: Option<([f32; 3], [f32; 3])>,
    /// Whether the pointer hand's trigger is held this frame.
    pub click: bool,
}

pub struct InputActions {
    action_set: xr::ActionSet,
    left_hand: xr::Path,
    right_hand: xr::Path,
    left_aim: xr::Action<xr::Posef>,
    right_aim: xr::Action<xr::Posef>,
    left_aim_space: Option<xr::Space>,
    right_aim_space: Option<xr::Space>,
    left_stick: xr::Action<xr::Vector2f>,
    right_stick: xr::Action<xr::Vector2f>,
    trigger_left: xr::Action<f32>,
    trigger_right: xr::Action<f32>,
    btn_a: xr::Action<bool>,
    btn_b: xr::Action<bool>,
    btn_x: xr::Action<bool>,
    btn_y: xr::Action<bool>,
    menu_click: xr::Action<bool>,
    thumbstick_left_click: xr::Action<bool>,
    thumbstick_right_click: xr::Action<bool>,
    squeeze_left: xr::Action<f32>,
    squeeze_right: xr::Action<f32>,
    left_spell: u8,
    right_spell: u8,
    last_squeeze_left: bool,
    last_squeeze_right: bool,
    last_menu: bool,
    last_thumbstick_right_click: bool,
    last_thumbstick_left_click: bool,
    last_btn_x_click: bool,
    last_btn_y_click: bool,
    last_btn_b_time: i64,
    last_btn_b_click: bool,
    pointer: PointerState,
}

impl InputActions {
    pub fn new(instance: &xr::Instance) -> Result<Self, Box<dyn std::error::Error>> {
        let action_set = instance.create_action_set("gameplay", "Gameplay", 0)?;
        let left_hand = instance.string_to_path("/user/hand/left")?;
        let right_hand = instance.string_to_path("/user/hand/right")?;
        let left_aim = action_set.create_action("left_aim", "Left aim", &[left_hand])?;
        let right_aim = action_set.create_action("right_aim", "Right aim", &[right_hand])?;
        let left_stick = action_set.create_action("left_stick", "Left stick", &[])?;
        let right_stick = action_set.create_action("right_stick", "Right stick", &[])?;
        let trigger_left = action_set.create_action("trigger_left", "Cast left", &[])?;
        let trigger_right = action_set.create_action("trigger_right", "Cast right", &[])?;
        let btn_a = action_set.create_action("btn_a", "Respawn", &[])?;
        let btn_b = action_set.create_action("btn_b", "Demolish", &[])?;
        let btn_x = action_set.create_action("btn_x", "Button x", &[])?;
        let btn_y = action_set.create_action("btn_y", "Button y", &[])?;
        let thumbstick_left_click =
            action_set.create_action("left_thumbstick_click", "left thumbstick click", &[])?;
        let thumbstick_right_click =
            action_set.create_action("right_thumbstick_click", "right thumbstick click", &[])?;
        let menu_click = action_set.create_action("menu_click", "Menu", &[])?;
        let squeeze_left = action_set.create_action("squeeze_left", "Squeeze Left", &[])?;
        let squeeze_right = action_set.create_action("squeeze_right", "Squeeze Right", &[])?;

        instance.suggest_interaction_profile_bindings(
            instance.string_to_path("/interaction_profiles/oculus/touch_controller")?,
            &[
                xr::Binding::new(
                    &left_aim,
                    instance.string_to_path("/user/hand/left/input/aim/pose")?,
                ),
                xr::Binding::new(
                    &right_aim,
                    instance.string_to_path("/user/hand/right/input/aim/pose")?,
                ),
                xr::Binding::new(
                    &left_stick,
                    instance.string_to_path("/user/hand/left/input/thumbstick")?,
                ),
                xr::Binding::new(
                    &right_stick,
                    instance.string_to_path("/user/hand/right/input/thumbstick")?,
                ),
                xr::Binding::new(
                    &trigger_left,
                    instance.string_to_path("/user/hand/left/input/trigger/value")?,
                ),
                xr::Binding::new(
                    &trigger_right,
                    instance.string_to_path("/user/hand/right/input/trigger/value")?,
                ),
                xr::Binding::new(
                    &squeeze_left,
                    instance.string_to_path("/user/hand/left/input/squeeze/value")?,
                ),
                xr::Binding::new(
                    &squeeze_right,
                    instance.string_to_path("/user/hand/right/input/squeeze/value")?,
                ),
                xr::Binding::new(
                    &btn_a,
                    instance.string_to_path("/user/hand/right/input/a/click")?,
                ),
                xr::Binding::new(
                    &btn_b,
                    instance.string_to_path("/user/hand/right/input/b/click")?,
                ),
                xr::Binding::new(
                    &btn_x,
                    instance.string_to_path("/user/hand/left/input/x/click")?,
                ),
                xr::Binding::new(
                    &btn_y,
                    instance.string_to_path("/user/hand/left/input/y/click")?,
                ),
                xr::Binding::new(
                    &thumbstick_left_click,
                    instance.string_to_path("/user/hand/left/input/thumbstick/click")?,
                ),
                xr::Binding::new(
                    &thumbstick_right_click,
                    instance.string_to_path("/user/hand/right/input/thumbstick/click")?,
                ),
                xr::Binding::new(
                    &menu_click,
                    instance.string_to_path("/user/hand/left/input/menu/click")?,
                ),
            ],
        )?;

        Ok(Self {
            action_set,
            left_hand,
            right_hand,
            left_aim,
            right_aim,
            left_aim_space: None,
            right_aim_space: None,
            left_stick,
            right_stick,
            trigger_left,
            trigger_right,
            btn_a,
            btn_b,
            btn_x,
            btn_y,
            thumbstick_left_click,
            thumbstick_right_click,
            menu_click,
            squeeze_left,
            squeeze_right,
            left_spell: 0,    // Default it to fireball spell
            right_spell: 3,   // Default it to mana spell
            last_squeeze_left: false,
            last_squeeze_right: false,
            last_menu: false,
            last_thumbstick_right_click: false,
            last_thumbstick_left_click: false,
            last_btn_x_click: false,
            last_btn_y_click: false,
            last_btn_b_click: false,
            last_btn_b_time: 0,
            pointer: PointerState::default(),
        })
    }

    pub fn attach(
        &mut self,
        session: &xr::Session<xr::Vulkan>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        session.attach_action_sets(&[&self.action_set])?;
        self.left_aim_space = self
            .left_aim
            .create_space(session, self.left_hand, xr::Posef::IDENTITY)
            .ok();
        self.right_aim_space = self
            .right_aim
            .create_space(session, self.right_hand, xr::Posef::IDENTITY)
            .ok();
        Ok(())
    }

    /// The pointer state produced by the most recent `poll`.
    pub fn pointer(&self) -> &PointerState {
        &self.pointer
    }

    /// Syncs the action set and reads every action; call once per XR
    /// frame (not per sim tick — the same reading feeds however many
    /// sim ticks fire within one frame's accumulator burst).
    pub fn poll(
        &mut self,
        session: &xr::Session<xr::Vulkan>,
        stage_space: &xr::Space,
        display_time: xr::Time,
        flyer: &Flyer,
        screen_size: (f32, f32),
        owned: [bool; 26],
        bindable: [bool; 26],
        is_mc2: bool,
        grabbed: bool,
    ) -> FlightInput {
        let _ = session.sync_actions(&[(&self.action_set).into()]);

        // Controller pointer: when the cursor is free (grabbed == false)
        // raycast the right-hand aim pose against a virtual UI panel placed
        // in front of the player's head.
        self.pointer = PointerState::default();
        if !grabbed {
            if let (Ok((_, views)), Some(space)) = (
                session.locate_views(
                    xr::ViewConfigurationType::PRIMARY_STEREO,
                    display_time,
                    stage_space,
                ),
                &self.right_aim_space,
            ) {
                if let Some(head) = views.first() {
                    let panel = compute_panel(head, flyer, screen_size);
                    if let Ok(loc) = space.locate(stage_space, display_time) {
                        let needed = xr::SpaceLocationFlags::POSITION_VALID
                            | xr::SpaceLocationFlags::ORIENTATION_VALID;
                        if loc.location_flags.contains(needed) {
                            let pos_stage = loc.pose.position;
                            let q = loc.pose.orientation;
                            let fwd_stage = xr_init::quat_rotate(q, [0.0, 0.0, -1.0]);

                            let (sy, cy) = flyer.yaw.sin_cos();
                            let rot =
                                |v: [f32; 3]| [v[0] * cy - v[2] * sy, v[1], v[0] * sy + v[2] * cy];

                            let pos_world = add(
                                [flyer.x, flyer.y, flyer.z],
                                rot([pos_stage.x, pos_stage.y, pos_stage.z]),
                            );
                            let dir_world = rot(fwd_stage);

                            let n = cross(panel.right, panel.up);
                            let denom = dot(dir_world, n);
                            if denom.abs() > 1e-6 {
                                let t = dot(sub(panel.origin, pos_world), n) / denom;
                                if t >= 0.0 {
                                    let hit = add(pos_world, scale(dir_world, t));
                                    let to_hit = sub(hit, panel.origin);
                                    let ox = dot(to_hit, panel.right);
                                    let oy = dot(to_hit, panel.up);
                                    let center_x = screen_size.0 * 0.5;
                                    let center_y = screen_size.1 * 0.5;
                                    let px = center_x + ox / POINTER_PANEL_SCALE;
                                    let py = center_y + oy / POINTER_PANEL_SCALE;
                                    if px >= 0.0
                                        && px <= screen_size.0
                                        && py >= 0.0
                                        && py <= screen_size.1
                                    {
                                        self.pointer.screen_pos = Some((px, py));
                                        self.pointer.beam = Some((pos_world, hit));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let axis = |a: &xr::Action<xr::Vector2f>| {
            a.state(session, xr::Path::NULL)
                .map(|s| s.current_state)
                .unwrap_or(xr::Vector2f { x: 0.0, y: 0.0 })
        };
        let pressed = |a: &xr::Action<bool>| {
            a.state(session, xr::Path::NULL)
                .map(|s| s.current_state)
                .unwrap_or(false)
        };
        let value = |a: &xr::Action<f32>| {
            a.state(session, xr::Path::NULL)
                .map(|s| s.current_state)
                .unwrap_or(0.0)
        };
        if !grabbed {
            self.pointer.click = value(&self.trigger_right) > 0.5;
        }

        let next_spell = |spell: u8| {
            for i in spell + 1..26 {
                if owned[i as usize] && bindable[i as usize] {
                    return i;
                }
            }
            for i in 0..26 {
                if owned[i as usize] && bindable[i as usize] {
                    return i;
                }
            }
            128
        };

        let left = axis(&self.left_stick);
        let right = axis(&self.right_stick);

        let mut extra_data = 0;

        if pressed(&self.menu_click) && !self.last_menu {
            // Spell/Book
            extra_data |= 1;
        }
        if pressed(&self.btn_y) && !self.last_btn_y_click {
            // Pause Button
            extra_data |= 2;
        }

        // Pitch Delta would normally be on right.y; but pitch is VERY annoying in vr; so we are fixing it based on moving forward backwards.
        let pitch_delta = if left.y < 0.0 {
            0.3
        } else if (left.y > 0.0) {
            -0.3
        } else {
            0.0
        }; //  right.y * PITCH_RATE_PER_TICK;

        let mut equip_left = 128;
        let mut equip_right = 128;
        let mut mc2_select = None;

        if is_mc2 {
            if !self.last_squeeze_left && value(&self.squeeze_left) > 0.5 {
                self.left_spell = next_spell(self.left_spell);
                if self.left_spell < 128 {
                    mc2_select = Some((self.left_spell, 0, 0));
                }
            };
            if !self.last_squeeze_right && value(&self.squeeze_right) > 0.5 {
                self.right_spell = next_spell(self.right_spell);
                if self.right_spell < 128 {
                    mc2_select = Some((self.right_spell, 0, 1));
                }
            };
            if !self.last_thumbstick_right_click && pressed(&self.thumbstick_right_click) {
                if self.right_spell == 3 {
                    self.right_spell = 16; // castle spell
                } else {
                    self.right_spell = 3; // possess spell
                }
                mc2_select = Some((self.right_spell, 0, 1));
            }
            if !self.last_thumbstick_left_click && pressed(&self.thumbstick_left_click) {
                if self.left_spell == 0 {
                    self.left_spell = 15; // Lightning
                } else if self.left_spell == 15 {
                    self.left_spell = 7 // Meteor
                } else {
                    self.left_spell = 0; // Fireball
                }
                mc2_select = Some((self.left_spell, 0, 0));
            }


        } else {
            equip_left = if !self.last_squeeze_left && value(&self.squeeze_left) > 0.5 {
                self.left_spell = next_spell(self.left_spell);
                self.left_spell
            } else {
                128
            };
            equip_right = if !self.last_squeeze_right && value(&self.squeeze_right) > 0.5 {
                self.right_spell = next_spell(self.right_spell);
                self.right_spell
            } else {
                128
            };
            if !self.last_thumbstick_right_click && pressed(&self.thumbstick_right_click) {
                if self.right_spell == 3 {
                    self.right_spell = 16; // castle spell
                } else {
                    self.right_spell = 3; // possess spell
                }
                equip_right = self.right_spell;
            }

            if !self.last_thumbstick_left_click && pressed(&self.thumbstick_left_click) {
                if self.left_spell == 0 {
                    self.left_spell = 15; // Lightning
                } else if self.left_spell == 15 {
                    self.left_spell = 7 // Meteor
                } else {
                    self.left_spell = 0; // Fireball
                }
                equip_left = self.left_spell;
            }
        }


        let demolish = if pressed(&self.btn_b)&& !self.last_btn_b_click {
            // 300ms debounce for demolish button, to avoid accidental demolish
            if self.last_btn_b_time > 0 && display_time.as_nanos() - self.last_btn_b_time < 300_000_000 {
                true
            } else {
                self.last_btn_b_time = display_time.as_nanos();
                false
            }
        } else {
            false
        };

        self.last_squeeze_right = value(&self.squeeze_right) > 0.5;
        self.last_squeeze_left = value(&self.squeeze_left) > 0.5;
        self.last_menu = pressed(&self.menu_click);
        self.last_thumbstick_right_click = pressed(&self.thumbstick_right_click);
        self.last_thumbstick_left_click = pressed(&self.thumbstick_left_click);
        self.last_btn_x_click = pressed(&self.btn_x);
        self.last_btn_y_click = pressed(&self.btn_y);
        self.last_btn_b_click = pressed(&self.btn_b);

        FlightInput {
            thrust: left.y * 4.0,
            strafe: left.x * 4.0,
            yaw_delta: right.x * YAW_RATE_PER_TICK,
            pitch_delta,
            fire_left: value(&self.trigger_left) > 0.5,
            fire_right: value(&self.trigger_right) > 0.5,
            respawn: pressed(&self.btn_a),
            demolish,
            equip_left: (equip_left < 128).then(|| mgc_sim::mc1::spells::SpellId(equip_left)),
            equip_right: (equip_right < 128).then(|| mgc_sim::mc1::spells::SpellId(equip_right)),
            mc2_select,
            extra_data,
            ..Default::default()
        }
    }
}

struct Panel {
    origin: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
}

/// Place a virtual UI panel in world space based on the current head pose.
fn compute_panel(head: &xr::View, flyer: &Flyer, _screen_size: (f32, f32)) -> Panel {
    let head_pos_stage = head.pose.position;
    let head_q = head.pose.orientation;
    let head_fwd_stage = xr_init::quat_rotate(head_q, [0.0, 0.0, -1.0]);
    let head_right_stage = xr_init::quat_rotate(head_q, [1.0, 0.0, 0.0]);
    let head_up_stage = xr_init::quat_rotate(head_q, [0.0, 1.0, 0.0]);

    let (sy, cy) = flyer.yaw.sin_cos();
    let rot = |v: [f32; 3]| [v[0] * cy - v[2] * sy, v[1], v[0] * sy + v[2] * cy];

    let head_pos_world = add(
        [flyer.x, flyer.y, flyer.z],
        rot([head_pos_stage.x, head_pos_stage.y, head_pos_stage.z]),
    );
    let head_fwd_world = rot(head_fwd_stage);
    let head_right_world = rot(head_right_stage);
    let head_up_world = rot(head_up_stage);

    Panel {
        origin: add(
            head_pos_world,
            scale(head_fwd_world, POINTER_PANEL_DISTANCE),
        ),
        right: head_right_world,
        up: [-head_up_world[0], -head_up_world[1], -head_up_world[2]],
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
