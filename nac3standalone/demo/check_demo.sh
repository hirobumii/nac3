#!/usr/bin/env bash

set -e

if [ -z "$1" ]; then
  echo "Requires at least one argument"
  exit 1
fi

demo="${@:-1}"
set -- "${@:1:$(($# - 1))}"

echo -n "Checking $demo... "
./interpret_demo.py "$demo" > interpreted.log
./run_demo.sh "$@" "$demo" > run.log
diff -Nau interpreted.log run.log
echo "ok"
