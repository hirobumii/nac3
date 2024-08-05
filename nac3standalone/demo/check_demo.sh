#!/usr/bin/env bash

set -e

if [ -z "$1" ]; then
    echo "No argument supplied"
    exit 1
fi

declare -a nac3args
while [ $# -ge 2 ]; do
  case "$1" in
    --help)
      echo "Usage: check_demo.sh [-i686] -- demo [NAC3ARGS...]"
      exit
      ;;
    -i686)
      i686=1
      ;;
    --)
      shift
      break
      ;;
    *)
      break
      ;;
  esac
  shift
done

demo="$1"
shift
while [ $# -gt 1 ]; do
  nac3args+=("$1")
  shift
done


echo "### Checking $demo..."

echo ">>>>>> Running $demo with the Python interpreter"
./interpret_demo.py "$demo" > interpreted.log

if [ -n "$i686" ]; then
  echo "...... Trying NAC3's 32-bit code generator output"
  ./run_demo.sh -i686 --out run_32.log "${nac3args[@]}" "$demo"
  diff -Nau interpreted.log run_32.log
fi

echo "...... Trying NAC3's 64-bit code generator output"
./run_demo.sh --out run_64.log "${nac3args[@]}" "$demo"
diff -Nau interpreted.log run_64.log

echo "...... OK"

rm -f interpreted.log \
  run_32.log run_64.log
