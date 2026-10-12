//! `bern doctor`: whether this computer has what each target needs to
//! build, and what to install when it does not.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The oldest Rust that builds Bern.
const RUST: (u32, u32) = (1, 99);
/// The oldest macOS SDK the SwiftUI target compiles against (Xcode 26).
const MACOS_SDK: (u32, u32) = (26, 0);
const GTK: (u32, u32) = (4, 14);
const LIBADWAITA: (u32, u32) = (1, 5);
const WASM_TARGET: &str = "wasm32-unknown-unknown";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Ready(String),
    /// What is missing, and how to get it.
    Missing(String, String),
    /// The target builds on another operating system.
    Elsewhere(&'static str),
}

pub struct Check {
    pub title: &'static str,
    /// Platforms of the targets this check covers, as `Bern.toml` names
    /// them; empty for the checks every target needs.
    pub platforms: &'static [&'static str],
    pub status: Status,
}

/// What is checked: title, platforms, the operating system that builds
/// them (`None` for any), and the check.
type Spec = (
    &'static str,
    &'static [&'static str],
    Option<(&'static str, &'static str)>,
    fn() -> Status,
);

const SPECS: &[Spec] = &[
    ("Rust", &[], None, rust),
    (
        "macOS · SwiftUI",
        &["macos"],
        Some(("macos", "macOS")),
        macos,
    ),
    ("iOS · SwiftUI", &["ios"], Some(("macos", "macOS")), ios),
    (
        "Windows · WinUI 3",
        &["windows"],
        Some(("windows", "Windows")),
        windows,
    ),
    ("Linux · GTK 4", &["linux"], Some(("linux", "Linux")), linux),
    ("Web · WebAssembly", &["browser"], None, web),
];

fn run(spec: &Spec) -> Check {
    let (title, platforms, host, check) = *spec;
    let status = match host {
        Some((os, name)) if os != std::env::consts::OS => Status::Elsewhere(name),
        _ => check(),
    };
    Check {
        title,
        platforms,
        status,
    }
}

/// Every check, for this computer.
pub fn checks() -> Vec<Check> {
    SPECS.iter().map(run).collect()
}

/// The check for targets on `platform`, if Bern knows the platform.
pub fn check(platform: &str) -> Option<Check> {
    SPECS
        .iter()
        .find(|(_, platforms, _, _)| platforms.contains(&platform))
        .map(run)
}

pub fn print(checks: &[Check]) {
    for check in checks {
        match &check.status {
            Status::Ready(detail) => {
                eprintln!("\x1b[1;32m[✓]\x1b[0m {}: {detail}", check.title)
            }
            Status::Missing(problem, fix) => {
                eprintln!("\x1b[1;31m[✗]\x1b[0m {}: {problem}", check.title);
                for line in fix.lines() {
                    eprintln!("      {line}");
                }
            }
            Status::Elsewhere(os) => {
                eprintln!("\x1b[2m[-] {}: builds on {os}\x1b[0m", check.title)
            }
        }
    }
}

/// Run `program` and return its standard output, if it succeeds.
fn output(program: impl AsRef<std::ffi::OsStr>, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// The first `major.minor` in `text`.
fn version(text: &str) -> Option<(u32, u32)> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find_map(|word| {
            let mut parts = word.split('.');
            let major = parts.next()?.parse().ok()?;
            let minor = parts.next()?.parse().ok()?;
            Some((major, minor))
        })
}

fn rust() -> Status {
    let Some(rustc) = output("rustc", &["--version"]) else {
        return Status::Missing(
            "rustc is not installed".into(),
            "install Rust from https://rustup.rs".into(),
        );
    };
    let Some(cargo) = output("cargo", &["--version"]) else {
        return Status::Missing(
            "cargo is not installed".into(),
            "install Rust from https://rustup.rs".into(),
        );
    };
    let found = version(&rustc).unwrap_or((0, 0));
    if found < RUST {
        return Status::Missing(
            format!("{rustc}; Bern needs Rust {}.{}", RUST.0, RUST.1),
            "rustup update".into(),
        );
    }
    Status::Ready(format!("{rustc}, {cargo}"))
}

