#!/usr/bin/env bash

set -e

if [ -z "$1" ]; then
    echo "No argument supplied"
    exit 1
fi

declare -a nac3args
while [ $# -ge 1 ]; do
  case "$1" in
    --help)
      echo "Usage: run_demo.sh [--help] [--out OUTFILE] [--debug] -- [NAC3ARGS...]"
      exit
      ;;
    --out)
      shift
      outfile="$1"
      ;;
    --debug)
      debug=1
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

while [ $# -ge 1 ]; do
  nac3args+=("$1")
  shift
done

if [ -n "$debug" ] && [ -e ../../target/debug/nac3standalone ]; then
    nac3standalone=../../target/debug/nac3standalone
elif [ -e ../../target/release/nac3standalone ]; then
    nac3standalone=../../target/release/nac3standalone
else
    # used by Nix builds
    nac3standalone=../../target/x86_64-unknown-linux-gnu/release/nac3standalone
fi

rm -f ./*.o ./*.bc demo

$nac3standalone "${nac3args[@]}"

clang -c -std=gnu11 -Wall -Wextra -O3 -o demo.o demo.c
clang -lm -o demo module.o demo.o

if [ -z "$outfile" ]; then
  ./demo
else
  ./demo > "$outfile"
fi
