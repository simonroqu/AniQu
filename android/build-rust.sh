#!/bin/sh
# Baut aniqu-ffi für Android (.so nach app/src/main/jniLibs) und erzeugt die Kotlin-Anbindung.
# Braucht: rustup mit Android-Zielen, cargo-ndk, NDK. Standard: alles unter ~/Android (ohne root, eigenes rustup,
# damit das System-Rust unberührt bleibt). Ziele: ABIS="arm64-v8a x86_64" (x86_64 für Waydroid/Emulator).
set -e
cd "$(dirname "$0")"
SDK=${ANDROID_HOME:-$HOME/Android/Sdk}
export ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -n1)}
export RUSTUP_HOME=${RUSTUP_HOME:-$HOME/Android/rust/rustup}
export PATH="$HOME/Android/rust/cargo/bin:$PATH"
export CARGO_TARGET_DIR=../target/android
PROFILE=${PROFILE:-release}

args=""
for abi in ${ABIS:-arm64-v8a x86_64}; do args="$args -t $abi"; done
# shellcheck disable=SC2086
cargo ndk $args -o app/src/main/jniLibs build -p aniqu-ffi --profile "$PROFILE"

# Kotlin-Code aus der Bibliothek (Host-Build reicht, die Schnittstelle ist überall gleich).
cargo build -q -p aniqu-ffi
cargo run -q -p aniqu-ffi --bin uniffi-bindgen -- generate --library "$CARGO_TARGET_DIR/debug/libaniqu_ffi.so" \
  --language kotlin --out-dir app/src/main/java --no-format
