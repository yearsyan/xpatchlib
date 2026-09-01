//! Android JNI bindings over [`xpatchlib_core`]. The companion Java class
//! `io.github.yearsyan.xpatch.XPatch` (packaging/android) declares the matching
//! `native` methods; this crate exports `libxpatchlib_jni.so` for every ABI.
//!
//! Replay only: this crate builds `xpatchlib-core` without the `produce`
//! feature, so patch production code is not compiled in at all — the app
//! can replay patches, never build one.
//!
//! All entry points throw `io.github.yearsyan.xpatch.XPatchException` on failure
//! and never return partially patched data.

use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::{jbyteArray, jlong, jobjectArray, jsize};
use jni::JNIEnv;

const EXCEPTION_CLASS: &str = "io/github/yearsyan/xpatch/XPatchException";

fn throw(env: &mut JNIEnv, error: xpatchlib_core::Error) {
    let message = error.to_string();
    // Throwing is best effort: if it fails the caller still sees the null
    // return value.
    let _ = env.throw_new(EXCEPTION_CLASS, message);
}

fn to_java<'local>(env: &mut JNIEnv<'local>, bytes: &[u8]) -> JByteArray<'local> {
    let array = env.new_byte_array(bytes.len() as jsize);
    match array {
        Ok(array) => {
            // jbyte == i8 on every supported platform.
            let signed = unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const i8, bytes.len()) };
            if env.set_byte_array_region(&array, 0, signed).is_err() {
                return JByteArray::default();
            }
            array
        }
        Err(_) => JByteArray::default(),
    }
}

fn from_java(env: &mut JNIEnv, array: JByteArray) -> Option<Vec<u8>> {
    let len = env.get_array_length(&array).ok()?;
    let mut signed = vec![0i8; len as usize];
    env.get_byte_array_region(&array, 0, &mut signed).ok()?;
    Some(unsafe { std::slice::from_raw_parts(signed.as_ptr() as *const u8, signed.len()) }.to_vec())
}

/// Converts a java.lang.String path into an owned UTF-8 String.
fn java_path<'local>(
    env: &mut JNIEnv<'local>,
    value: &JString<'local>,
) -> Option<String> {
    let java = env.get_string(value).ok()?;
    let cstr: &std::ffi::CStr = &java; // JavaStr derefs to JNIStr then CStr
    cstr.to_str().ok().map(str::to_owned)
}

/// Returns the algorithm names compiled into this library.
#[no_mangle]
pub extern "system" fn Java_io_github_yearsyan_xpatch_XPatch_nativeAlgorithms<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jobjectArray {
    let names = xpatchlib_core::algorithms();
    let strings: Vec<JObject<'local>> = match names
        .iter()
        .map(|name| env.new_string(name).map(JObject::from))
        .collect::<Result<_, _>>()
    {
        Ok(strings) => strings,
        Err(_) => return JObject::default().into_raw(),
    };
    match env.new_object_array(strings.len() as jsize, "java/lang/String", JObject::null()) {
        Ok(array) => {
            for (index, string) in strings.iter().enumerate() {
                let _ = env.set_object_array_element(&array, index as jsize, string);
            }
            array.into_raw()
        }
        Err(_) => JObject::default().into_raw(),
    }
}

/// Replays `patch` against `base`. Verifies both the base hash and the
/// result hash before returning anything.
#[no_mangle]
pub extern "system" fn Java_io_github_yearsyan_xpatch_XPatch_nativeApply<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    patch: JByteArray<'local>,
    base: JByteArray<'local>,
) -> jbyteArray {
    let (Some(patch), Some(base)) = (from_java(&mut env, patch), from_java(&mut env, base)) else {
        return JByteArray::default().into_raw();
    };
    match xpatchlib_core::apply(&patch, &base) {
        Ok(updated) => to_java(&mut env, &updated).into_raw(),
        Err(err) => {
            throw(&mut env, err);
            JByteArray::default().into_raw()
        }
    }
}

/// Streaming, file-based replay: replays the patch file against the base
/// file, writing the result straight to `outPath`. Memory stays bounded by
/// the patch bytes plus small fixed buffers regardless of bundle size.
/// Both hashes are verified exactly like `nativeApply`; on failure the
/// output file is removed and an `XPatchException` is thrown.
#[no_mangle]
pub extern "system" fn Java_io_github_yearsyan_xpatch_XPatch_nativeApplyFile<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    patch_path: JString<'local>,
    base_path: JString<'local>,
    out_path: JString<'local>,
) {
    let (Some(patch), Some(base), Some(out)) = (
        java_path(&mut env, &patch_path),
        java_path(&mut env, &base_path),
        java_path(&mut env, &out_path),
    ) else {
        let _ = env.throw_new(EXCEPTION_CLASS, "patch/base/out paths must be valid UTF-8 Strings");
        return;
    };
    if let Err(err) = xpatchlib_core::apply_file(&patch, &base, &out) {
        throw(&mut env, err);
    }
}

/// Returns the result size recorded in the patch envelope, or -1 when the
/// patch cannot be parsed. Lets the caller pre-flight disk space before
/// downloading.
#[no_mangle]
pub extern "system" fn Java_io_github_yearsyan_xpatch_XPatch_nativeResultSize<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    patch: JByteArray<'local>,
) -> jlong {
    let Some(patch) = from_java(&mut env, patch) else {
        return -1;
    };
    match xpatchlib_core::patch_info(&patch) {
        Ok(info) => info.result_size as jlong,
        Err(err) => {
            throw(&mut env, err);
            -1
        }
    }
}
