#!/usr/bin/env bash
# Regenerates the README screenshots using the simulated desk (no hardware needed).
#
#   scripts/screenshots.sh [output-dir]       (default: docs/screenshots)
#
# On a headless Linux machine, run it under a virtual display:
#   xvfb-run -a -s "-screen 0 1920x1200x24" scripts/screenshots.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:-$root/docs/screenshots}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

cargo build --release --locked -p desk_app --manifest-path "$root/Cargo.toml"
bin="${CARGO_TARGET_DIR:-$root/target}/release/desk-ctrl"
[[ -f "$bin.exe" ]] && bin="$bin.exe"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
desk='"desk":{"name":"CTRL DESK","id":"demo"}'

# shot <name> <config-json> [extra env assignments...]
shot() {
    local name="$1" config="$2"
    shift 2
    echo "$config" > "$tmp/$name.json"
    echo "  $name.png"
    env DESK_CTRL_DEMO=1 \
        DESK_CTRL_CONFIG="$tmp/$name.json" \
        DESK_CTRL_SCREENSHOT="$out/$name.png" \
        DESK_CTRL_SCREENSHOT_DELAY="${DELAY:-3}" \
        "$@" "$bin"
    [[ -s "$out/$name.png" ]] || { echo "error: $name.png was not written" >&2; exit 1; }
}

echo "Writing screenshots to $out"
shot dashboard-light "{\"theme\":\"Light\",$desk}"
shot dashboard-dark "{\"theme\":\"Dark\",$desk}"
shot settings "{\"theme\":\"Light\",\"primary_light\":[45,91,138],$desk}" DESK_CTRL_SETTINGS=1
shot onboarding-welcome '{"theme":"Light"}'
shot onboarding-find '{"theme":"Light"}' DESK_CTRL_STEP=find
shot onboarding-connect '{"theme":"Light"}' DESK_CTRL_STEP=connect DESK_CTRL_SELECT="CTRL DESK"
shot onboarding-calibrate '{"theme":"Light"}' DESK_CTRL_STEP=calibrate DESK_CTRL_SELECT="CTRL DESK"
shot onboarding-personalise '{"theme":"Dark"}' DESK_CTRL_STEP=personalise DESK_CTRL_SELECT="CTRL DESK"
# The app icon, rendered by the app itself.
DESK_CTRL_EXPORT_ICONS="$tmp/icons" "$bin"
cp "$tmp/icons/icon-256.png" "$out/icon.png"
echo "  icon.png"
echo "Done."
