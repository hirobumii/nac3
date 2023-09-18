#!/usr/bin/env bash

set -e

if [ -z "$1" ]; then
    echo "No argument supplied"
    exit 1
fi

if [ -e ../../target/release/nac3standalone ]; then
    nac3standalone=../../target/release/nac3standalone
else
    # used by Nix builds
    nac3standalone=../../target/x86_64-unknown-linux-gnu/release/nac3standalone
fi

rm -f *.o *.bc
$nac3standalone --emit-llvm "$@"
gcc -c -std=c11 -Wall -Wextra -pedantic-errors -Werror=pedantic -O3 -o demo.o demo.c
clang -S -Wall -Wextra -O3 -emit-llvm -o irrt.bc ../../nac3core/src/codegen/irrt/irrt.c
lli --extra-object demo.o --extra-module irrt.bc main.bc
