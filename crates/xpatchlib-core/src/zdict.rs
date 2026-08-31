//! The zdict codec treats the base bundle as a zstd dictionary: the patch is
//! simply the new bundle compressed with the old one priming the encoder
//! ("patch-from" style). It cannot copy-and-reorder like bsdiff, but both
//! directions run at compression speed, memory is bounded by the window, and
//! the same zstd machinery is battle-tested on every platform — a good fit
//! for low-end clients or very frequent patch generation. Replay builds
//! carry only the decompressor path.

use crate::error::{Error, Result};

/// Back-reference distance cap: zstd cannot reference base content farther
/// back than this from the current output position.
const MAX_WINDOW_LOG: u32 = 27; // 128 MiB
#[cfg(feature = "produce")]
const MIN_WINDOW_LOG: u32 = 17;
#[cfg(feature = "produce")]
const LEVEL: i32 = 9;

pub struct Zdict;

impl crate::Codec for Zdict {
    fn name(&self) -> &'static str {
        "zdict"
    }

    #[cfg(feature = "produce")]
    fn create(&self, base: &[u8], updated: &[u8]) -> Result<Vec<u8>> {
        let mut compressor = zstd::bulk::Compressor::with_dictionary(LEVEL, base)
            .map_err(|e| Error::Codec(format!("zdict: {e}")))?;
        compressor
            .window_log(window_log_for(base.len() + updated.len()))
            .map_err(|e| Error::Codec(format!("zdict: {e}")))?;
        compressor
            .compress(updated)
            .map_err(|e| Error::Codec(format!("zdict: {e}")))
    }

    fn apply(&self, base: &[u8], payload: &[u8], expected_new_size: u64) -> Result<Vec<u8>> {
        if payload.is_empty() {
            return if expected_new_size == 0 {
                Ok(Vec::new())
            } else {
                Err(Error::CorruptPatch("empty zdict payload".into()))
            };
        }
        let cap = usize::try_from(expected_new_size)
            .map_err(|_| Error::CorruptPatch("expected size exceeds address space".into()))?;
        let mut decompressor = zstd::bulk::Decompressor::with_dictionary(base)
            .map_err(|e| Error::Codec(format!("zdict: {e}")))?;
        decompressor
            .window_log_max(MAX_WINDOW_LOG)
            .map_err(|e| Error::Codec(format!("zdict: {e}")))?;
        decompressor
            .decompress(payload, cap)
            .map_err(|e| Error::Codec(format!("zstd: {e}")))
    }
}

/// Picks the smallest power of two that covers base plus the new bundle,
/// clamped to the zstd window limits.
#[cfg(feature = "produce")]
fn window_log_for(span: usize) -> u32 {
    let mut log = MIN_WINDOW_LOG;
    while (1usize << log) < span && log < MAX_WINDOW_LOG {
        log += 1;
    }
    log.min(MAX_WINDOW_LOG)
}
