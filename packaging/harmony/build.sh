#!/usr/bin/env bash
# Builds the ohos arm64 static lib for the NAPI adapter. The HAR itself is
# assembled by DevEco/hvigor (or `hvigorw assembleHar`) after this step.
# Replay only: xpatchlib-ffi builds xpatchlib-core without the "produce"
# feature, so no patch generation code ships to devices.
# Requires: rustup target add aarch64-unknown-linux-ohos, OHOS NDK clang
# (DevEco Studio: /Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/native).
set -euo pipefail
cd "$(dirname "$0")"
ROOT="$(cd ../.. && pwd)"

TARGET=aarch64-unknown-linux-ohos
rustup target list --installed | grep -q "^$TARGET$" || rustup target add "$TARGET"

OHOS_NDK="${OHOS_NDK_HOME:-}"
if [[ -z "$OHOS_NDK" ]]; then
  for candidate in \
    /Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/native \
    ~/Library/OpenHarmony/Sdk/default/openharmony/native \
    ~/Library/OpenHarmony/Sdk/*/openharmony/native \
    ~/openharmony/sdk/native \
    /opt/openharmony/sdk/native; do
    [[ -x "$candidate/llvm/bin/clang" ]] && OHOS_NDK="$candidate" && break
  done
fi
if [[ -z "$OHOS_NDK" || ! -x "$OHOS_NDK/llvm/bin/clang" ]]; then
  echo "error: OpenHarmony NDK clang not found; set OHOS_NDK_HOME" >&2
  exit 1
fi

# Cargo reads CC_/AR_ with the target triple spelled in underscores; the
# hyphenated form is not a valid shell identifier to export.
TARGET_ENV="${TARGET//-/_}"
export "CC_$TARGET_ENV=$OHOS_NDK/llvm/bin/clang"
export "AR_$TARGET_ENV=$OHOS_NDK/llvm/bin/llvm-ar"
export RUSTFLAGS="-C linker=$OHOS_NDK/llvm/bin/clang \
-C link-arg=--target=aarch64-linux-ohos \
-C link-arg=-nostdlib++ \
-L$OHOS_NDK/llvm/lib/aarch64-linux-ohos \
-L$OHOS_NDK/llvm/lib/aarch64-linux-ohos/usc"

cargo build --release --manifest-path "$ROOT/crates/xpatchlib-ffi/Cargo.toml" --target "$TARGET"
mkdir -p libs/arm64-v8a
cp "$ROOT/target/$TARGET/release/libxpatchlib_ffi.a" libs/arm64-v8a/
echo "==> libs/arm64-v8a/libxpatchlib_ffi.a ready; build the HAR with hvigorw assembleHar"
