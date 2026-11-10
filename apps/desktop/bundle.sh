#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
: "${NESTODO_BASE_URL:?Set NESTODO_BASE_URL to your server, such as https://nestodo.example.com}"

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

cat >"$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>Nestodo</string>
  <key>CFBundleExecutable</key>
  <string>Nestodo</string>
  <key>CFBundleIconFile</key>
  <string>Nestodo</string>
  <key>CFBundleIdentifier</key>
  <string>io.github.yagipass.nestodo.desktop</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Nestodo</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${version}</string>
  <key>CFBundleVersion</key>
  <string>${version}</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.productivity</string>
  <key>LSMinimumSystemVersion</key>
  <string>${MACOSX_DEPLOYMENT_TARGET}</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

codesign --force --sign - "$app"
echo "Built $PWD/$app"
