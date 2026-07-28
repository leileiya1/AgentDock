#!/bin/sh
set -eu

INSTANCE=${AGENTDOCK_LIMA_INSTANCE:-agentdock-isolated}
FORBIDDEN_ROOT=${AGENTDOCK_E2E_ROOT:-/Users/sapiece/innovationProject/test}

if ! command -v limactl >/dev/null 2>&1; then
  echo "FAIL: limactl is not installed" >&2
  exit 69
fi
status=$(limactl list "$INSTANCE" --format '{{.Status}}' 2>/dev/null || true)
if [ "$status" != "Running" ]; then
  echo "FAIL: Lima instance $INSTANCE is not running" >&2
  exit 69
fi

limactl shell "$INSTANCE" sh -eu -c '
  test -x /usr/local/sbin/agentflow-offline
  test "$(stat -c "%U:%G:%a" /usr/local/sbin/agentflow-offline)" = "root:root:755"
  test ! -e "$1"
  tmp=$(mktemp -d)
  trap '\''rm -rf "$tmp"'\'' EXIT
  printf isolated >"$tmp/probe"
  test "$(cat "$tmp/probe")" = isolated
' agentflow-isolation-probe "$FORBIDDEN_ROOT"

limactl shell "$INSTANCE" sudo -n /usr/local/sbin/agentflow-offline -- /bin/sh -eu -c '
  command -v curl >/dev/null
  command -v setpriv >/dev/null
  test -z "$(/usr/sbin/ip -4 route show default)"
  test -z "$(/usr/sbin/ip -6 route show default)"
  ! curl -4 -fsS --connect-timeout 3 https://example.com >/dev/null 2>&1
  ! curl -6 -fsS --connect-timeout 3 https://example.com >/dev/null 2>&1
  ! sudo -n true >/dev/null 2>&1
  printf AGENTFLOW_FAIL_CLOSED_OK
' | grep -q AGENTFLOW_FAIL_CLOSED_OK

echo "PASS: Lima has no host project mount; protected commands have no IPv4/IPv6 route and cannot regain sudo."
