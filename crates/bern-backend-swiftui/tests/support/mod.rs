//! What the tests of UIKit-based Apple targets share.

use bern_backend_swiftui::MINIMUM_IOS;
use std::path::Path;
use std::process::Command;

pub mod cache;

/// Compile the iOS Swift generated into `output`, when the simulator SDK is
/// installed. An app of both front ends has one in `Sources` for iOS 16 and
/// one in `Sources17` for iOS 17 and later, and each is compiled for its own
/// earliest system, which is what notices a call the older one lacks.
pub fn check_ios(root: &Path, output: &Path) {
    let sources = output.join("Sources");
    let header = sources.join("bern.h");
    if output.join("Sources17").is_dir() {
        check_front_end(
            root,
            &sources,
            &header,
            ("iphonesimulator", "ios", "16.0"),
            &["BernCompat16.swift", "BernLaunch16.swift"],
        );
        check_front_end(
            root,
            &output.join("Sources17"),
            &header,
            ("iphonesimulator", "ios", MINIMUM_IOS),
            &["BernEntry17.swift"],
        );
    } else {
        check_front_end(
            root,
            &sources,
            &header,
            ("iphonesimulator", "ios", MINIMUM_IOS),
            &[],
        );
    }
}

/// Compile a front end as far as the compiler's own checks go, which is
/// further than type-checking: it decides, for example, whether a stored
/// property is initialized.
pub fn check_front_end(
    root: &Path,
    sources: &Path,
    header: &Path,
    (sdk, platform, minimum): (&str, &str, &str),
    extra: &[&str],
) {
    if !Command::new("xcrun")
        .args(["--sdk", sdk, "--show-sdk-path"])
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("no {sdk} SDK; skipping the check");
        return;
    }
    let architecture = if std::env::consts::ARCH == "x86_64" {
        "x86_64"
    } else {
        "arm64"
    };
    let mut command = Command::new("xcrun");
    command
        .args([
            "--sdk",
            sdk,
            "swiftc",
            "-wmo",
            "-emit-sil",
            "-o",
            "/dev/null",
        ])
        .args(["-swift-version", "6", "-parse-as-library"])
        .arg("-target")
        .arg(format!(
            "{architecture}-apple-{platform}{minimum}-simulator"
        ))
        .arg("-module-cache-path")
        .arg(cache::module_cache(root))
        .arg("-import-objc-header")
        .arg(header);
    for file in [
        "BernRuntime.swift",
        "BernMedia.swift",
        "BernStyle.swift",
        "BernUIKit.swift",
        "BernGenerated.swift",
    ]
    .iter()
    .chain(extra)
    {
        command.arg(sources.join(file));
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "swiftc rejected the front end for {platform} {minimum}:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
