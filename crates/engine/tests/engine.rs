use engine::{Command, Engine, Gesture, World, MAX_BLOCK};
use versum_dsp::testkit::{count_allocs, CountingAlloc};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

const SR: f32 = 48000.0;

fn g(x: f32, y: f32) -> Command {
    Command::Gesture(Gesture {
        x,
        y,
        z: 0.0,
        pinch: 0.0,
    })
}

fn render(e: &mut Engine, n: usize, block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut ol, mut or) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
    let mut done = 0;
    while done < n {
        let len = block.min(n - done);
        e.process(&mut l[..len], &mut r[..len]);
        ol.extend_from_slice(&l[..len]);
        or.extend_from_slice(&r[..len]);
        done += len;
    }
    (ol, or)
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn blockwise_equals_per_sample() {
    let mut a = Engine::new(SR, 7);
    let mut b = Engine::new(SR, 7);
    a.push(g(0.4, 0.9));
    b.push(g(0.4, 0.9));
    let (al, ar) = render(&mut a, 20000, MAX_BLOCK);
    let (bl, br) = render(&mut b, 20000, 1);
    assert_eq!(bits(&al), bits(&bl));
    assert_eq!(bits(&ar), bits(&br));
    assert!(al.iter().any(|&x| x != 0.0));
}

#[test]
fn no_alloc_in_process() {
    let mut e = Engine::new(SR, 3);
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    let (_, n) = count_allocs(|| {
        for i in 0..1000 {
            e.push(g((i % 10) as f32 / 10.0, 0.8));
            match i % 250 {
                100 => e.push(Command::Seed(i as u32)),
                150 => e.push(Command::SwitchWorld(0)),
                200 => e.push(Command::SwitchWorld(1)),
                _ => {}
            }
            e.process(&mut l, &mut r);
        }
    });
    assert_eq!(n, 0);
}

#[test]
fn step_lands_on_exact_sample() {
    let mut e = Engine::new(SR, 1);
    e.push(Command::SwitchWorld(0));
    e.push(Command::Tempo(120.0));
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    let mut out = Vec::new();
    let mut at_45 = 0;
    let mut at_46 = (0, 0);
    for blk in 0..=46 {
        e.process(&mut l, &mut r);
        out.extend_from_slice(&l);
        if blk == 45 {
            at_45 = e.telemetry().step_count;
        }
        if blk == 46 {
            let t = e.telemetry();
            at_46 = (t.step_count, t.steps[t.step_count as usize - 1].index);
        }
    }
    assert_eq!(at_45, 1, "only step 0 before sample 6016");
    assert_eq!(
        at_46,
        (2, 1),
        "step index 1 after block covering sample 6000"
    );
    let first = out
        .iter()
        .enumerate()
        .skip(1000)
        .find(|(_, x)| x.abs() > 0.01)
        .map(|(i, _)| i);
    assert_eq!(first, Some(6000));
}

#[test]
fn outputs_finite_and_bounded() {
    let mut e = Engine::new(SR, 5);
    let mut rng = versum_dsp::Rng::new(99);
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    for _ in 0..(10 * 48000 / MAX_BLOCK) {
        e.push(g(rng.next_f64() as f32, rng.next_f64() as f32));
        e.process(&mut l, &mut r);
        for &s in l.iter().chain(r.iter()) {
            assert!(s.is_finite() && s.abs() <= 1.0, "{s}");
        }
        let t = e.telemetry();
        assert!(t.peak_l <= 1.0 && t.peak_r <= 1.0);
    }
}

#[test]
fn command_queue_overflow_is_counted() {
    let mut e = Engine::new(SR, 1);
    for i in 0..100 {
        e.push(g(i as f32 / 100.0, 0.5));
    }
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    e.process(&mut l, &mut r);
    assert_eq!(e.telemetry().dropped_commands, 36);
    // the queue is free again after the block
    e.push(g(0.1, 0.1));
    e.process(&mut l, &mut r);
    assert_eq!(e.telemetry().dropped_commands, 36);
}

