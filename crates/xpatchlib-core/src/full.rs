//! The full codec ships the new bundle recompressed and is not a delta at
//! all. It exists as the baseline every real codec must beat: if a patch is
//! larger than the full payload, the catalog should just point the client
//! at the full download. Keeping it in the registry lets tooling compare
//! options uniformly.

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
