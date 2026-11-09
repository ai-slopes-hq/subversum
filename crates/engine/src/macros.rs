//! Gesture -> XM (amber macros) and the Z-hold envelope. Ported literally from the legacy
//! `applyMacros` amber branch and the `ZH` timer (legacy/index.html 1868-1880, 1936-1942).
use crate::protocol::Gesture;

/// Per-note timbre multipliers, read at note start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xm {
    pub dec: f32,
    pub fm: f32,
    pub fold: f32,
    pub cut: f32,
}

fn smoothstep(a: f32, z: f32, v: f32) -> f32 {
    let t = ((v - a) / (z - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Axis 0..1 -> legacy macro -1..1 (`XY.nx = clamp(2x-1)`); non-finite is the centre.
fn axis(v: f32) -> f32 {
    if v.is_finite() {
        (v.clamp(0.0, 1.0)) * 2.0 - 1.0
    } else {
        0.0
    }
}

/// Legacy formulas on macro values `lpg`, `color` in -1..1 and `zh` in 0..1.
pub fn xm_macros(lpg: f32, color: f32, zh: f32) -> Xm {
    let zh = if zh.is_finite() {
        zh.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let u = (color + 1.0) / 2.0;
    let dec = (2.0f32).powf(lpg * 1.5);
    let fm = if u < 0.5 {
        2.0 * u
    } else {
        1.0 + 0.5 * (2.0 * u - 1.0)
    };
    let fold = if u < 0.5 {
        smoothstep(0.12, 0.5, u)
    } else {
        1.0 + 1.5 * (2.0 * u - 1.0)
    };
    let cut = (2.0f32).powf(color * 0.8);
    Xm {
        dec: dec * (2.0f32).powf(0.9 * zh),
        fm: fm * (1.0 + 0.35 * zh),
        fold,
        cut: cut * (2.0f32).powf(0.4 * zh),
    }
}

/// X (0..1) is the gate (`lpg`), Y (0..1, up is 1) is `color`; `zh` is the Z-hold 0..1.
pub fn xm(g: &Gesture, zh: f32) -> Xm {
    xm_macros(axis(g.x), axis(g.y), zh)
}

/// Z: one second of nothing, then it opens along a smoothstep over 11 s while a finger is held.
/// Without a finger it decays by 0.8 per 100 ms (snapping to 0 under 0.003) and the hold timer restarts.
pub struct ZHold {
    sr: f32,
    held_s: f32,
    zh: f32,
    release_acc: f32,
}

impl ZHold {
    pub fn new(sr: f32) -> Self {
        Self {
            sr,
            held_s: 0.0,
            zh: 0.0,
            release_acc: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.held_s = 0.0;
        self.zh = 0.0;
        self.release_acc = 0.0;
    }

    /// Advances by `n` samples and returns ZH in 0..1.
    pub fn tick_block(&mut self, held: bool, n: usize) -> f32 {
        let dt = n as f32 / self.sr;
        if held {
            self.release_acc = 0.0;
            self.held_s += dt;
            let u = ((self.held_s - 1.0) / 11.0).clamp(0.0, 1.0);
            self.zh = u * u * (3.0 - 2.0 * u);
        } else {
            self.held_s = 0.0;
            if self.zh > 0.0 {
                self.release_acc += dt;
                while self.release_acc >= 0.1 {
                    self.release_acc -= 0.1;
                    self.zh *= 0.8;
                    if self.zh < 0.003 {
                        self.zh = 0.0;
                        break;
                    }
                }
            }
        }
        self.zh
    }
}
