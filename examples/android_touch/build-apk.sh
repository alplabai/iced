#!/usr/bin/env bash
# Builds and signs a debug APK for the android_touch example (arm64 only).
#
# Needs ANDROID_HOME and ANDROID_NDK_ROOT, plus the NDK linker variables in
# CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER / CC_aarch64_linux_android.
# cargo-apk 0.10 cannot be used: it trips over the `examples/*` member glob.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
target_dir=${CARGO_TARGET_DIR:-$here/../../target}
build_tools=$ANDROID_HOME/build-tools/${BUILD_TOOLS_VERSION:-35.0.0}
platform=$ANDROID_HOME/platforms/android-${PLATFORM_VERSION:-34}/android.jar
out=$target_dir/apk
keystore=${KEYSTORE:-$HOME/.android/debug.keystore}

cargo build --release --target aarch64-linux-android -p android_touch

rm -rf "$out" && mkdir -p "$out/lib/arm64-v8a"
cp "$target_dir/aarch64-linux-android/release/libandroid_touch.so" "$out/lib/arm64-v8a/"

"$build_tools/aapt2" link -o "$out/unaligned.apk" -I "$platform" --manifest "$here/AndroidManifest.xml"
(cd "$out" && zip -q -r unaligned.apk lib)
"$build_tools/zipalign" -f -p 4 "$out/unaligned.apk" "$out/aligned.apk"

if [ ! -f "$keystore" ]; then
    keytool -genkeypair -keystore "$keystore" -storepass android -alias androiddebugkey \
        -keypass android -keyalg RSA -validity 10000 -dname "CN=Android Debug,O=Android,C=US"
fi

"$build_tools/apksigner" sign --ks "$keystore" --ks-pass pass:android \
    --out "$out/android_touch.apk" "$out/aligned.apk"

echo "$out/android_touch.apk"
