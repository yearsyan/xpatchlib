# xpatchlib (HarmonyOS)

Deterministic binary delta patch **replay** for app update bundles (XPDL
format). Patches are produced by the Node toolchain
([@lynfe/xpatchlib](https://www.npmjs.com/package/@lynfe/xpatchlib)) and
replayed here bit-for-bit — the HAR ships no patch generation code.

```bash
ohpm install xpatchlib
```

```ets
import { applyPatch } from 'xpatchlib';

const bytes = applyPatch(patch, localBundle); // throws on verification failure
```

- Homepage: https://github.com/yearsyan/xpatchlib
- License: MIT
