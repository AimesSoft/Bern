#!/bin/bash
# Measure Hello Worlds that build.sh packed, on this Mac and in one
# session, so that every framework meets the same machine:
#
#   benchmarks/measure.sh <dir> [runs]
#
# <dir> holds what build.sh wrote. Writes <dir>/results.json: for each
# framework the package size, the median time from start to the first
# window on screen and the median memory, with every run and the machine.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
dir="$(cd "$1" && pwd)"
runs="${2:-10}"
apps="$dir/apps"
rm -rf "$apps"
mkdir -p "$apps"
swiftc -O "$here/measure.swift" -o "$dir/measure"

builds=()
executables=()
for build in "$dir"/*.build.json; do
    framework="$(basename "$build" .build.json)"
    mkdir -p "$apps/$framework"
    tar -xzf "$dir/$framework.tar.gz" -C "$apps/$framework"
    builds+=("$build")
    executables+=("$apps/$framework/$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["executable"])' "$build")")
done

"$dir/measure" "$runs" "${executables[@]}" >"$dir/measured.json"
python3 - "$dir" "${builds[@]}" <<'PY'
import json, platform, statistics, subprocess, sys

directory, builds = sys.argv[1], sys.argv[2:]
measured = json.load(open(f"{directory}/measured.json"))
chip = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True).stdout.strip()
memory = subprocess.run(["sysctl", "-n", "hw.memsize"], capture_output=True, text=True).stdout.strip()
results = []
for build, runs in zip(builds, measured):
    info = json.load(open(build))
    results.append({
        "framework": info["framework"],
        "version": info["version"],
        "package_bytes": info["package_bytes"],
        "launch_s": statistics.median(runs["launch_s"]),
        "memory_bytes": statistics.median(runs["memory_bytes"]),
        "launch_runs_s": runs["launch_s"],
        "memory_runs_bytes": runs["memory_bytes"],
        "machine": chip,
        "machine_memory_bytes": int(memory or 0),
        "macos": platform.mac_ver()[0],
    })
results.sort(key=lambda result: result["framework"])
json.dump({"results": results}, open(f"{directory}/results.json", "w"), indent=2)
for r in results:
    print(f'{r["framework"]:<13} {r["version"]:<16} {r["package_bytes"] / 1e6:7.1f} MB '
          f'{r["launch_s"]:6.2f} s {r["memory_bytes"] / 1e6:6.0f} MB  {r["machine"]}')
PY
