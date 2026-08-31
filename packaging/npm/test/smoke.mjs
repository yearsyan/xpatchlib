import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { algorithms, createPatch, applyPatch } = require('..');

const oldBundle = Buffer.from('the quick brown fox jumps over the lazy dog. '.repeat(4000));
const newBundle = Buffer.concat([
  oldBundle.subarray(0, 40000),
  Buffer.from("require('new-module');\n".repeat(200)),
  oldBundle.subarray(60000),
]);

const patch = createPatch('bsdiff', oldBundle, newBundle);
const restored = applyPatch(patch, oldBundle);
if (Buffer.compare(Buffer.from(restored), newBundle) !== 0) {
  throw new Error('bsdiff round trip diverged');
}
console.log(`xpatchlib wasm smoke ok: ${algorithms().join(',')} patch=${patch.length}B`);
