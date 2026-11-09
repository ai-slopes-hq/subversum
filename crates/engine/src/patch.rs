//! Amber patch: `roll()` is a line-by-line port of legacy `roll()` / `rollLfo()` (legacy/index.html).
//! The order of rng draws is part of the contract: a seed must give the same patch as the legacy page.
use versum_dsp::lfo::LfoShape;
use versum_dsp::Rng;

const FM_RATIOS: [f64; 10] = [0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0, 7.0];
const FM_MAX: f64 = 4.5;
const DEPTH_MAX: f64 = 0.8;
const SYM_MAX: f64 = 0.08;
const RES_MAX: f64 = 18.0;
const LFO_RATE_MIN: f64 = 0.02;
const LFO_RATE_MAX: f64 = 3.0;
const LFO_CHANCE: f64 = 0.5;
const LFO_SHAPES: [LfoShape; 4] = [
    LfoShape::Sine,
    LfoShape::Sine,
    LfoShape::Sine,
    LfoShape::Triangle,
];
const LFO_TARGETS: [LfoTarget; 7] = [
    LfoTarget::Pitch,
    LfoTarget::Ratio,
    LfoTarget::Index,
    LfoTarget::Fold,
    LfoTarget::Sym,
    LfoTarget::Cut,
    LfoTarget::Res,
];
const WAVES: [Wave; 3] = [Wave::Sine, Wave::Tri, Wave::Saw];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    Sine,
    Tri,
    Saw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LfoTarget {
    Pitch,
    Ratio,
    Index,
    Fold,
    Sym,
    Cut,
    Res,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LfoSpec {
    pub target: LfoTarget,
    pub freq_hz: f32,
    pub shape: LfoShape,
    pub depth: f32,
    /// Starting phase in radians (legacy convention), `0..2*pi`.
    pub phase: f32,
}

/// Silent sine twin that only drives the background when there is no audio LFO (legacy `vis[0]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisLfo {
    pub freq_hz: f32,
    pub depth: f32,
    pub phase: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Patch {
    pub oct: i32,
    pub semi: i32,
    /// Base pitch in Hz (`440 * 2^((midi-69)/12)`).
    pub freq: f32,
    pub wave: Wave,
    pub fm_ratio: f32,
    pub fm_idx: f32,
    pub fm_sweep: f32,
    pub depth: f32,
    pub sym: f32,
    pub sweep: f32,
    /// Seconds until the gate is nearly closed.
    pub decay: f32,
    /// Attack time in seconds (legacy `attack`).
    pub attack_s: f32,
    pub cutoff: f32,
    pub res_db: f32,
    pub vel: f32,
    pub lfos: [Option<LfoSpec>; 2],
    pub vis: VisLfo,
}

fn rnd(rng: &mut Rng, a: f64, b: f64) -> f64 {
    a + rng.next_f64() * (b - a)
}

fn lrnd(rng: &mut Rng, a: f64, b: f64) -> f64 {
    rnd(rng, a.ln(), b.ln()).exp()
}

fn pick<T: Copy>(rng: &mut Rng, arr: &[T]) -> T {
    arr[(rng.next_f64() * arr.len() as f64).floor() as usize]
}

fn roll_lfo(rng: &mut Rng) -> [Option<LfoSpec>; 2] {
    let mut out = [None, None];
    if rng.next_f64() >= LFO_CHANCE {
        return out;
    }
    let n = 1 + usize::from(rng.next_f64() < 0.2);
    for slot in out.iter_mut().take(n) {
        let target = pick(rng, &LFO_TARGETS);
        let shape = pick(rng, &LFO_SHAPES);
        let rate = lrnd(rng, LFO_RATE_MIN, LFO_RATE_MAX);
        let depth = rnd(rng, 0.15, 0.6);
        let phase = rnd(rng, 0.0, std::f64::consts::TAU);
        *slot = Some(LfoSpec {
            target,
            freq_hz: rate as f32,
            shape,
            depth: depth as f32,
            phase: phase as f32,
        });
    }
    out
}

/// Roll an amber patch. Draw order matches legacy `roll()` exactly.
pub fn roll(rng: &mut Rng) -> Patch {
    let oct: i32 = if rng.next_f64() < 0.5 { 1 } else { 2 };
    let semi = (rng.next_f64() * 12.0).floor() as i32;
    let midi = 12 * (oct + 1) + semi;
    let freq = 440.0 * 2f64.powf((midi - 69) as f64 / 12.0);
    let wave = pick(rng, &WAVES);
    let ratio = pick(rng, &FM_RATIOS);
    let fm_ratio = ratio
        * if rng.next_f64() < 0.25 {
            1.0 + rnd(rng, -0.02, 0.02)
        } else {
            1.0
        };
    let fm_idx = rnd(rng, 0.0, FM_MAX);
    let fm_sweep = rnd(rng, 0.05, 1.0);
    let depth = rnd(rng, 0.06, DEPTH_MAX);
    let sym = rnd(rng, -1.0, 1.0) * SYM_MAX;
    let sweep = rnd(rng, 0.2, 1.0);
    let decay = lrnd(rng, 0.12, 1.8);
    let attack = rnd(rng, 0.001, 0.014);
    let cutoff = lrnd(rng, 900.0, 9000.0);
    let res = rnd(rng, 0.0, RES_MAX);
    let lfos = roll_lfo(rng);
    let vis = VisLfo {
        freq_hz: lrnd(rng, 0.03, 0.3) as f32,
        depth: 1.0,
        phase: rnd(rng, 0.0, std::f64::consts::TAU) as f32,
    };
    Patch {
        oct,
        semi,
        freq: freq as f32,
        wave,
        fm_ratio: fm_ratio as f32,
        fm_idx: fm_idx as f32,
        fm_sweep: fm_sweep as f32,
        depth: depth as f32,
        sym: sym as f32,
        sweep: sweep as f32,
        decay: decay as f32,
        attack_s: attack as f32,
        cutoff: cutoff as f32,
        res_db: res as f32,
        vel: 1.0,
        lfos,
        vis,
    }
}
