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

echo -n "Checking $demo... "
./interpret_demo.py "$demo" > interpreted.log
./run_demo.sh "${nac3args[@]}" "$demo" > run.log
./run_demo_lli.sh "${nac3args[@]}" "$demo" > run_lli.log
diff -Nau interpreted.log run.log
diff -Nau interpreted.log run_lli.log
echo "ok"

rm -f interpreted.log run.log run_lli.log