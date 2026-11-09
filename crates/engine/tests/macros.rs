//! Expected numbers come from evaluating the legacy `applyMacros` amber branch in node
//! (legacy/index.html 1936-1942), not from this crate. Gesture axes are 0..1; legacy MAC is 2v-1.
use engine::macros::{xm, ZHold};
use engine::Gesture;

const TOL: f32 = 1e-5;

/// Gesture that carries legacy macro values lpg/color in -1..1.
fn g(lpg: f32, color: f32) -> Gesture {
    Gesture {
        x: (lpg + 1.0) / 2.0,
        y: (color + 1.0) / 2.0,
        z: 0.0,
        pinch: 0.0,
    }
}

fn check(lpg: f32, color: f32, zh: f32, want: [f32; 4]) {
    let r = xm(&g(lpg, color), zh);
    for (got, w, n) in [
        (r.dec, want[0], "dec"),
        (r.fm, want[1], "fm"),
        (r.fold, want[2], "fold"),
        (r.cut, want[3], "cut"),
    ] {
        assert!(
            (got - w).abs() <= TOL,
            "lpg {lpg} color {color} zh {zh} {n}: got {got}, want {w}"
        );
    }
}

#[test]
fn xm_neutral() {
    check(0.0, 0.0, 0.0, [1.0, 1.0, 1.0, 1.0]);
    let r = xm(
        &Gesture {
            x: 0.5,
            y: 0.5,
            z: 0.0,
            pinch: 0.0,
        },
        0.0,
    );
    assert_eq!((r.dec, r.fm, r.fold, r.cut), (1.0, 1.0, 1.0, 1.0));
}

#[test]
fn xm_extremes() {
    check(1.0, 1.0, 0.0, [2.828_427, 1.5, 2.5, 1.741_101_1]);
    check(-1.0, -1.0, 0.0, [0.353_553_4, 0.0, 0.0, 0.574_349_2]);
    check(
        0.5,
        -0.3,
        1.0,
        [3.138_336_4, 0.945, 0.655_562_04, 1.117_287_3],
    );
    check(0.0, 0.0, 1.0, [1.866_066, 1.35, 1.0, 1.319_507_5]);
}

#[test]
fn fm_branches_at_color_boundary() {
    // u < 0.5: fm = 2u, fold = smoothstep(0.12, 0.5, u)
    check(0.0, -0.5, 0.0, [1.0, 0.5, 0.271_030_77, 0.757_858_3]);
    check(0.0, -0.6, 0.0, [1.0, 0.4, 0.114_302_38, 0.716_977_66]);
    check(0.0, -0.76, 0.0, [1.0, 0.24, 0.0, 0.656_105_9]);
    // just below the boundary, continuous with the u >= 0.5 branch (fm = 1 + 0.5(2u-1), fold = 1 + 1.5(2u-1))
    check(0.0, -0.0001, 0.0, [1.0, 0.9999, 0.999_999_95, 0.999_944_6]);
    check(0.0, 0.0, 0.0, [1.0, 1.0, 1.0, 1.0]);
    check(0.0, 1.0, 0.0, [1.0, 1.5, 2.5, 1.741_101_1]);
}

#[test]
fn zhold_delay_and_ramp() {
    let sr = 48_000.0;
    let blk = 128usize;
    let blocks = |s: f32| (s * sr / blk as f32) as usize;
    let mut z = ZHold::new(sr);
    let mut zh = 0.0;
    for _ in 0..blocks(0.9) {
        zh = z.tick_block(true, blk);
    }
    assert_eq!(zh, 0.0);
    for _ in 0..blocks(0.2) {
        zh = z.tick_block(true, blk);
    }
    assert!(zh > 0.0 && zh < 0.01, "just after 1 s: {zh}");
    let mut prev = zh;
    for _ in 0..blocks(11.0) {
        zh = z.tick_block(true, blk);
        assert!(zh >= prev && zh <= 1.0);
        prev = zh;
    }
    assert!(zh >= 0.95, "after ~12 s: {zh}");
    // release returns toward 0
    for _ in 0..blocks(0.5) {
        zh = z.tick_block(false, blk);
    }
    assert!(zh < 0.5, "after 0.5 s release: {zh}");
    for _ in 0..blocks(3.0) {
        zh = z.tick_block(false, blk);
    }
    assert_eq!(zh, 0.0);
    // a new hold starts from zero again, with its own delay
    for _ in 0..blocks(0.9) {
        zh = z.tick_block(true, blk);
    }
    assert_eq!(zh, 0.0);
    z.reset();
    assert_eq!(z.tick_block(false, blk), 0.0);
}

#[test]
fn xm_finite_for_nonfinite_gesture() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for gs in [
            Gesture {
                x: bad,
                y: 0.5,
                z: 0.0,
                pinch: 0.0,
            },
            Gesture {
                x: 0.5,
                y: bad,
                z: 0.0,
                pinch: 0.0,
            },
            Gesture {
                x: bad,
                y: bad,
                z: bad,
                pinch: bad,
            },
        ] {
            let r = xm(&gs, 0.0);
            assert!(
                r.dec.is_finite() && r.fm.is_finite() && r.fold.is_finite() && r.cut.is_finite()
            );
            assert_eq!((r.dec, r.fm, r.fold, r.cut), (1.0, 1.0, 1.0, 1.0), "{gs:?}");
        }
    }
    let r = xm(
        &Gesture {
            x: 0.5,
            y: 0.5,
            z: 0.0,
            pinch: 0.0,
        },
        f32::NAN,
    );
    assert_eq!((r.dec, r.fm, r.cut), (1.0, 1.0, 1.0));
}
