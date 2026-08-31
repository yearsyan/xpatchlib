//! Round-trip and adversarial tests for every registered codec. These
//! exercise the producer, so the whole file compiles only with the
//! `produce` feature.

#![cfg(feature = "produce")]

use xpatchlib_core::{algorithms, apply, create, patch_info, Error};

/// xorshift64* — deterministic, no dependencies.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn byte(&mut self) -> u8 {
        (self.next_u64() >> 32) as u8
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

/// Synthetic app bundle: numbered module functions with bulky string
/// payloads, stable framework blocks, JSON-ish literals and cross-module
/// requires — the shape minified JavaScript actually has.
fn synthesize_bundle(seed: u64, size: usize) -> Vec<u8> {
    let mut rng = Rng(seed | 1);
    let token = |rng: &mut Rng, len: usize| -> String {
        const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
        (0..len)
            .map(|_| ALPHABET[rng.below(ALPHABET.len() as u64) as usize] as char)
            .collect()
    };
    let mut out: Vec<u8> = Vec::with_capacity(size + 512);
    out.extend_from_slice(b"// __d(function(global, require, module, exports) {\n");
    let mut module = 0u64;
    while out.len() < size {
        module += 1;
        match module % 7 {
            0 => {
                out.extend_from_slice(
                    format!(
                        "__d({}, function(m){{m.exports={{id:{},flags:{},created:\"{}\"}};}});\n",
                        module, module, rng.below(1 << 20), token(&mut rng, 24)
                    )
                    .as_bytes(),
                );
            }
            1..=3 => {
                out.extend_from_slice(
                    format!(
                        "__d({}, function(a,b,c){{var v{}={};for(var i=0;i<{};i++){{v{}=(v{}*{}+%)={};}}c.exports='payload-{}-{}';}});\n",
                        module,
                        module,
                        rng.below(1 << 16),
                        rng.below(64) + 8,
                        module,
                        module,
                        rng.below(900) + 100,
                        rng.below(4093),
                        module,
                        { let len = rng.below(180) as usize + 40; token(&mut rng, len) }
                    )
                    .as_bytes(),
                );
            }
            4 => {
                let block = format!("/* stable framework block {} */\n", module % 13);
                out.extend_from_slice(block.repeat(20).as_bytes());
            }
            5 => {
                let parts: Vec<String> = (0..64)
                    .map(|_| match rng.below(3) {
                        0 => format!("{{\"k\":{},\"t\":{}}}", rng.below(1 << 20), token(&mut rng, 12)),
                        1 => format!("{}", rng.below(1 << 28)),
                        _ => format!("\"{}\"", token(&mut rng, 30)),
                    })
                    .collect();
                out.extend_from_slice(
                    format!("__d({}, function(a,b){{a.injectionBatch([{}]);}});\n", module, parts.join(",")).as_bytes(),
                );
            }
            _ => {
                out.extend_from_slice(
                    format!(
                        "require({});require({});require({});\n",
                        rng.below(module) + 1,
                        rng.below(module) + 1,
                        rng.below(module) + 1
                    )
                    .as_bytes(),
                );
            }
        }
    }
    out.extend_from_slice(b"// bundle trailing marker\n");
    out
}

/// Derives the "next release" of a bundle: drops, reorders, edits and
/// inserts lines, and splices in one new dependency-sized block.
fn mutate_bundle(seed: u64, base: &[u8]) -> Vec<u8> {
    let mut rng = Rng(seed | 1);
    let text = String::from_utf8(base.to_vec()).unwrap();
    let lines: Vec<&str> = text.split('\n').collect();
    let mut kept: Vec<String> = Vec::with_capacity(lines.len() + 64);
    for line in lines {
        if line.contains("framework block") {
            kept.push(line.to_string()); // framework code stays put
        } else if rng.below(100) < 3 {
            // dropped line
        } else if rng.below(100) < 6 {
            kept.push(line.replacen("payload-", "payload-v2-", 1));
        } else if rng.below(100) < 2 {
            kept.push(line.to_string());
            kept.push(line.to_string());
        } else {
            kept.push(line.to_string());
        }
    }
    let cut = kept.len() * 2 / 5;
    let mut next: Vec<String> = Vec::with_capacity(kept.len() + 64);
    next.extend(kept[..cut].iter().cloned());
    for i in 0..48 {
        let filler = format!("{:?}", "x".repeat(rng.below(120) as usize));
        next.push(format!(
            "__d(dep{}, function(a,b,c){{b.run({},{});}});\n",
            i,
            rng.below(500),
            filler
        ));
    }
    next.extend(kept[cut..].iter().cloned());
    if next.len() > 200 {
        let (i, j) = (next.len() / 4, next.len() * 3 / 4);
        let tail: Vec<String> = next[j..j + 16].to_vec();
        next.splice(i..i + 16, tail);
    }
    next.join("\n").into_bytes()
}

