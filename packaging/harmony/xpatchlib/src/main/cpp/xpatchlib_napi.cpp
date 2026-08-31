// HarmonyOS NAPI adapter over the xpatchlib C ABI. Compiled by hvigor via
// CMakeLists.txt against the ohos staticlib produced by build.sh.
//
// Replay only: the static lib is built from xpatchlib-ffi without the
// "produce" feature, so this adapter could not build a patch even if it
// wanted to — apps only replay patches they download.
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "napi/native_api.h"
#include "xpatchlib.h"

namespace {

// Copies the FFI buffer into an ArkTS ArrayBuffer and frees it.
napi_value TakeBuffer(napi_env env, uint8_t *ptr, size_t len) {
    napi_value array_buffer;
    void *data = nullptr;
    napi_create_arraybuffer(env, len, &data, &array_buffer);
    if (len > 0 && data != nullptr) {
        memcpy(data, ptr, len);
    }
    xpatchlib_free(ptr, len);
    return array_buffer;
}

// Accepts ArrayBuffer or TypedArray views.
bool GetBytes(napi_env env, napi_value value, const uint8_t **out, size_t *len) {
    bool typed = false;
    if (napi_is_typedarray(env, value, &typed) == napi_ok && typed) {
        napi_typedarray_type type;
        size_t elements = 0;
        void *data = nullptr;
        napi_value buffer;
        size_t offset = 0;
        if (napi_get_typedarray_info(env, value, &type, &elements, &data, &buffer, &offset) != napi_ok) {
            return false;
        }
        *out = static_cast<const uint8_t *>(data);
        *len = elements; // uint8 views only; validated by the caller below
        return type == napi_uint8_array;
    }
    bool detached = false;
    if (napi_is_detached_arraybuffer(env, value, &detached) != napi_ok || detached) {
        return false;
    }
    void *data = nullptr;
    size_t byte_length = 0;
    if (napi_get_arraybuffer_info(env, value, &data, &byte_length) != napi_ok) {
        return false;
    }
    *out = static_cast<const uint8_t *>(data);
    *len = byte_length;
    return true;
}

napi_value Algorithms(napi_env env, napi_callback_info /*info*/) {
    napi_value result;
    size_t count = xpatchlib_algorithm_count();
    napi_create_array_with_length(env, count, &result);
    for (size_t i = 0; i < count; i++) {
        const char *name = xpatchlib_algorithm_name(i);
        napi_value item;
        napi_create_string_utf8(env, name == nullptr ? "" : name, NAPI_AUTO_LENGTH, &item);
        napi_set_element(env, result, i, item);
    }
    return result;
}

napi_value Apply(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value args[2];
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    if (argc != 2) {
        napi_throw_error(env, nullptr, "applyPatch(patch, base) expects 2 arguments");
        return nullptr;
    }
    const uint8_t *patch = nullptr;
    const uint8_t *base = nullptr;
    size_t patch_len = 0;
    size_t base_len = 0;
    if (!GetBytes(env, args[0], &patch, &patch_len) || !GetBytes(env, args[1], &base, &base_len)) {
        napi_throw_error(env, nullptr, "patch and base must be Uint8Array or ArrayBuffer");
        return nullptr;
    }
    uint8_t *out = nullptr;
    size_t out_len = 0;
    int status = xpatchlib_apply(patch, patch_len, base, base_len, &out, &out_len);
    if (status != XPATCHLIB_OK) {
        std::string message = "applyPatch failed with code " + std::to_string(status);
        napi_throw_error(env, nullptr, message.c_str());
        return nullptr;
    }
    return TakeBuffer(env, out, out_len);
}

} // namespace

static napi_value Init(napi_env env, napi_value exports) {
    napi_value algorithms_fn;
    napi_create_function(env, "algorithms", NAPI_AUTO_LENGTH, Algorithms, nullptr, &algorithms_fn);
    napi_set_named_property(env, exports, "algorithms", algorithms_fn);

    napi_value apply_fn;
    napi_create_function(env, "applyPatch", NAPI_AUTO_LENGTH, Apply, nullptr, &apply_fn);
    napi_set_named_property(env, exports, "applyPatch", apply_fn);
    return exports;
}

static napi_module g_module = {
    .nm_version = 1,
    .nm_flags = 0,
    .nm_filename = nullptr,
    .nm_register_func = Init,
    .nm_modname = "xpatchlib",
    .nm_priv = nullptr,
    .reserved = {nullptr},
};

extern "C" __attribute__((visibility("default"))) napi_module *GetModule(void) {
    return &g_module;
}
