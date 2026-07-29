#!/usr/bin/env bash
#
# Release gate for the macOS desktop bundle.
# Enforces the three checks the distribution must survive on any Mac, not just
# the build machine (see AgentDock_前端实时操作问题清单_2026-07-22.md P0-03):
#   1. codesign --verify --deep --strict   (structural signature integrity)
#   2. codesign -d / spctl --assess        (Developer ID + notarization / Gatekeeper)
#   3. hdiutil verify + SHA-256            (DMG integrity)
#
# The actual pass/fail logic lives in the tested `verify-macos-bundle` binary
# (crates/release-engine), so this wrapper only locates the built artifacts.
#
# USAGE
#   scripts/verify-macos-bundle.sh                 # auto-locate under the tauri bundle dir
#   scripts/verify-macos-bundle.sh <app> <dmg> [expected-dmg-sha256]
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "verify-macos-bundle 只能在 macOS 上运行（需要 codesign/spctl/hdiutil）。" >&2
  exit 2
fi

APP="${1:-}"
DMG="${2:-}"
EXPECTED_SHA="${3:-}"

BUNDLE_DIR="apps/desktop/src-tauri/target/release/bundle"

if [[ -z "$APP" ]]; then
  APP="$(find "$BUNDLE_DIR/macos" -maxdepth 1 -name '*.app' -print -quit 2>/dev/null || true)"
fi
if [[ -z "$DMG" ]]; then
  DMG="$(find "$BUNDLE_DIR/dmg" -maxdepth 1 -name '*.dmg' -print -quit 2>/dev/null || true)"
fi

if [[ -z "$APP" || ! -d "$APP" ]]; then
  echo "找不到 .app：请先运行 'bun run app:build'，或显式传入路径。" >&2
  exit 2
fi
if [[ -z "$DMG" || ! -f "$DMG" ]]; then
  echo "找不到 .dmg：请先运行 'bun run app:build'，或显式传入路径。" >&2
  exit 2
fi

echo "校验 App：$APP"
echo "校验 DMG：$DMG"

# Run directly (no pipe) so the gate's exit code is never masked.
if [[ -n "$EXPECTED_SHA" ]]; then
  cargo run --quiet -p agentflow-release-engine --bin verify-macos-bundle -- "$APP" "$DMG" "$EXPECTED_SHA"
else
  cargo run --quiet -p agentflow-release-engine --bin verify-macos-bundle -- "$APP" "$DMG"
fi