#[test]
fn step_queue_overflow_keeps_newest_and_counts() {
    let mut e = Engine::new(SR, 1);
    e.push(Command::Tempo(120.0));
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    for _ in 0..937 {
        e.process(&mut l, &mut r); // steps 0..=19 (sample 114000 < 119936)
    }
    let t = e.telemetry();
    assert_eq!(t.step_count, 16);
    assert_eq!(t.dropped_steps, 4);
    assert_eq!(t.steps[15].index, 19);
    assert_eq!(t.steps[0].index, 4);
    assert!(t.steps.windows(2).all(|w| w[0].index < w[1].index));
    e.ack_steps();
    assert_eq!(
        (e.telemetry().step_count, e.telemetry().dropped_steps),
        (0, 0)
    );
}

#[test]
fn peaks_decay_after_silence() {
    let mut e = Engine::new(SR, 1);
    e.push(Command::SwitchWorld(0));
    e.push(g(0.5, 1.0));
    render(&mut e, 48000, MAX_BLOCK);
    assert!(e.telemetry().peak_l > 0.1);
    e.push(g(0.5, 0.0));
    e.push(Command::Hold(true));
    render(&mut e, 5 * 14400, MAX_BLOCK);
    assert!(e.telemetry().peak_l < 0.01, "{}", e.telemetry().peak_l);
    assert!(e.telemetry().peak_r < 0.01);
}

#[test]
fn switch_world_and_seed_reset_state() {
    let mut e = Engine::new(SR, 9);
    let mut take = |c: Command| {
        e.push(c);
        e.push(g(0.3, 0.8));
        render(&mut e, 4 * MAX_BLOCK, MAX_BLOCK)
    };
    let o1 = take(Command::Seed(1));
    let o2 = take(Command::Seed(2));
    let o3 = take(Command::Seed(1));
    assert_ne!(bits(&o1.0), bits(&o2.0));
    assert_eq!(bits(&o1.0), bits(&o3.0));
    assert_eq!(bits(&o1.1), bits(&o3.1));
    let o4 = take(Command::SwitchWorld(0));
    let o5 = take(Command::SwitchWorld(0));
    assert_eq!(bits(&o4.0), bits(&o5.0));
}

#[test]
fn frame_counts_samples() {
    let mut e = Engine::new(SR, 1);
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    for _ in 0..4 {
        e.process(&mut l, &mut r);
    }
    assert_eq!(e.telemetry().frame, 512);
    e.process(&mut l[..1], &mut r[..1]);
    assert_eq!(e.telemetry().frame, 513);
    e.push(Command::Seed(3));
    e.process(&mut l[..1], &mut r[..1]);
    assert_eq!(e.telemetry().frame, 514, "Seed does not reset frame");
}

struct CountWorld {
    taps: std::rc::Rc<std::cell::Cell<u32>>,
    fills: std::rc::Rc<std::cell::Cell<u32>>,
}

impl World for CountWorld {
    fn on_gesture(&mut self, _: &Gesture) {}
    fn on_step(&mut self, _: &engine::StepEvent) {}
    fn render(&mut self, _: &mut [f32], _: &mut [f32]) {}
    fn reset(&mut self, _: u32) {}
    fn on_ptap(&mut self) {
        self.taps.set(self.taps.get() + 1);
    }
    fn fill_telemetry(&self, t: &mut engine::Telemetry) {
        self.fills.set(self.fills.get() + 1);
        t.bpm = 77.0;
        t.pattern[15] = 1.0;
    }
}

#[test]
fn ptap_is_queued() {
    let taps = std::rc::Rc::new(std::cell::Cell::new(0));
    let fills = std::rc::Rc::new(std::cell::Cell::new(0));
    let w = CountWorld {
        taps: taps.clone(),
        fills: fills.clone(),
    };
    let mut e = Engine::with_world(SR, Box::new(w), 1);
    e.push(Command::PTap);
    e.push(Command::PTap);
    assert_eq!(taps.get(), 0, "applied only in process");
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    e.process(&mut l, &mut r);
    assert_eq!(taps.get(), 2);
    assert_eq!(fills.get(), 1, "fill_telemetry once per process");
    assert_eq!(e.telemetry().bpm, 77.0);
    assert_eq!(e.telemetry().pattern[15], 1.0);
    assert_eq!(e.telemetry().version, 2);
}

