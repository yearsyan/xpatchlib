//! The block codec is an xdelta-style greedy matcher: every position of the
//! base is fingerprinted with a 16-byte window, the new bundle walks those
//! fingerprints, and matches may extend backwards into unmatched bytes. It
//! generates patches far faster than bsdiff (no suffix array) while still
//! beating plain recompression when content moved rather than changed.
//! The index builder is producer-only; replay needs nothing but the control
//! stream.

#[cfg(feature = "produce")]
use crate::bsdiff::common_prefix;
use crate::control::{apply_control_stream, decode_control_stream};
#[cfg(feature = "produce")]
use crate::control::{serialize_segments, SegmentBuilder};
#[cfg(feature = "produce")]
use crate::error::Error;
use crate::error::Result;

#[cfg(feature = "produce")]
const WINDOW: usize = 16;
#[cfg(feature = "produce")]
const MIN_MATCH: usize = 24;
#[cfg(feature = "produce")]
const MAX_CANDIDATES: usize = 8;
#[cfg(feature = "produce")]
const MAX_INPUT: usize = i32::MAX as usize;

pub struct Block;

impl crate::Codec for Block {
    fn name(&self) -> &'static str {
        "block"
    }

    #[cfg(feature = "produce")]
    fn create(&self, base: &[u8], updated: &[u8]) -> Result<Vec<u8>> {
        if base.len() > MAX_INPUT || updated.len() > MAX_INPUT {
            return Err(Error::Codec("block: input exceeds 2 GiB".into()));
        }
        let index = BlockIndex::build(base);

        let mut builder = SegmentBuilder::new();
        let mut scan = 0usize;
        while scan + WINDOW <= updated.len() {
            let fingerprint = window_fingerprint(&updated[scan..scan + WINDOW]);
            let mut best = (0usize, 0usize); // (length, base_pos)
            for candidate in index.candidates(fingerprint) {
                let pos = candidate as usize;
                let length = common_prefix(base, pos, &updated[scan..]);
                if length > best.0 {
                    best = (length, pos);
                }
            }
            if best.0 < MIN_MATCH {
                builder.extra(updated[scan]);
                scan += 1;
                continue;
            }
            // Extend backwards into the unmatched tail so shifted regions
            // that only anchor mid-window still copy in one piece.
            let mut back = 0usize;
            while back < builder.pending_len()
                && best.1 > back
                && base[best.1 - back - 1] == updated[scan - back - 1]
            {
                back += 1;
            }
            builder.reclaim_pending(back);
            builder.match_region(scan - back, best.0 + back, best.1 - back);
            scan += best.0;
        }
        while scan < updated.len() {
            builder.extra(updated[scan]);
            scan += 1;
        }
        serialize_segments(base, updated, &builder.finish())
    }

    fn apply(&self, base: &[u8], payload: &[u8], expected_new_size: u64) -> Result<Vec<u8>> {
        let (tuples, diff, extra) = decode_control_stream(payload, expected_new_size)?;
        apply_control_stream(base, &tuples, &diff, &extra, expected_new_size)
    }
}

/// Maps window fingerprints to base positions, sorted so equal fingerprints
/// form contiguous runs.
#[cfg(feature = "produce")]
struct BlockIndex {
    entries: Vec<(u32, i32)>,
}

#[cfg(feature = "produce")]
impl BlockIndex {
    fn build(base: &[u8]) -> Self {
        if base.len() < WINDOW {
            return BlockIndex { entries: Vec::new() };
        }
        let mut entries = Vec::with_capacity(base.len() - WINDOW + 1);
        for i in 0..=base.len() - WINDOW {
            entries.push((window_fingerprint(&base[i..i + WINDOW]), i as i32));
        }
        entries.sort_unstable();
        BlockIndex { entries }
    }

    fn candidates(&self, fingerprint: u32) -> impl Iterator<Item = i32> + '_ {
        let start = self.entries.partition_point(|e| e.0 < fingerprint);
        self.entries[start..]
            .iter()
            .take_while(move |e| e.0 == fingerprint)
            .take(MAX_CANDIDATES)
            .map(|e| e.1)
    }
}

/// FNV-1a over the match window; collisions are fine because candidates are
/// verified byte by byte before use.
#[cfg(feature = "produce")]
fn window_fingerprint(window: &[u8]) -> u32 {
    let mut hash: u32 = 2166136261;
    for &c in window {
        hash ^= c as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}
