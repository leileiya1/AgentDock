#!/usr/bin/env bash
set -euo pipefail

desktop_dir="$(cd "$(dirname "$0")/.." && pwd)"
workspace_dir="$(cd "$desktop_dir/../.." && pwd)"
target_triple="$(rustc -vV | sed -n 's/^host: //p')"

cargo build --manifest-path "$workspace_dir/Cargo.toml" --release \
  -p agentflow-daemon --bin agentflowd
mkdir -p "$desktop_dir/src-tauri/binaries"
cp "$workspace_dir/target/release/agentflowd" \
  "$desktop_dir/src-tauri/binaries/agentflowd-$target_triple"
chmod 755 "$desktop_dir/src-tauri/binaries/agentflowd-$target_triple"
