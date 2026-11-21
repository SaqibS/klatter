#!/bin/sh
# Builds Klatter's voice for VoiceOver: the app with its speech synthesis
# extension inside, for the Mac, or iPhone and iPad.
#
#   ./build.sh mac          build for the Mac (Apple silicon and Intel), into build/
#   ./build.sh mac install  build, put it in /Applications, register the voice
#   ./build.sh device       build for a real iPhone or iPad
#   ./build.sh simulator    build for the iOS Simulator
#
# Signed with the team in project.yml; a free Apple account works. macOS and
# iOS list a third-party voice only when it is signed with a real Apple
# certificate and its audio component is sandbox safe. Needs Rust, Xcode and
# xcodegen, with the targets: rustup target add aarch64-apple-darwin
# x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
set -eu
cd "$(dirname "$0")"
ROOT=$(cd ../.. && pwd)
WHERE=${1:-mac}
ACTION=${2:-}
lib() { cargo build --release -q -p klatter-ffi --target "$1" --manifest-path "$ROOT/Cargo.toml"; echo "$ROOT/target/$1/release/libklatter.a"; }

case $WHERE in
	mac)
		mkdir -p lib/macosx
		export MACOSX_DEPLOYMENT_TARGET=13.0
		lipo -create "$(lib aarch64-apple-darwin)" "$(lib x86_64-apple-darwin)" -output lib/macosx/libklatter.a
		SCHEME=Klatter; SDK=macosx; DEST=generic/platform=macOS; PRODUCTS=Release ;;
	device)
		mkdir -p lib/iphoneos
		export IPHONEOS_DEPLOYMENT_TARGET=16.0
		cp "$(lib aarch64-apple-ios)" lib/iphoneos/libklatter.a
		SCHEME=KlatteriOS; SDK=iphoneos; DEST=generic/platform=iOS; PRODUCTS=Release-iphoneos ;;
	simulator)
		mkdir -p lib/iphonesimulator
		export IPHONEOS_DEPLOYMENT_TARGET=16.0
		lipo -create "$(lib aarch64-apple-ios-sim)" "$(lib x86_64-apple-ios)" -output lib/iphonesimulator/libklatter.a
		SCHEME=KlatteriOS; SDK=iphonesimulator; DEST="generic/platform=iOS Simulator"; PRODUCTS=Release-iphonesimulator ;;
	*) echo "usage: ./build.sh mac [install] | device | simulator" >&2; exit 2 ;;
esac

xcodegen generate
xcodebuild -project Klatter.xcodeproj -scheme $SCHEME -configuration Release -sdk $SDK \
	-derivedDataPath build/dd -destination "$DEST" -allowProvisioningUpdates build
APP=build/dd/Build/Products/$PRODUCTS/Klatter.app
echo "built $APP"

if [ "$WHERE" = mac ] && [ "$ACTION" = install ]; then
	LSREGISTER=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
	pkill -x KlatterVoice 2>/dev/null || true
	pkill -x Klatter 2>/dev/null || true
	rm -rf /Applications/Klatter.app
	cp -R "$APP" /Applications/
	pluginkit -r "$APP/Contents/PlugIns/KlatterVoice.appex" 2>/dev/null || true
	"$LSREGISTER" -u "$APP" 2>/dev/null || true
	"$LSREGISTER" -f /Applications/Klatter.app
	pluginkit -a /Applications/Klatter.app/Contents/PlugIns/KlatterVoice.appex 2>/dev/null || true
	open -g /Applications/Klatter.app
	echo "installed /Applications/Klatter.app"
fi
