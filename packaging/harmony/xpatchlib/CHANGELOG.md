# Changelog

## 0.2.0 (2026-09-01)

- New streaming, file-based replay: `applyPatchToFile(patchPath, basePath,
  outPath)` replays directly between files with memory bounded by the
  patch size plus small fixed buffers — the base bundle, the diff stream
  and the result never materialize in memory (peak previously ~3x bundle
  size). Verification is unchanged: base hash before replay, result size
  and hash after; on failure any partial output file is removed.
- The in-memory `applyPatch(patch, base)` stays available for callers that
  already hold bytes.

## 0.1.3 (2026-09-01)

- Packaging/CI only, no runtime changes: release assets are now frozen
  on first publish (immutable-registry bytes) and the CocoaPods trunk
  spec sha256 is verified against the GitHub release zip on every tag.

## 0.1.2 (2026-09-01)

- CI now assembles the HAR and publishes to ohpm on tag releases.

## 0.1.1 (2026-09-01)

- Rename the patch envelope magic to XPDL (format version 1).
- Replay-only build: the HAR contains no patch generation code.

## 0.1.0 (2026-08-31)

- Initial release: XPDL delta patch replay for app update bundles.
- bsdiff / zdict / block / full replay codecs, dual SHA-256 verification
  (base hash before replay, result hash after).
- ArkTS facade: `algorithms()`, `applyPatch(patch, base)`.