fn macos() -> Status {
    let fix = "install Xcode 26 or newer from the App Store, then run\nsudo xcode-select --switch /Applications/Xcode.app";
    let Some(sdk) = output("xcrun", &["--sdk", "macosx", "--show-sdk-version"]) else {
        return Status::Missing("no macOS SDK".into(), fix.into());
    };
    if version(&sdk).is_none_or(|found| found < MACOS_SDK) {
        return Status::Missing(
            format!(
                "macOS SDK {sdk}; SwiftUI needs SDK {}.{} or newer",
                MACOS_SDK.0, MACOS_SDK.1
            ),
            fix.into(),
        );
    }
    let Some(swift) = output("xcrun", &["swiftc", "--version"]) else {
        return Status::Missing("no Swift compiler".into(), fix.into());
    };
    let swift = swift
        .lines()
        .find_map(|line| {
            let at = line.find("Swift version ")?;
            line[at + "Swift version ".len()..]
                .split_whitespace()
                .next()
        })
        .unwrap_or("found");
    Status::Ready(format!("macOS SDK {sdk}, Swift {swift}"))
}

/// The iOS simulator SDK and the Rust target for the simulator. A
/// simulator to run in is `bern run`'s concern, not the build's.
fn ios() -> Status {
    let fix = "install Xcode 26 or newer from the App Store with the iOS platform, then run\nsudo xcode-select --switch /Applications/Xcode.app";
    let Some(sdk) = output("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-version"]) else {
        return Status::Missing("no iOS simulator SDK".into(), fix.into());
    };
    if version(&sdk).is_none_or(|found| found < MACOS_SDK) {
        return Status::Missing(
            format!(
                "iOS SDK {sdk}; SwiftUI needs SDK {}.{} or newer",
                MACOS_SDK.0, MACOS_SDK.1
            ),
            fix.into(),
        );
    }
    let simulator = if std::env::consts::ARCH == "x86_64" {
        "x86_64-apple-ios"
    } else {
        "aarch64-apple-ios-sim"
    };
    let Some(sysroot) = output("rustc", &["--print", "sysroot"]) else {
        return Status::Missing(
            "rustc is not installed".into(),
            "install Rust from https://rustup.rs".into(),
        );
    };
    if !Path::new(&sysroot)
        .join("lib/rustlib")
        .join(simulator)
        .is_dir()
    {
        return Status::Missing(
            format!("the {simulator} target is not installed"),
            format!("rustup target add {simulator}"),
        );
    }
    Status::Ready(format!("iOS simulator SDK {sdk}, the {simulator} target"))
}

fn windows() -> Status {
    let program_files = std::env::var_os("ProgramFiles(x86)")
        .map_or_else(|| PathBuf::from(r"C:\Program Files (x86)"), PathBuf::from);
    let tools = "Visual Studio 2022 or newer with \"Desktop development with C++\"\n(the Build Tools are enough): https://visualstudio.microsoft.com/downloads/";
    let vswhere = program_files.join(r"Microsoft Visual Studio\Installer\vswhere.exe");
    let component = if std::env::consts::ARCH == "aarch64" {
        "Microsoft.VisualStudio.Component.VC.Tools.ARM64"
    } else {
        "Microsoft.VisualStudio.Component.VC.Tools.x86.x64"
    };
    let found = output(
        &vswhere,
        &[
            "-latest",
            "-products",
            "*",
            "-requires",
            component,
            "-property",
            "displayName",
            // The name is localized; without this it comes in the
            // console code page.
            "-utf8",
        ],
    )
    .filter(|name| !name.is_empty());
    let Some(visual_studio) = found else {
        return Status::Missing("no C++ build tools".into(), tools.into());
    };
    let Some(sdk) = windows_sdk(&program_files.join(r"Windows Kits\10\bin")) else {
        return Status::Missing(
            format!("{visual_studio}, but no Windows SDK with cppwinrt.exe"),
            "add a Windows 11 SDK in the Visual Studio Installer".into(),
        );
    };
    let host = output("rustc", &["-vV"]).unwrap_or_default();
    if !host.contains("windows-msvc") {
        return Status::Missing(
            "Rust does not target the MSVC toolchain".into(),
            "rustup default stable-msvc".into(),
        );
    }
    Status::Ready(format!("{visual_studio}, Windows SDK {sdk}"))
}

