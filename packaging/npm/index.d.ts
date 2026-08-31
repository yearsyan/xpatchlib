/**
 * Deterministic binary delta patches for app update bundles (XPDL
 * format). Producer side of the toolchain: mirrors the xpatchlib core
 * shared by Android (AAR), iOS (Pod) and HarmonyOS (ohpm) — those clients
 * replay only, this package also builds patches.
 */

/** Algorithm names compiled into this module. */
export function algorithms(): string[];

/**
 * Builds an XPDL patch that turns `base` into `updated` using the named
 * algorithm ("bsdiff" for smallest patches, "zdict" for speed).
 */
export function createPatch(algorithm: string, base: Uint8Array, updated: Uint8Array): Uint8Array;

/**
 * Replays `patch` against `base`. Verifies both the base hash and the
 * result hash; throws on any mismatch.
 */
export function applyPatch(patch: Uint8Array, base: Uint8Array): Uint8Array;

/** Decoded envelope header of a patch. */
export interface PatchInfo {
  algorithm: string;
  baseSize: number;
  resultSize: number;
  payloadLen: number;
  baseHash: Uint8Array;
  resultHash: Uint8Array;
}

/** Decodes only the envelope header of a patch. */
export function patchInfo(patch: Uint8Array): PatchInfo;
