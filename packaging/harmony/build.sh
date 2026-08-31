#!/usr/bin/env bash
# Prepares the xpatchlib HAR module (packaging/harmony/xpatchlib) for hvigor:
# builds the ohos arm64 static lib and stages the C header, then leaves the
# HAR assembly to DevEco/hvigor (`hvigorw assembleHar`).
# Replay only: xpatchlib-ffi builds xpatchlib-core without the "produce"
# feature, so no patch generation code ships to devices.
# Requires: rustup target add aarch64-unknown-linux-ohos, OHOS NDK clang
# (DevEco Studio: /Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/native).
set -euo pipefail
cd "$(dirname "$0")"
ROOT="$(cd ../.. && pwd)"
MODULE="xpatchlib"

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
mkdir -p "$MODULE/libs/arm64-v8a"
cp "$ROOT/target/$TARGET/release/libxpatchlib_ffi.a" "$MODULE/libs/arm64-v8a/"
# The HAR must be self-contained: consumers compile the NAPI adapter from
# source against their own SDK, so the C header ships next to it.
cp "$ROOT/crates/xpatchlib-ffi/include/xpatchlib.h" "$MODULE/src/main/cpp/"

echo "==> $MODULE/libs/arm64-v8a/libxpatchlib_ffi.a + xpatchlib.h staged"
echo "==> assemble the HAR:  DEVECO_SDK_HOME=<devco sdk> hvigorw assembleHar --mode module -p product=default"
echo "==> publish to ohpm:  ohpm publish $MODULE/build/default/outputs/default/xpatchlib.har \\" 
echo "    --publish_id <id> --key_path ~/.ohpm/ohpm_publish   (or set both via 'ohpm config set')"
