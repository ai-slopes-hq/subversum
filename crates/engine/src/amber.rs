//! Amber world: one FM voice (Vco + FM modulator -> fold Shaper -> HP -> vactrol gate), the patch LFOs and the P-tap.
//! Port of legacy `play()` / `amberStab()` (legacy/index.html 704-824, 841-844); the chain structure follows the spec,
//! the numeric constants follow legacy. Everything is advanced per sample, so the output does not depend on how
//! `render` is split into blocks. Nothing here allocates after `new`.
use crate::macros::{xm, Xm, ZHold};
use crate::melody::{character_of, make_seq, Character, Melody};
use crate::patch::{roll, LfoTarget, Patch, Wave};
use crate::protocol::{Gesture, StepEvent, Telemetry, World, PATTERN_LEN};
use versum_dsp::biquad::{Biquad, BiquadKind};
use versum_dsp::lfo::Lfo;
use versum_dsp::lpg::{conductance, Circuit, Mode, Vactrol};
use versum_dsp::shaper::Shaper;
use versum_dsp::vco::{Vco, VcoIn, VcoOpts, SAW, SINE, TRI};
use versum_dsp::{Block, Rng};

const FM_MAX: f32 = 4.5;
const DEPTH_MAX: f32 = 0.8;
const SYM_MAX: f32 = 0.08;
const MASTER: f32 = 0.85;
const CROSSFADE_S: f32 = 0.005;
const FOLD_TABLE: usize = 2049;
const FOLD_LOBES: f64 = 5.0;
const HP_HZ: f64 = 22.0;
const HP_Q_DB: f64 = 0.7;
/// FM depth fed to `Vco`: `f = f0 * (1 + 4 * fm_depth * fm)`, so `fm` carries `index * ratio * sin`.
const VCO_FM_DEPTH: f32 = 0.25;

// LFO amounts per target, scaled by the LFO depth (legacy `play()` LFO switch).
const LFO_PITCH_CENTS: f32 = 14.0;
const LFO_RATIO_CENTS: f32 = 120.0;
const LFO_INDEX: f32 = 0.25 * FM_MAX;
const LFO_FOLD: f32 = 0.18;
const LFO_SYM: f32 = 0.8 * SYM_MAX;
const LFO_CUT_CENTS: f32 = 1100.0;
const LFO_RES_DB: f32 = 3.0;

/// Starting map from the legacy low-pass-gate envelope onto the vactrol gate. The parts of the patch that legacy
/// put on a resonant filter (cutoff, resonance) can only drive the gate here, which has no resonance.
#[derive(Clone, Copy)]
struct LpgMap {
    /// Vactrol fall time constant, seconds.
    fall_s: f32,
    /// How long the gate is driven open at note start, seconds (the legacy attack).
    hold_s: f32,
    /// Drive height 0..1 while held: opens the light level, so it brightens and loudens together.
    drive: f32,
    /// Output level before the resonance trim (legacy `0.9 * vel`).
    amp: f32,
    res_db: f32,
}

impl LpgMap {
    // tuned by ear in Task 13
    fn new(tau: f32, attack_s: f32, cutoff_hz: f32, res_db: f32, vel: f32) -> LpgMap {
        let cut_norm =
            ((cutoff_hz.max(84.0) / 70.0).ln() / (16000.0f32 / 70.0).ln()).clamp(0.0, 1.0);
        LpgMap {
            fall_s: (tau * 0.6).max(1e-3),
            hold_s: attack_s.max(0.006),
            drive: (vel.clamp(0.0, 1.0) * (0.3 + 0.7 * cut_norm)).clamp(0.0, 1.0),
            amp: 0.9 * vel,
            res_db,
        }
    }

    /// Legacy: resonance adds level, so it is trimmed.
    fn level(&self, res_db: f32) -> f32 {
        let res = res_db.clamp(0.0, 20.0);
        self.amp * (1.0 - 0.45 * res / 18.0)
    }
}

