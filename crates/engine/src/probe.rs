use crate::protocol::{Gesture, StepEvent, World};
use versum_dsp::ramp::Ramp;
use versum_dsp::rng::Rng;

const TONE_AMP: f64 = 0.4;
const BLIP_AMP: f64 = 0.3;
const BLIP_HZ: f64 = 1000.0;
const BLIP_MS: f32 = 10.0;

/// Probe world: a sine whose pitch follows `x` (110-880 Hz, exponential) and level follows `y`,
/// plus a 10 ms linearly decaying 1 kHz blip on every step. Silent until a gesture sets `y > 0`.
pub struct ProbeWorld {
    sr: f64,
    rng: Rng,
    phase: f64,
    freq: f64,
    vol: Ramp,
    blip_len: u32,
    blip_n: u32,
    blip_vel: f32,
}

impl ProbeWorld {
    pub fn new(sr: f32, seed: u32) -> ProbeWorld {
        let mut w = ProbeWorld {
            sr: sr as f64,
            rng: Rng::new(seed),
            phase: 0.0,
            freq: 110.0,
            vol: Ramp::new(sr, 20.0),
            blip_len: ((sr * BLIP_MS / 1000.0).round() as u32).max(1),
            blip_n: u32::MAX,
            blip_vel: 0.0,
        };
        w.reset(seed);
        w
    }
}

impl World for ProbeWorld {
    fn on_gesture(&mut self, g: &Gesture) {
        if g.x.is_finite() {
            self.freq = 110.0 * 8.0f64.powf(g.x.clamp(0.0, 1.0) as f64);
        }
        if g.y.is_finite() {
            self.vol.set_target(g.y.clamp(0.0, 1.0));
        }
    }

    fn on_step(&mut self, s: &StepEvent) {
        self.blip_n = 0;
        self.blip_vel = s.velocity;
    }

    fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        for (ls, rs) in l.iter_mut().zip(r.iter_mut()) {
            let mut y = TONE_AMP * self.vol.next() as f64 * (std::f64::consts::TAU * self.phase).sin();
            self.phase += self.freq / self.sr;
            self.phase -= self.phase.floor();
            if self.blip_n < self.blip_len {
                let env = 1.0 - self.blip_n as f64 / self.blip_len as f64;
                let t = self.blip_n as f64 / self.sr;
                y += BLIP_AMP * self.blip_vel as f64 * env * (std::f64::consts::TAU * BLIP_HZ * t).cos();
                self.blip_n += 1;
            }
            *ls = y as f32;
            *rs = y as f32;
        }
    }

    fn reset(&mut self, seed: u32) {
        self.rng = Rng::new(seed);
        self.phase = self.rng.next_f64();
        self.freq = 110.0;
        self.vol.reset();
        self.blip_n = u32::MAX;
        self.blip_vel = 0.0;
    }
}
