/// Peak follower: instant attack, exponential release with a 300 ms time constant.
#[derive(Clone, Debug)]
pub struct PeakFollower {
    v: f32,
    coef: f32,
}

impl PeakFollower {
    pub fn new(sr: f32) -> PeakFollower {
        let coef = (-1.0 / (0.3 * sr as f64)).exp() as f32;
        PeakFollower { v: 0.0, coef }
    }

    pub fn tick(&mut self, x: f32) {
        self.v = x.abs().max(self.v * self.coef);
    }

    pub fn value(&self) -> f32 {
        self.v
    }

    pub fn reset(&mut self) {
        self.v = 0.0;
    }
}
