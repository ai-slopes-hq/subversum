//! The engine lives in one process-wide static, and cargo runs tests on parallel threads,
//! so every test takes `LOCK` first to serialize access (the real host is single-threaded).
use engine::{Command, Engine, Telemetry, MAX_BLOCK};
use std::sync::{Mutex, MutexGuard};
use versum_dsp::testkit::{count_allocs, CountingAlloc};
use versum_wasm::*;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

fn tel() -> Telemetry {
    unsafe { *(engine_telemetry() as *const Telemetry) }
}

fn tel_bytes() -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(engine_telemetry(), engine_telemetry_size() as usize).to_vec() }
}

fn out(p: *const f32, n: usize) -> Vec<f32> {
    unsafe { std::slice::from_raw_parts(p, n).to_vec() }
}

#[test]
fn process_zero_and_oversized_frames_are_safe() {
    let _g = lock();
    // The rendered output is checked against a reference engine; `frame` counts samples.
    let mut reference = Engine::new(48000.0, 1);
    let (mut rl, mut rr) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    engine_init(48000.0, 1);
    engine_command(0, 0.5, 0.9, 0.0, 0.0);
    reference.push(Command::Gesture(engine::Gesture { x: 0.5, y: 0.9, z: 0.0, pinch: 0.0 }));
    // The buffers are statics and may hold output from an earlier test: 0 must leave them as is.
    let before = out(engine_out_l(), MAX_BLOCK);
    engine_process(0);
    assert_eq!(tel().frame, 0);
    assert_eq!(out(engine_out_l(), MAX_BLOCK), before);
    for frames in [100_000, u32::MAX, 129] {
        reference.process(&mut rl, &mut rr);
        engine_process(frames);
        assert_eq!(out(engine_out_l(), MAX_BLOCK), rl.to_vec());
        assert_eq!(out(engine_out_r(), MAX_BLOCK), rr.to_vec());
        assert!(rl.iter().all(|x| x.is_finite()));
    }
    assert_eq!(tel().frame, 3 * MAX_BLOCK as u32);
    assert!(rl.iter().any(|&x| x != 0.0));
}

#[test]
fn init_twice_does_not_leak_or_keep_state() {
    let _g = lock();
    engine_init(48000.0, 1);
    engine_command(0, 0.5, 0.9, 0.0, 0.0);
    for _ in 0..10 {
        engine_process(128);
    }
    assert!(tel().frame > 0);
    engine_init(48000.0, 1);
    assert_eq!(tel().frame, 0);
    let (_, n2) = count_allocs(|| engine_init(48000.0, 1));
    engine_process(128);
    let (_, n3) = count_allocs(|| engine_init(48000.0, 1));
    assert!(n3 <= n2, "allocs grew: {n2} -> {n3}");
    assert_eq!(tel().frame, 0);
}

#[test]
fn telemetry_size_matches_struct() {
    let _g = lock();
    assert_eq!(engine_telemetry_size() as usize, std::mem::size_of::<Telemetry>());
}

#[test]
fn command_kinds_round_trip() {
    let _g = lock();
    let mut reference = Engine::new(48000.0, 5);
    reference.push(Command::Tempo(90.0));
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    engine_init(48000.0, 5);
    engine_command(3, 90.0, 0.0, 0.0, 0.0);
    for _ in 0..200 {
        reference.process(&mut l, &mut r);
        engine_process(128);
    }
    let want = unsafe {
        std::slice::from_raw_parts(reference.telemetry() as *const Telemetry as *const u8, tel_bytes().len())
    };
    assert_eq!(tel_bytes(), want);
    assert!(tel().step_count > 0);
    // And the tempo really differs from the default.
    engine_init(48000.0, 5);
    for _ in 0..200 {
        engine_process(128);
    }
    assert_ne!(tel_bytes(), want);
    engine_ack();
    assert_eq!(tel().step_count, 0);
}
