//! Wire protocol between the host (UI / audio thread glue) and the engine.
//! All types are `#[repr(C)]` and `Copy`; nothing here allocates.

pub const MAX_BLOCK: usize = 128;
pub const CMD_QUEUE: usize = 64;
pub const STEP_QUEUE: usize = 16;
pub const PROTOCOL_VERSION: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gesture {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub pinch: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Gesture(Gesture),
    SwitchWorld(u32),
    Seed(u32),
    Tempo(f32),
    Hold(bool),
    Lock(bool),
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepEvent {
    pub index: u32,
    pub note: u32,
    pub velocity: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Telemetry {
    pub version: u32,
    /// Samples rendered since engine creation (not reset by Seed/SwitchWorld).
    pub frame: u32,
    pub peak_l: f32,
    pub peak_r: f32,
    /// Valid entries in `steps` (the newest ones, oldest first).
    pub step_count: u32,
    pub dropped_steps: u32,
    pub dropped_commands: u32,
    pub steps: [StepEvent; STEP_QUEUE],
}

impl Default for Telemetry {
    fn default() -> Telemetry {
        Telemetry {
            version: PROTOCOL_VERSION,
            frame: 0,
            peak_l: 0.0,
            peak_r: 0.0,
            step_count: 0,
            dropped_steps: 0,
            dropped_commands: 0,
            steps: [StepEvent::default(); STEP_QUEUE],
        }
    }
}

/// A sound world. After construction it must not allocate or block.
pub trait World {
    fn on_gesture(&mut self, g: &Gesture);
    fn on_step(&mut self, s: &StepEvent);
    /// Adds this world's sound into `l`/`r` (the engine zeroes them first).
    fn render(&mut self, l: &mut [f32], r: &mut [f32]);
    /// Returns the world to its initial state, deterministically from `seed`.
    fn reset(&mut self, seed: u32);
}

// Layout guard: web/audio/protocol.ts mirrors these sizes and offsets. Change both together.
const _: () = {
    use core::mem::{align_of, offset_of, size_of};
    assert!(size_of::<Telemetry>() == 220);
    assert!(size_of::<StepEvent>() == 12);
    assert!(align_of::<Telemetry>() == 4);
    assert!(offset_of!(Telemetry, version) == 0);
    assert!(offset_of!(Telemetry, frame) == 4);
    assert!(offset_of!(Telemetry, peak_l) == 8);
    assert!(offset_of!(Telemetry, peak_r) == 12);
    assert!(offset_of!(Telemetry, step_count) == 16);
    assert!(offset_of!(Telemetry, dropped_steps) == 20);
    assert!(offset_of!(Telemetry, dropped_commands) == 24);
    assert!(offset_of!(Telemetry, steps) == 28);
};