fn cases() -> Vec<(&'static str, Vec<u8>, Vec<u8>)> {
    let mut rng = Rng(11);
    let base = synthesize_bundle(42, 512 * 1024);
    let updated = mutate_bundle(43, &base);
    let mut random_a = vec![0u8; 64 * 1024];
    let mut random_b = vec![0u8; 64 * 1024];
    for (a, b) in random_a.iter_mut().zip(random_b.iter_mut()) {
        *a = rng.byte();
        *b = rng.byte();
    }
    let shifted = {
        let mut v = Vec::with_capacity(random_a.len());
        v.extend_from_slice(&random_a[32 * 1024..]);
        v.extend_from_slice(&random_a[..32 * 1024]);
        v
    };
    vec![
        ("both empty", Vec::new(), Vec::new()),
        ("empty base", Vec::new(), b"a brand new bundle".to_vec()),
        (
            "empty result",
            b"everything gets deleted".to_vec(),
            Vec::new(),
        ),
        (
            "identical",
            b"hello world hello world".to_vec(),
            b"hello world hello world".to_vec(),
        ),
        (
            "prefix edit",
            b"the quick brown fox".to_vec(),
            b"THE quick brown fox".to_vec(),
        ),
        ("suffix append", b"prefix".to_vec(), b"prefix plus a longer suffix".to_vec()),
        ("unrelated random", random_a.clone(), random_b),
        ("shifted block", random_a.clone(), shifted),
        ("synthetic bundle", base, updated),
    ]
}

#[test]
fn registry_lists_all_codecs() {
    assert_eq!(algorithms(), vec!["bsdiff", "zdict", "block", "full"]);
}

#[test]
fn create_apply_round_trip_all_codecs() {
    for algorithm in algorithms() {
        for (name, base, updated) in cases() {
            let patch = create(algorithm, &base, &updated)
                .unwrap_or_else(|e| panic!("{algorithm}/{name} create: {e}"));
            let info = patch_info(&patch)
                .unwrap_or_else(|e| panic!("{algorithm}/{name} patch_info: {e}"));
            assert_eq!(info.algorithm, algorithm, "{algorithm}/{name}");
            assert_eq!(info.base_size, base.len() as u64, "{algorithm}/{name}");
            assert_eq!(info.result_size, updated.len() as u64, "{algorithm}/{name}");
            let restored = apply(&patch, &base)
                .unwrap_or_else(|e| panic!("{algorithm}/{name} apply: {e}"));
            assert_eq!(restored, updated, "{algorithm}/{name} output diverges");
        }
    }
}

#[test]
fn apply_rejects_wrong_base() {
    let base = synthesize_bundle(1, 64 * 1024);
    let updated = mutate_bundle(2, &base);
    let patch = create("bsdiff", &base, &updated).unwrap();
    match apply(&patch, &updated) {
        Err(Error::BaseMismatch { .. }) => {}
        other => panic!("expected BaseMismatch, got {other:?}"),
    }
}

#[test]
fn apply_rejects_corrupt_payload() {
    let base = synthesize_bundle(3, 256 * 1024);
    let updated = mutate_bundle(4, &base);
    for algorithm in algorithms() {
        let patch = create(algorithm, &base, &updated).unwrap();
        for offset in [patch.len() / 3, patch.len() / 2, patch.len() - 5] {
            let mut corrupt = patch.clone();
            corrupt[offset] ^= 0xff;
            assert!(
                apply(&corrupt, &base).is_err(),
                "{algorithm}: corrupt payload at {offset} applied without error"
            );
        }
    }
}

#[test]
fn apply_rejects_truncated_and_forged_patches() {
    let base = b"some base bundle".to_vec();
    let patch = create("zdict", &base, b"some base bundle v2").unwrap();
    assert!(apply(&patch[..patch.len() - 1], &base).is_err());
    let mut bad_magic = patch.clone();
    bad_magic[0] = b'Q';
    assert!(apply(&bad_magic, &base).is_err());
}

#[test]
fn create_is_deterministic() {
    let base = synthesize_bundle(5, 128 * 1024);
    let updated = mutate_bundle(6, &base);
    for algorithm in algorithms() {
        let first = create(algorithm, &base, &updated).unwrap();
        for _ in 0..3 {
            let again = create(algorithm, &base, &updated).unwrap();
            assert_eq!(first, again, "{algorithm}: create is not deterministic");
        }
    }
}

#[test]
fn delta_patches_beat_the_full_baseline() {
    let base = synthesize_bundle(7, 512 * 1024);
    let updated = mutate_bundle(8, &base);
    let full = create("full", &base, &updated).unwrap();
    for algorithm in ["bsdiff", "zdict", "block"] {
        let patch = create(algorithm, &base, &updated).unwrap();
        assert!(
            patch.len() < full.len(),
            "{algorithm} patch ({} bytes) is not smaller than the full baseline ({} bytes)",
            patch.len(),
            full.len()
        );
    }
}
