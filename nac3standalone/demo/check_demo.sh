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

# A demo starting with `# EXPECT:` lines are expected-fail-by-raise tests.
# Since the standalone runtime reports a NAC3 exception through `__nac3_raise`
# instead of unwinding, these tests are regression tests that compare with the
# expected output embedded in the demo itself, similar to `insta` snapshot tests in Rust.
expected=""
if grep -q '^# EXPECT:' "$demo"; then
  expected=expected.log
  # Preserving the indentation of the expected output by stripping the leading `# `.
  sed -n -E 's/^# EXPECT:( |$)//p' "$demo" > "$expected"
fi

# Strip the line:column from `Location:` lines in the log, replacing them with `LINE:COL`
# to improve the stability of the diff.
normalize_log() {
  sed -E -i 's|^(    Location: [^:]*):[0-9]+:[0-9]+$|\1:LINE:COL|' "$1"
}

# Runs a demo that is expected to raise, failing the check if it exits successfully.
run_failing_demo() {
  local outfile="$1"
  shift

  local status=0
  ./run_demo.sh --out "$outfile" "$@" || status=$?
  if [ "$status" -eq 0 ]; then
    echo "!!!!!! $demo exited successfully, but was expected to raise"
    exit 1
  fi

  normalize_log "$outfile"
}

declare -a runopts
if [ -n "$debug" ]; then
  runopts+=(--debug)
fi

if [ -n "$expected" ]; then
  echo ">>>>>> $demo is expected to raise; comparing against its embedded EXPECT block"
else
  echo ">>>>>> Running $demo with the Python interpreter"
  ./interpret_demo.py "$demo" > interpreted.log
fi

if [ -n "$i686" ]; then
  echo "...... Trying NAC3's 32-bit code generator output"
  if [ -n "$expected" ]; then
    run_failing_demo run_32.log "${runopts[@]}" -i686 -- "${nac3args[@]}" "$demo"
    diff -Nau "$expected" run_32.log
  else
    ./run_demo.sh "${runopts[@]}" -i686 --out run_32.log -- "${nac3args[@]}" "$demo"
    diff -Nau interpreted.log run_32.log
  fi
fi

echo "...... Trying NAC3's 64-bit code generator output"
if [ -n "$expected" ]; then
  run_failing_demo run_64.log "${runopts[@]}" -- "${nac3args[@]}" "$demo"
  diff -Nau "$expected" run_64.log
else
  ./run_demo.sh "${runopts[@]}" --out run_64.log -- "${nac3args[@]}" "$demo"
  diff -Nau interpreted.log run_64.log
fi

echo "...... OK"

rm -f interpreted.log expected.log \
  run_32.log run_64.log