fn collect_steps(e: &mut Engine, n: usize, block: usize) -> Vec<(u64, engine::StepEvent)> {
    // (frame at the end of the block that reported the step, event)
    let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
    let mut out = Vec::new();
    let mut done = 0;
    while done < n {
        let len = block.min(n - done);
        e.process(&mut l[..len], &mut r[..len]);
        let t = e.telemetry();
        for s in &t.steps[..t.step_count as usize] {
            out.push((t.frame as u64, *s));
        }
        e.ack_steps();
        done += len;
    }
    out
}

#[test]
fn steps_follow_melody() {
    let seed = 12345;
    let mut e = Engine::new(SR, seed);
    let mut w = engine::amber::AmberWorld::new(SR, seed);
    let bpm = w.melody().bpm as f64;
    let bar = (SR as f64 * 60.0 / bpm * 4.0) as usize;
    let steps = collect_steps(&mut e, 4 * bar, MAX_BLOCK);
    assert!(steps.len() >= 64);
    for (_, s) in &steps {
        let m = World::next_step(&mut w, s.index);
        assert_eq!(
            (s.note, s.velocity),
            (m.note, m.velocity),
            "step {}",
            s.index
        );
        assert!(
            m.velocity == 0.0 || (24..=47).contains(&m.note),
            "note {}",
            m.note
        );
    }
    assert_eq!(steps[0].1.velocity, 1.0);
}

#[test]
fn step_boundaries_exact_at_bpm_110_122_135() {
    for bpm in [110.0f32, 122.0, 135.0] {
        let mut e = Engine::new(SR, 4);
        e.push(Command::Tempo(bpm));
        let period = SR as f64 * 60.0 / bpm as f64 / 4.0;
        let steps = collect_steps(&mut e, 5 * 48000, MAX_BLOCK);
        assert!(steps.len() > 20);
        for (frame, s) in &steps {
            let at = (s.index as f64 * period).ceil() as u64;
            let blk_end = (at / 128 + 1) * 128;
            assert_eq!(*frame, blk_end, "bpm {bpm} step {}", s.index);
        }
        // sample-exact with 1-sample blocks
        let mut e = Engine::new(SR, 4);
        e.push(Command::Tempo(bpm));
        for (frame, s) in &collect_steps(&mut e, 48000, 1) {
            assert_eq!(*frame, (s.index as f64 * period).ceil() as u64 + 1);
        }
    }
}

#[test]
fn seed_adopts_melody_bpm() {
    let mut e = Engine::new(SR, 1);
    e.push(Command::Tempo(200.0));
    render(&mut e, 1000, MAX_BLOCK);
    let seed = 99;
    e.push(Command::Seed(seed));
    let bpm = engine::amber::AmberWorld::new(SR, seed).melody().bpm as f64;
    assert!((bpm - 120.0).abs() > 0.5);
    let steps = collect_steps(&mut e, 48000, 1);
    let period = SR as f64 * 60.0 / bpm / 4.0;
    assert!(steps.len() >= 4);
    assert_eq!(steps[0].0, 1001, "step 0 plays at once");
    for (frame, s) in &steps {
        assert_eq!(*frame, (s.index as f64 * period).ceil() as u64 + 1 + 1000);
    }
}

#[test]
fn hold_and_tempo_keep_phase() {
    let mut e = Engine::new(SR, 6);
    e.push(Command::Tempo(120.0));
    let period = SR as f64 * 60.0 / 120.0 / 4.0;
    let mut steps = collect_steps(&mut e, 10000, MAX_BLOCK);
    e.push(Command::Hold(true));
    let held = collect_steps(&mut e, 48000, MAX_BLOCK);
    assert!(held.is_empty(), "no steps while held");
    e.push(Command::Hold(false));
    steps.extend(collect_steps(&mut e, 10000, MAX_BLOCK));
    let idx: Vec<u32> = steps.iter().map(|s| s.1.index).collect();
    assert_eq!(
        idx,
        (0..idx.len() as u32).collect::<Vec<_>>(),
        "no duplicates or gaps"
    );
    // step 2 is due 10752 samples of running time after step 0 = 2*period
    // the first 10000 samples plus 48000 held samples: step 2 lands right after resume
    let s2 = steps.iter().find(|s| s.1.index == 2).unwrap().0;
    let due = (2.0 * period).ceil() as u64 - 10000;
    assert_eq!(s2, 10000 + 48000 + (due / 128 + 1) * 128);

    // Tempo mid-bar keeps the pending step's deadline
    let mut e = Engine::new(SR, 6);
    e.push(Command::Tempo(120.0));
    let mut st = collect_steps(&mut e, 3000, MAX_BLOCK);
    e.push(Command::Tempo(240.0));
    st.extend(collect_steps(&mut e, 20000, MAX_BLOCK));
    let s1 = st.iter().find(|s| s.1.index == 1).unwrap().0;
    let rep = |at: u64| 3000 + ((at - 3000) / 128 + 1) * 128; // block-end frame (blocks restart at 3000)
    assert_eq!(s1, rep(period.ceil() as u64), "pending step unmoved");
    let s2 = st.iter().find(|s| s.1.index == 2).unwrap().0;
    let due2 = (period + period / 2.0).ceil() as u64;
    assert_eq!(s2, rep(due2), "next one uses new period");
}

