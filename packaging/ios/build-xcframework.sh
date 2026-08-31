#!/usr/bin/env bash
# Builds libxpatchlib staticlibs and wraps them in XPatchlib.xcframework.
# Replay only: xpatchlib-ffi builds xpatchlib-core without the "produce"
# feature, so no patch generation code ships to devices.
# Requires Xcode; simulator slices are included when the targets are
# installed (rustup target add aarch64-apple-ios-sim x86_64-apple-ios).
set -euo pipefail
cd "$(dirname "$0")"
ROOT="$(cd ../.. && pwd)"
OUT="build"
rm -rf "$OUT"
mkdir -p "$OUT"

SLICES=()
add_slice() {
  local target="$1"
  rustup target list --installed | grep -q "^$target$" || return 0
  echo "==> $target"
  # iOS >= 12 libSystem provides ___chkstk_darwin; 13.0 matches the podspec.
  export IPHONEOS_DEPLOYMENT_TARGET=13.0
  cargo build --release --manifest-path "$ROOT/crates/xpatchlib-ffi/Cargo.toml" --target "$target"
  local dir="$OUT/$target"
  mkdir -p "$dir/include"
  cp "$ROOT/target/$target/release/libxpatchlib_ffi.a" "$dir/"
  cp "$ROOT/crates/xpatchlib-ffi/include/xpatchlib.h" "$dir/include/"
  SLICES+=("-library" "$dir/libxpatchlib_ffi.a" "-headers" "$dir/include")
}

add_slice aarch64-apple-ios
add_slice aarch64-apple-ios-sim
add_slice x86_64-apple-ios

[[ ${#SLICES[@]} -gt 0 ]] || { echo "error: no iOS targets installed" >&2; exit 1; }
xcodebuild -create-xcframework "${SLICES[@]}" -output "$OUT/XPatchlib.xcframework"
zip -qr "$OUT/XPatchlib.xcframework.zip" "$OUT/XPatchlib.xcframework"
echo "==> built $OUT/XPatchlib.xcframework(.zip)"
echo "publish: upload the zip and point the podspec :url at it (bump :sha256)"
