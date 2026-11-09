//! Render scenarios: 3 seeds x 5 gestures x 8 s; output must be finite, bounded and sensibly loud.
use engine::{Command, Engine, Gesture, MAX_BLOCK};

const SR: f32 = 48000.0;
const SECS: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Neutral,
    X,
    Y,
    Z,
    PTap,
}

/// Wire mapping: axes 0..1, neutral 0.5; z = seconds held since press.
fn gesture_at(s: Scenario, t: f32) -> Gesture {
    let (x, y, z) = match s {
        Scenario::Neutral | Scenario::PTap => (0.5, 0.5, 0.0),
        Scenario::X => (1.0, 0.5, 0.0),
        Scenario::Y => (0.5, 0.0, 0.0),
        Scenario::Z => (0.5, 0.5, t),
    };
    Gesture {
        x,
        y,
        z,
        pinch: 0.0,
    }
}

struct Stats {
    rms_db: f32,
    peak: f32,
    dc: f32,
}

fn run(seed: u32, s: Scenario) -> Stats {
    let total = (SECS * SR) as usize;
    let mut e = Engine::new(SR, seed);
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    let (mut sum, mut sum_sq, mut peak) = (0.0f64, 0.0f64, 0.0f32);
    let mut ptaps = [false; 2];
    let mut done = 0;
    while done < total {
        let n = MAX_BLOCK.min(total - done);
        let t = done as f32 / SR;
        e.push(Command::Gesture(gesture_at(s, t)));
        if let Scenario::PTap = s {
            for (i, at) in [1.0f32, 4.0].iter().enumerate() {
                if !ptaps[i] && t >= *at {
                    ptaps[i] = true;
                    e.push(Command::PTap);
                }
            }
        }
        e.process(&mut l[..n], &mut r[..n]);
        for i in 0..n {
            for v in [l[i], r[i]] {
                assert!(v.is_finite(), "{seed} {s:?}: non-finite at {}", done + i);
                peak = peak.max(v.abs());
                sum += v as f64;
                sum_sq += (v as f64) * (v as f64);
            }
        }
        done += n;
    }
    let cnt = (total * 2) as f64;
    Stats {
        rms_db: (20.0 * (sum_sq / cnt).sqrt().max(1e-12).log10()) as f32,
        peak,
        dc: (sum / cnt).abs() as f32,
    }
}

#[test]
fn render_scenarios_are_finite_and_in_range() {
    let mut bad = Vec::new();
    for seed in [0u32, 7, 4294967295] {
        for s in [
            Scenario::Neutral,
            Scenario::X,
            Scenario::Y,
            Scenario::Z,
            Scenario::PTap,
        ] {
            let st = run(seed, s);
            println!(
                "seed {seed} {s:?}: rms {:.2} dBFS peak {:.3} dc {:.2e}",
                st.rms_db, st.peak, st.dc
            );
            if st.peak > 1.0 {
                bad.push(format!("{seed} {s:?}: peak {}", st.peak));
            }
            // Y at the bottom closes the Lpg (legacy macro -1): a dark, quiet extreme by design,
            // pending the Lpg tuning by ear (Task 13 gate), so its floor is looser.
            let floor = if matches!(s, Scenario::Y) {
                -45.0
            } else {
                -30.0
            };
            if !(floor..=-8.0).contains(&st.rms_db) {
                bad.push(format!("{seed} {s:?}: rms {} dB", st.rms_db));
            }
            if st.dc >= 1e-3 {
                bad.push(format!("{seed} {s:?}: dc {}", st.dc));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
