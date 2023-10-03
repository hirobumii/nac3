#!/usr/bin/env bash

set -e

if [ -z "$1" ]; then
    echo "No argument supplied"
    exit 1
fi

declare -a nac3args
while [ $# -ge 1 ]; do
  case "$1" in
    --out)
      shift
      outfile="$1"
      ;;
    --lli)
      use_lli=1
      ;;
    *)
      nac3args+=("$1")
      ;;
  esac
  shift
done

if [ -e ../../target/release/nac3standalone ]; then
    nac3standalone=../../target/release/nac3standalone
else
    # used by Nix builds
    nac3standalone=../../target/x86_64-unknown-linux-gnu/release/nac3standalone
fi

if [ -z "$use_lli" ]; then
  rm -f "*.o" demo

  $nac3standalone "${nac3args[@]}"

  clang -c -std=gnu11 -Wall -Wextra -O3 -o demo.o demo.c
  clang -lm -o demo module.o demo.o

  if [ -z "$outfile" ]; then
    ./demo
  else
    ./demo > "$outfile"
  fi
else
  rm -f "*.o" "*.bc" demo

  $nac3standalone --emit-llvm "${nac3args[@]}"

  clang -c -std=gnu11 -Wall -Wextra -O3 -emit-llvm -o demo.bc demo.c

  if [ -z "$outfile" ]; then
    lli --extra-module demo.bc --extra-module irrt.bc main.bc
  else
    lli --extra-module demo.bc --extra-module irrt.bc main.bc > "$outfile"
  fi
fi
