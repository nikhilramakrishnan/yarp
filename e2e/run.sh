#!/usr/bin/env bash
# yarp end-to-end matrix: drives the real binary on a pty through every
# supported shell present on this machine and asserts blocks are recorded
# with the right command labels and exit codes.
#
#   cd go && ./e2e/run.sh
set -u

here="$(cd "$(dirname "$0")" && pwd)"
root="$(dirname "$here")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "building yarp..."
(cd "$root" && CGO_ENABLED=0 go build -o "$work/yarp" ./cmd/yarp) || exit 1

pass=0 fail=0 skip=0

check() { # shell marker-cmd fail-cmd
  local shell="$1" marker="$2" failcmd="$3"
  local name; name="$(basename "$shell")"
  if [ ! -x "$shell" ]; then
    echo "SKIP  $name (not installed)"
    skip=$((skip + 1))
    return
  fi
  local home="$work/home-$name"
  YARP_HOME="$home" timeout 120 python3 "$here/driver.py" "$work/yarp" "$shell" \
    "$marker" "$failcmd" "exit" >"$work/$name.out" 2>&1
  local blocks="$home"/history/*.jsonl
  if ! grep -q "\"cmd\":\"$marker\"" $blocks 2>/dev/null; then
    echo "FAIL  $name: marker command not recorded"
    fail=$((fail + 1))
    return
  fi
  if ! grep "\"cmd\":\"$failcmd\"" $blocks 2>/dev/null | grep -q '"exit_code":[1-9]'; then
    echo "FAIL  $name: failing command's nonzero exit not recorded"
    fail=$((fail + 1))
    return
  fi
  echo "PASS  $name"
  pass=$((pass + 1))
}

check /bin/bash      "echo yarp-e2e-marker" "false"
check /usr/bin/zsh   "echo yarp-e2e-marker" "false"
check /usr/bin/fish  "echo yarp-e2e-marker" "false"
check /usr/bin/pwsh  "echo yarp-e2e-marker" "Get-ChildItem yarp-missing-dir"

echo "e2e: $pass passed, $fail failed, $skip skipped"
[ "$fail" -eq 0 ]
