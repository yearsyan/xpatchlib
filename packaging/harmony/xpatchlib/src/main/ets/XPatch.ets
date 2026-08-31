/**
 * Deterministic binary delta patches for app update bundles (XPDL
 * format). Thin ArkTS facade over the NAPI module — the JS side never
 * touches buffers directly.
 *
 * Replay only: patches are produced by the build toolchain / server side.
 * The HAR ships no patch production code at all.
 */
import xpatchlib from 'libxpatchlib.so';

export function algorithms(): string[] {
  return xpatchlib.algorithms() as string[];
}

export function applyPatch(patch: Uint8Array, base: Uint8Array): Uint8Array {
  const buffer = xpatchlib.applyPatch(patch, base) as ArrayBuffer;
  return new Uint8Array(buffer);
}
