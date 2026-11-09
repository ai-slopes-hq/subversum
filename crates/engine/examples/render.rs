//! Offline render of the probe world to a 24-bit stereo WAV.
//! Usage: render --seconds 4 --seed 1 --out file.wav
use engine::{Command, Engine, Gesture, MAX_BLOCK};
use std::io::Write;

const SR: u32 = 48000;

fn main() {
    let (mut seconds, mut seed, mut out) = (4.0f32, 1u32, String::from("render.wav"));
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let v = args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--seconds" => seconds = v.parse().expect("--seconds"),
            "--seed" => seed = v.parse().expect("--seed"),
            "--out" => out = v,
            _ => panic!("unknown argument {a}"),
        }
    }
    let total = (seconds * SR as f32).round() as usize;
    let mut e = Engine::new(SR as f32, seed);
    let mut data = Vec::with_capacity(total * 6);
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    let mut done = 0;
    while done < total {
        let n = MAX_BLOCK.min(total - done);
        let x = 0.5 + 0.5 * (done as f32 / SR as f32 * 0.5).sin();
        e.push(Command::Gesture(Gesture { x, y: 0.7, z: 0.0, pinch: 0.0 }));
        e.process(&mut l[..n], &mut r[..n]);
        for i in 0..n {
            for s in [l[i], r[i]] {
                let v = (s.clamp(-1.0, 1.0) * 8388607.0).round() as i32;
                data.extend_from_slice(&v.to_le_bytes()[..3]);
            }
        }
        done += n;
    }
    let mut f = std::fs::File::create(&out).expect("create output");
    let len = data.len() as u32;
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + len).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes()); // PCM
    h.extend_from_slice(&2u16.to_le_bytes());
    h.extend_from_slice(&SR.to_le_bytes());
    h.extend_from_slice(&(SR * 6).to_le_bytes());
    h.extend_from_slice(&6u16.to_le_bytes());
    h.extend_from_slice(&24u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&len.to_le_bytes());
    f.write_all(&h).and_then(|_| f.write_all(&data)).expect("write");
}
