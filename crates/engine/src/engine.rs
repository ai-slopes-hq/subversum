use crate::peaks::PeakFollower;
use crate::probe::ProbeWorld;
use crate::protocol::*;
use versum_dsp::dc::DcBlock;
use versum_dsp::knee::Knee;
use versum_dsp::Block;

const DEFAULT_BPM: f32 = 120.0;
const SCALE: [u32; 5] = [0, 3, 5, 7, 10];

/// Realtime engine. All state is preallocated in `new`; `push`/`process`/`telemetry` never allocate.
///
/// Commands are queued by `push` and applied at the start of the next `process` call, in order.
/// Steps (sixteenth notes) are generated on the sample grid: a step fires before rendering the
/// first sample at or after its due time, so blocking never moves it. A `Tempo` change affects
/// the distance to steps scheduled after it; the already-pending step keeps its due time.
pub struct Engine {
    sr: f32,
    world: Box<dyn World>,
    queue: [Command; CMD_QUEUE],
    queued: usize,
    knee: [Knee; 2],
    dc: [DcBlock; 2],
    peak: [PeakFollower; 2],
    tel: Telemetry,
    bpm: f32,
    hold: bool,
    lock: bool,
    /// Samples until the next step is due (<= 0 means due now).
    to_next: f64,
    step_index: u32,
}

impl Engine {
    pub fn new(sr: f32, seed: u32) -> Engine {
        let mut e = Engine {
            sr,
            world: Box::new(ProbeWorld::new(sr, seed)),
            queue: [Command::Hold(false); CMD_QUEUE],
            queued: 0,
            knee: [Knee::new(1.0, 0.9, 0.1), Knee::new(1.0, 0.9, 0.1)],
            dc: [DcBlock::new(5.0, sr), DcBlock::new(5.0, sr)],
            peak: [PeakFollower::new(sr), PeakFollower::new(sr)],
            tel: Telemetry::default(),
            bpm: DEFAULT_BPM,
            hold: false,
            lock: false,
            to_next: 0.0,
            step_index: 0,
        };
        e.reseed(seed);
        e
    }

    /// Queues a command; if the queue is full the command is dropped and counted.
    pub fn push(&mut self, c: Command) {
        if self.queued < CMD_QUEUE {
            self.queue[self.queued] = c;
            self.queued += 1;
        } else {
            self.tel.dropped_commands = self.tel.dropped_commands.saturating_add(1);
        }
    }

    pub fn telemetry(&self) -> &Telemetry {
        &self.tel
    }

    pub fn ack_steps(&mut self) {
        self.tel.step_count = 0;
        self.tel.dropped_steps = 0;
    }

    /// Whether `Lock` is engaged (reserved for worlds that need it; the probe ignores it).
    pub fn locked(&self) -> bool {
        self.lock
    }

    /// Deliberate reset: Seed/SwitchWorld discard unacknowledged steps without counting them in `dropped_steps`.
    fn reseed(&mut self, seed: u32) {
        self.world.reset(seed);
        for i in 0..2 {
            self.knee[i].reset();
            self.dc[i].reset();
            self.peak[i].reset();
        }
        self.to_next = 0.0;
        self.step_index = 0;
        self.tel.peak_l = 0.0;
        self.tel.peak_r = 0.0;
        self.ack_steps();
    }

    fn apply(&mut self, c: Command) {
        match c {
            Command::Gesture(g) => self.world.on_gesture(&g),
            // Only one world exists so far: switching resets it (seed 0).
            Command::SwitchWorld(_) => self.reseed(0),
            Command::Seed(s) => self.reseed(s),
            Command::Tempo(b) => {
                if b.is_finite() {
                    self.bpm = b.clamp(20.0, 300.0);
                }
            }
            Command::Hold(h) => self.hold = h,
            Command::Lock(l) => self.lock = l,
        }
    }

    fn fire_step(&mut self) {
        let i = self.step_index;
        self.step_index = i.wrapping_add(1);
        let ev = StepEvent {
            index: i,
            note: 48 + SCALE[(i % 5) as usize] + 12 * ((i / 5) % 2),
            velocity: if i.is_multiple_of(4) { 1.0 } else { 0.7 },
        };
        let t = &mut self.tel;
        if (t.step_count as usize) < STEP_QUEUE {
            t.steps[t.step_count as usize] = ev;
            t.step_count += 1;
        } else {
            // Queue full: drop the oldest, keep the newest.
            t.steps.copy_within(1.., 0);
            t.steps[STEP_QUEUE - 1] = ev;
            t.dropped_steps = t.dropped_steps.saturating_add(1);
        }
        self.world.on_step(&ev);
        self.to_next += self.sr as f64 * 60.0 / self.bpm as f64 / 4.0;
    }

    /// `l` and `r` must have equal length (<= `MAX_BLOCK`).
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        debug_assert!(l.len() == r.len() && l.len() <= MAX_BLOCK);
        let n = l.len().min(r.len());
        for i in 0..self.queued {
            let c = self.queue[i];
            self.apply(c);
        }
        self.queued = 0;

        let mut pos = 0;
        while pos < n {
            if !self.hold && self.to_next <= 0.0 {
                self.fire_step();
            }
            let chunk = if self.hold {
                n - pos
            } else {
                ((self.to_next.ceil() as usize).max(1)).min(n - pos)
            };
            let end = pos + chunk;
            l[pos..end].fill(0.0);
            r[pos..end].fill(0.0);
            self.world.render(&mut l[pos..end], &mut r[pos..end]);
            if !self.hold {
                self.to_next -= chunk as f64;
            }
            pos = end;
        }

        for i in 0..n {
            let a = self.dc[0].tick(self.knee[0].tick(l[i]));
            let b = self.dc[1].tick(self.knee[1].tick(r[i]));
            l[i] = a;
            r[i] = b;
            self.peak[0].tick(a);
            self.peak[1].tick(b);
        }
        self.tel.peak_l = self.peak[0].value();
        self.tel.peak_r = self.peak[1].value();
        self.tel.frame = self.tel.frame.wrapping_add(1);
    }
}
