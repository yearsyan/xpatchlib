//! File-based, streaming replay for clients that apply patches against
//! on-disk bundles.
//!
//! [`apply_file`] keeps the same integrity contract as the in-memory
//! [`apply`](crate::apply) — base hash verified before replay, result size
//! and result hash verified after — but never materializes the base, the
//! diff stream or the result in memory:
//!
//! - the patch envelope itself is read into memory (patches are small by
//!   construction: producers never publish a patch that does not beat the
//!   full download);
//! - the base file is streamed through SHA-256 for verification, then read
//!   at the offsets the control tuples ask for;
//! - diff/extra streams decompress only as far as replay consumes them;
//! - the result flows through a fixed 64 KiB write buffer into the output
//!   file while being hashed incrementally.
//!
//! Peak memory is therefore the patch plus a few hundred KiB of fixed
//! buffers, regardless of bundle size. On any failure the output file is
//! removed — callers never see partially trusted bytes on disk, and are
//! expected to publish the result with their own temp-file + rename step.

use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::{control, envelope, full, zdict};

/// Replays the patch file at `patch_path` against the base file at
/// `base_path`, writing the result to `out_path`. All three paths must be
/// distinct; the call replaces any existing `out_path` content and, on any
/// error (including base or result hash mismatch), removes it again.
///
/// ```ignore
/// xpatchlib_core::apply_file(patch_file, base_zip, out_zip)?;
/// ```
pub fn apply_file<P: AsRef<Path>, Q: AsRef<Path>, R: AsRef<Path>>(
    patch_path: P,
    base_path: Q,
    out_path: R,
) -> Result<()> {
    let patch_path = patch_path.as_ref();
    let base_path = base_path.as_ref();
    let out_path = out_path.as_ref();

    let patch = fs::read(patch_path).map_err(|e| Error::Io(format!("read patch: {e}")))?;
    let parsed = envelope::parse(&patch)?;
    // Reject names outside the registry before touching any output file,
    // with the same UnknownAlgorithm error the in-memory path returns.
    crate::codec(parsed.algorithm)?;
    verify_base_file(&parsed, base_path)?;

    let mut sink = OutputSink::create(out_path)?;
    let replayed = match parsed.algorithm {
        "bsdiff" | "block" => control::apply_streaming(
            parsed.payload,
            base_path,
            parsed.base_size,
            &mut sink,
            parsed.result_size,
        ),
        "full" => full::apply_streaming(parsed.payload, &mut sink, parsed.result_size),
        "zdict" => zdict::apply_streaming(parsed.payload, base_path, &mut sink, parsed.result_size),
        // codec() has already accepted the name, so reaching here means a
        // registered codec was added without a streaming replay — refuse
        // instead of quietly falling back to the in-memory path.
        other => Err(Error::CorruptPatch(format!(
            "algorithm {other:?} has no streaming replay"
        ))),
    };
    match replayed {
        Ok(()) => sink.finish(parsed.result_size, &parsed.result_hash),
        Err(err) => {
            drop(sink); // removes the partial output file
            Err(err)
        }
    }
}

/// Stream-verifies the base file against the envelope's pinned base hash.
fn verify_base_file(parsed: &envelope::Envelope<'_>, base_path: &Path) -> Result<()> {
    let mismatch = || Error::BaseMismatch {
        have: parsed.base_size as usize, // size already equals; hash differs
        expect: parsed.base_size,
    };
    let meta = fs::metadata(base_path).map_err(|e| Error::Io(format!("stat base: {e}")))?;
    if meta.len() != parsed.base_size {
        return Err(Error::BaseMismatch {
            have: meta.len() as usize,
            expect: parsed.base_size,
        });
    }
    let mut file = File::open(base_path).map_err(|e| Error::Io(format!("open base: {e}")))?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut chunk)
            .map_err(|e| Error::Io(format!("read base: {e}")))?;
        if n == 0 {
            break;
        }
        hasher.update(&chunk[..n]);
    }
    let hash: [u8; 32] = hasher.finalize().into();
    if hash != parsed.base_hash {
        return Err(mismatch());
    }
    Ok(())
}

/// Output file wrapper that hashes every byte on the way through and
/// removes itself on drop unless [`OutputSink::finish`] committed it.
struct OutputSink {
    file: BufWriter<File>,
    hasher: Sha256,
    written: u64,
    path: PathBuf,
    committed: bool,
}

impl OutputSink {
    fn create(path: &Path) -> Result<Self> {
        let file = File::create(path).map_err(|e| Error::Io(format!("create output: {e}")))?;
        Ok(OutputSink {
            file: BufWriter::with_capacity(64 * 1024, file),
            hasher: Sha256::new(),
            written: 0,
            path: path.to_path_buf(),
            committed: false,
        })
    }

    /// Flushes, verifies size and result hash, syncs to disk and commits.
    /// Any error leaves the file to be removed by Drop.
    fn finish(mut self, expected_size: u64, expected_hash: &[u8; 32]) -> Result<()> {
        self.file
            .flush()
            .map_err(|e| Error::Io(format!("flush output: {e}")))?;
        if self.written != expected_size {
            return Err(Error::ChecksumMismatch);
        }
        let hash: [u8; 32] = self.hasher.clone().finalize().into();
        if &hash != expected_hash {
            return Err(Error::ChecksumMismatch);
        }
        self.file
            .get_ref()
            .sync_all()
            .map_err(|e| Error::Io(format!("sync output: {e}")))?;
        self.committed = true;
        Ok(())
    }
}

impl Write for OutputSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.file.write(buf)?;
        self.hasher.update(&buf[..n]);
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

impl Drop for OutputSink {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}
