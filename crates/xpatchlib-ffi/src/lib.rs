//! C ABI over [`xpatchlib_core`]: one static or shared library serves every
//! consumer that is not Android JNI or WASM — the iOS app embeds the
//! staticlib directly, and the HarmonyOS NAPI adapter (packaging/harmony)
//! calls these symbols from C++.
//!
//! Replay only: this crate builds `xpatchlib-core` without the `produce`
//! feature, so patch production code is not compiled in at all. Clients
//! replay patches produced by the Node toolchain; they can never build one.
//!
//! Buffers returned by `xpatchlib_apply` are heap allocations owned by the
//! caller until released with [`xpatchlib_free`].

use std::ffi::{c_char, CStr};
use std::slice;

use xpatchlib_core::{self as core, Error};

/// Status codes returned by every entry point.
pub const XPATCHLIB_OK: i32 = 0;
pub const XPATCHLIB_ERR_UNKNOWN_ALGORITHM: i32 = 1;
pub const XPATCHLIB_ERR_CORRUPT_PATCH: i32 = 2;
pub const XPATCHLIB_ERR_BASE_MISMATCH: i32 = 3;
pub const XPATCHLIB_ERR_CHECKSUM: i32 = 4;
pub const XPATCHLIB_ERR_CODEC: i32 = 5;
pub const XPATCHLIB_ERR_INVALID_ARG: i32 = 6;

fn status(error: &Error) -> i32 {
    match error {
        Error::UnknownAlgorithm(_) => XPATCHLIB_ERR_UNKNOWN_ALGORITHM,
        Error::CorruptPatch(_) => XPATCHLIB_ERR_CORRUPT_PATCH,
        Error::BaseMismatch { .. } => XPATCHLIB_ERR_BASE_MISMATCH,
        Error::ChecksumMismatch => XPATCHLIB_ERR_CHECKSUM,
        Error::Codec(_) => XPATCHLIB_ERR_CODEC,
    }
}

/// Interprets a (pointer, length) pair as a slice, rejecting the NULL +
/// non-zero combination that would be undefined behavior.
unsafe fn input<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], i32> {
    if ptr.is_null() {
        if len == 0 {
            return Ok(&[]);
        }
        return Err(XPATCHLIB_ERR_INVALID_ARG);
    }
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

/// Hands an owned buffer to the caller.
fn hand_over(out: *mut *mut u8, out_len: *mut usize, bytes: Vec<u8>) -> i32 {
    if bytes.is_empty() {
        if !out.is_null() {
            unsafe { *out = std::ptr::null_mut() };
        }
        if !out_len.is_null() {
            unsafe { *out_len = 0 };
        }
        return XPATCHLIB_OK;
    }
    let mut boxed = bytes.into_boxed_slice();
    if !out.is_null() {
        unsafe { *out = boxed.as_mut_ptr() };
    }
    if !out_len.is_null() {
        unsafe { *out_len = boxed.len() };
    }
    std::mem::forget(boxed);
    XPATCHLIB_OK
}

/// Number of replay algorithms compiled into this library.
#[no_mangle]
pub extern "C" fn xpatchlib_algorithm_count() -> usize {
    core::algorithms().len()
}

/// Stable, NUL-terminated name of the algorithm at `index`, or NULL when
/// out of range. The pointers live for the lifetime of the library.
#[no_mangle]
pub extern "C" fn xpatchlib_algorithm_name(index: usize) -> *const c_char {
    static NAMES: std::sync::OnceLock<Vec<&'static CStr>> = std::sync::OnceLock::new();
    let names = NAMES.get_or_init(|| {
        core::algorithms()
            .iter()
            .map(|name| {
                // leak: process-lifetime constants
                Box::leak(format!("{name}\0").into_bytes().into_boxed_slice()) as &'static [u8]
            })
            .map(|bytes| CStr::from_bytes_with_nul(bytes).expect("nul terminated"))
            .collect()
    });
    names
        .get(index)
        .map(|name| name.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Replays `patch` against `base`. Verifies both the base hash and the
/// result hash; on any mismatch nothing is written to `*out`.
///
/// # Safety
///
/// Buffer pointers must be valid for reads of their paired lengths, and
/// `out`/`out_len` for writes when non-NULL.
#[no_mangle]
pub unsafe extern "C" fn xpatchlib_apply(
    patch: *const u8,
    patch_len: usize,
    base: *const u8,
    base_len: usize,
    out: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    let (patch, base) = match (input(patch, patch_len), input(base, base_len)) {
        (Ok(p), Ok(b)) => (p, b),
        (Err(code), _) | (_, Err(code)) => return code,
    };
    match core::apply(patch, base) {
        Ok(updated) => hand_over(out, out_len, updated),
        Err(err) => status(&err),
    }
}

/// Decodes the envelope header: the algorithm name pointer (static, NUL
/// terminated), the base size and the result size. Any output pointer may
/// be NULL to skip that field.
///
/// # Safety
///
/// `patch` must be valid for reads of `patch_len` bytes; the output
/// pointers for writes when non-NULL.
#[no_mangle]
pub unsafe extern "C" fn xpatchlib_patch_info(
    patch: *const u8,
    patch_len: usize,
    algorithm: *mut *const c_char,
    base_size: *mut u64,
    result_size: *mut u64,
) -> i32 {
    let patch = match input(patch, patch_len) {
        Ok(p) => p,
        Err(code) => return code,
    };
    match core::patch_info(patch) {
        Ok(info) => {
            if !algorithm.is_null() {
                let index = core::algorithms()
                    .iter()
                    .position(|name| *name == info.algorithm)
                    .expect("patch_info returns a registered algorithm");
                unsafe { *algorithm = xpatchlib_algorithm_name(index) };
            }
            if !base_size.is_null() {
                unsafe { *base_size = info.base_size };
            }
            if !result_size.is_null() {
                unsafe { *result_size = info.result_size };
            }
            XPATCHLIB_OK
        }
        Err(err) => status(&err),
    }
}

/// Releases a buffer produced by [`xpatchlib_apply`]. Passing NULL or
/// (NULL, 0) is a no-op.
///
/// # Safety
///
/// `ptr`/`len` must describe a buffer this library handed out and not have
/// been freed already.
#[no_mangle]
pub unsafe extern "C" fn xpatchlib_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        drop(unsafe { Vec::from_raw_parts(ptr, len, len) });
    }
}
