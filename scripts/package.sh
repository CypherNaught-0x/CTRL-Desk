#!/usr/bin/env bash
# Packages an already-built release binary of the desktop app.
#
#   cargo build --release --locked -p desk_app --target <target>
#   scripts/package.sh <target> <version> [out-dir]      (default out-dir: dist)
#
# Produces a .zip (Windows), a .app bundle in a .zip (macOS) or a .tar.gz (Linux).
set -euo pipefail

target="$1"
version="$2"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${3:-$root/dist}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

bin_dir="${CARGO_TARGET_DIR:-$root/target}/$target/release"
name="desk-ctrl-$version-$target"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

# Renders the icons with the built binary (no window is opened). A binary
# cross-compiled for another architecture may not run here; then use a host build.
icons() {
    DESK_CTRL_EXPORT_ICONS="$1" "$2" 2>/dev/null ||
        DESK_CTRL_EXPORT_ICONS="$1" cargo run --release --locked -q -p desk_app --manifest-path "$root/Cargo.toml"
}

docs() {
    cp "$root/README.md" "$1/"
    cp "$root/app/assets/fonts/OFL.txt" "$1/LICENSE-Inter-OFL.txt"
}

case "$target" in
*windows*)
    mkdir "$stage/$name"
    cp "$bin_dir/desk-ctrl.exe" "$stage/$name/"
    docs "$stage/$name"
    (cd "$stage" && 7z a -tzip -bso0 "$out/$name.zip" "$name")
    echo "$out/$name.zip"
    ;;
*apple-darwin*)
    app="$stage/CTRL Desk.app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$bin_dir/desk-ctrl" "$app/Contents/MacOS/desk-ctrl"
    docs "$app/Contents/Resources"
    icons "$stage/png" "$bin_dir/desk-ctrl"
    iconset="$stage/AppIcon.iconset"
    mkdir "$iconset"
    for size in 16 32 128 256 512; do
        cp "$stage/png/icon-$size.png" "$iconset/icon_${size}x${size}.png"
        cp "$stage/png/icon-$((size * 2)).png" "$iconset/icon_${size}x${size}@2x.png"
    done
    iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"
    cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>CTRL Desk</string>
    <key>CFBundleDisplayName</key><string>CTRL Desk</string>
    <key>CFBundleIdentifier</key><string>io.github.desk-ctrl</string>
    <key>CFBundleExecutable</key><string>desk-ctrl</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSBluetoothAlwaysUsageDescription</key>
    <string>CTRL Desk uses Bluetooth to find and move your standing desk.</string>
</dict>
</plist>
PLIST
    # Ad-hoc signature so the bundle (with its Info.plist) runs on Apple Silicon.
    codesign --force --deep --sign - "$app"
    (cd "$stage" && ditto -c -k --keepParent "CTRL Desk.app" "$out/$name.zip")
    echo "$out/$name.zip"
    ;;
*linux*)
    mkdir "$stage/$name"
    cp "$bin_dir/desk-ctrl" "$stage/$name/"
    docs "$stage/$name"
    icons "$stage/png" "$bin_dir/desk-ctrl"
    cp "$stage/png/icon-256.png" "$stage/$name/ctrl-desk.png"
    cat > "$stage/$name/ctrl-desk.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=CTRL Desk
Comment=Control your standing desk over Bluetooth
Exec=desk-ctrl
Icon=ctrl-desk
Categories=Utility;
DESKTOP
    tar -czf "$out/$name.tar.gz" -C "$stage" "$name"
    echo "$out/$name.tar.gz"
    ;;
*)
    echo "unsupported target: $target" >&2
    exit 1
    ;;
esac
