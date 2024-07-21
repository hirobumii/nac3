#!/usr/bin/env bash

set -e

if [ -z "$1" ]; then
  echo "Requires at least one argument"
  exit 1
fi

declare -a nac3args
while [ $# -gt 1 ]; do
  nac3args+=("$1")
  shift
done
demo="$1"

echo "### Checking $demo..."

# Get reference output
echo ">>>>>> Running $demo with the Python interpreter"
./interpret_demo.py "$demo" > interpreted.log

echo "...... Trying NAC3's 32-bit code generator output"
./run_demo.sh --out run_32.log "${nac3args[@]}" -s 32 "$demo"

echo "...... Trying NAC3's 32-bit code generator output with --lli"
./run_demo.sh --lli --out run_lli_32.log "${nac3args[@]}" -s 32 "$demo"

echo "...... Trying NAC3's 64-bit code generator output"
./run_demo.sh --out run_64.log "${nac3args[@]}" -s 64 "$demo"

echo "...... Trying NAC3's 64-bit code generator output with --lli"
./run_demo.sh --lli --out run_lli_64.log "${nac3args[@]}" -s 64 "$demo"

diff -Nau interpreted.log run_32.log
diff -Nau interpreted.log run_64.log
diff -Nau interpreted.log run_lli_32.log
diff -Nau interpreted.log run_lli_64.log

echo "...... OK"

rm -f interpreted.log \
  run_32.log run_lli_32.log \
  run_64.log run_lli_64.log