//! The control stream is the shared payload format of the bsdiff and block
//! codecs: the classic bsdiff triple (add, copy, seek) followed by per-byte
//! deltas and verbatim bytes, each in its own zstd stream.
//!
//! Patch semantics match bspatch: for every tuple, `add` bytes are read from
//! the diff stream and added modulo 256 to `base[base_pos..]`, then `copy`
//! bytes are taken verbatim from the extra stream, then `base_pos` advances
//! by `seek` (which may be negative). A tuple's seek therefore positions the
//! tuple that follows it.
//!
//! Only the decoding half is compiled into replay builds; the segment
//! builder and serializer behind the `produce` feature never reach a
//! client binary.

use crate::error::{Error, Result};

const STREAM_MAGIC: &[u8; 4] = b"XPBS";
const STREAM_VERSION: u8 = 1;
const TUPLE_SIZE: usize = 24;
/// magic + version + three length fields.
const STREAM_HEADER: usize = 4 + 1 + 8 + 8 + 8;
/// zstd level for the three streams (deterministic: bulk API, single thread).
#[cfg(feature = "produce")]
const ZSTD_LEVEL: i32 = 9;

#[derive(Clone, Copy)]
pub(crate) struct ControlTuple {
    pub add: i64,
    pub copy: i64,
    pub seek: i64,
}

/// One run of producer output: either a region copied from the base (with
/// per-byte deltas) or bytes that exist only in the new bundle.
#[cfg(feature = "produce")]
pub(crate) enum Segment {
    Copy { pos: usize, len: usize },
    Extra(Vec<u8>),
}

/// Turns scan-time match decisions into segments while keeping a small tail
/// of unmatched bytes that a later backward-extended match may still absorb.
#[cfg(feature = "produce")]
pub(crate) struct SegmentBuilder {
    segments: Vec<Segment>,
    pending: Vec<u8>,
}

#[cfg(feature = "produce")]
impl SegmentBuilder {
    pub fn new() -> Self {
        SegmentBuilder {
            segments: Vec::new(),
            pending: Vec::new(),
        }
    }

    pub fn extra(&mut self, data: u8) {
        self.pending.push(data);
    }

    /// Records a copy region `updated[new_start..new_start+length]` taken
    /// from `base[base_pos..base_pos+length]` and flushes any pending
    /// unmatched bytes that precede it.
    pub fn match_region(&mut self, _new_start: usize, length: usize, base_pos: usize) {
        if !self.pending.is_empty() {
            self.segments.push(Segment::Extra(std::mem::take(&mut self.pending)));
        }
        self.segments.push(Segment::Copy { pos: base_pos, len: length });
    }

    /// Shrinks the pending tail by `n` bytes absorbed by a
    /// backward-extended match.
    /// Length of the pending tail (matchers consult it before extending
    /// backwards).
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn reclaim_pending(&mut self, n: usize) {
        if n > 0 && n <= self.pending.len() {
            let keep = self.pending.len() - n;
            self.pending.truncate(keep);
        }
    }

    pub fn finish(mut self) -> Vec<Segment> {
        if !self.pending.is_empty() {
            self.segments.push(Segment::Extra(std::mem::take(&mut self.pending)));
        }
        self.segments
    }
}

#[cfg(feature = "produce")]
#[allow(clippy::too_many_arguments)]
fn emit_copy(
    tuples: &mut Vec<ControlTuple>,
    diff: &mut Vec<u8>,
    extra: &mut Vec<u8>,
    last_copy: &mut Option<(usize, usize, usize)>,
    pending: &mut Vec<u8>,
    next_pos: Option<usize>,
    base: &[u8],
    updated: &[u8],
) {
    match last_copy.take() {
        None => {
            // Position the pointer for the first read; it starts at 0.
            let seek = match next_pos {
                Some(p) if p > 0 => p as i64,
                _ => 0,
            };
            if !pending.is_empty() || seek != 0 {
                tuples.push(ControlTuple { add: 0, copy: pending.len() as i64, seek });
                extra.append(pending);
            }
        }
        Some((pos, len, new_start)) => {
            let seek = match next_pos {
                Some(p) => p as i64 - pos as i64,
                None => 0,
            };
            tuples.push(ControlTuple { add: len as i64, copy: pending.len() as i64, seek });
            for i in 0..len {
                diff.push(updated[new_start + i].wrapping_sub(base[pos + i]));
            }
            extra.append(pending);
        }
    }
}

