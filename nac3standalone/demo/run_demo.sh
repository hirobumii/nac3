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

rm -f "*.o" demo

$nac3standalone "$@"
clang -c -std=gnu11 -Wall -Wextra -O3 -o demo.o demo.c
clang -o demo module.o demo.o
./demo
