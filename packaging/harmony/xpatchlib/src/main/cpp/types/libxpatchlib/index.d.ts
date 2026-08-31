/** Algorithm names compiled into the native library. */
export const algorithms: () => string[];

/** Replays `patch` against `base`; throws when verification fails. */
export const applyPatch: (patch: Uint8Array, base: Uint8Array) => ArrayBuffer;