/// Encodes the segments as a control stream payload.
#[cfg(feature = "produce")]
pub(crate) fn serialize_segments(
    base: &[u8],
    updated: &[u8],
    segments: &[Segment],
) -> Result<Vec<u8>> {
    let mut tuples: Vec<ControlTuple> = Vec::new();
    let mut diff: Vec<u8> = Vec::new();
    let mut extra: Vec<u8> = Vec::new();
    let mut last_copy: Option<(usize, usize, usize)> = None; // (pos, len, new_start)
    let mut pending: Vec<u8> = Vec::new();
    let mut new_cursor: usize = 0; // new-bundle offset of the next segment

    for segment in segments {
        match segment {
            Segment::Copy { pos, len } => {
                emit_copy(
                    &mut tuples, &mut diff, &mut extra, &mut last_copy, &mut pending,
                    Some(*pos), base, updated,
                );
                last_copy = Some((*pos, *len, new_cursor));
                new_cursor += len;
            }
            Segment::Extra(data) => {
                pending.extend_from_slice(data);
                new_cursor += data.len();
            }
        }
    }
    emit_copy(
        &mut tuples, &mut diff, &mut extra, &mut last_copy, &mut pending,
        None, base, updated,
    );

    let mut ctrl_bytes = vec![0u8; tuples.len() * TUPLE_SIZE];
    for (i, tuple) in tuples.iter().enumerate() {
        let offset = i * TUPLE_SIZE;
        ctrl_bytes[offset..offset + 8].copy_from_slice(&(tuple.add as u64).to_le_bytes());
        ctrl_bytes[offset + 8..offset + 16].copy_from_slice(&(tuple.copy as u64).to_le_bytes());
        ctrl_bytes[offset + 16..offset + 24].copy_from_slice(&(tuple.seek as u64).to_le_bytes());
    }

    let compressed_ctrl = zstd_compress(&ctrl_bytes)?;
    let compressed_diff = zstd_compress(&diff)?;
    let compressed_extra = zstd_compress(&extra)?;

    let mut out = Vec::with_capacity(STREAM_HEADER + compressed_ctrl.len() + compressed_diff.len() + compressed_extra.len());
    out.extend_from_slice(STREAM_MAGIC);
    out.push(STREAM_VERSION);
    out.extend_from_slice(&(compressed_ctrl.len() as u64).to_le_bytes());
    out.extend_from_slice(&(compressed_diff.len() as u64).to_le_bytes());
    out.extend_from_slice(&(compressed_extra.len() as u64).to_le_bytes());
    out.extend_from_slice(&compressed_ctrl);
    out.extend_from_slice(&compressed_diff);
    out.extend_from_slice(&compressed_extra);
    Ok(out)
}

/// Splits a control stream payload into its three streams. `max_output`
/// bounds the decompressed sizes: every tuple besides at most one seek-only
/// leading tuple contributes at least one output byte, so the streams are
/// capped at small multiples of the expected output size.
pub(crate) fn decode_control_stream(
    payload: &[u8],
    max_output: u64,
) -> Result<(Vec<ControlTuple>, Vec<u8>, Vec<u8>)> {
    if payload.len() < STREAM_HEADER {
        return Err(Error::CorruptPatch("control stream too short".into()));
    }
    if &payload[0..4] != STREAM_MAGIC {
        return Err(Error::CorruptPatch("bad control stream magic".into()));
    }
    if payload[4] != STREAM_VERSION {
        return Err(Error::CorruptPatch(format!(
            "unsupported control stream version {}",
            payload[4]
        )));
    }
    let ctrl_len = u64::from_le_bytes(payload[5..13].try_into().unwrap());
    let diff_len = u64::from_le_bytes(payload[13..21].try_into().unwrap());
    let extra_len = u64::from_le_bytes(payload[21..29].try_into().unwrap());
    let rest = (payload.len() - STREAM_HEADER) as u64;
    if ctrl_len + diff_len + extra_len != rest {
        return Err(Error::CorruptPatch("control stream length mismatch".into()));
    }

    let max_ctrl = TUPLE_SIZE as u64 * (2 * max_output + 8);
    let max_stream = max_output + 1;
    let ctrl_bytes = zstd_decompress(&payload[STREAM_HEADER..STREAM_HEADER + ctrl_len as usize], max_ctrl)?;
    let diff = zstd_decompress(
        &payload[STREAM_HEADER + ctrl_len as usize..STREAM_HEADER + (ctrl_len + diff_len) as usize],
        max_stream,
    )?;
    let extra = zstd_decompress(
        &payload[STREAM_HEADER + (ctrl_len + diff_len) as usize..],
        max_stream,
    )?;
    if ctrl_bytes.len() % TUPLE_SIZE != 0 {
        return Err(Error::CorruptPatch("control stream not tuple aligned".into()));
    }
    // Alignment was validated above, so the remainder is always empty.
    let (chunks, _) = ctrl_bytes.as_chunks::<TUPLE_SIZE>();
    let tuples = chunks
        .iter()
        .map(|raw| ControlTuple {
            add: u64::from_le_bytes(raw[0..8].try_into().unwrap()) as i64,
            copy: u64::from_le_bytes(raw[8..16].try_into().unwrap()) as i64,
            seek: u64::from_le_bytes(raw[16..24].try_into().unwrap()) as i64,
        })
        .collect();
    Ok((tuples, diff, extra))
}

