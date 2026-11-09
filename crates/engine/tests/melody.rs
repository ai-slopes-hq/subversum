//! `make_seq()` must reproduce the legacy melody for every fixture seed, on the same rng stream after `roll()`.
use engine::melody::{character_of, make_seq, SCALES};
use engine::patch::roll;
use versum_dsp::Rng;

fn close(got: f64, want: &serde_json::Value, what: &str, seed: u32) {
    let w = want.as_f64().unwrap();
    let tol = 1e-6 * w.abs().max(1e-12);
    assert!(
        (got - w).abs() <= tol,
        "seed {seed} {what}: got {got}, want {w}"
    );
}

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/legacy_seeds.json")).unwrap()
}

#[test]
fn melody_matches_legacy_for_all_fixture_seeds() {
    let fx = fixture();
    for s in fx["seeds"].as_array().unwrap() {
        let seed = s["seed"].as_u64().unwrap() as u32;
        let mut keys: Vec<&str> = s.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        keys.sort();
        let mut compared = vec![
            "seed",
            "rng_first8",
            "rng_first8_u32", // rng test's oracle
            "draws_after_roll",
            "draws_total",
            "patch", // patch test's oracle
            "bpm",
            "root",
            "scale",
            "scale_iv",
            "pool_len",
            "character",
            "steps",
        ];
        compared.sort();
        assert_eq!(keys, compared, "seed {seed}: fixture key set");

        let mut rng = Rng::new(seed);
        let p = roll(&mut rng);
        let m = make_seq(&mut rng);
        assert_eq!(m.bpm as i64, s["bpm"].as_i64().unwrap(), "seed {seed} bpm");
        assert_eq!(
            m.root as i64,
            s["root"].as_i64().unwrap(),
            "seed {seed} root"
        );
        let iv: Vec<u64> = s["scale_iv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap())
            .collect();
        let got_iv: Vec<u64> = SCALES[m.scale].iter().map(|&x| x as u64).collect();
        assert_eq!(got_iv, iv, "seed {seed} scale intervals");
        let pool_len = (24..=47u32)
            .filter(|n| SCALES[m.scale].contains(&((n + 12 - m.root) % 12)))
            .count();
        assert_eq!(
            pool_len as u64,
            s["pool_len"].as_u64().unwrap(),
            "seed {seed} pool_len"
        );

        let steps = s["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 16);
        for (i, (g, w)) in m.steps.iter().zip(steps).enumerate() {
            if w.is_null() {
                assert_eq!(g.velocity, 0.0, "seed {seed} step {i} should be a rest");
            } else {
                assert_eq!(
                    g.note as u64,
                    w["note"].as_u64().unwrap(),
                    "seed {seed} step {i} note"
                );
                close(g.velocity as f64, &w["velocity"], "velocity", seed);
            }
        }

        let ch = character_of(&m, &p);
        let wc = &s["character"];
        close(ch.bright, &wc["bright"], "bright", seed);
        close(ch.act, &wc["act"], "act", seed);
        close(ch.cold, &wc["cold"], "cold", seed);

        // total draws: the next draw must be the one after the fixture's `draws_total`
        let n = s["draws_total"].as_u64().unwrap();
        let mut fresh = Rng::new(seed);
        for _ in 0..n {
            fresh.next_f64();
        }
        assert_eq!(
            rng.next_u32(),
            fresh.next_u32(),
            "seed {seed} draw count (want {n})"
        );
    }
}

#[test]
fn step_zero_is_always_full_velocity() {
    for seed in 0..1000u32 {
        let mut rng = Rng::new(seed);
        roll(&mut rng);
        assert_eq!(make_seq(&mut rng).steps[0].velocity, 1.0, "seed {seed}");
    }
}

#[test]
fn notes_stay_in_pool() {
    for seed in 0..1000u32 {
        let mut rng = Rng::new(seed);
        roll(&mut rng);
        let m = make_seq(&mut rng);
        for (i, st) in m.steps.iter().enumerate().filter(|(_, s)| s.velocity > 0.0) {
            assert!(
                (24..=47).contains(&st.note),
                "seed {seed} step {i} note {}",
                st.note
            );
            let deg = (st.note + 12 - m.root) % 12;
            assert!(
                SCALES[m.scale].contains(&deg),
                "seed {seed} step {i} not in scale"
            );
        }
    }
}

#[test]
fn bpm_in_range() {
    for seed in 0..1000u32 {
        let mut rng = Rng::new(seed);
        roll(&mut rng);
        let b = make_seq(&mut rng).bpm;
        assert!((110.0..=135.0).contains(&b), "seed {seed} bpm {b}");
    }
}
