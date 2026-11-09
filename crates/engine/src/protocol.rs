//! Wire protocol between the host (UI / audio thread glue) and the engine.
//! All types are `#[repr(C)]` and `Copy`; nothing here allocates.

pub const MAX_BLOCK: usize = 128;
pub const CMD_QUEUE: usize = 64;
pub const STEP_QUEUE: usize = 16;
pub const PATTERN_LEN: usize = 16;
pub const PROTOCOL_VERSION: u32 = 2;

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
    PTap,
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
    pub bpm: f32,
    pub root: u32,
    pub scale: u32,
    pub hue: f32,
    pub brightness: f32,
    pub activity: f32,
    pub gate_env: f32,
    pub pattern: [f32; PATTERN_LEN],
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
            bpm: 0.0,
            root: 0,
            scale: 0,
            hue: 0.0,
            brightness: 0.0,
            activity: 0.0,
            gate_env: 0.0,
            pattern: [0.0; PATTERN_LEN],
        }
    }
}

/// A sound world. After construction it must not allocate or block.
pub trait World {
    fn on_gesture(&mut self, g: &Gesture);
    fn on_step(&mut self, s: &StepEvent);
    /// The step at sequence position `index` (velocity 0 is a rest); the engine plays it via `on_step`.
    fn next_step(&mut self, index: u32) -> StepEvent {
        StepEvent {
            index,
            note: 60,
            velocity: 1.0,
        }
    }
    /// Adds this world's sound into `l`/`r` (the engine zeroes them first).
    fn render(&mut self, l: &mut [f32], r: &mut [f32]);
    /// Returns the world to its initial state, deterministically from `seed`.
    fn reset(&mut self, seed: u32);
    /// Pattern-tap from the host; worlds that have no use for it ignore it.
    fn on_ptap(&mut self) {}
    /// Fills the world-owned telemetry fields (bpm..pattern); called once per `process`.
    fn fill_telemetry(&self, _t: &mut Telemetry) {}
}

// Layout guard: web/audio/protocol.ts mirrors these sizes and offsets. Change both together.
const _: () = {
    use core::mem::{align_of, offset_of, size_of};
    assert!(size_of::<Telemetry>() == 312);
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
    assert!(offset_of!(Telemetry, bpm) == 220);
    assert!(offset_of!(Telemetry, root) == 224);
    assert!(offset_of!(Telemetry, scale) == 228);
    assert!(offset_of!(Telemetry, hue) == 232);
    assert!(offset_of!(Telemetry, brightness) == 236);
    assert!(offset_of!(Telemetry, activity) == 240);
    assert!(offset_of!(Telemetry, gate_env) == 244);
    assert!(offset_of!(Telemetry, pattern) == 248);
};
