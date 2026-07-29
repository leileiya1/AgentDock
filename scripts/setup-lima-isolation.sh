#!/bin/sh
set -eu

INSTANCE=${AGENTDOCK_LIMA_INSTANCE:-agentdock-isolated}
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CONFIG="$ROOT/config/lima/agentdock-isolated.yaml"

if ! command -v limactl >/dev/null 2>&1; then
  echo "limactl is required (tested with Lima 2.2.0)" >&2
  exit 69
fi

installed=$(limactl --version | awk '{print $3}')
if [ "$installed" != "2.2.0" ]; then
  echo "unsupported Lima version: $installed (expected 2.2.0)" >&2
  exit 65
fi

if ! limactl list "$INSTANCE" --quiet 2>/dev/null | grep -Fxq "$INSTANCE"; then
  limactl create --tty=false --name "$INSTANCE" "$CONFIG"
fi
limactl start --tty=false "$INSTANCE"
"$ROOT/scripts/verify-lima-isolation.sh"
