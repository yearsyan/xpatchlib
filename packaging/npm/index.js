'use strict';

// Loader for the wasm-bindgen nodejs-target glue. The wasm binary embeds
// the same xpatchlib core that ships to Android/iOS/HarmonyOS, so patches
// produced here replay bit-for-bit on every client. This package is the
// producer side; the mobile artifacts build the core without the
// "produce" feature and can only replay.
module.exports = require('./wasm/xpatchlib_wasm.js');