/// Per-sample modulation sums from the patch LFOs.
#[derive(Clone, Copy, Default)]
struct Mods {
    pitch_c: f32,
    ratio_c: f32,
    index: f32,
    fold: f32,
    sym: f32,
    cut_mul: f32,
    res_db: f32,
}

/// Everything a channel needs to start a note.
#[derive(Clone, Copy)]
struct Setup {
    wave: usize,
    cv: f32,
    fr: f32,
    idx0: f32,
    fm_sweep: f32,
    dep: f32,
    sweep: f32,
    sym: f32,
    fm_k: f32,
    fold_k: f32,
    lpg: LpgMap,
    left: u32,
}

/// One note on one channel.
struct Chan {
    /// A note waiting for this channel to finish fading out (click-free steal).
    pending: Option<Setup>,
    car: Vco,
    modu: Vco,
    shaper: Shaper<FOLD_TABLE>,
    hp: Biquad,
    vactrol: Vactrol,
    circuit: Circuit,
    active: bool,
    gain: f32,
    target: f32,
    // note parameters (fixed at note start)
    wave: usize,
    cv: f32,
    log2_fr: f32,
    fr: f32,
    idx0: f32,
    fm_sweep: f32,
    dep: f32,
    sweep: f32,
    sym: f32,
    fm_k: f32,
    fold_k: f32,
    // per-sample state
    e_fm: f32,
    e_fold: f32,
    left: u32,
    hold: f32,
    lpg: LpgMap,
    level: f32,
}

impl Chan {
    fn new(sr: f32, table: &[f32; FOLD_TABLE]) -> Chan {
        let mut car = Vco::new();
        car.set_sample_rate(sr as f64);
        let mut modu = Vco::new();
        modu.set_sample_rate(sr as f64);
        let mut hp = Biquad::new(sr as f64);
        hp.set(BiquadKind::Highpass, HP_HZ, HP_Q_DB);
        Chan {
            car,
            modu,
            shaper: Shaper::new(table),
            hp,
            vactrol: Vactrol::new(sr as f64, false),
            circuit: Circuit::new(Mode::Combo, sr as f64),
            pending: None,
            active: false,
            gain: 0.0,
            target: 0.0,
            wave: SINE,
            cv: 0.0,
            log2_fr: 0.0,
            fr: 1.0,
            idx0: 0.0,
            fm_sweep: 1.0,
            dep: 0.0,
            sweep: 1.0,
            sym: 0.0,
            fm_k: 0.0,
            fold_k: 0.0,
            e_fm: 0.0,
            e_fold: 0.0,
            left: 0,
            hold: 0.0,
            lpg: LpgMap::new(0.1, 0.006, 1000.0, 0.0, 0.0),
            level: 0.0,
        }
    }

    /// Resets the voice state and starts the note from silence (gain 0, ramping up).
    fn begin(&mut self, u: &Setup) {
        self.silence();
        self.target = 1.0;
        self.active = true;
        self.wave = u.wave;
        self.cv = u.cv;
        self.fr = u.fr;
        self.log2_fr = u.fr.log2();
        self.idx0 = u.idx0;
        self.fm_sweep = u.fm_sweep;
        self.dep = u.dep;
        self.sweep = u.sweep;
        self.sym = u.sym;
        self.fm_k = u.fm_k;
        self.fold_k = u.fold_k;
        self.e_fm = 1.0;
        self.e_fold = 1.0;
        self.lpg = u.lpg;
        self.left = u.left;
        self.hold = u.lpg.hold_s;
    }

    fn silence(&mut self) {
        self.pending = None;
        self.car.reset();
        self.modu.reset();
        self.shaper.reset();
        self.hp.reset();
        self.vactrol.reset();
        self.circuit.reset();
        self.active = false;
        self.gain = 0.0;
        self.target = 0.0;
        self.left = 0;
        self.level = 0.0;
    }

