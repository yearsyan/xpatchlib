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

/**
 * Streaming, file-based replay: replays the patch file against the base
 * file, writing the result straight to `outPath`. Memory stays bounded by
 * the patch size plus small fixed buffers regardless of bundle size.
 * Verification matches applyPatch (base hash before, result size and hash
 * after); on failure any partially written output is removed and an Error
 * is thrown. All three paths must be distinct.
 */
export function applyPatchToFile(patchPath: string, basePath: string, outPath: string): void {
  xpatchlib.applyPatchToFile(patchPath, basePath, outPath);
}
