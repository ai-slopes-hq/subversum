use engine::amber::AmberWorld;
use engine::{Gesture, StepEvent, Telemetry, World, MAX_BLOCK};
use versum_dsp::testkit::{count_allocs, CountingAlloc};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

const SR: f32 = 48000.0;

fn note(index: u32, note: u32, velocity: f32) -> StepEvent {
    StepEvent {
        index,
        note,
        velocity,
    }
}

/// Renders `n` samples in blocks of `block`, calling `at(sample_index, world)` before each block start
/// (events scheduled on block boundaries only).
fn render(w: &mut AmberWorld, n: usize, block: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n);
    let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
    let mut done = 0;
    while done < n {
        let b = block.min(n - done);
        l[..b].fill(0.0);
        r[..b].fill(0.0);
        w.render(&mut l[..b], &mut r[..b]);
        out.extend_from_slice(&l[..b]);
        done += b;
    }
    out
}

/// Plays a scripted performance: step events at fixed sample positions, rendered with the given block size.
/// Event positions are multiples of 1 and 17 and 128 (lcm), so every block size sees them on a block edge.
fn perform(seed: u32, block: usize) -> Vec<f32> {
    let mut w = AmberWorld::new(SR, seed);
    let unit = 17 * 128; // 2176 samples
    let mut out = Vec::new();
    w.on_gesture(&Gesture {
        x: 0.7,
        y: 0.3,
        z: 0.0,
        pinch: 0.4,
    });
    for k in 0..46u32 {
        let st = w.next_step(k);
        w.on_step(&st);
        if k == 20 {
            w.on_ptap();
        }
        out.extend(render(&mut w, unit, block));
    }
    out
}

#[test]
fn deterministic_by_seed() {
    let a = perform(5, 128);
    let b = perform(5, 128);
    assert_eq!(a.len(), b.len());
    assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
    assert!(a.len() >= 2 * SR as usize);
    let c = perform(6, 128);
    assert!(a.iter().zip(&c).any(|(x, y)| x.to_bits() != y.to_bits()));
    assert!(a.iter().any(|x| x.abs() > 0.01), "silent");
}

#[test]
fn blockwise_equals_per_sample() {
    for seed in [1, 2, 3, 50] {
        let one = perform(seed, 1);
        let b17 = perform(seed, 17);
        let b128 = perform(seed, 128);
        assert!(one
            .iter()
            .zip(&b17)
            .all(|(x, y)| x.to_bits() == y.to_bits()));
        assert!(one
            .iter()
            .zip(&b128)
            .all(|(x, y)| x.to_bits() == y.to_bits()));
    }
}

#[test]
fn no_alloc_in_render() {
    let mut w = AmberWorld::new(SR, 9);
    let mut tel = Telemetry::default();
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    let (_, n) = count_allocs(|| {
        for i in 0..100u32 {
            if i % 7 == 0 {
                let st = w.next_step(i / 7);
                w.on_step(&st);
            }
            if i == 50 {
                w.on_ptap();
            }
            if i % 11 == 0 {
                w.on_gesture(&Gesture {
                    x: (i % 10) as f32 / 10.0,
                    y: 0.8,
                    z: 1.0,
                    pinch: -0.5,
                });
            }
            l.fill(0.0);
            r.fill(0.0);
            w.render(&mut l, &mut r);
            w.fill_telemetry(&mut tel);
        }
    });
    assert_eq!(n, 0);
}

#[test]
fn finite_and_bounded() {
    let axes = [-1.0f32, 0.0, 1.0];
    for seed in 0..10u32 {
        for &x in &axes {
            for &y in &axes {
                for &z in &axes {
                    for pinch in [-1.0f32, 1.0] {
                        let mut w = AmberWorld::new(SR, seed);
                        w.on_gesture(&Gesture { x, y, z, pinch });
                        let mut peak = 0.0f32;
                        for k in 0..3u32 {
                            let st = w.next_step(k);
                            w.on_step(&st);
                            if k == 1 {
                                w.on_ptap();
                            }
                            for s in render(&mut w, 3000, 128) {
                                assert!(s.is_finite(), "NaN seed {seed} {x} {y} {z} {pinch}");
                                peak = peak.max(s.abs());
                            }
                        }
                        assert!(peak <= 1.0, "peak {peak} seed {seed} {x} {y} {z} {pinch}");
                    }
                }
            }
        }
    }
}

