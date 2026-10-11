#!/bin/bash
# Build and measure Hello Worlds on this Mac:
#
#   benchmarks/run.sh [--runs N] <framework>...
#
# Writes benchmarks/out/results.json. See build.sh for the frameworks.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
runs=10
if [ "${1:-}" = "--runs" ]; then
    runs="$2"
    shift 2
fi
rm -rf "$here/out"
for framework in "$@"; do
    "$here/build.sh" "$framework" "$here/out"
done
"$here/measure.sh" "$here/out" "$runs"
