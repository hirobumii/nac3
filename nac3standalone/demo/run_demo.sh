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
      echo "Usage: run_demo.sh [--help] [--out OUTFILE] [--debug] [-i386] -- [NAC3ARGS...]"
      exit
      ;;
    --out)
      shift
      outfile="$1"
      ;;
    --debug)
      debug=1
      ;;
    -i386)
      i386=1
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

if [ -z "$i386" ]; then
  $nac3standalone "${nac3args[@]}"

  clang -c -std=gnu11 -Wall -Wextra -O3 -o demo.o demo.c
  clang -lm -Wl,--no-warn-search-mismatch -o demo module.o demo.o
else
   # Enable SSE2 to avoid rounding errors with X87's 80-bit fp precision computations

   $nac3standalone --triple i386-pc-linux-gnu --target-features +sse2 "${nac3args[@]}"

   clang -m32 -c -std=gnu11 -Wall -Wextra -O3 -msse2 -o demo.o demo.c
   clang -m32 -lm -Wl,--no-warn-search-mismatch -o demo module.o demo.o
fi

if [ -z "$outfile" ]; then
  ./demo
else
  ./demo > "$outfile"
fi