#[test]
fn note_overlap_has_no_click() {
    for seed in [3u32, 11, 24] {
        let mut w = AmberWorld::new(SR, seed);
        w.on_step(&note(0, 40, 1.0));
        let first = render(&mut w, 144, 144); // 3 ms
        w.on_step(&note(1, 45, 1.0));
        let rest = render(&mut w, 24000, 128);
        let mut all = first.clone();
        all.extend(rest);
        let diffs: Vec<f32> = all.windows(2).map(|p| (p[1] - p[0]).abs()).collect();
        // junction: a 2 ms window around the second note-on (sample 144)
        let junction = diffs[100..250].iter().cloned().fold(0.0f32, f32::max);
        let elsewhere = diffs[250..].iter().cloned().fold(0.0f32, f32::max);
        assert!(
            junction <= 2.0 * elsewhere,
            "seed {seed}: junction {junction} vs rest {elsewhere}"
        );
    }
}

fn centroid(x: &[f32]) -> f64 {
    // naive DFT magnitude on a Hann window, 512 bins up to 12 kHz is enough for a comparison
    let n = x.len();
    let win: Vec<f64> = (0..n)
        .map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos())
        .collect();
    let (mut num, mut den) = (0.0, 0.0);
    for k in 1..(n / 8) {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &s) in x.iter().enumerate() {
            let ph = std::f64::consts::TAU * (k * i) as f64 / n as f64;
            re += s as f64 * win[i] * ph.cos();
            im -= s as f64 * win[i] * ph.sin();
        }
        let mag = (re * re + im * im).sqrt();
        num += k as f64 * mag;
        den += mag;
    }
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

#[test]
fn ptap_replaces_voice_and_is_brighter() {
    let mut higher = 0;
    let seeds = [2u32, 4, 7, 12, 20, 33];
    for &seed in &seeds {
        let mut a = AmberWorld::new(SR, seed);
        a.on_step(&note(0, 36, 0.7));
        render(&mut a, 4800, 128);
        let ya = render(&mut a, 1024, 128);

        let mut b = AmberWorld::new(SR, seed);
        b.on_step(&note(0, 36, 0.7));
        render(&mut b, 4800 - 128, 128);
        b.on_ptap();
        render(&mut b, 128, 128);
        let yb = render(&mut b, 1024, 128);
        if centroid(&yb) > centroid(&ya) {
            higher += 1;
        }
        // the tap must not leave silence
        assert!(
            yb.iter().any(|s| s.abs() > 1e-4),
            "seed {seed} silent after tap"
        );
    }
    assert!(
        higher >= seeds.len() - 1,
        "only {higher} of {} brighter",
        seeds.len()
    );
}

#[test]
fn pause_silences() {
    for seed in [1u32, 8, 30] {
        let mut w = AmberWorld::new(SR, seed);
        w.on_step(&note(0, 36, 1.0));
        let y = render(&mut w, 4800, 128);
        assert!(y.iter().any(|s| s.abs() > 1e-3));
        w.on_step(&note(1, 0, 0.0));
        let tail = render(&mut w, 24000, 128);
        let peak = tail[(24000 - 100)..]
            .iter()
            .fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak < 1e-3, "seed {seed}: peak {peak}");
    }
}

#[test]
fn telemetry_fields() {
    let mut w = AmberWorld::new(SR, 17);
    let mut t = Telemetry::default();
    w.fill_telemetry(&mut t);
    assert!((110.0..=135.0).contains(&t.bpm), "bpm {}", t.bpm);
    let m = *w.melody();
    assert_eq!(t.pattern.len(), 16);
    for i in 0..16 {
        assert_eq!(t.pattern[i], m.steps[i].velocity);
    }
    assert_eq!(t.root, m.root);
    assert_eq!(t.scale as usize, m.scale);
    assert!((0.0..=1.0).contains(&t.brightness));
    assert!((0.0..=1.0).contains(&t.activity));
    assert!((0.0..360.0).contains(&t.hue));
    assert_eq!(t.gate_env, 0.0);
    w.on_step(&note(0, 36, 1.0));
    render(&mut w, 2400, 128);
    w.fill_telemetry(&mut t);
    assert!(t.gate_env > 0.0 && t.gate_env <= 1.0, "gate {}", t.gate_env);
    let st = w.next_step(18);
    assert_eq!(st.index, 18);
    assert_eq!(st.note, m.steps[2].note);
}
