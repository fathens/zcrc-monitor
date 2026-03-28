#!/bin/bash
# Android 向け Rust ライブラリをビルドし、jniLibs に配置するスクリプト
#
# 前提:
#   cargo install cargo-ndk
#   rustup target add aarch64-linux-android x86_64-linux-android
#   sdkmanager "ndk;28.0.13004108" "platforms;android-34" "build-tools;35.0.0"
#
# 使い方:
#   ./android/build-rust.sh [release]

set -euo pipefail

cd "$(dirname "$0")/.."

# --- Android SDK / NDK 環境検出 ---
detect_android_sdk() {
    if [ -n "${ANDROID_HOME:-}" ] && [ -d "$ANDROID_HOME" ]; then
        echo "$ANDROID_HOME"
    elif [ -n "${ANDROID_SDK_ROOT:-}" ] && [ -d "$ANDROID_SDK_ROOT" ]; then
        echo "$ANDROID_SDK_ROOT"
    elif [ -d "/opt/homebrew/share/android-commandlinetools" ]; then
        echo "/opt/homebrew/share/android-commandlinetools"
    elif [ -d "$HOME/Library/Android/sdk" ]; then
        echo "$HOME/Library/Android/sdk"
    else
        echo ""
    fi
}

SDK_ROOT=$(detect_android_sdk)
if [ -z "$SDK_ROOT" ]; then
    echo "ERROR: Android SDK not found."
    echo "Set ANDROID_HOME or install via: brew install --cask android-commandlinetools"
    exit 1
fi

# NDK
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    NDK_DIR=$(find "$SDK_ROOT/ndk" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort -V | tail -1)
    if [ -z "$NDK_DIR" ]; then
        echo "ERROR: NDK not found under $SDK_ROOT/ndk/"
        echo "Install via: sdkmanager --sdk_root=$SDK_ROOT \"ndk;28.0.13004108\""
        exit 1
    fi
    export ANDROID_NDK_HOME="$NDK_DIR"
fi

# android.jar（Slint の build.rs が必要とする）
if [ -z "${ANDROID_JAR:-}" ]; then
    PLATFORM_DIR=$(find "$SDK_ROOT/platforms" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort -V | tail -1)
    if [ -n "$PLATFORM_DIR" ] && [ -f "$PLATFORM_DIR/android.jar" ]; then
        export ANDROID_JAR="$PLATFORM_DIR/android.jar"
    else
        echo "ERROR: android.jar not found under $SDK_ROOT/platforms/"
        echo "Install via: sdkmanager --sdk_root=$SDK_ROOT \"platforms;android-34\""
        exit 1
    fi
fi

export ANDROID_HOME="$SDK_ROOT"
export ANDROID_SDK_ROOT="$SDK_ROOT"

echo "SDK:  $SDK_ROOT"
echo "NDK:  $ANDROID_NDK_HOME"
echo "JAR:  $ANDROID_JAR"
echo ""

# --- ビルド ---
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

echo ""
echo "Done. Native libraries placed in $JNILIBS_DIR"
