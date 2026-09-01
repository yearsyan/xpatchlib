//! Deterministic binary delta patches for app update bundles.
//!
//! Producers register [`Codec`] implementations behind [`create`] and
//! [`apply`], which wrap the codec payload in a versioned envelope ("XPDL")
//! that pins the SHA-256 of both the base and the resulting bundle so
//! clients can verify integrity end to end.
//!
//! ```ignore
//! // Producing requires the "produce" feature (on by default; the mobile
//! // replay builds turn it off):
//! let patch = xpatchlib_core::create("bsdiff", &old_bundle, &new_bundle)?;
//! // Replaying is all a mobile client ever does:
//! let restored = xpatchlib_core::apply(&patch, &old_bundle)?;
//! assert_eq!(restored, new_bundle);
//! ```
//!
//! The envelope omits timestamps and other run-varying metadata: for a fixed
//! codec, `create` is byte-for-byte deterministic across platforms, which
//! lets patches live in content-addressed object stores alongside the
//! bundles they upgrade.
//!
//! Registered codecs: `bsdiff` (smallest patches, suffix-array producer),
//! `zdict` (base as zstd dictionary, fastest both ways), `block`
//! (fingerprint greedy matcher), `full` (recompressed baseline).
//!
//! The `produce` feature gates every code path that builds patches. The
//! mobile crates (`xpatchlib-ffi` for iOS and HarmonyOS, `xpatchlib-jni`
//! for Android) compile this crate with the feature off, so suffix-array
//! construction, matchers and zstd compression never reach a client
//! binary — clients link the replay side only.

mod block;
mod bsdiff;
mod control;
mod envelope;
mod error;
mod full;
#[cfg(feature = "produce")]
mod sais;
mod stream;
mod zdict;

pub use error::{Error, Result};

/// One delta codec. Implementations must be safe for concurrent use and
/// must produce deterministic output for identical input.
pub trait Codec: Send + Sync {
    /// Identifier stored in the envelope; short and stable ("bsdiff", ...).
    fn name(&self) -> &'static str;
    /// Encodes the payload needed to rebuild `updated` from `base`.
    /// Compiled only with the `produce` feature — replay builds never
    /// carry producer code.
    #[cfg(feature = "produce")]
    fn create(&self, base: &[u8], updated: &[u8]) -> Result<Vec<u8>>;
    /// Rebuilds `updated` from `base` and `payload`. `expected_new_size` is
    /// the size recorded in the envelope and must bound decompression and
    /// output buffers.
    fn apply(&self, base: &[u8], payload: &[u8], expected_new_size: u64) -> Result<Vec<u8>>;
}

static CODECS: &[&dyn Codec] = &[&bsdiff::Bsdiff, &zdict::Zdict, &block::Block, &full::Full];

/// Lists every registered codec name.
pub fn algorithms() -> Vec<&'static str> {
    CODECS.iter().map(|c| c.name()).collect()
}

/// Returns the codec registered under `name`.
pub fn codec(name: &str) -> Result<&'static dyn Codec> {
    CODECS
        .iter()
        .copied()
        .find(|c| c.name() == name)
        .ok_or_else(|| Error::UnknownAlgorithm(name.to_string()))
}

/// Builds a patch that turns `base` into `updated` using the named codec.
/// The returned bytes are the complete envelope and are safe to store or
/// transfer verbatim. Requires the `produce` feature — this runs on the
/// Node toolchain or a server, never on a phone.
#[cfg(feature = "produce")]
pub fn create(algorithm: &str, base: &[u8], updated: &[u8]) -> Result<Vec<u8>> {
    let codec = codec(algorithm)?;
    let payload = codec.create(base, updated)?;
    Ok(envelope::encode(algorithm, base, updated, &payload))
}

/// Rebuilds the updated bundle from `base` and `patch`. Both the base hash
/// and the result hash are verified; on any mismatch this returns an error
/// and never returns partially trusted bytes.
pub fn apply(patch: &[u8], base: &[u8]) -> Result<Vec<u8>> {
    let parsed = envelope::parse(patch)?;
    let codec = codec(parsed.algorithm)?;
    envelope::verify_base(&parsed, base)?;
    let updated = codec.apply(base, parsed.payload, parsed.result_size)?;
    envelope::verify_result(&parsed, &updated)?;
    Ok(updated)
}

/// File-based, streaming counterpart of [`apply`]: replays the patch file
/// against the base file and writes the result to a file, holding only the
/// patch plus a few fixed buffers in memory (the in-memory path peaks at
/// roughly base + diff + result). The verification contract is identical —
/// base hash before, result size and hash after — and on any failure the
/// output file is removed.
pub use stream::apply_file;

/// Decoded envelope header of a patch: cheap enough for catalog building
/// without keeping codec payloads in memory.
#[derive(Debug, Clone)]
pub struct PatchInfo {
    pub algorithm: String,
    pub base_size: u64,
    pub base_hash: [u8; 32],
    pub result_size: u64,
    pub result_hash: [u8; 32],
    pub payload_len: u64,
}

/// Decodes only the envelope header of a patch.
pub fn patch_info(patch: &[u8]) -> Result<PatchInfo> {
    let parsed = envelope::parse(patch)?;
    Ok(PatchInfo {
        algorithm: parsed.algorithm.to_string(),
        base_size: parsed.base_size,
        base_hash: parsed.base_hash,
        result_size: parsed.result_size,
        result_hash: parsed.result_hash,
        payload_len: parsed.payload.len() as u64,
    })
}
