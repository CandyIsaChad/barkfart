#!/usr/bin/env bash
# Runs the desktop app under strace and fails if it opens any IP socket
# connection. Usage: tools/netcheck.sh [seconds]   (needs strace, a display)
set -euo pipefail
cd "$(dirname "$0")/.."
secs="${1:-20}"
./gradlew -q desktop:installDist
log="$(mktemp)"
timeout "$secs" strace -f -qq -e trace=connect,sendto,sendmsg -o "$log" \
  desktop/build/install/desktop/bin/desktop >/dev/null 2>&1 || true
total=$(grep -c . "$log" || true)
ip=$(grep -E 'AF_INET6?' "$log" || true)
echo "syscalls traced: $total"
if [ -n "$ip" ]; then
  echo "FAIL: IP network activity:"; echo "$ip"; exit 1
fi
echo "PASS: no AF_INET/AF_INET6 connections in ${secs}s"
grep -oE 'sun_path="[^"]*"|AF_NETLINK' "$log" | sort | uniq -c || true