    /// One sample of this note, before the crossfade gain.
    #[inline]
    fn tick(&mut self, m: &Mods, fade_step: f32) -> f32 {
        // crossfade gain
        if self.gain < self.target {
            self.gain = (self.gain + fade_step).min(self.target);
        } else if self.gain > self.target {
            self.gain = (self.gain - fade_step).max(self.target);
        }
        if self.target == 0.0 && self.gain == 0.0 {
            if let Some(u) = self.pending.take() {
                // the stolen voice has faded out: start the waiting note from silence
                self.begin(&u);
            } else {
                self.active = false;
                self.level = 0.0;
                return 0.0;
            }
        }

        let idx =
            (self.idx0 * (self.fm_sweep + (1.0 - self.fm_sweep) * self.e_fm) + m.index).max(0.0);
        self.e_fm *= self.fm_k;
        let cents = m.pitch_c / 1200.0;
        let opts = VcoOpts::default();
        let mi = VcoIn {
            pitch_cv: self.cv + self.log2_fr + cents + m.ratio_c / 1200.0,
            pw: 0.5,
            ..VcoIn::default()
        };
        let modv = self.modu.tick(&opts, &mi)[SINE] as f32;
        let ci = VcoIn {
            pitch_cv: self.cv + cents,
            fm: idx * self.fr * modv,
            fm_depth: VCO_FM_DEPTH,
            pw: 0.5,
            ..VcoIn::default()
        };
        let carrier = self.car.tick(&opts, &ci)[self.wave] as f32;

        let pre = (self.dep * (self.sweep + (1.0 - self.sweep) * self.e_fold) + m.fold).max(0.0);
        self.e_fold *= self.fold_k;
        let folded = self.shaper.tick(carrier * pre + self.sym + m.sym);
        let y = self.hp.tick(folded);

        let e = if self.left > 0 {
            self.left -= 1;
            self.lpg.drive
        } else {
            0.0
        };
        let l = self.vactrol.step(e as f64, self.lpg.fall_s as f64) as f32;
        self.level = l;
        let g = conductance(((l * m.cut_mul) as f64).min(1.0));
        let out =
            self.circuit.step(y as f64, g) as f32 * self.lpg.level(self.lpg.res_db + m.res_db);
        if self.left == 0 && l < 1e-5 && self.target > 0.0 {
            // the note has died out by itself
            self.active = false;
            self.level = 0.0;
        }
        if out.is_finite() {
            out * self.gain
        } else {
            self.silence();
            0.0
        }
    }
}

struct LfoSlot {
    lfo: Lfo,
    target: LfoTarget,
    /// Depth times the unit of the target.
    amount: f32,
}

pub struct AmberWorld {
    sr: f32,
    rng_seed: u32,
    patch: Patch,
    melody: Melody,
    character: Character,
    chans: [Chan; 2],
    lfos: [Option<LfoSlot>; 2],
    gesture: Gesture,
    zhold: ZHold,
    hold_mode: bool,
    zh: f32,
    last_midi: Option<u32>,
    gate_env: f32,
}

/// Legacy master limiter curve: linear to 0.8, then a tanh knee towards 1 (never exceeds 1).
fn soft_limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.8 {
        x
    } else {
        x.signum() * (0.8 + 0.2 * ((a - 0.8) / 0.2).tanh())
    }
}

fn midi_cv(midi: u32) -> f32 {
    (midi as f32 - 60.0) / 12.0
}

impl AmberWorld {
    pub fn new(sr: f32, seed: u32) -> AmberWorld {
        let mut table = Box::new([0.0f32; FOLD_TABLE]);
        for (i, t) in table.iter_mut().enumerate() {
            let x = i as f64 / (FOLD_TABLE - 1) as f64 * 2.0 - 1.0;
            *t = (std::f64::consts::PI * FOLD_LOBES * x).sin() as f32;
        }
        let mut rng = Rng::new(seed);
        let patch = roll(&mut rng);
        let melody = make_seq(&mut rng);
        let character = character_of(&melody, &patch);
        let mut w = AmberWorld {
            sr,
            rng_seed: seed,
            patch,
            melody,
            character,
            chans: [Chan::new(sr, &table), Chan::new(sr, &table)],
            lfos: [None, None],
            gesture: Gesture {
                x: 0.5,
                y: 0.5,
                z: 0.0,
                pinch: 0.0,
            },
            zhold: ZHold::new(sr),
            hold_mode: false,
            zh: 0.0,
            last_midi: None,
            gate_env: 0.0,
        };
        w.build_lfos();
        w
    }

