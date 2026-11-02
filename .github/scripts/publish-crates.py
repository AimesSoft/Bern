#!/usr/bin/env python3
"""Publish every Bern crate to crates.io, each after the Bern crates it needs.

The crates are released together at one version. A crate whose version is
already on crates.io is skipped, so running this again finishes a release
that stopped part way. crates.io lets an account publish a few new crates in
a row and then one every ten minutes; when it answers with that limit, this
waits and tries again.

  publish-crates.py --tag v0.2.0   publish; the tag must name the version
  publish-crates.py --dry-run      package and build the crates, upload nothing
"""

import json
import subprocess
import sys
import time
import urllib.error
import urllib.request

# Outside the workspace: only the applications that use them build them.
# bern-gpui is not published yet.
OUTSIDE = ["crates/bern-gtk/Cargo.toml"]
WAIT = 10 * 60
TRIES = 40


def packages(manifest=None):
    command = ["cargo", "metadata", "--no-deps", "--format-version", "1"]
    if manifest:
        command += ["--manifest-path", manifest]
    return json.loads(subprocess.check_output(command))["packages"]


def crates():
    """The crates to publish: name -> (package, manifest outside the workspace)."""
    found = {p["name"]: (p, None) for p in packages() if p["publish"] != []}
    for manifest in OUTSIDE:
        for p in packages(manifest):
            found[p["name"]] = (p, manifest)
    return found


def order(found):
    """Every crate after the Bern crates it depends on, dev-dependencies
    included: crates.io takes a crate only when all of them are there.
    Dependencies without a version (a crate on itself) are dropped when
    packaging."""
    ordered = []

    def visit(name, path):
        if name in ordered:
            return
        if name in path:
            sys.exit("dependency cycle: " + " -> ".join(path + [name]))
        for dependency in found[name][0]["dependencies"]:
            other = dependency["name"]
            if other in found and other != name and dependency["req"] != "*":
                visit(other, path + [name])
        ordered.append(name)

    for name in sorted(found):
        visit(name, [])
    return ordered


def published(name, version):
    """Whether crates.io has `name` at `version`, read from its index."""
    lower = name.lower()
    if len(lower) <= 2:
        prefix = str(len(lower))
    elif len(lower) == 3:
        prefix = f"3/{lower[0]}"
    else:
        prefix = f"{lower[:2]}/{lower[2:4]}"
    request = urllib.request.Request(
        f"https://index.crates.io/{prefix}/{lower}",
        headers={"User-Agent": "Bern release (https://github.com/AimesSoft/Bern)"},
    )
    try:
        with urllib.request.urlopen(request) as response:
            lines = response.read().decode().splitlines()
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise
    return any(json.loads(line)["vers"] == version for line in lines)


def publish(name, manifest):
    command = ["cargo", "publish", "--locked"]
    command += ["--manifest-path", manifest] if manifest else ["-p", name]
    for _ in range(TRIES):
        result = subprocess.run(command, stderr=subprocess.PIPE, text=True)
        sys.stderr.write(result.stderr)
        if result.returncode == 0:
            return
        if "429" not in result.stderr:
            sys.exit(f"publishing {name} failed")
        print(f"crates.io asks to wait: {name} again in ten minutes", flush=True)
        time.sleep(WAIT)
    sys.exit(f"crates.io kept refusing {name}")


def main():
    arguments = sys.argv[1:]
    found = crates()
    ordered = order(found)
    versions = {found[name][0]["version"] for name in ordered}
    if len(versions) != 1:
        sys.exit(f"the crates are at different versions: {sorted(versions)}")
    version = versions.pop()
    print(f"Bern {version}: {' '.join(ordered)}", flush=True)

    if arguments == ["--dry-run"]:
        inside = [name for name in ordered if found[name][1] is None]
        command = ["cargo", "package", "--locked"]
        for name in inside:
            command += ["-p", name]
        subprocess.run(command, check=True)
        # Built only once the crates they need are on crates.io.
        for name in ordered:
            if found[name][1]:
                subprocess.run(
                    ["cargo", "package", "--list", "--manifest-path", found[name][1]],
                    check=True,
                    stdout=subprocess.DEVNULL,
                )
        return

    if len(arguments) != 2 or arguments[0] != "--tag":
        sys.exit(__doc__)
    if arguments[1] != f"v{version}":
        sys.exit(f"the tag {arguments[1]} is not the crates' version {version}")
    for name in ordered:
        if published(name, version):
            print(f"{name} {version} is already on crates.io", flush=True)
            continue
        publish(name, found[name][1])


if __name__ == "__main__":
    main()
