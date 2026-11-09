//! `roll()` must reproduce the legacy patch for every fixture seed, including the number of rng draws.
use engine::patch::{roll, LfoTarget, Wave};
use versum_dsp::lfo::LfoShape;
use versum_dsp::Rng;

fn close(got: f32, want: &serde_json::Value, what: &str, seed: u32) {
    let w = want.as_f64().unwrap();
    let g = got as f64;
    let tol = 1e-6 * w.abs().max(1e-12);
    assert!(
        (g - w).abs() <= tol,
        "seed {seed} {what}: got {g}, want {w}"
    );
}

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/legacy_seeds.json")).unwrap()
}

#[test]
fn patch_matches_legacy_for_all_fixture_seeds() {
    let fx = fixture();
    for s in fx["seeds"].as_array().unwrap() {
        let seed = s["seed"].as_u64().unwrap() as u32;
        let mut rng = Rng::new(seed);
        let p = roll(&mut rng);
        let w = &s["patch"];
        // every legacy key must be compared below (or be a known-ignored one); none silently dropped
        let mut keys: Vec<&str> = w.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        keys.sort();
        let mut compared = vec![
            "oct", "semi", "freq", "wave", "fmRatio", "fmIdx", "fmSweep", "depth", "sym", "sweep",
            "decay", "attack", "cutoff", "res", "vel", "lfo", "vis",
            "lfoNodes", // always null in legacy roll(); runtime audio nodes, not a patch value
        ];
        compared.sort();
        assert_eq!(
            keys, compared,
            "seed {seed}: fixture patch keys vs compared keys"
        );
        assert_eq!(p.oct as i64, w["oct"].as_i64().unwrap(), "seed {seed} oct");
        assert_eq!(
            p.semi as i64,
            w["semi"].as_i64().unwrap(),
            "seed {seed} semi"
        );
        close(p.freq, &w["freq"], "freq", seed);
        let wave = match w["wave"][0].as_str().unwrap() {
            "sine" => Wave::Sine,
            "triangle" => Wave::Tri,
            "sawtooth" => Wave::Saw,
            o => panic!("unknown wave {o}"),
        };
        assert_eq!(p.wave, wave, "seed {seed} wave");
        close(p.fm_ratio, &w["fmRatio"], "fmRatio", seed);
        close(p.fm_idx, &w["fmIdx"], "fmIdx", seed);
        close(p.fm_sweep, &w["fmSweep"], "fmSweep", seed);
        close(p.depth, &w["depth"], "depth", seed);
        close(p.sym, &w["sym"], "sym", seed);
        close(p.sweep, &w["sweep"], "sweep", seed);
        close(p.decay, &w["decay"], "decay", seed);
        close(p.attack_s, &w["attack"], "attack", seed);
        close(p.cutoff, &w["cutoff"], "cutoff", seed);
        close(p.res_db, &w["res"], "res", seed);
        close(p.vel, &w["vel"], "vel", seed);

        let lfos = w["lfo"].as_array().unwrap();
        for i in 0..2 {
            match (&p.lfos[i], lfos.get(i)) {
                (None, None) => {}
                (Some(g), Some(l)) => {
                    let t = match l["target"].as_str().unwrap() {
                        "pitch" => LfoTarget::Pitch,
                        "ratio" => LfoTarget::Ratio,
                        "index" => LfoTarget::Index,
                        "fold" => LfoTarget::Fold,
                        "sym" => LfoTarget::Sym,
                        "cut" => LfoTarget::Cut,
                        "res" => LfoTarget::Res,
                        o => panic!("unknown target {o}"),
                    };
                    assert_eq!(g.target, t, "seed {seed} lfo{i} target");
                    let sh = match l["shape"].as_str().unwrap() {
                        "sine" => LfoShape::Sine,
                        "triangle" => LfoShape::Triangle,
                        o => panic!("unknown shape {o}"),
                    };
                    assert_eq!(g.shape, sh, "seed {seed} lfo{i} shape");
                    close(g.freq_hz, &l["rate"], "lfo rate", seed);
                    close(g.depth, &l["depth"], "lfo depth", seed);
                    close(g.phase, &l["phase"], "lfo phase", seed);
                }
                (g, l) => panic!("seed {seed} lfo{i} presence: got {g:?}, want {l:?}"),
            }
        }
        assert!(lfos.len() <= 2);

        let vis = &w["vis"][0];
        assert_eq!(w["vis"].as_array().unwrap().len(), 1);
        assert_eq!(vis["shape"].as_str().unwrap(), "sine");
        close(p.vis.freq_hz, &vis["rate"], "vis rate", seed);
        close(p.vis.depth, &vis["depth"], "vis depth", seed);
        close(p.vis.phase, &vis["phase"], "vis phase", seed);

        // exact draw count: the next draw must be the fixture's draw number `draws_after_roll`
        let n = s["draws_after_roll"].as_u64().unwrap();
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
fn roll_is_deterministic() {
    for seed in [0u32, 1, 42, 99999] {
        let a = roll(&mut Rng::new(seed));
        let b = roll(&mut Rng::new(seed));
        assert_eq!(a, b);
    }
}