#[test]
fn seed_u32_reproducible() {
    for seed in [0u32, 16777217, 4294967295] {
        let run = || {
            let mut e = Engine::new(SR, 1);
            e.push(Command::Seed(seed));
            e.push(g(0.6, 0.7));
            render(&mut e, 30000, MAX_BLOCK)
        };
        let (a, b) = (run(), run());
        assert_eq!(bits(&a.0), bits(&b.0));
        assert_eq!(bits(&a.1), bits(&b.1));
        assert!(a.0.iter().any(|&x| x != 0.0), "seed {seed} is audible");
    }
    // 16777217 differs from 16777216 (not squashed through f32)
    let at = |s| {
        let mut e = Engine::new(SR, 1);
        e.push(Command::Seed(s));
        render(&mut e, 30000, MAX_BLOCK).0
    };
    assert_ne!(bits(&at(16777216)), bits(&at(16777217)));
}

#[test]
fn switch_world_during_note_is_clean() {
    let mut e = Engine::new(SR, 8);
    e.push(g(0.5, 0.6));
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    let mut last_frame = 0;
    let mut energy = [0.0f64; 4]; // before 100, 100..200, 200..300, 300..400
    for i in 0..400 {
        match i {
            100 => e.push(Command::SwitchWorld(0)),
            150 => e.push(Command::Seed(77)),
            200 => e.push(Command::SwitchWorld(1)),
            250 => e.push(Command::Seed(78)),
            _ => {}
        }
        e.process(&mut l, &mut r);
        for &s in l.iter().chain(r.iter()) {
            assert!(s.is_finite() && s.abs() <= 1.0, "block {i}: {s}");
        }
        energy[i / 100] += l.iter().map(|&x| (x as f64) * (x as f64)).sum::<f64>();
        let t = e.telemetry();
        assert!(t.peak_l <= 1.0 && t.peak_r <= 1.0);
        assert_eq!(t.frame, last_frame + 128, "frame continues at block {i}");
        last_frame = t.frame;
    }
    // sound before the switch, and the new worlds keep sounding (not stuck silent)
    assert!(energy[0] > 1.0, "{energy:?}");
    assert!(
        energy[1] > 1.0 && energy[2] > 1.0 && energy[3] > 1.0,
        "{energy:?}"
    );
}

#[test]
fn nonfinite_gesture_is_safe() {
    for world in [0u32, 1] {
        let mut e = Engine::new(SR, 2);
        e.push(Command::SwitchWorld(world));
        let bad = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
        let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
        for i in 0..300 {
            let v = bad[i % 3];
            e.push(Command::Gesture(Gesture {
                x: v,
                y: bad[(i + 1) % 3],
                z: bad[(i + 2) % 3],
                pinch: v,
            }));
            e.process(&mut l, &mut r);
            for &s in l.iter().chain(r.iter()) {
                assert!(s.is_finite() && s.abs() <= 1.0, "world {world}: {s}");
            }
        }
    }
}

#[test]
fn engine_starts_in_amber() {
    let mut e = Engine::new(SR, 5);
    render(&mut e, 128, MAX_BLOCK);
    let t = e.telemetry();
    assert!(t.bpm > 0.0, "amber fills telemetry");
    assert!(t.pattern.iter().any(|&p| p > 0.0));
}
