//! The full codec ships the new bundle recompressed and is not a delta at
//! all. It exists as the baseline every real codec must beat: if a patch is
//! larger than the full payload, the catalog should just point the client
//! at the full download. Keeping it in the registry lets tooling compare
//! options uniformly.

use std::io::Read;

use crate::error::{Error, Result};

#[cfg(feature = "produce")]
const LEVEL: i32 = 9;

pub struct Full;

impl crate::Codec for Full {
    fn name(&self) -> &'static str {
        "full"
    }

    #[cfg(feature = "produce")]
    fn create(&self, _base: &[u8], updated: &[u8]) -> Result<Vec<u8>> {
        zstd::bulk::compress(updated, LEVEL).map_err(|e| Error::Codec(format!("full: {e}")))
    }

    fn apply(&self, _base: &[u8], payload: &[u8], expected_new_size: u64) -> Result<Vec<u8>> {
        if payload.is_empty() {
            return if expected_new_size == 0 {
                Ok(Vec::new())
            } else {
                Err(Error::CorruptPatch("empty full payload".into()))
            };
        }
        let cap = usize::try_from(expected_new_size)
            .map_err(|_| Error::CorruptPatch("expected size exceeds address space".into()))?;
        zstd::bulk::decompress(payload, cap)
            .map_err(|e| Error::CorruptPatch(format!("zstd: {e}")))
    }
}

/// Streaming counterpart of [`Codec::apply`](crate::Codec::apply) for the
/// file-based entry points: the payload decompresses straight into `out`,
/// capped at `expected_new_size`, without materializing the bundle.
pub(crate) fn apply_streaming(
    payload: &[u8],
    out: &mut dyn std::io::Write,
    expected_new_size: u64,
) -> Result<()> {
    if payload.is_empty() {
        return if expected_new_size == 0 {
            Ok(())
        } else {
            Err(Error::CorruptPatch("empty full payload".into()))
        };
    }
    let expected = usize::try_from(expected_new_size)
        .map_err(|_| Error::CorruptPatch("expected size exceeds address space".into()))?;
    let mut decoder = zstd::stream::read::Decoder::new(payload)
        .map_err(|e| Error::CorruptPatch(format!("zstd: {e}")))?;
    let mut chunk = vec![0u8; 64 * 1024];
    let mut written = 0usize;
    loop {
        let n = decoder
            .read(&mut chunk)
            .map_err(|e| Error::CorruptPatch(format!("zstd: {e}")))?;
        if n == 0 {
            break;
        }
        written += n;
        if written > expected {
            return Err(Error::CorruptPatch("decompressed stream exceeds bound".into()));
        }
        out.write_all(&chunk[..n])
            .map_err(|e| Error::Io(format!("write output: {e}")))?;
    }
    Ok(())
}
