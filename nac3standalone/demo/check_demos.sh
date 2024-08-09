#!/usr/bin/env bash

set -e

if [ "$1" == "--help" ]; then
    echo "Usage: check_demos.sh [CHECKARGS...] [--] [NAC3ARGS...]"
    exit
fi

count=0
for demo in src/*.py; do
  ./check_demo.sh "$@" "$demo"
  ((count += 1))
done

echo "Ran $count demo checks - PASSED"
