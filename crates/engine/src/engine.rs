use crate::amber::AmberWorld;
use crate::peaks::PeakFollower;
use crate::probe::ProbeWorld;
use crate::protocol::*;
use versum_dsp::dc::DcBlock;
use versum_dsp::knee::Knee;
use versum_dsp::stepclock::StepClock;
use versum_dsp::Block;

const DEFAULT_BPM: f32 = 120.0;
const WORLD_PROBE: u32 = 0;
const WORLD_AMBER: u32 = 1;

/// Realtime engine. All state is preallocated in `new`; `push`/`process`/`telemetry` never allocate.
///
/// Commands are queued by `push` and applied at the start of the next `process` call, in order.
/// Steps (sixteenth notes) are generated on the sample grid: a step fires before rendering the
/// first sample at or after its due time, so blocking never moves it. A `Tempo` change affects
/// the distance to steps scheduled after it; the already-pending step keeps its due time.
pub struct Engine {
    world: Box<dyn World>,
    /// Id of `world`, and the preallocated inactive built-in world (swapped in by `SwitchWorld`).
    world_id: u32,
    spare: Option<(u32, Box<dyn World>)>,
    queue: [Command; CMD_QUEUE],
    queued: usize,
    knee: [Knee; 2],
    dc: [DcBlock; 2],
    peak: [PeakFollower; 2],
    tel: Telemetry,
    hold: bool,
    lock: bool,
    clock: StepClock,
    step_index: u32,
}

impl Engine {
    pub fn new(sr: f32, seed: u32) -> Engine {
        let mut e = Engine::build(
            sr,
            Box::new(AmberWorld::new(sr, seed)),
            WORLD_AMBER,
            Some((WORLD_PROBE, Box::new(ProbeWorld::new(sr, seed)))),
        );
        e.reseed(seed);
        e
    }

    /// Engine with a single custom world; `SwitchWorld` just resets it.
    pub fn with_world(sr: f32, world: Box<dyn World>, seed: u32) -> Engine {
        let mut e = Engine::build(sr, world, u32::MAX, None);
        e.reseed(seed);
        e
    }

    fn build(
        sr: f32,
        world: Box<dyn World>,
        world_id: u32,
        spare: Option<(u32, Box<dyn World>)>,
    ) -> Engine {
        Engine {
            world,
            world_id,
            spare,
            queue: [Command::Hold(false); CMD_QUEUE],
            queued: 0,
            knee: [Knee::new(1.0, 0.9, 0.1), Knee::new(1.0, 0.9, 0.1)],
            dc: [DcBlock::new(5.0, sr), DcBlock::new(5.0, sr)],
            peak: [PeakFollower::new(sr), PeakFollower::new(sr)],
            tel: Telemetry::default(),
            hold: false,
            lock: false,
            clock: StepClock::new(sr as f64, DEFAULT_BPM as f64),
            step_index: 0,
        }
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
        self.world.on_hold(self.hold);
        self.clock.reset();
        if let Some(b) = self.world.bpm() {
            self.clock.set_bpm(b as f64);
        }
        self.step_index = 0;
        self.tel.peak_l = 0.0;
        self.tel.peak_r = 0.0;
        self.ack_steps();
    }

    fn apply(&mut self, c: Command) {
        match c {
            Command::Gesture(g) => self.world.on_gesture(&g),
            // Switching resets the target world (seed 0); an unknown id just resets the current one.
            Command::SwitchWorld(id) => {
                if id != self.world_id {
                    if let Some((sid, _)) = &self.spare {
                        if *sid == id {
                            if let Some((sid, w)) = self.spare.take() {
                                let old = std::mem::replace(&mut self.world, w);
                                self.spare = Some((self.world_id, old));
                                self.world_id = sid;
                            }
                        }
                    }
                }
                self.reseed(0);
            }
            Command::Seed(s) => self.reseed(s),
            Command::Tempo(b) => {
                self.clock.set_bpm(b as f64);
            }
            Command::Hold(h) => {
                self.hold = h;
                self.world.on_hold(h);
            }
            Command::Lock(l) => self.lock = l,
            Command::PTap => self.world.on_ptap(),
        }
    }

    fn fire_step(&mut self) {
        let i = self.step_index;
        self.step_index = i.wrapping_add(1);
        let ev = self.world.next_step(i);
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
        self.clock.consume();
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
            if !self.hold && self.clock.next_in() <= 0.0 {
                self.fire_step();
            }
            let chunk = if self.hold {
                n - pos
            } else {
                ((self.clock.next_in().ceil() as usize).max(1)).min(n - pos)
            };
            let end = pos + chunk;
            l[pos..end].fill(0.0);
            r[pos..end].fill(0.0);
            self.world.render(&mut l[pos..end], &mut r[pos..end]);
            if !self.hold {
                self.clock.advance(chunk);
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
        self.tel.frame = self.tel.frame.wrapping_add(n as u32);
        self.world.fill_telemetry(&mut self.tel);
    }
}
