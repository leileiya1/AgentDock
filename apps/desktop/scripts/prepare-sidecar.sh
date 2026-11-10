#!/usr/bin/env bash
set -euo pipefail

desktop_dir="$(cd "$(dirname "$0")/.." && pwd)"
workspace_dir="$(cd "$desktop_dir/../.." && pwd)"
target_triple="$(rustc -vV | sed -n 's/^host: //p')"

sidecar_profile="${AGENTFLOW_SIDECAR_PROFILE:-release}"
case "$sidecar_profile" in
  release) profile_args=(--release) ;;
  debug) profile_args=(--profile dev) ;;
  *) echo "AGENTFLOW_SIDECAR_PROFILE must be debug or release" >&2; exit 2 ;;
esac
cargo build --manifest-path "$workspace_dir/Cargo.toml" "${profile_args[@]}" \
  -p agentflow-daemon --bin agentflowd
mkdir -p "$desktop_dir/src-tauri/binaries"
cp "$workspace_dir/target/$sidecar_profile/agentflowd" \
  "$desktop_dir/src-tauri/binaries/agentflowd-$target_triple"
chmod 755 "$desktop_dir/src-tauri/binaries/agentflowd-$target_triple"
