#!/usr/bin/env bash
# Assembles io.github.yearsyan:xpatchlib:<version>.aar from the xpatchlib-jni
# crate. The AAR is replay-only: xpatchlib-jni builds xpatchlib-core without
# the "produce" feature, so no patch generation code ships to devices.
# Requires: an Android NDK (found via ANDROID_NDK_HOME, ANDROID_HOME /
# ANDROID_SDK_ROOT, ~/android-sdk or ~/Library/Android/sdk), a JDK
# (javac + jar) and rustup android targets:
#   rustup target add aarch64-linux-android armv7-linux-androideabi \
#     x86_64-linux-android i686-linux-android
set -euo pipefail
cd "$(dirname "$0")"
ROOT="$(cd ../.. && pwd)"

# Collect NDK candidates from every well-known location, newest version wins.
find_ndk() {
  local -a roots=()
  [[ -n "${ANDROID_NDK_HOME:-}" ]] && roots+=("$ANDROID_NDK_HOME")
  local sdk d
  for sdk in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}" "$HOME/android-sdk" "$HOME/Library/Android/sdk"; do
    [[ -n "$sdk" && -d "$sdk/ndk" ]] || continue
    while IFS= read -r d; do roots+=("$d"); done < <(ls -d "$sdk"/ndk/* 2>/dev/null || true)
  done
  local best=""
  for d in "${roots[@]}"; do
    [[ -x "$d/ndk-build" ]] && best="$d"
  done
  echo "$best"
}
NDK_HOME="$(find_ndk)"
if [[ -z "$NDK_HOME" || ! -d "$NDK_HOME" ]]; then
  echo "error: Android NDK not found; set ANDROID_NDK_HOME or ANDROID_HOME" >&2
  exit 1
fi
# The prebuilt dir name varies by NDK release and host (darwin-x86_64, ...).
TOOLCHAIN_BIN="$(echo "$NDK_HOME"/toolchains/llvm/prebuilt/*/bin)"
[[ -d "$TOOLCHAIN_BIN" ]] || { echo "error: NDK llvm toolchain not found in $NDK_HOME" >&2; exit 1; }
command -v javac >/dev/null || { echo "error: javac (JDK) required" >&2; exit 1; }
echo "==> NDK: $NDK_HOME"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
OUT="build/aar"
rm -rf "$OUT"
mkdir -p "$OUT/jni" "$OUT/classes"

# rust targets -> ABI names (add/remove to match your device matrix).
# $1 rust triple, $2 NDK clang triple (armv7 gains an "a"), $3 ABI name.
build() {
  local target="$1" ndk_triple="$2" abi="$3"
  local clang="$TOOLCHAIN_BIN/${ndk_triple}21-clang"
  [[ -x "$clang" ]] || { echo "error: $clang not found" >&2; exit 1; }
  local env_triple="${target//-/_}"
  local upper; upper="$(printf '%s' "$env_triple" | tr '[:lower:]' '[:upper:]')"
  echo "==> $target ($abi)"
  export "CC_${env_triple}=$clang"
  export "AR_${env_triple}=$TOOLCHAIN_BIN/llvm-ar"
  export "CARGO_TARGET_${upper}_LINKER=$clang"
  cargo build --release --manifest-path "$ROOT/crates/xpatchlib-jni/Cargo.toml" --target "$target"
  mkdir -p "$OUT/jni/$abi"
  cp "$ROOT/target/$target/release/libxpatchlib_jni.so" "$OUT/jni/$abi/"
}
build aarch64-linux-android aarch64-linux-android arm64-v8a
build armv7-linux-androideabi armv7a-linux-androideabi armeabi-v7a
build x86_64-linux-android x86_64-linux-android x86_64
build i686-linux-android i686-linux-android x86

javac --release 8 -d "$OUT/classes" $(find src -name '*.java')
jar cf "$OUT/classes.jar" -C "$OUT/classes" .
rm -rf "$OUT/classes"

cp AndroidManifest.xml "$OUT/"
(cd "$OUT" && zip -qr "../xpatchlib-$VERSION.aar" AndroidManifest.xml classes.jar jni)
echo "==> built $(cd "$OUT" && pwd)/xpatchlib-$VERSION.aar"
echo "publish: mv xpatchlib-$VERSION.aar  <maven-repo>/io/github/yearsyan/xpatchlib/$VERSION/"