/// Replays the tuples against `base`, enforcing every bound so a malicious
/// patch can neither read out of range nor exceed the expected output size.
pub(crate) fn apply_control_stream(
    base: &[u8],
    tuples: &[ControlTuple],
    diff: &[u8],
    extra: &[u8],
    expected_new_size: u64,
) -> Result<Vec<u8>> {
    let expected = usize::try_from(expected_new_size)
        .map_err(|_| Error::CorruptPatch("expected size exceeds address space".into()))?;
    let mut out = Vec::with_capacity(expected);
    let (mut diff_pos, mut extra_pos, mut base_pos): (usize, usize, i64) = (0, 0, 0);

    for (i, tuple) in tuples.iter().enumerate() {
        if tuple.add < 0 || tuple.copy < 0 {
            return Err(Error::CorruptPatch(format!("negative tuple at {i}")));
        }
        let (add, copy) = (tuple.add as usize, tuple.copy as usize);
        if diff_pos + add > diff.len() || extra_pos + copy > extra.len() {
            return Err(Error::CorruptPatch(format!("stream overrun at tuple {i}")));
        }
        if add > 0 && (base_pos < 0 || base_pos + tuple.add > base.len() as i64) {
            return Err(Error::CorruptPatch(format!("base overrun at tuple {i}")));
        }
        if out.len() + add + copy > expected {
            return Err(Error::CorruptPatch(format!(
                "output exceeds expected size at tuple {i}"
            )));
        }
        for j in 0..add {
            out.push(base[(base_pos + j as i64) as usize].wrapping_add(diff[diff_pos + j]));
        }
        out.extend_from_slice(&extra[extra_pos..extra_pos + copy]);
        diff_pos += add;
        extra_pos += copy;
        base_pos += tuple.seek;
    }
    if out.len() != expected {
        return Err(Error::CorruptPatch(format!(
            "output is {} bytes, expected {}",
            out.len(), expected
        )));
    }
    Ok(out)
}

/// Deterministically compresses data (bulk API, single threaded, fixed
/// level): identical input always yields identical output, which the
/// content-addressed object store relies on.
#[cfg(feature = "produce")]
pub(crate) fn zstd_compress(data: &[u8]) -> Result<Vec<u8>> {
    zstd::bulk::compress(data, ZSTD_LEVEL).map_err(|e| Error::Codec(format!("zstd: {e}")))
}

/// Decompresses data while capping the bytes a corrupt or malicious stream
/// can make the decoder produce.
pub(crate) fn zstd_decompress(data: &[u8], max_bytes: u64) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    let cap = usize::try_from(max_bytes)
        .map_err(|_| Error::CorruptPatch("decompression bound exceeds address space".into()))?;
    let out = zstd::bulk::decompress(data, cap).map_err(|e| Error::CorruptPatch(format!("zstd: {e}")))?;
    if out.len() as u64 > max_bytes {
        return Err(Error::CorruptPatch("decompressed stream exceeds bound".into()));
    }
    Ok(out)
}
