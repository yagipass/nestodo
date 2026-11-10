#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

export MACOSX_DEPLOYMENT_TARGET=14.0
cargo build --release --locked

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
app=target/release/Nestodo.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/nestodo-desktop "$app/Contents/MacOS/Nestodo"

for library in $(otool -L "$app/Contents/MacOS/Nestodo" | awk '/\/nix\/store\/.*libiconv/ { print $1 }'); do
  install_name_tool -change "$library" /usr/lib/libiconv.2.dylib "$app/Contents/MacOS/Nestodo"
done
if otool -L "$app/Contents/MacOS/Nestodo" | grep -q /nix/store; then
  echo "The app still links a library from /nix/store, so it would not start on another Mac." >&2
  exit 1
fi

iconset=$(mktemp -d)/Nestodo.iconset
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" ../web/public/icon-512.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" ../web/public/icon-512.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Nestodo.icns"
rm -rf "$(dirname "$iconset")"
cp ../../THIRD_PARTY_NOTICES.txt "$app/Contents/Resources/"

sed -e "s/@version@/$version/g" -e "s/@minimumSystemVersion@/$MACOSX_DEPLOYMENT_TARGET/g" Info.plist >"$app/Contents/Info.plist"

codesign --force --sign - "$app"
echo "Built $PWD/$app"
