//! Amber melody: `make_seq()` is a line-by-line port of legacy `makeSeq()` / `makeStep()` / `characterOf()`
//! (skin = amber, no carried-over tempo/key). It continues the rng stream right after `patch::roll()`.
//! The order of rng draws is part of the contract: a seed must give the same melody as the legacy page.
use crate::patch::Patch;
use versum_dsp::Rng;

const LOW: u32 = 24;
const HIGH: u32 = 47;
const BPM_MIN: f64 = 110.0;
const BPM_MAX: f64 = 135.0;
const STEPS: usize = 16;
const DEPTH_MAX: f64 = 0.8;
const FM_MAX: f64 = 4.5;
const WALK: [i32; 9] = [-3, -2, -1, -1, 0, 1, 1, 2, 3];

/// Legacy `SCALES` (interval sets, semitones above the root), in legacy order.
pub const SCALES: [&[u32]; 7] = [
    &[0, 2, 4, 5, 7, 9, 11], // MAJOR
    &[0, 2, 3, 5, 7, 8, 10], // MINOR
    &[0, 2, 3, 5, 7, 9, 10], // DORIAN
    &[0, 2, 4, 6, 7, 9, 11], // LYDIAN
    &[0, 2, 4, 5, 7, 9, 10], // MIXOLYDIAN
    &[0, 3, 5, 7, 10],       // MIN PENTA
    &[0, 2, 4, 7, 9],        // MAJ PENTA
];

/// `velocity == 0.0` is a rest (legacy `null`); a rest has `note == 0`, which is meaningless.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub note: u32,
    pub velocity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Melody {
    pub bpm: f32,
    pub root: u32,
    /// Index into [`SCALES`].
    pub scale: usize,
    pub steps: [Step; 16],
}

/// Legacy `characterOf()` result, each value in `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Character {
    pub bright: f64,
    pub act: f64,
    pub cold: f64,
}

const REST: Step = Step {
    note: 0,
    velocity: 0.0,
};

fn pick<T: Copy>(rng: &mut Rng, arr: &[T]) -> T {
    arr[(rng.next_f64() * arr.len() as f64).floor() as usize]
}

fn make_step(rng: &mut Rng, pool: &[u32], idx: &mut usize) -> Step {
    if rng.next_f64() < 0.22 {
        return REST;
    }
    if rng.next_f64() < 0.15 {
        *idx = (rng.next_f64() * pool.len() as f64).floor() as usize;
    } else {
        let d = pick(rng, &WALK);
        *idx = (*idx as i32 + d).clamp(0, pool.len() as i32 - 1) as usize;
    }
    let note = pool[*idx];
    let velocity = if rng.next_f64() < 0.25 {
        1.0
    } else {
        0.62 + rng.next_f64() * (0.85 - 0.62)
    };
    Step {
        note,
        velocity: velocity as f32,
    }
}

pub fn make_seq(rng: &mut Rng) -> Melody {
    let bpm = (BPM_MIN + rng.next_f64() * (BPM_MAX - BPM_MIN)).round();
    let root = (rng.next_f64() * 12.0).floor() as u32;
    let scale = (rng.next_f64() * SCALES.len() as f64).floor() as usize;
    let mut pool_buf = [0u32; (HIGH - LOW + 1) as usize];
    let mut n = 0;
    for m in LOW..=HIGH {
        if SCALES[scale].contains(&((m + 12 - root) % 12)) {
            pool_buf[n] = m;
            n += 1;
        }
    }
    let pool = &pool_buf[..n];
    let mut idx = (rng.next_f64() * pool.len() as f64).floor() as usize;
    let mut steps = [REST; STEPS];
    for s in steps.iter_mut() {
        *s = make_step(rng, pool, &mut idx);
    }
    // the first step is always a clear note (legacy): a rest becomes the pool note at the current index
    if steps[0].velocity == 0.0 {
        steps[0].note = pool[idx];
    }
    steps[0].velocity = 1.0;
    Melody {
        bpm: bpm as f32,
        root,
        scale,
        steps,
    }
}

/// Legacy `characterOf(seq, patch)` for the plain (non-dub) path: how the picture mirrors the sound.
pub fn character_of(m: &Melody, p: &Patch) -> Character {
    let c01 = |x: f64| x.clamp(0.0, 1.0);
    let lnr = |x: f64, a: f64, b: f64| c01((x.ln() - a.ln()) / (b.ln() - a.ln()));
    let (cut, res) = (p.cutoff as f64, p.res_db as f64);
    let env = p.depth as f64 / DEPTH_MAX;
    let len = lnr(p.decay as f64, 0.12, 1.8);
    let space = 0.1;
    let fm = p.fm_idx as f64 / FM_MAX;
    let bright = c01(lnr(cut, 900.0, 9000.0) * 0.85 + 0.15 * env);
    let res01 = c01(res / 20.0);
    let dens = m.steps.iter().filter(|s| s.velocity > 0.0).count() as f64 / STEPS as f64;
    let tempo = c01((m.bpm as f64 - BPM_MIN) / (BPM_MAX - BPM_MIN));
    let act =
        c01((0.3 * res01 + 0.25 * env + 0.2 * dens + 0.15 * tempo + 0.1 * (1.0 - len) - 0.3) / 0.5);
    let cold = c01((0.45 * bright + 0.2 * space + 0.2 * res01 + 0.15 * fm - 0.2) / 0.6);
    Character { bright, act, cold }
}
