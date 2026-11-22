#!/bin/sh
# Runs build-legacy.sh in an x86_64 Linux container, for a Mac (or anything that
# cannot run NDK r10e's Linux tools). Pass the folder holding android-ndk-r10e:
#
#   ./build-legacy-docker.sh ~/android-sdk/legacy-ndk
set -eu
cd "$(dirname "$0")"
ROOT=$(cd ../.. && pwd)
NDKDIR=$(cd "${1:?the folder that holds android-ndk-r10e}" && pwd)
# Rust is installed once into a volume and kept; LEGACY_RUST_VOLUME picks it.
VOLUME=${LEGACY_RUST_VOLUME:-klatter-legacy-rust}
docker volume create "$VOLUME" >/dev/null
docker run --rm --platform linux/amd64 \
	-v "$ROOT":/src -v "$NDKDIR":/ndk:ro -v "$VOLUME":/rust \
	-e CARGO_HOME=/rust/cargo -e RUSTUP_HOME=/rust/rustup \
	-e ANDROID_NDK_R10E=/ndk/android-ndk-r10e \
	ubuntu:24.04 bash -c '
set -e
export DEBIAN_FRONTEND=noninteractive PATH=/rust/cargo/bin:$PATH
apt-get update -qq >/dev/null && apt-get install -y -qq curl ca-certificates gcc g++ >/dev/null 2>&1
[ -x /rust/cargo/bin/rustup ] || curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal --no-modify-path >/dev/null 2>&1
rustup target add arm-linux-androideabi >/dev/null 2>&1
cd /src/plat/android && ./build-legacy.sh'
