/** Algorithm names compiled into the native library. */
export const algorithms: () => string[];

/** Replays `patch` against `base`; throws when verification fails. */
export const applyPatch: (patch: Uint8Array, base: Uint8Array) => ArrayBuffer;

/**
 * Streaming, file-based replay with bounded memory; throws when
 * verification fails (any partial output is removed first).
 */
export const applyPatchToFile: (patchPath: string, basePath: string, outPath: string) => void;
