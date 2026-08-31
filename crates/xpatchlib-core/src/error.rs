use std::fmt;

/// Every failure mode of patch creation and replay.
#[derive(Debug)]
pub enum Error {
    /// The algorithm name is not compiled into this build.
    UnknownAlgorithm(String),
    /// The patch bytes cannot be parsed or decoded.
    CorruptPatch(String),
    /// The base handed to [`apply`](crate::apply) does not hash to the base
    /// recorded in the envelope (wrong version, truncation, ...).
    BaseMismatch { have: usize, expect: u64 },
    /// The patched output failed its SHA-256 check.
    ChecksumMismatch,
    /// A codec-internal failure (zstd, allocation, oversized input).
    Codec(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownAlgorithm(name) => write!(f, "unknown algorithm {name:?}"),
            Error::CorruptPatch(detail) => write!(f, "corrupt patch: {detail}"),
            Error::BaseMismatch { have, expect } => write!(
                f,
                "base does not match patch: have {have} bytes, patch expects {expect}"
            ),
            Error::ChecksumMismatch => write!(f, "patched result failed checksum"),
            Error::Codec(detail) => write!(f, "codec failure: {detail}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