/// The newest Windows SDK below `bin` that has `cppwinrt.exe`.
fn windows_sdk(bin: &Path) -> Option<String> {
    let mut versions: Vec<(Vec<u32>, String)> = std::fs::read_dir(bin)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let parts: Option<Vec<u32>> = name.split('.').map(|part| part.parse().ok()).collect();
            let has_cppwinrt = ["x64", "arm64", "x86"]
                .iter()
                .any(|arch| entry.path().join(arch).join("cppwinrt.exe").is_file());
            has_cppwinrt.then_some((parts?, name))
        })
        .collect();
    versions.sort();
    versions.pop().map(|(_, name)| name)
}

fn linux() -> Status {
    let packages = "Debian and Ubuntu: sudo apt install libgtk-4-dev libadwaita-1-dev gstreamer1.0-libav\nFedora: sudo dnf install gtk4-devel libadwaita-devel";
    if output("pkg-config", &["--version"]).is_none() {
        return Status::Missing("pkg-config is not installed".into(), packages.into());
    }
    let mut found = Vec::new();
    for (module, name, needed) in [
        ("gtk4", "GTK", GTK),
        ("libadwaita-1", "libadwaita", LIBADWAITA),
    ] {
        let Some(installed) = output("pkg-config", &["--modversion", module]) else {
            return Status::Missing(format!("{name} is not installed"), packages.into());
        };
        if version(&installed).is_none_or(|version| version < needed) {
            return Status::Missing(
                format!(
                    "{name} {installed}; Bern needs {}.{} or newer",
                    needed.0, needed.1
                ),
                packages.into(),
            );
        }
        found.push(format!("{name} {installed}"));
    }
    Status::Ready(found.join(", "))
}

fn web() -> Status {
    let Some(sysroot) = output("rustc", &["--print", "sysroot"]) else {
        return Status::Missing(
            "rustc is not installed".into(),
            "install Rust from https://rustup.rs".into(),
        );
    };
    if Path::new(&sysroot)
        .join("lib/rustlib")
        .join(WASM_TARGET)
        .is_dir()
    {
        Status::Ready(format!("the {WASM_TARGET} target"))
    } else {
        Status::Missing(
            format!("the {WASM_TARGET} target is not installed"),
            format!("rustup target add {WASM_TARGET}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_from_tool_output() {
        assert_eq!(version("rustc 1.99.0 (abc 2026-09-01)"), Some((1, 99)));
        assert_eq!(version("26.1"), Some((26, 1)));
        assert_eq!(version("4.14.2"), Some((4, 14)));
        assert_eq!(version("none"), None);
        assert!((1, 98) < RUST && (1, 100) > RUST);
    }

    #[test]
    fn checks_cover_every_platform_once() {
        let checks = checks();
        for platform in ["macos", "windows", "linux", "browser"] {
            assert_eq!(
                checks
                    .iter()
                    .filter(|check| check.platforms.contains(&platform))
                    .count(),
                1,
                "{platform}"
            );
        }
        let elsewhere = checks
            .iter()
            .filter(|check| matches!(check.status, Status::Elsewhere(_)))
            .count();
        assert_eq!(elsewhere, 2, "two desktops build on other systems");
    }
}
