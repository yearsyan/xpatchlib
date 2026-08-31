use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// Patch envelope magic ("XPDL").
pub const MAGIC: &[u8; 4] = b"XPDL";
/// Current envelope format version.
pub const VERSION: u8 = 1;
const MAX_ALGORITHM_NAME: usize = 64;

/// Layout (all integers little endian):
///
/// ```text
///  0:4   magic "XPDL"
///  4     format version (1)
///  5     algorithm name length
///  6:6+n algorithm name (ASCII)
///  +8    base size
///  +32   base SHA-256
///  +8    result size
///  +32   result SHA-256
///  +8    payload length
///  +p    algorithm payload
/// ```
///
/// The envelope omits timestamps and other run-varying metadata: for a fixed
/// algorithm, `create` is byte-for-byte deterministic, which lets patches
/// live in content-addressed object stores alongside the bundles they
/// upgrade.
pub struct Envelope<'a> {
    pub algorithm: &'a str,
    pub base_size: u64,
    pub base_hash: [u8; 32],
    pub result_size: u64,
    pub result_hash: [u8; 32],
    pub payload: &'a [u8],
}

#[cfg(feature = "produce")]
pub fn encode(
    algorithm: &str,
    base: &[u8],
    updated: &[u8],
    payload: &[u8],
) -> Vec<u8> {
    let base_hash: [u8; 32] = Sha256::digest(base).into();
    let result_hash: [u8; 32] = Sha256::digest(updated).into();
    let mut out = Vec::with_capacity(payload.len() + 96 + algorithm.len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(algorithm.len() as u8);
    out.extend_from_slice(algorithm.as_bytes());
    out.extend_from_slice(&(base.len() as u64).to_le_bytes());
    out.extend_from_slice(&base_hash);
    out.extend_from_slice(&(updated.len() as u64).to_le_bytes());
    out.extend_from_slice(&result_hash);
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn parse(patch: &[u8]) -> Result<Envelope<'_>> {
    if patch.len() < 6 {
        return Err(Error::CorruptPatch("patch too short".into()));
    }
    if &patch[0..4] != MAGIC {
        return Err(Error::CorruptPatch("bad magic".into()));
    }
    if patch[4] != VERSION {
        return Err(Error::CorruptPatch(format!(
            "unsupported version {}",
            patch[4]
        )));
    }
    let name_len = patch[5] as usize;
    if name_len == 0 || name_len > MAX_ALGORITHM_NAME {
        return Err(Error::CorruptPatch("bad algorithm name length".into()));
    }
    let mut offset = 6;
    if patch.len() < offset + name_len + 16 + 64 {
        return Err(Error::CorruptPatch("truncated header".into()));
    }
    let algorithm = std::str::from_utf8(&patch[offset..offset + name_len])
        .map_err(|_| Error::CorruptPatch("algorithm name is not ASCII".into()))?;
    offset += name_len;
    let base_size = u64::from_le_bytes(patch[offset..offset + 8].try_into().unwrap());
    offset += 8;
    let base_hash: [u8; 32] = patch[offset..offset + 32].try_into().unwrap();
    offset += 32;
    let result_size = u64::from_le_bytes(patch[offset..offset + 8].try_into().unwrap());
    offset += 8;
    let result_hash: [u8; 32] = patch[offset..offset + 32].try_into().unwrap();
    offset += 32;
    let payload_len = u64::from_le_bytes(patch[offset..offset + 8].try_into().unwrap());
    offset += 8;
    if payload_len != (patch.len() - offset) as u64 {
        return Err(Error::CorruptPatch("payload length mismatch".into()));
    }
    Ok(Envelope {
        algorithm,
        base_size,
        base_hash,
        result_size,
        result_hash,
        payload: &patch[offset..],
    })
}

/// Verifies that `base` matches the envelope's pinned base hash.
pub fn verify_base(envelope: &Envelope<'_>, base: &[u8]) -> Result<()> {
    if envelope.base_size != base.len() as u64 {
        return Err(Error::BaseMismatch {
            have: base.len(),
            expect: envelope.base_size,
        });
    }
    let hash: [u8; 32] = Sha256::digest(base).into();
    if hash != envelope.base_hash {
        return Err(Error::BaseMismatch {
            have: base.len(),
            expect: envelope.base_size,
        });
    }
    Ok(())
}

/// Verifies the replay output against the envelope's pinned result hash.
pub fn verify_result(envelope: &Envelope<'_>, updated: &[u8]) -> Result<()> {
    if updated.len() as u64 != envelope.result_size {
        return Err(Error::ChecksumMismatch);
    }
    let hash: [u8; 32] = Sha256::digest(updated).into();
    if hash != envelope.result_hash {
        return Err(Error::ChecksumMismatch);
    }
    Ok(())
}
