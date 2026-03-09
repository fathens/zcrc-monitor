#!/bin/bash
# Android 向け Rust ライブラリをビルドし、jniLibs に配置するスクリプト
#
# 前提:
#   cargo install cargo-ndk
#   rustup target add aarch64-linux-android x86_64-linux-android
#
# 使い方:
#   ./android/build-rust.sh [release]

set -euo pipefail

cd "$(dirname "$0")/.."

PROFILE="${1:-debug}"
if [ "$PROFILE" = "release" ]; then
    CARGO_FLAGS="--release"
    TARGET_DIR="release"
else
    CARGO_FLAGS=""
    TARGET_DIR="debug"
fi

JNILIBS_DIR="android/app/src/main/jniLibs"

for ARCH in arm64-v8a x86_64; do
    echo "Building for $ARCH ($PROFILE)..."
    cargo ndk -t "$ARCH" build --lib $CARGO_FLAGS

    case "$ARCH" in
        arm64-v8a) RUST_TARGET="aarch64-linux-android" ;;
        x86_64)    RUST_TARGET="x86_64-linux-android" ;;
    esac

    mkdir -p "$JNILIBS_DIR/$ARCH"
    cp "target/$RUST_TARGET/$TARGET_DIR/libzcrc_monitor.so" "$JNILIBS_DIR/$ARCH/"
done

echo "Done. Native libraries placed in $JNILIBS_DIR"
