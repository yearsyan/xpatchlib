# @lynfe/xpatchlib (wasm)

Deterministic binary delta patches for app update bundles — **producer side** of
the [xpatchlib](https://github.com/yearsyan/xpatchlib) toolchain. Runs in Node
during the build/publish step to generate XPDL patches that Android, iOS and
HarmonyOS clients replay bit-for-bit.

## Install

```bash
npm install --save-dev @lynfe/xpatchlib
```

## Usage

```js
import { algorithms, createPatch, applyPatch, patchInfo } from '@lynfe/xpatchlib';

// oldBundle: bytes of the previously published bundle
// newBundle: bytes of the bundle about to be published
const patch = createPatch('bsdiff', oldBundle, newBundle);

// catalog metadata for the client's pre-flight checks
const info = patchInfo(patch); // { algorithm, baseSize, resultSize, payloadLen, baseHash, resultHash }

// sanity check that the patch replays (same core as the mobile clients)
const restored = applyPatch(patch, oldBundle);
```

Algorithms: `bsdiff` (smallest patches), `zdict` (fastest), `block`
(middle ground), `full` (recompressed baseline — use it to decide when a
delta is not worth shipping).

Patches are byte-for-byte deterministic for identical input: safe to store
in content-addressed object stores. The envelope pins SHA-256 of both the
base and the result; clients verify both before and after replay.

## License

MIT
