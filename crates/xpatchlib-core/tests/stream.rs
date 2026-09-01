//! File-based streaming replay tests. Producers are exercised to build
//! the patches, so the whole file compiles only with the `produce`
//! feature; the streaming replay itself is what the mobile clients run.

#![cfg(feature = "produce")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use xpatchlib_core::{apply, apply_file, create, Error};

/// Scratch directory per test process; removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "xpatchlib-stream-{}-{}-{}",
            name,
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Deterministic pseudo-random data, no dependencies.
fn blob(seed: u64, size: usize) -> Vec<u8> {
    let mut state = seed | 1;
    (0..size)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 32) as u8
        })
        .collect()
}

fn cases() -> Vec<(&'static str, Vec<u8>, Vec<u8>)> {
    let base = blob(7, 400 * 1024);
    // A "next release": mostly the base with edits, splices and new data.
    let mut updated = blob(8, 40 * 1024);
    updated.extend_from_slice(&base[30 * 1024..300 * 1024]);
    updated[..1024].iter_mut().for_each(|b| *b ^= 0x55);
    updated.extend_from_slice(&blob(9, 120 * 1024));
    vec![
        ("both empty", Vec::new(), Vec::new()),
        ("empty base", Vec::new(), b"a brand new bundle".to_vec()),
        ("empty result", b"everything gets deleted".to_vec(), Vec::new()),
        ("identical", b"hello world".to_vec(), b"hello world".to_vec()),
        ("prefix edit", b"the quick brown fox".to_vec(), b"THE quick brown fox".to_vec()),
        ("suffix append", b"prefix".to_vec(), b"prefix plus a longer suffix".to_vec()),
        ("synthetic bundle", base, updated),
    ]
}

#[test]
fn apply_file_round_trips_all_codecs() {
    for algorithm in xpatchlib_core::algorithms() {
        for (name, base, updated) in cases() {
            let dir = TempDir::new("roundtrip");
            let patch = create(algorithm, &base, &updated)
                .unwrap_or_else(|e| panic!("{algorithm}/{name} create: {e}"));
            std::fs::write(dir.path("patch.xpdl"), &patch).unwrap();
            std::fs::write(dir.path("base.bin"), &base).unwrap();
            apply_file(
                dir.path("patch.xpdl"),
                dir.path("base.bin"),
                dir.path("out.bin"),
            )
            .unwrap_or_else(|e| panic!("{algorithm}/{name} apply_file: {e}"));
            let restored = std::fs::read(dir.path("out.bin")).unwrap();
            assert_eq!(restored, updated, "{algorithm}/{name} output diverges");
        }
    }
}

#[test]
fn apply_file_matches_the_in_memory_replay() {
    let (base, updated) = (blob(21, 300 * 1024), blob(22, 340 * 1024));
    for algorithm in ["bsdiff", "zdict", "block", "full"] {
        let patch = create(algorithm, &base, &updated).unwrap();
        let dir = TempDir::new("parity");
        std::fs::write(dir.path("patch.xpdl"), &patch).unwrap();
        std::fs::write(dir.path("base.bin"), &base).unwrap();
        apply_file(dir.path("patch.xpdl"), dir.path("base.bin"), dir.path("out.bin")).unwrap();
        assert_eq!(
            std::fs::read(dir.path("out.bin")).unwrap(),
            apply(&patch, &base).unwrap(),
            "{algorithm}: streaming and in-memory replays diverge"
        );
    }
}

#[test]
fn apply_file_removes_output_on_failure() {
    let (base, updated) = (blob(31, 200 * 1024), blob(32, 240 * 1024));
    let patch = create("bsdiff", &base, &updated).unwrap();
    let dir = TempDir::new("failure");
    std::fs::write(dir.path("patch.xpdl"), &patch).unwrap();

    // Wrong base: hash check fails before any output byte is written.
    std::fs::write(dir.path("base.bin"), blob(33, 200 * 1024)).unwrap();
    match apply_file(dir.path("patch.xpdl"), dir.path("base.bin"), dir.path("out.bin")) {
        Err(Error::BaseMismatch { .. }) => {}
        other => panic!("expected BaseMismatch, got {other:?}"),
    }
    assert!(!dir.path("out.bin").exists(), "failed replay left an output file");

    // Corrupt payload (diff stream region): replay fails mid-stream.
    let mut corrupt = patch.clone();
    let last = corrupt.len() - 4;
    corrupt[last] ^= 0xff;
    std::fs::write(dir.path("corrupt.xpdl"), &corrupt).unwrap();
    std::fs::write(dir.path("base.bin"), &base).unwrap();
    assert!(apply_file(dir.path("corrupt.xpdl"), dir.path("base.bin"), dir.path("out2.bin")).is_err());
    assert!(!dir.path("out2.bin").exists(), "failed replay left an output file");

    // Forged result hash: replay produces bytes that fail the final check.
    let mut forged = patch.clone();
    forged[6 + "bsdiff".len() + 8 + 32 + 8] ^= 0xff; // first byte of result hash
    std::fs::write(dir.path("forged.xpdl"), &forged).unwrap();
    match apply_file(dir.path("forged.xpdl"), dir.path("base.bin"), dir.path("out3.bin")) {
        Err(Error::ChecksumMismatch) => {}
        other => panic!("expected ChecksumMismatch, got {other:?}"),
    }
    assert!(!dir.path("out3.bin").exists(), "failed replay left an output file");

    // Missing patch file surfaces as Io, not a panic.
    match apply_file(dir.path("absent.xpdl"), dir.path("base.bin"), dir.path("out4.bin")) {
        Err(Error::Io(_)) => {}
        other => panic!("expected Io, got {other:?}"),
    }
}

#[test]
fn apply_file_replaces_existing_output() {
    let (base, updated) = (blob(41, 64 * 1024), blob(42, 72 * 1024));
    let patch = create("block", &base, &updated).unwrap();
    let dir = TempDir::new("replace");
    std::fs::write(dir.path("patch.xpdl"), &patch).unwrap();
    std::fs::write(dir.path("base.bin"), &base).unwrap();
    std::fs::write(dir.path("out.bin"), b"stale bytes from an earlier attempt").unwrap();
    apply_file(dir.path("patch.xpdl"), dir.path("base.bin"), dir.path("out.bin")).unwrap();
    assert_eq!(std::fs::read(dir.path("out.bin")).unwrap(), updated);
}
