//! The Rust mulberry32 must reproduce the legacy JS PRNG stream (fixtures from tools/dump_legacy_seeds.mjs).
use versum_dsp::Rng;

#[test]
fn rng_matches_legacy_stream() {
    let fx: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/legacy_seeds.json")).unwrap();
    let seeds = fx["seeds"].as_array().unwrap();
    assert_eq!(seeds.len(), 6);
    for s in seeds {
        let seed = s["seed"].as_u64().unwrap() as u32;
        let f = s["rng_first8"].as_array().unwrap();
        let u = s["rng_first8_u32"].as_array().unwrap();
        assert_eq!(f.len(), 8);
        let (mut a, mut b) = (Rng::new(seed), Rng::new(seed));
        for i in 0..8 {
            let want_u = u[i].as_f64().unwrap() as u32;
            assert_eq!(a.next_u32(), want_u, "seed {seed} draw {i} (u32)");
            let want_f = f[i].as_f64().unwrap();
            assert_eq!(b.next_f64(), want_f, "seed {seed} draw {i} (f64)");
        }
    }
}
