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
const YAW_RATE_PER_TICK: f32 = 1.2 / mgc_sim::TICK_RATE_HZ as f32;
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
        let btn_x = action_set.create_action("btn_x", "Equip left (dev)", &[])?;
        let btn_y = action_set.create_action("btn_y", "Equip right (dev)", &[])?;

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
    pub fn poll(&self, session: &xr::Session<xr::Vulkan>) -> FlightInput {
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

        FlightInput {
            thrust: left.y,
            strafe: left.x,
            yaw_delta: right.x * YAW_RATE_PER_TICK,
            pitch_delta: right.y * PITCH_RATE_PER_TICK,
            fire_left: value(&self.trigger_left) > 0.5,
            fire_right: value(&self.trigger_right) > 0.5,
            respawn: pressed(&self.btn_a),
            demolish: pressed(&self.btn_b),
            equip_left: pressed(&self.btn_x).then(|| mgc_sim::mc1::spells::SpellId(0)),
            equip_right: pressed(&self.btn_y).then(|| mgc_sim::mc1::spells::SpellId(0)),
            ..Default::default()
        }
    }
}
