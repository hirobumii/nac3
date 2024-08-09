#!/usr/bin/env bash

set -e

if [ -z "$1" ]; then
    echo "No argument supplied"
    exit 1
fi

declare -a nac3args
while [ $# -gt 1 ]; do
  case "$1" in
    --help)
      echo "Usage: check_demo.sh [--debug] [-i686] -- [NAC3ARGS...] demo"
      exit
      ;;
    --debug)
      debug=1
      ;;
    -i686)
      i686=1
      ;;
    --)
      shift
      break
      ;;
    *)
      echo "Unrecognized argument \"$1\""
      exit 1
      ;;
  esac
  shift
done

while [ $# -gt 1 ]; do
  nac3args+=("$1")
  shift
done
demo="$1"


echo "### Checking $demo..."

echo ">>>>>> Running $demo with the Python interpreter"
./interpret_demo.py "$demo" > interpreted.log

if [ -n "$i686" ]; then
  echo "...... Trying NAC3's 32-bit code generator output"
  if [ -n "$debug" ]; then
    ./run_demo.sh --debug -i686 --out run_32.log -- "${nac3args[@]}" "$demo"
  else
    ./run_demo.sh -i686 --out run_32.log -- "${nac3args[@]}" "$demo"
  fi
  diff -Nau interpreted.log run_32.log
fi

echo "...... Trying NAC3's 64-bit code generator output"
if [ -n "$debug" ]; then
  ./run_demo.sh --debug --out run_64.log -- "${nac3args[@]}" "$demo"
else
  ./run_demo.sh --out run_64.log -- "${nac3args[@]}" "$demo"
fi
diff -Nau interpreted.log run_64.log

echo "...... OK"

rm -f interpreted.log \
  run_32.log run_64.log
