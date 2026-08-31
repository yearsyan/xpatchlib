//! The bsdiff algorithm (Percival 2003): build a suffix array over the base
//! bundle, walk the new bundle finding the longest match for every scan
//! position via binary search, and emit classic bsdiff control tuples. It
//! consistently produces the smallest patches for minified JavaScript and
//! Hermes bytecode, at the cost of a suffix-array build on the producer
//! side. Replay stays linear time and needs no index at all, which is what
//! the client cares about — everything below besides `apply` is compiled
//! only with the `produce` feature.

use crate::control::{apply_control_stream, decode_control_stream};
#[cfg(feature = "produce")]
use crate::control::serialize_segments;
#[cfg(feature = "produce")]
use crate::control::SegmentBuilder;
#[cfg(feature = "produce")]
use crate::error::Error;
use crate::error::Result;
#[cfg(feature = "produce")]
use crate::sais::suffix_array;

/// Shortest copy region worth a control tuple.
#[cfg(feature = "produce")]
const MIN_MATCH: usize = 8;
/// Bounds the base size so suffix positions fit the SA-IS integer alphabet.
#[cfg(feature = "produce")]
const MAX_INPUT: usize = i32::MAX as usize;

pub struct Bsdiff;

impl crate::Codec for Bsdiff {
    fn name(&self) -> &'static str {
        "bsdiff"
    }

    #[cfg(feature = "produce")]
    fn create(&self, base: &[u8], updated: &[u8]) -> Result<Vec<u8>> {
        if base.len() > MAX_INPUT || updated.len() > MAX_INPUT {
            return Err(Error::Codec("bsdiff: input exceeds 2 GiB".into()));
        }
        let sa = suffix_array(base);

        let mut builder = SegmentBuilder::new();
        let mut scan = 0usize;
        while scan < updated.len() {
            let (length, pos) = longest_match(&sa, base, updated, scan);
            if length < MIN_MATCH {
                builder.extra(updated[scan]);
                scan += 1;
                continue;
            }
            builder.match_region(scan, length, pos);
            scan += length;
        }
        serialize_segments(base, updated, &builder.finish())
    }

    fn apply(&self, base: &[u8], payload: &[u8], expected_new_size: u64) -> Result<Vec<u8>> {
        let (tuples, diff, extra) = decode_control_stream(payload, expected_new_size)?;
        apply_control_stream(base, &tuples, &diff, &extra, expected_new_size)
    }
}

/// Finds the longest prefix of `updated[scan..]` that also occurs in `base`,
/// using the suffix array. The two suffixes adjacent to the binary search
/// boundary are the only candidates: every other suffix shares a strictly
/// shorter prefix with the target.
#[cfg(feature = "produce")]
fn longest_match(sa: &[i32], base: &[u8], updated: &[u8], scan: usize) -> (usize, usize) {
    let target = &updated[scan..];
    let (mut lo, mut hi) = (0usize, sa.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if compare_suffix(base, sa[mid] as usize, target) < 0 {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let mut best = (0usize, 0usize);
    for idx in [lo.wrapping_sub(1), lo] {
        if idx >= sa.len() {
            continue;
        }
        let p = sa[idx] as usize;
        if p >= base.len() {
            continue; // the empty suffix entry
        }
        let l = common_prefix(base, p, target);
        if l > best.0 {
            best = (l, p);
        }
    }
    best
}

/// Orders `base[p:]` against `target` the way the suffix array does: the
/// shorter prefix sorts first on ties, and the empty suffix (p == len)
/// sorts below everything.
#[cfg(feature = "produce")]
fn compare_suffix(base: &[u8], p: usize, target: &[u8]) -> i32 {
    let suffix = &base[p..];
    let limit = suffix.len().min(target.len());
    for i in 0..limit {
        match suffix[i].cmp(&target[i]) {
            std::cmp::Ordering::Less => return -1,
            std::cmp::Ordering::Greater => return 1,
            std::cmp::Ordering::Equal => {}
        }
    }
    match (suffix.len(), target.len()) {
        (a, b) if a == b => 0,
        (a, _) if a == limit => -1, // suffix exhausted (or equal prefix, target longer)
        (_, _) => 1,                // target exhausted, suffix continues
    }
}

/// Longest common prefix of `base[p:]` and `target`. Producer-side matchers
/// (bsdiff binary search, block candidate verification) are its only users.
#[cfg(feature = "produce")]
pub(crate) fn common_prefix(base: &[u8], p: usize, target: &[u8]) -> usize {
    let suffix = &base[p..];
    let limit = suffix.len().min(target.len());
    let mut i = 0;
    while i < limit && suffix[i] == target[i] {
        i += 1;
    }
    i
}
