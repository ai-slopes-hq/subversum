//! Offline render of the engine to a 24-bit stereo WAV.
//! Usage: render [--seed N] [--gesture neutral|x|y|z|ptap] [--secs S] [--out path.wav]
//! Axes are 0..1 on the wire (neutral 0.5): x = 1.0, y = 0.0 (macro -1), z = seconds held,
//! ptap = PTap at 1 s and 4 s. Default output: target/wav/seed<N>_<gesture>.wav.
use engine::{Command, Engine, Gesture, MAX_BLOCK};
use std::io::Write;

const SR: u32 = 48000;

fn gesture_at(name: &str, t: f32) -> Gesture {
    let (x, y, z) = match name {
        "neutral" | "ptap" => (0.5, 0.5, 0.0),
        "x" => (1.0, 0.5, 0.0),
        "y" => (0.5, 0.0, 0.0),
        "z" => (0.5, 0.5, t),
        _ => panic!("unknown gesture {name} (neutral|x|y|z|ptap)"),
    };
    Gesture {
        x,
        y,
        z,
        pinch: 0.0,
    }
}

fn wav_header(data_len: u32) -> Vec<u8> {
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data_len).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes()); // PCM
    h.extend_from_slice(&2u16.to_le_bytes());
    h.extend_from_slice(&SR.to_le_bytes());
    h.extend_from_slice(&(SR * 6).to_le_bytes());
    h.extend_from_slice(&6u16.to_le_bytes());
    h.extend_from_slice(&24u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data_len.to_le_bytes());
    h
}

fn main() {
    let (mut secs, mut seed, mut gesture, mut out) =
        (8.0f32, 1u32, String::from("neutral"), None::<String>);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let v = args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--secs" | "--seconds" => secs = v.parse().expect("--secs"),
            "--seed" => seed = v.parse().expect("--seed"),
            "--gesture" => gesture = v,
            "--out" => out = Some(v),
            _ => panic!("unknown argument {a}"),
        }
    }
    let out = out.unwrap_or_else(|| {
        format!(
            "{}/../../target/wav/seed{seed}_{gesture}.wav",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    let total = (secs * SR as f32).round() as usize;
    let mut e = Engine::new(SR as f32, seed);
    let mut data = Vec::with_capacity(total * 6);
    let (mut l, mut r) = ([0.0f32; MAX_BLOCK], [0.0f32; MAX_BLOCK]);
    let mut ptaps = [false; 2];
    let mut done = 0;
    while done < total {
        let n = MAX_BLOCK.min(total - done);
        let t = done as f32 / SR as f32;
        e.push(Command::Gesture(gesture_at(&gesture, t)));
        if gesture == "ptap" {
            for (i, at) in [1.0f32, 4.0].iter().enumerate() {
                if !ptaps[i] && t >= *at {
                    ptaps[i] = true;
                    e.push(Command::PTap);
                }
            }
        }
        e.process(&mut l[..n], &mut r[..n]);
        for i in 0..n {
            for s in [l[i], r[i]] {
                let v = (s.clamp(-1.0, 1.0) * 8388607.0).round() as i32;
                data.extend_from_slice(&v.to_le_bytes()[..3]);
            }
        }
        done += n;
    }
    if let Some(dir) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(dir).expect("create output dir");
    }
    let mut f = std::fs::File::create(&out).expect("create output");
    f.write_all(&wav_header(data.len() as u32))
        .and_then(|_| f.write_all(&data))
        .expect("write");
}
