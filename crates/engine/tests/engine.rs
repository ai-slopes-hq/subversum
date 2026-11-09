use engine::{Command, Engine, Gesture, MAX_BLOCK};
use versum_dsp::testkit::{count_allocs, CountingAlloc};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

const SR: f32 = 48000.0;

fn g(x: f32, y: f32) -> Command {
    Command::Gesture(Gesture { x, y, z: 0.0, pinch: 0.0 })
}

fn render(e: &mut Engine, n: usize, block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut ol, mut or) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
    let mut done = 0;
    while done < n {
        let len = block.min(n - done);
        e.process(&mut l[..len], &mut r[..len]);
        ol.extend_from_slice(&l[..len]);
        or.extend_from_slice(&r[..len]);
        done += len;
    }
    (ol, or)
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn blockwise_equals_per_sample() {
    let mut a = Engine::new(SR, 7);
    let mut b = Engine::new(SR, 7);
    a.push(g(0.4, 0.9));
    b.push(g(0.4, 0.9));
    let (al, ar) = render(&mut a, 20000, MAX_BLOCK);
    let (bl, br) = render(&mut b, 20000, 1);
    assert_eq!(bits(&al), bits(&bl));
    assert_eq!(bits(&ar), bits(&br));
    assert!(al.iter().any(|&x| x != 0.0));
}

#[test]
fn no_alloc_in_process() {
    let mut e = Engine::new(SR, 3);
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    let (_, n) = count_allocs(|| {
        for i in 0..1000 {
            e.push(g((i % 10) as f32 / 10.0, 0.8));
            e.process(&mut l, &mut r);
        }
    });
    assert_eq!(n, 0);
}

#[test]
fn step_lands_on_exact_sample() {
    let mut e = Engine::new(SR, 1);
    e.push(Command::Tempo(120.0));
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    let mut out = Vec::new();
    let mut at_45 = 0;
    let mut at_46 = (0, 0);
    for blk in 0..=46 {
        e.process(&mut l, &mut r);
        out.extend_from_slice(&l);
        if blk == 45 {
            at_45 = e.telemetry().step_count;
        }
        if blk == 46 {
            let t = e.telemetry();
            at_46 = (t.step_count, t.steps[t.step_count as usize - 1].index);
        }
    }
    assert_eq!(at_45, 1, "only step 0 before sample 6016");
    assert_eq!(at_46, (2, 1), "step index 1 after block covering sample 6000");
    let first = out.iter().enumerate().skip(1000).find(|(_, x)| x.abs() > 0.01).map(|(i, _)| i);
    assert_eq!(first, Some(6000));
}

#[test]
fn outputs_finite_and_bounded() {
    let mut e = Engine::new(SR, 5);
    let mut rng = versum_dsp::Rng::new(99);
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    for _ in 0..(10 * 48000 / MAX_BLOCK) {
        e.push(g(rng.next_f64() as f32, rng.next_f64() as f32));
        e.process(&mut l, &mut r);
        for &s in l.iter().chain(r.iter()) {
            assert!(s.is_finite() && s.abs() <= 1.0, "{s}");
        }
        let t = e.telemetry();
        assert!(t.peak_l <= 1.0 && t.peak_r <= 1.0);
    }
}

#[test]
fn command_queue_overflow_is_counted() {
    let mut e = Engine::new(SR, 1);
    for i in 0..100 {
        e.push(g(i as f32 / 100.0, 0.5));
    }
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    e.process(&mut l, &mut r);
    assert_eq!(e.telemetry().dropped_commands, 36);
    // the queue is free again after the block
    e.push(g(0.1, 0.1));
    e.process(&mut l, &mut r);
    assert_eq!(e.telemetry().dropped_commands, 36);
}

#[test]
fn step_queue_overflow_keeps_newest_and_counts() {
    let mut e = Engine::new(SR, 1);
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    for _ in 0..937 {
        e.process(&mut l, &mut r); // steps 0..=19 (sample 114000 < 119936)
    }
    let t = e.telemetry();
    assert_eq!(t.step_count, 16);
    assert_eq!(t.dropped_steps, 4);
    assert_eq!(t.steps[15].index, 19);
    assert_eq!(t.steps[0].index, 4);
    assert!(t.steps.windows(2).all(|w| w[0].index < w[1].index));
    e.ack_steps();
    assert_eq!((e.telemetry().step_count, e.telemetry().dropped_steps), (0, 0));
}

#[test]
fn peaks_decay_after_silence() {
    let mut e = Engine::new(SR, 1);
    e.push(g(0.5, 1.0));
    render(&mut e, 48000, MAX_BLOCK);
    assert!(e.telemetry().peak_l > 0.1);
    e.push(g(0.5, 0.0));
    e.push(Command::Hold(true));
    render(&mut e, 5 * 14400, MAX_BLOCK);
    assert!(e.telemetry().peak_l < 0.01, "{}", e.telemetry().peak_l);
    assert!(e.telemetry().peak_r < 0.01);
}

#[test]
fn switch_world_and_seed_reset_state() {
    let mut e = Engine::new(SR, 9);
    let mut take = |c: Command| {
        e.push(c);
        e.push(g(0.3, 0.8));
        render(&mut e, 4 * MAX_BLOCK, MAX_BLOCK)
    };
    let o1 = take(Command::Seed(1));
    let o2 = take(Command::Seed(2));
    let o3 = take(Command::Seed(1));
    assert_ne!(bits(&o1.0), bits(&o2.0));
    assert_eq!(bits(&o1.0), bits(&o3.0));
    assert_eq!(bits(&o1.1), bits(&o3.1));
    let o4 = take(Command::SwitchWorld(0));
    let o5 = take(Command::SwitchWorld(0));
    assert_eq!(bits(&o4.0), bits(&o5.0));
}
