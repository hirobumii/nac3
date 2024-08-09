#!/usr/bin/env bash

set -e

: "${DEMO_LINALG_STUB:=linalg/target/release/liblinalg.a}"
: "${DEMO_LINALG_STUB32:=linalg/target/i686-unknown-linux-gnu/release/liblinalg.a}"

if [ -z "$1" ]; then
    echo "No argument supplied"
    exit 1
fi

declare -a nac3args
while [ $# -ge 1 ]; do
  case "$1" in
    --help)
      echo "Usage: run_demo.sh [--help] [--out OUTFILE] [--debug] [-i686] -- [NAC3ARGS...] demo"
      exit
      ;;
    --out)
      shift
      outfile="$1"
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

if [ -z "$i686" ]; then
  $nac3standalone "${nac3args[@]}"
  clang -c -std=gnu11 -Wall -Wextra -O3 -o demo.o demo.c
  clang -o demo module.o demo.o $DEMO_LINALG_STUB -lm -Wl,--no-warn-search-mismatch
else
  $nac3standalone --triple i686-unknown-linux-gnu --target-features +sse2 "${nac3args[@]}"
  clang -m32 -c -std=gnu11 -Wall -Wextra -O3 -msse2 -o demo.o demo.c
  clang -m32 -o demo module.o demo.o $DEMO_LINALG_STUB32 -lm -Wl,--no-warn-search-mismatch
fi

if [ -z "$outfile" ]; then
  ./demo
else
  ./demo > "$outfile"
fi
