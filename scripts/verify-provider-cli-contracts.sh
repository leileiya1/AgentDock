#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
matrix="$repo_root/config/provider-compatibility.json"
allow_unverified=false
if [[ "${1:-}" == "--allow-unverified" ]]; then
  allow_unverified=true
fi

verify_cli() {
  local name="$1"
  local command_name="$2"
  local executable
  if [[ "$name" == "codex" && -n "${AGENTFLOW_CODEX_BIN:-}" ]]; then
    executable="$AGENTFLOW_CODEX_BIN"
  elif [[ "$name" == "claude" && -n "${AGENTFLOW_CLAUDE_BIN:-}" ]]; then
    executable="$AGENTFLOW_CLAUDE_BIN"
  elif [[ "$name" == "qodercli" && -n "${AGENTFLOW_QODER_BIN:-}" ]]; then
    executable="$AGENTFLOW_QODER_BIN"
  elif [[ "$name" == "grok" && -n "${AGENTFLOW_GROK_BIN:-}" ]]; then
    executable="$AGENTFLOW_GROK_BIN"
  elif [[ "$name" == "codex" && -x "/Applications/ChatGPT.app/Contents/Resources/codex" ]]; then
    # Mirrors AgentFlow's macOS resolver: the ChatGPT-bundled binary is the cache-compatible
    # candidate and can differ from an older standalone `codex` earlier on PATH.
    executable="/Applications/ChatGPT.app/Contents/Resources/codex"
  else
    executable="$(command -v "$command_name")"
  fi
  local version_output
  version_output="$("$executable" --version 2>&1)"
  local help_output
  if [[ "$name" == "codex" ]]; then
    help_output="$("$executable" exec --help 2>&1)"
  else
    help_output="$("$executable" --help 2>&1)"
  fi
  MATRIX_PATH="$matrix" PROVIDER_NAME="$name" VERSION_OUTPUT="$version_output" \
    HELP_OUTPUT="$help_output" ALLOW_UNVERIFIED="$allow_unverified" node <<'NODE'
const fs = require("fs");
const matrix = JSON.parse(fs.readFileSync(process.env.MATRIX_PATH, "utf8"));
if (matrix.schemaVersion !== 1) throw new Error("unsupported support matrix schema");
const entry = matrix.providers[process.env.PROVIDER_NAME];
if (!entry) throw new Error(`missing matrix entry: ${process.env.PROVIDER_NAME}`);
const version = (process.env.VERSION_OUTPUT.match(/\b\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?\b/) || [])[0];
if (!version) throw new Error(`cannot parse ${process.env.PROVIDER_NAME} version`);
const missing = entry.requiredFlags.filter((flag) => !process.env.HELP_OUTPUT.includes(flag));
if (missing.length) throw new Error(`${process.env.PROVIDER_NAME} missing flags: ${missing.join(", ")}`);
if (process.env.ALLOW_UNVERIFIED !== "true" && !entry.verifiedVersions.includes(version)) {
  throw new Error(`${process.env.PROVIDER_NAME} ${version} is not a pinned verified version`);
}
console.log(`${entry.displayName} ${version}: CLI contract OK`);
NODE
}

verify_cli claude claude
verify_cli codex codex
verify_cli qodercli qodercli
verify_cli grok grok