    pub fn melody(&self) -> &Melody {
        &self.melody
    }

    /// Legacy Z semantics: a finger opens Z; no finger in hold mode decays it; no finger otherwise zeroes it at once.
    fn zh_advance(&mut self, n: usize) -> f32 {
        self.zh = if self.gesture.z > 0.0 {
            self.zhold.tick_block(true, n)
        } else if self.hold_mode {
            self.zhold.tick_block(false, n)
        } else {
            self.zhold.reset();
            0.0
        };
        self.zh
    }

    /// Current Z-hold value (0..1) as of the last rendered sample.
    pub fn zh(&self) -> f32 {
        self.zh
    }

    fn build_lfos(&mut self) {
        let sr = self.sr as f64;
        for (slot, spec) in self.lfos.iter_mut().zip(self.patch.lfos.iter()) {
            *slot = spec.map(|s| {
                let unit = match s.target {
                    LfoTarget::Pitch => LFO_PITCH_CENTS,
                    LfoTarget::Ratio => LFO_RATIO_CENTS,
                    LfoTarget::Index => LFO_INDEX,
                    LfoTarget::Fold => LFO_FOLD,
                    LfoTarget::Sym => LFO_SYM,
                    LfoTarget::Cut => LFO_CUT_CENTS,
                    LfoTarget::Res => LFO_RES_DB,
                };
                LfoSlot {
                    lfo: Lfo::new(
                        s.shape,
                        s.freq_hz as f64,
                        s.phase as f64 / std::f64::consts::TAU,
                        sr,
                    ),
                    target: s.target,
                    amount: unit * s.depth,
                }
            });
        }
    }

    /// Starts a note on the quietest channel; the other one fades out.
    fn start_note(&mut self, midi: u32, step_vel: f32, stab: bool) {
        let zh = self.zh_advance(0);
        let x: Xm = xm(&self.gesture, zh);
        let p = self.patch;
        let (attack, cutoff, fm_idx, decay, vel) = if stab {
            (
                0.002,
                (p.cutoff * 3.0).min(16000.0),
                p.fm_idx * 1.4,
                p.decay * 1.3,
                1.2,
            )
        } else {
            (p.attack_s, p.cutoff, p.fm_idx, p.decay, step_vel)
        };
        let tau = (decay / 3.0 * x.dec).max(1e-3);
        let d = if self.gesture.pinch.is_finite() {
            self.gesture.pinch.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let fr = if d < 0.0 {
            p.fm_ratio + (p.fm_ratio.round().max(1.0) - p.fm_ratio) * -d
        } else {
            p.fm_ratio * (0.55 * d).exp2()
        };
        let cut_eff = (cutoff * x.cut).clamp(84.0, 16000.0);
        let sr = self.sr;

        let u = Setup {
            wave: match p.wave {
                Wave::Sine => SINE,
                Wave::Tri => TRI,
                Wave::Saw => SAW,
            },
            cv: midi_cv(midi),
            fr,
            idx0: (fm_idx * x.fm).min(FM_MAX),
            fm_sweep: p.fm_sweep,
            dep: (p.depth * x.fold).min(DEPTH_MAX),
            sweep: p.sweep,
            sym: p.sym,
            fm_k: (-1.0 / (tau * 0.7 * sr)).exp(),
            fold_k: (-1.0 / (tau * 0.9 * sr)).exp(),
            lpg: LpgMap::new(tau, attack, cut_eff, p.res_db, vel),
            left: 0,
        };
        let u = Setup {
            left: (u.lpg.hold_s * sr).round() as u32,
            ..u
        };

        let pick = if !self.chans[0].active {
            0
        } else if !self.chans[1].active || self.chans[1].gain < self.chans[0].gain {
            1
        } else {
            0
        };
        // the other voice fades out; a note still waiting there is superseded
        let other = &mut self.chans[1 - pick];
        other.target = 0.0;
        other.pending = None;
        let c = &mut self.chans[pick];
        if c.active && c.gain > 0.0 {
            // audible: fade it out over the crossfade time, the new note starts when it is silent
            c.target = 0.0;
            c.pending = Some(u);
        } else {
            c.begin(&u);
        }
    }

    fn mods(&mut self) -> Mods {
        let mut m = Mods {
            cut_mul: 1.0,
            ..Mods::default()
        };
        let mut cut_c = 0.0;
        for slot in self.lfos.iter_mut().flatten() {
            let v = slot.lfo.tick() * slot.amount;
            match slot.target {
                LfoTarget::Pitch => m.pitch_c += v,
                LfoTarget::Ratio => m.ratio_c += v,
                LfoTarget::Index => m.index += v,
                LfoTarget::Fold => m.fold += v,
                LfoTarget::Sym => m.sym += v,
                LfoTarget::Cut => cut_c += v,
                LfoTarget::Res => m.res_db += v,
            }
        }
        if cut_c != 0.0 {
            m.cut_mul = (cut_c / 1200.0).exp2();
        }
        m
    }
}

impl World for AmberWorld {
    fn on_gesture(&mut self, g: &Gesture) {
        self.gesture = *g;
    }

