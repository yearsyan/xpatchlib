//! Compares every registered codec against a pair of bundle builds.
//! Producer-side tooling, so the whole file compiles only with the
//! `produce` feature.
//!
//!     cargo run -p xpatchlib-core --example bench -- old.js new.js
//!     cargo run -p xpatchlib-core --example bench -- -size 2097152

#![cfg(feature = "produce")]

use std::time::Instant;

use xpatchlib_core::{algorithms, apply, create};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn token(rng: &mut Rng, len: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
    (0..len)
        .map(|_| ALPHABET[rng.below(ALPHABET.len() as u64) as usize] as char)
        .collect()
}

fn synthesize(seed: u64, size: usize) -> Vec<u8> {
    let mut rng = Rng(seed | 1);
    let mut out = b"// __d(function(global, require, module, exports) {\n".to_vec();
    let mut module = 0u64;
    while out.len() < size {
        module += 1;
        match module % 5 {
            0 => {
                let payload_len = rng.below(180) as usize + 40;
                out.extend_from_slice(
                    format!(
                        "__d({}, function(a,b,c){{c.exports='payload-{}-{}';}});\n",
                        module,
                        module,
                        token(&mut rng, payload_len)
                    )
                    .as_bytes(),
                );
            }
            1 => out.extend_from_slice(
                format!("/* stable framework block {} */\n", module % 13).repeat(20).as_bytes(),
            ),
            2 => out.extend_from_slice(
                format!(
                    "__d({}, function(a,b){{a.injectionBatch([{}]);}});\n",
                    module,
                    (0..48)
                        .map(|_| match rng.below(3) {
                            0 => format!("{{\"k\":{}}}", rng.below(1 << 20)),
                            1 => format!("{}", rng.below(1 << 28)),
                            _ => format!("\"{}\"", token(&mut rng, 30)),
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                )
                .as_bytes(),
            ),
            _ => out.extend_from_slice(
                format!(
                    "require({});require({});require({});\n",
                    rng.below(module) + 1,
                    rng.below(module) + 1,
                    rng.below(module) + 1
                )
                .as_bytes(),
            ),
        }
    }
    out
}

fn mutate(seed: u64, base: &[u8]) -> Vec<u8> {
    let mut rng = Rng(seed | 1);
    let text = String::from_utf8(base.to_vec()).expect("bundle is utf-8");
    let lines: Vec<&str> = text.split('\n').collect();
    let mut kept: Vec<String> = Vec::new();
    for line in lines {
        if line.contains("framework block") {
            kept.push(line.to_string());
        } else if rng.below(100) < 3 {
        } else if rng.below(100) < 6 {
            kept.push(line.replacen("payload-", "payload-v2-", 1));
        } else {
            kept.push(line.to_string());
        }
    }
    let cut = kept.len() * 2 / 5;
    let mut next: Vec<String> = kept[..cut].to_vec();
    for i in 0..48 {
        next.push(format!(
            "__d(dep{}, function(a,b,c){{c.exports={:?};}});\n",
            i,
            token(&mut rng, 90)
        ));
    }
    next.extend(kept[cut..].iter().cloned());
    next.join("\n").into_bytes()
}

fn human_bytes(n: usize) -> String {
    if n >= 1 << 20 {
        format!("{:.2} MiB", n as f64 / (1 << 20) as f64)
    } else if n >= 1 << 10 {
        format!("{:.1} KiB", n as f64 / (1 << 10) as f64)
    } else {
        format!("{n} B")
    }
}

fn main() {
    let mut size = 2usize << 20;
    let mut seed = 42u64;
    let mut files: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-size" => size = args.next().expect("-size needs a value").parse().expect("size"),
            "-seed" => seed = args.next().expect("-seed needs a value").parse().expect("seed"),
            _ => files.push(arg),
        }
    }

    let (base, updated, source) = if files.len() == 2 {
        (
            std::fs::read(&files[0]).expect("read old bundle"),
            std::fs::read(&files[1]).expect("read new bundle"),
            format!("{} -> {}", files[0], files[1]),
        )
    } else {
        let base = synthesize(seed, size);
        let updated = mutate(seed + 1, &base);
        (base, updated, format!("synthetic bundle (seed={seed})"))
    };

    println!(
        "base {}   result {}   {source}",
        human_bytes(base.len()),
        human_bytes(updated.len())
    );
    println!();
    println!("{:<10} {:>12} {:>8} {:>10} {:>10} {:>8}", "algorithm", "patch", "ratio", "diff", "apply", "verify");
    for algorithm in algorithms() {
        let started = Instant::now();
        let patch = match create(algorithm, &base, &updated) {
            Ok(patch) => patch,
            Err(err) => {
                println!("{algorithm:<10} FAILED: {err}");
                continue;
            }
        };
        let diff_ms = started.elapsed().as_millis();

        let started = Instant::now();
        let restored = apply(&patch, &base).expect("apply");
        let apply_ms = started.elapsed().as_millis();

        let verified = if restored == updated { "ok" } else { "MISMATCH" };
        println!(
            "{:<10} {:>12} {:>8} {:>9}ms {:>9}ms {:>8}",
            algorithm,
            human_bytes(patch.len()),
            format!("{:.2}%", patch.len() as f64 * 100.0 / updated.len() as f64),
            diff_ms,
            apply_ms,
            verified
        );
    }
}
