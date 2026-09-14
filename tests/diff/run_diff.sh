#!/bin/bash
# Differential test: run every probe under the bytecode VM and under a forced
# tree-walk (ZEN_NO_BC=1) and require identical stdout+exit code.
# Usage: ZEN_BIN=/path/to/zen bash tests/diff/run_diff.sh
set -u
ZEN=${ZEN_BIN:-zen}
DIR=$(cd "$(dirname "$0")" && pwd)
pass=0
fail=0
for f in "$DIR"/*.z; do
  name=$(basename "$f")
  out1=$("$ZEN" "$f" 2>&1)
  rc1=$?
  out2=$(ZEN_NO_BC=1 "$ZEN" "$f" 2>&1)
  rc2=$?
  if [ "$rc1" -eq "$rc2" ] && [ "$out1" == "$out2" ]; then
    pass=$((pass+1))
    echo "SAME   $name (rc=$rc1)"
  else
    fail=$((fail+1))
    echo "DIFF   $name (bc_rc=$rc1 walk_rc=$rc2)"
  fi
done
echo "----"
echo "same: $pass  diff: $fail"
[ "$fail" -eq 0 ]