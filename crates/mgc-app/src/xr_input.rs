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

use mgc_sim::FlightInput;
use openxr as xr;

/// Turn rate at full stick deflection, radians/tick (24 Hz sim) — a
/// traditional flight-stick feel, independent of head orientation.
const YAW_RATE_PER_TICK: f32 = 1.5 / mgc_sim::TICK_RATE_HZ as f32; // was 1.2
const PITCH_RATE_PER_TICK: f32 = 0.8 / mgc_sim::TICK_RATE_HZ as f32;

pub struct InputActions {
    action_set: xr::ActionSet,
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
}

impl InputActions {
    pub fn new(instance: &xr::Instance) -> Result<Self, Box<dyn std::error::Error>> {
        let action_set = instance.create_action_set("gameplay", "Gameplay", 0)?;
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
            left_spell: 0,
            right_spell: 0,
            last_squeeze_left: false,
            last_squeeze_right: false,
            last_menu: false,
        })
    }

    pub fn attach(
        &self,
        session: &xr::Session<xr::Vulkan>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        session.attach_action_sets(&[&self.action_set])?;
        Ok(())
    }

    /// Syncs the action set and reads every action; call once per XR
    /// frame (not per sim tick — the same reading feeds however many
    /// sim ticks fire within one frame's accumulator burst).
    pub fn poll(&mut self, session: &xr::Session<xr::Vulkan>, owned: [bool; 24]) -> FlightInput {
        let _ = session.sync_actions(&[(&self.action_set).into()]);

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

        let left = axis(&self.left_stick);
        let right = axis(&self.right_stick);

        let mut extra_data = 0;

        if pressed(&self.menu_click) && !self.last_menu {
            extra_data |= 1;
        }

        let pitch_delta = if left.y < 0.0 {
            0.3
        } else if (left.y > 0.0) {
            -0.3
        } else {
            0.0
        };

        let flight_input = FlightInput {
            thrust: left.y * 4.0,
            strafe: left.x * 4.0,
            yaw_delta: right.x * YAW_RATE_PER_TICK,
            pitch_delta: pitch_delta, // right.y * PITCH_RATE_PER_TICK,
            fire_left: value(&self.trigger_left) > 0.5,
            fire_right: value(&self.trigger_right) > 0.5,
            respawn: pressed(&self.btn_a),
            demolish: pressed(&self.btn_b),
            equip_left: (!self.last_squeeze_left && value(&self.squeeze_left) > 0.5).then(|| {
                self.left_spell += 1;
                if self.left_spell >= 24 {
                    self.left_spell = 0;
                }
                mgc_sim::mc1::spells::SpellId(self.left_spell)
            }),
            equip_right: (!self.last_squeeze_right && value(&self.squeeze_right) > 0.5).then(
                || {
                    self.right_spell += 1;
                    if self.right_spell >= 24 {
                        self.right_spell = 0;
                    }
                    mgc_sim::mc1::spells::SpellId(self.right_spell)
                },
            ),
            extra_data,
            ..Default::default()
        };

        self.last_squeeze_right = value(&self.squeeze_right) > 0.5;
        self.last_squeeze_left = value(&self.squeeze_left) > 0.5;
        self.last_menu = pressed(&self.menu_click);

        flight_input
    }
}
