#!/bin/bash
# Build one framework's Hello World in release, as its own tools package
# it, and measure it on this Mac:
#
#   benchmarks/run.sh <framework> [runs]
#
# Frameworks: bern gpui dioxus tauri electron flutter react-native avalonia qt
#
# Writes benchmarks/out/<framework>.json: the package size, the median
# time from start to the first window on screen, and the median memory
# (physical footprint of the app and the processes it brought up), with
# every run. Toolchains a runner lacks are installed into a temporary
# folder; nothing here touches the system beyond what Homebrew installs
# for Qt.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
framework="$1"
runs="${2:-5}"
work="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/bern-benchmarks/$framework"
rm -rf "$work"
mkdir -p "$work" "$here/out"
swiftc -O "$here/measure.swift" -o "$work/measure"

# What each case sets: the package whose size counts, the program to
# start, and the framework's version.
package=""
executable=""
version=""

rust_app() {
    cp -R "$here/apps/$1" "$work/app"
    if [ -d "$work/icons" ]; then cp -R "$work/icons" "$work/app/icons"; fi
    (cd "$work/app" && cargo build --release --quiet)
    package="$work/app/target/release/hello-$1"
    executable="$package"
}

case "$framework" in
bern)
    (cd "$root" && cargo run --quiet --release -p bern-cli -- build --manifest examples/hello/Bern.toml --target macos --release)
    package="$root/target/bern/macos/Hello.app"
    executable="$package/Contents/MacOS/Hello"
    version="$(cd "$root" && git describe --always --dirty)"
    ;;
gpui)
    rust_app gpui
    version="$(cd "$work/app" && cargo pkgid gpui | sed 's/.*[@#]//')"
    ;;
dioxus)
    rust_app dioxus
    version="$(cd "$work/app" && cargo pkgid dioxus | sed 's/.*[@#]//')"
    ;;
tauri)
    # Tauri embeds a window icon; any PNG does.
    mkdir -p "$work/icons"
    cp "$root/crates/bern-target-api/assets/icon/round-512.png" "$work/icons/icon.png"
    rust_app tauri
    version="$(cd "$work/app" && cargo pkgid tauri | sed 's/.*[@#]//')"
    ;;
electron)
    cp -R "$here/apps/electron" "$work/app"
    (cd "$work/app" && npm install --no-audit --no-fund --silent \
        && npx --no-install electron-packager . Hello --platform=darwin --arch="$(uname -m | sed 's/x86_64/x64/')" --out=out --overwrite --quiet)
    package="$(echo "$work"/app/out/Hello-darwin-*/Hello.app)"
    executable="$package/Contents/MacOS/Hello"
    version="$(node -p "require('$work/app/node_modules/electron/package.json').version")"
    ;;
flutter)
    if ! command -v flutter >/dev/null; then
        git clone --quiet --depth 1 --branch stable https://github.com/flutter/flutter.git "$work/sdk"
        export PATH="$work/sdk/bin:$PATH"
    fi
    flutter config --no-analytics --enable-macos-desktop >/dev/null
    (cd "$work" && flutter create --platforms=macos --project-name hello app >/dev/null)
    cp "$here/apps/flutter/main.dart" "$work/app/lib/main.dart"
    (cd "$work/app" && flutter build macos --release >/dev/null)
    package="$work/app/build/macos/Build/Products/Release/hello.app"
    executable="$package/Contents/MacOS/hello"
    version="$(flutter --version --machine | python3 -c 'import json,sys; print(json.load(sys.stdin)["frameworkVersion"])')"
    ;;
react-native)
    rn=0.83
    (cd "$work" && npx --yes @react-native-community/cli@latest init Hello --version "$rn" --skip-install --skip-git-init --install-pods false >/dev/null)
    cp "$here/apps/react-native/App.tsx" "$work/Hello/App.tsx"
    (cd "$work/Hello" && npm install --no-audit --no-fund --silent \
        && npx --yes react-native-macos-init --version "$rn" >/dev/null \
        && cd macos && pod install --silent \
        && xcodebuild -quiet -workspace Hello.xcworkspace -scheme Hello-macOS -configuration Release \
            -derivedDataPath build CODE_SIGNING_ALLOWED=NO)
    package="$work/Hello/macos/build/Build/Products/Release/Hello.app"
    executable="$package/Contents/MacOS/Hello"
    version="$(node -p "require('$work/Hello/node_modules/react-native-macos/package.json').version")"
    ;;
avalonia)
    cp -R "$here/apps/avalonia" "$work/app"
    rid="osx-$(uname -m | sed 's/x86_64/x64/')"
    (cd "$work/app" && dotnet publish -c Release -r "$rid" --self-contained true -o out --nologo -v quiet)
    package="$work/app/out"
    executable="$package/Hello"
    version="$(sed -n 's/.*Include="Avalonia" Version="\([^"]*\)".*/\1/p' "$work/app/Hello.csproj")"
    ;;
qt)
    brew list qt >/dev/null 2>&1 || brew install --quiet qt
    qt="$(brew --prefix qt)"
    cp -R "$here/apps/qt" "$work/app"
    (cd "$work/app" && cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="$qt" >/dev/null \
        && cmake --build build --parallel >/dev/null \
        && "$qt/bin/macdeployqt" build/hello.app)
    package="$work/app/build/hello.app"
    executable="$package/Contents/MacOS/hello"
    version="$("$qt/bin/qmake" -query QT_VERSION)"
    ;;
*)
    echo "unknown framework: $framework" >&2
    exit 2
    ;;
esac

size_kb="$(du -sk "$package" | cut -f1)"
measured="$("$work/measure" "$executable" "$runs")"
python3 - "$framework" "$version" "$size_kb" "$measured" >"$here/out/$framework.json" <<'PY'
import json, platform, statistics, subprocess, sys

framework, version, size_kb, measured = sys.argv[1], sys.argv[2], int(sys.argv[3]), json.loads(sys.argv[4])
chip = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True).stdout.strip()
memory = subprocess.run(["sysctl", "-n", "hw.memsize"], capture_output=True, text=True).stdout.strip()
print(json.dumps({
    "framework": framework,
    "version": version,
    "package_bytes": size_kb * 1024,
    "launch_s": statistics.median(measured["launch_s"]),
    "memory_bytes": statistics.median(measured["memory_bytes"]),
    "launch_runs_s": measured["launch_s"],
    "memory_runs_bytes": measured["memory_bytes"],
    "machine": chip,
    "machine_memory_bytes": int(memory or 0),
    "macos": platform.mac_ver()[0],
}, indent=2))
PY
cat "$here/out/$framework.json"