    /// The step at `index` (mod 16) of the melody; velocity 0 is a rest.
    fn next_step(&mut self, index: u32) -> StepEvent {
        let s = self.melody.steps[(index % PATTERN_LEN as u32) as usize];
        StepEvent {
            index,
            note: s.note,
            velocity: s.velocity,
        }
    }

    fn on_step(&mut self, s: &StepEvent) {
        if s.velocity > 0.0 {
            self.last_midi = Some(s.note);
            self.start_note(s.note, s.velocity, false);
        } else {
            // a rest lets the voice fade out in the crossfade time
            for c in self.chans.iter_mut() {
                c.target = 0.0;
                c.pending = None;
            }
        }
    }

    fn bpm(&self) -> Option<f32> {
        Some(self.melody.bpm)
    }

    fn on_hold(&mut self, held: bool) {
        self.hold_mode = held;
    }

    fn on_ptap(&mut self) {
        if let Some(m) = self.last_midi {
            self.start_note(m, 1.2, true);
        }
    }

    fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        let fade_step = 1.0 / (CROSSFADE_S * self.sr);
        let mut env = 0.0f32;
        for (ls, rs) in l.iter_mut().zip(r.iter_mut()) {
            self.zh_advance(1);
            let m = self.mods();
            let mut y = 0.0f32;
            env = 0.0;
            for c in self.chans.iter_mut() {
                if c.active {
                    y += c.tick(&m, fade_step);
                    env = env.max(c.level * c.gain);
                }
            }
            let y = if y.is_finite() {
                soft_limit(y * MASTER)
            } else {
                0.0
            };
            *ls += y;
            *rs += y;
        }
        self.gate_env = env.clamp(0.0, 1.0);
    }

    fn reset(&mut self, seed: u32) {
        self.rng_seed = seed;
        let mut rng = Rng::new(seed);
        self.patch = roll(&mut rng);
        self.melody = make_seq(&mut rng);
        self.character = character_of(&self.melody, &self.patch);
        for c in self.chans.iter_mut() {
            c.silence();
        }
        self.build_lfos();
        self.zhold.reset();
        self.zh = 0.0;
        self.last_midi = None;
        self.gate_env = 0.0;
    }

    fn fill_telemetry(&self, t: &mut Telemetry) {
        t.bpm = self.melody.bpm;
        t.root = self.melody.root;
        t.scale = self.melody.scale as u32;
        // legacy setAccent(): hue from coldness, warm amber to cold blue
        t.hue = (28.0 - 170.0 * self.character.cold as f32 + 360.0) % 360.0;
        t.brightness = self.character.bright as f32;
        t.activity = self.character.act as f32;
        t.gate_env = self.gate_env;
        for (p, s) in t.pattern.iter_mut().zip(self.melody.steps.iter()) {
            *p = s.velocity;
        }
    }
}
