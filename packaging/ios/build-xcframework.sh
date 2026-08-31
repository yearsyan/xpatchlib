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

# iOS >= 12 libSystem provides ___chkstk_darwin; 13.0 matches the podspec.
export IPHONEOS_DEPLOYMENT_TARGET=13.0

SLICES=()
# Library-based xcframeworks accept ONE library per platform variant, so the
# two simulator architectures are lipo'd into a fat slice. Skips quietly
# when a rustup target is not installed (CI installs all three).
add_slice() { # $1 = slice name, $2... = rust targets
  local name="$1"; shift
  local -a libs=()
  local target
  for target in "$@"; do
    rustup target list --installed | grep -q "^$target$" || continue
    echo "==> $target"
    cargo build --release --manifest-path "$ROOT/crates/xpatchlib-ffi/Cargo.toml" --target "$target"
    libs+=("$ROOT/target/$target/release/libxpatchlib_ffi.a")
  done
  [[ ${#libs[@]} -gt 0 ]] || return 0
  local dir="$OUT/$name"
  mkdir -p "$dir/include"
  cp "$ROOT/crates/xpatchlib-ffi/include/xpatchlib.h" "$dir/include/"
  if [[ ${#libs[@]} -eq 1 ]]; then
    cp "${libs[0]}" "$dir/libxpatchlib_ffi.a"
  else
    lipo -create "${libs[@]}" -output "$dir/libxpatchlib_ffi.a"
  fi
  SLICES+=("-library" "$dir/libxpatchlib_ffi.a" "-headers" "$dir/include")
}

add_slice device aarch64-apple-ios
add_slice simulator aarch64-apple-ios-sim x86_64-apple-ios

[[ ${#SLICES[@]} -gt 0 ]] || { echo "error: no iOS targets installed" >&2; exit 1; }
xcodebuild -create-xcframework "${SLICES[@]}" -output "$OUT/XPatchlib.xcframework"

# The zip doubles as the CocoaPods pod root (the podspec's :http source):
# after unpacking it must contain every relative path the podspec names —
# build/XPatchlib.xcframework, Module.modulemap, LICENSE, README.md.
ZIP="$(cd "$OUT" && pwd)/XPatchlib.xcframework.zip"
POD_ROOT="$OUT/pod-root"
rm -rf "$POD_ROOT"
mkdir -p "$POD_ROOT/build"
cp -R "$OUT/XPatchlib.xcframework" "$POD_ROOT/build/"
cp Module.modulemap README.md "$POD_ROOT/"
cp "$ROOT/LICENSE" "$POD_ROOT/"
(cd "$POD_ROOT" && rm -f "$ZIP" && zip -qr "$ZIP" build Module.modulemap LICENSE README.md)
echo "==> built $OUT/XPatchlib.xcframework(.zip)"
echo "publish: attach the zip to the GitHub Release, then fill :sha256 in XPatchlib.podspec:" \
  "shasum -a 256 <zip>"
