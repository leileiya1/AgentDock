#!/usr/bin/env bash
#
# Canonical local verification gate for AgentDock.
# Mirrors .github/workflows/ci.yml so "green locally" means "green in CI".
#
# WHY THIS EXISTS
# ---------------
# A test target that FAILS TO COMPILE runs zero tests. If you invoke cargo
# through a pipe — e.g. `cargo test --workspace | tail` — the shell reports the
# pipe's LAST command's exit code (tail's 0), not cargo's failure. A broken test
# build then looks green, and the whole suite silently stops protecting you.
#
# This script defends the foundation two ways:
#   1. `set -euo pipefail` — any stage that fails aborts the run immediately,
#      and a failure anywhere in a pipeline is NOT swallowed.
#   2. Every gate command runs DIRECTLY (never piped into a filter), so its exit
#      code can never be masked.
#
# USAGE
#   scripts/verify.sh          # fmt + clippy + full workspace tests + FE typecheck
#   scripts/verify.sh --full   # also regenerate TS bindings and fail on drift
#
set -euo pipefail

# Always operate from the repo root regardless of where the caller invoked us.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FULL=0
for arg in "$@"; do
  case "$arg" in
    --full) FULL=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

step() {
  # Print a banner, then run the command directly. No pipe → no masked exit code.
  printf '\n\033[1;34m▶ %s\033[0m\n' "$1"
  shift
  "$@"
}

# 1. Formatting — same check CI enforces.
step "cargo fmt --all --check" \
  cargo fmt --all --check

# 2. Lint + COMPILE every target including tests/benches. `-D warnings` makes a
#    warning fatal; --all-targets is what turns "a test file won't build" into a
#    hard failure here rather than a surprise in CI.
step "cargo clippy --workspace --all-targets -- -D warnings" \
  cargo clippy --workspace --all-targets -- -D warnings

# 3. Actually RUN the workspace tests. --no-fail-fast reports every failing
#    crate in one pass instead of stopping at the first.
step "cargo test --workspace --no-fail-fast" \
  cargo test --workspace --no-fail-fast

# 4. Frontend type safety (exhaustive BLOCKED_COPY, generated bindings, etc.).
step "bun run typecheck (apps/desktop)" \
  bash -c 'cd apps/desktop && bun run typecheck'

# 5. Frontend unit tests (bun:test + renderToStaticMarkup snapshots).
step "bun test (apps/desktop)" \
  bash -c 'cd apps/desktop && bun test'

# 6. Contract/bindings drift — heavier (compiles the Tauri backend), opt-in.
if [ "$FULL" -eq 1 ]; then
  step "bun run prepare:sidecar (apps/desktop)" \
    bash -c 'cd apps/desktop && bun run prepare:sidecar'
  step "cargo run -p xtask (regenerate bindings + schemas)" \
    cargo run -p xtask
  step "git diff --exit-code (bindings/schema drift guard)" \
    git diff --exit-code
fi

printf '\n\033[1;32m✓ all verification gates passed\033[0m\n'
