#!/usr/bin/env bash
# Rebuilds the wasm artifact in place. Apple clang has no wasm target, so
# the Homebrew LLVM toolchain is used when present.
set -euo pipefail
cd "$(dirname "$0")"

LLVM="${LLVM_BIN:-}"
if [[ -z "$LLVM" ]]; then
  if [[ -x /opt/homebrew/opt/llvm/bin/clang ]]; then
    LLVM=/opt/homebrew/opt/llvm/bin # macOS Homebrew LLVM (Apple clang has no wasm)
  elif command -v clang >/dev/null 2>&1; then
    LLVM="$(dirname "$(command -v clang)")" # Linux: apt/zypper clang ships wasm
  fi
fi
if [[ -n "$LLVM" && -x "$LLVM/clang" ]]; then
  export CC_wasm32_unknown_unknown="$LLVM/clang"
  export AR_wasm32_unknown_unknown="$LLVM/llvm-ar"
  export PATH="$LLVM:$PATH"
fi

wasm-pack build ../../crates/xpatchlib-wasm \
  --target nodejs --release --out-dir ../../packaging/npm/wasm

# wasm-pack writes a `.gitignore` (`*`) into the out dir, which would also
# exclude the wasm artifacts from `npm publish`; the repo root .gitignore
# already covers this directory for git, so drop the generated one.
rm -f ../../packaging/npm/wasm/.gitignore
