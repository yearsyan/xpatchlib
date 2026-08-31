/* xpatchlib — deterministic binary delta patches for app update bundles.
 *
 * One C ABI serves the iOS app (static lib) and the HarmonyOS NAPI adapter
 * (shared lib). Android uses the JNI wrapper crate; Node uses the wasm
 * build. All entry points are thread safe.
 *
 * Replay only: patches are produced by the Node toolchain / server side.
 * This library builds the core without the "produce" feature, so no patch
 * production code is compiled in and clients can only replay patches.
 *
 * Buffers returned via out/out_len are heap allocations owned by the caller
 * until released with xpatchlib_free().
 */

#ifndef XPATCHLIB_H
#define XPATCHLIB_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* status codes */
#define XPATCHLIB_OK 0
#define XPATCHLIB_ERR_UNKNOWN_ALGORITHM 1
#define XPATCHLIB_ERR_CORRUPT_PATCH 2
#define XPATCHLIB_ERR_BASE_MISMATCH 3
#define XPATCHLIB_ERR_CHECKSUM 4
#define XPATCHLIB_ERR_CODEC 5
#define XPATCHLIB_ERR_INVALID_ARG 6

/* Number of replay algorithms compiled into this library. */
size_t xpatchlib_algorithm_count(void);

/* Stable, NUL-terminated name of the algorithm at index, or NULL when out
 * of range. The pointers live for the lifetime of the library. */
const char *xpatchlib_algorithm_name(size_t index);

/* Replay patch against base. Verifies the base hash and the result hash;
 * on any mismatch nothing is written to *out and an error code is
 * returned. */
int xpatchlib_apply(const uint8_t *patch, size_t patch_len,
                    const uint8_t *base, size_t base_len,
                    uint8_t **out, size_t *out_len);

/* Decode the envelope header. Any output pointer may be NULL to skip the
 * field. The algorithm pointer is static (do not free). */
int xpatchlib_patch_info(const uint8_t *patch, size_t patch_len,
                         const char **algorithm,
                         uint64_t *base_size, uint64_t *result_size);

/* Release a buffer produced by xpatchlib_apply(). */
void xpatchlib_free(uint8_t *ptr, size_t len);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* XPATCHLIB_H */
