//! `extern "C"` surface of the engine for the AudioWorklet (wasm32, no wasm-bindgen).
//! The host talks to linear memory: it reads the output buffers and telemetry by pointer.

use engine::{Command, Engine, Gesture, MAX_BLOCK};

// SAFETY invariant for every access below: the AudioWorklet runs the module on a single thread
// and never re-enters an export, so these statics are never accessed concurrently or aliased.
// All `unsafe` in this crate is confined to the three accessors that follow.
static mut ENGINE: Option<Engine> = None;
static mut OUT_L: [f32; MAX_BLOCK] = [0.0; MAX_BLOCK];
static mut OUT_R: [f32; MAX_BLOCK] = [0.0; MAX_BLOCK];

// clippy's suggested `&mut ENGINE` would trip the static_mut_refs lint; the raw form is intended.
#[allow(clippy::deref_addrof)]
fn engine_slot() -> &'static mut Option<Engine> {
    // SAFETY: single-threaded, non-reentrant host (see invariant above); the reference is
    // dropped before the next export is called.
    unsafe { &mut *(&raw mut ENGINE) }
}

fn out_bufs(n: usize) -> (&'static mut [f32], &'static mut [f32]) {
    debug_assert!(n <= MAX_BLOCK);
    let n = n.min(MAX_BLOCK);
    // SAFETY: single-threaded (see invariant); two distinct statics, `n` bounded by MAX_BLOCK.
    unsafe {
        (
            core::slice::from_raw_parts_mut((&raw mut OUT_L).cast::<f32>(), n),
            core::slice::from_raw_parts_mut((&raw mut OUT_R).cast::<f32>(), n),
        )
    }
}

fn out_ptr(right: bool) -> *const f32 {
    // Taking the address of a static needs no dereference, hence no unsafe.
    if right {
        (&raw const OUT_R).cast::<f32>()
    } else {
        (&raw const OUT_L).cast::<f32>()
    }
}

/// (Re)creates the engine. The old one is dropped first so repeated inits do not grow memory.
#[no_mangle]
pub extern "C" fn engine_init(sample_rate: f32, seed: u32) {
    let slot = engine_slot();
    *slot = None;
    *slot = Some(Engine::new(sample_rate, seed));
}

/// kind: 0 Gesture(a=x,b=y,c=z,d=pinch), 1 SwitchWorld(a), 2 Seed(a), 3 Tempo(a),
/// 4 Hold(a != 0), 5 Lock(a != 0). Unknown kinds are ignored.
#[no_mangle]
pub extern "C" fn engine_command(kind: u32, a: f32, b: f32, c: f32, d: f32) {
    let cmd = match kind {
        0 => Command::Gesture(Gesture { x: a, y: b, z: c, pinch: d }),
        1 => Command::SwitchWorld(a as u32),
        2 => Command::Seed(a as u32),
        3 => Command::Tempo(a),
        4 => Command::Hold(a != 0.0),
        5 => Command::Lock(a != 0.0),
        _ => return,
    };
    if let Some(e) = engine_slot() {
        e.push(cmd);
    }
}

/// Renders `frames` samples into the output buffers. 0 does nothing; more than MAX_BLOCK is
/// clamped to MAX_BLOCK. Does nothing before `engine_init`.
#[no_mangle]
pub extern "C" fn engine_process(frames: u32) {
    let n = (frames as usize).min(MAX_BLOCK);
    if n == 0 {
        return;
    }
    if let Some(e) = engine_slot() {
        let (l, r) = out_bufs(n);
        e.process(l, r);
    }
}

#[no_mangle]
pub extern "C" fn engine_out_l() -> *const f32 {
    out_ptr(false)
}

#[no_mangle]
pub extern "C" fn engine_out_r() -> *const f32 {
    out_ptr(true)
}

/// Pointer to the telemetry struct, or null before `engine_init`.
#[no_mangle]
pub extern "C" fn engine_telemetry() -> *const u8 {
    match engine_slot() {
        Some(e) => (e.telemetry() as *const engine::Telemetry).cast::<u8>(),
        None => core::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn engine_telemetry_size() -> u32 {
    core::mem::size_of::<engine::Telemetry>() as u32
}

/// Marks the telemetry steps as consumed by the host.
#[no_mangle]
pub extern "C" fn engine_ack() {
    if let Some(e) = engine_slot() {
        e.ack_steps();
    }
}
