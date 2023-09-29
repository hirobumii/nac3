#!/usr/bin/env bash

set -e

count=0
for demo in src/*.py; do
  ./check_demo.sh "$@" "$demo"
  ((count += 1))
done

echo "Ran $count demo checks - PASSED"
