//! WASM bindings over [`xpatchlib_core`] for the Node build toolchain
//! (`@lynfe/xpatchlib`): the bundle upload step generates patches against
//! the previous published version and ships them alongside the full
//! bundle. This is the producer side — the mobile clients replay only.
//!
//! ```js
//! import { createPatch, applyPatch } from "@lynfe/xpatchlib";
//! const patch = createPatch("bsdiff", oldBundle, newBundle);
//! const restored = applyPatch(patch, oldBundle);
//! ```

use wasm_bindgen::prelude::*;

/// Lists the algorithm names compiled into this module.
#[wasm_bindgen(js_name = algorithms)]
pub fn algorithms() -> Vec<String> {
    xpatchlib_core::algorithms().into_iter().map(str::to_string).collect()
}

/// Builds an XPDL patch that turns `base` into `updated`.
#[wasm_bindgen(js_name = createPatch)]
pub fn create_patch(
    algorithm: &str,
    base: Vec<u8>,
    updated: Vec<u8>,
) -> Result<Vec<u8>, JsError> {
    xpatchlib_core::create(algorithm, &base, &updated).map_err(|e| JsError::new(&e.to_string()))
}

/// Replays `patch` against `base`. Verifies both the base hash and the
/// result hash before returning anything.
#[wasm_bindgen(js_name = applyPatch)]
pub fn apply_patch(patch: Vec<u8>, base: Vec<u8>) -> Result<Vec<u8>, JsError> {
    xpatchlib_core::apply(&patch, &base).map_err(|e| JsError::new(&e.to_string()))
}

/// Decoded envelope header of a patch, for catalog building.
#[wasm_bindgen]
pub struct PatchInfo {
    algorithm: String,
    base_size: f64,
    result_size: f64,
    payload_len: f64,
    base_hash: Vec<u8>,
    result_hash: Vec<u8>,
}

#[wasm_bindgen]
impl PatchInfo {
    #[wasm_bindgen(getter)]
    pub fn algorithm(&self) -> String {
        self.algorithm.clone()
    }
    #[wasm_bindgen(getter, js_name = baseSize)]
    pub fn base_size(&self) -> f64 {
        self.base_size
    }
    #[wasm_bindgen(getter, js_name = resultSize)]
    pub fn result_size(&self) -> f64 {
        self.result_size
    }
    #[wasm_bindgen(getter, js_name = payloadLen)]
    pub fn payload_len(&self) -> f64 {
        self.payload_len
    }
    #[wasm_bindgen(getter, js_name = baseHash)]
    pub fn base_hash(&self) -> Vec<u8> {
        self.base_hash.clone()
    }
    #[wasm_bindgen(getter, js_name = resultHash)]
    pub fn result_hash(&self) -> Vec<u8> {
        self.result_hash.clone()
    }
}

/// Decodes only the envelope header of a patch.
#[wasm_bindgen(js_name = patchInfo)]
pub fn patch_info(patch: Vec<u8>) -> Result<PatchInfo, JsError> {
    let info = xpatchlib_core::patch_info(&patch).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(PatchInfo {
        algorithm: info.algorithm,
        base_size: info.base_size as f64,
        result_size: info.result_size as f64,
        payload_len: info.payload_len as f64,
        base_hash: info.base_hash.to_vec(),
        result_hash: info.result_hash.to_vec(),
    })
}
