# Changelog

## 0.1.1 (2026-09-01)

- Rename the patch envelope magic to XPDL (format version 1).
- Replay-only build: the HAR contains no patch generation code.

## 0.1.0 (2026-08-31)

- Initial release: XPDL delta patch replay for app update bundles.
- bsdiff / zdict / block / full replay codecs, dual SHA-256 verification
  (base hash before replay, result hash after).
- ArkTS facade: `algorithms()`, `applyPatch(patch, base)`.
