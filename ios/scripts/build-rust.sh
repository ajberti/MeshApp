#!/bin/bash
set -euo pipefail

if [[ -f "${HOME}/.cargo/env" ]]; then
  # Xcode script phases do not include cargo on PATH.
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
fi
export PATH="${HOME}/.cargo/bin:/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin:${PATH}"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RUST_DIR="$ROOT/mesh-reference-implementation"
OUT_DIR="${SRCROOT:-$(cd "$(dirname "$0")/.." && pwd)}/build"
GEN_DIR="${SRCROOT:-$(cd "$(dirname "$0")/.." && pwd)}/MeshApp/Generated"
XCODE_PLATFORMS="${DEVELOPER_DIR}/Platforms"

PLATFORM_NAME="${PLATFORM_NAME:-iphonesimulator}"
ARCHS="${ARCHS:-arm64}"
CONFIGURATION="${CONFIGURATION:-Debug}"

case "$PLATFORM_NAME" in
  iphoneos) RUST_TARGET="aarch64-apple-ios" ;;
  macosx) RUST_TARGET="aarch64-apple-darwin" ;;
  iphonesimulator)
    if [[ "$ARCHS" == *x86_64* && "$ARCHS" != *arm64* ]]; then
      RUST_TARGET="x86_64-apple-ios"
    else
      RUST_TARGET="aarch64-apple-ios-sim"
    fi
    ;;
  *)
    echo "unsupported PLATFORM_NAME=$PLATFORM_NAME" >&2
    exit 1
    ;;
esac

PROFILE_FLAG="--release"
RELDIR="release"
if [[ "$CONFIGURATION" == "Debug" ]]; then
  PROFILE_FLAG=""
  RELDIR="debug"
fi

# Host bindgen must not inherit the iOS SDK. Prefer the CLT macOS 15 SDK;
# Xcode's newest macOS SDK has previously failed to link rustc on this machine.
if [[ -d /Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk ]]; then
  HOST_SDKROOT="/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk"
else
  HOST_SDKROOT="${XCODE_PLATFORMS}/MacOSX.platform/Developer/SDKs/MacOSX.sdk"
fi

# Always use the platform SDK. A leftover macOS SDKROOT (or
# MACOSX_DEPLOYMENT_TARGET) makes rustc link libiconv from macOS.
case "$PLATFORM_NAME" in
  iphoneos)
    export SDKROOT="${XCODE_PLATFORMS}/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk"
    unset MACOSX_DEPLOYMENT_TARGET
    ;;
  iphonesimulator)
    export SDKROOT="${XCODE_PLATFORMS}/iPhoneSimulator.platform/Developer/SDKs/iPhoneSimulator.sdk"
    unset MACOSX_DEPLOYMENT_TARGET
    ;;
  macosx)
    export SDKROOT="${HOST_SDKROOT}"
    export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-15.0}"
    ;;
esac

export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$RUST_DIR/target}"

cd "$RUST_DIR"
rustup target add "$RUST_TARGET" >/dev/null
cargo build -p mesh-bindings --lib --target "$RUST_TARGET" $PROFILE_FLAG
# Isolate host bindgen from the iOS sysroot Xcode exports as SDKROOT.
env -u IPHONEOS_DEPLOYMENT_TARGET \
  SDKROOT="${HOST_SDKROOT}" \
  MACOSX_DEPLOYMENT_TARGET="15.0" \
  cargo build -p mesh-bindings --features cli --bin uniffi-bindgen --quiet
"$CARGO_TARGET_DIR/debug/uniffi-bindgen" generate \
  --library "$CARGO_TARGET_DIR/$RUST_TARGET/$RELDIR/libmesh_bindings.a" \
  --language swift \
  --out-dir "$GEN_DIR"

mkdir -p "$OUT_DIR"
cp "$CARGO_TARGET_DIR/$RUST_TARGET/$RELDIR/libmesh_bindings.a" "$OUT_DIR/libmesh_bindings.a"

echo "built $RUST_TARGET ($RELDIR) -> $OUT_DIR/libmesh_bindings.a"
