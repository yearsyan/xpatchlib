# XPatchlib (iOS)

Deterministic binary delta patch **replay** for app update bundles — the
CocoaPods distribution of [xpatchlib](https://github.com/yearsyan/xpatchlib).

Patches are produced by the Node toolchain (`@lynfe/xpatchlib` on npm); this
Pod contains no patch generation code at all (the underlying core is built
without the `produce` cargo feature).

## Usage

```ruby
pod 'XPatchlib'
```

```swift
import XPatchlib

// patch: downloaded delta bytes; base: the bundle currently on disk.
// Verifies the base hash before replay and the result hash after.
var patchPtr: UnsafePointer<UInt8> = patch
var basePtr: UnsafePointer<UInt8> = base
var out: UnsafeMutablePointer<UInt8>? = nil
var outLen = 0
let status = XPatchlibApply(patchPtr, patch.count, basePtr, base.count, &out, &outLen)
guard status == XPATCHLIB_OK else { throw ... }
```

Same core as the Android AAR (`io.github.yearsyan:xpatchlib`) and the
HarmonyOS ohpm package (`xpatchlib`).

## License

MIT
