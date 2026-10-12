//! Where the Android tools are: the SDK and its NDK, Gradle and a JDK.
//! Variables name them explicitly; otherwise the places Android Studio
//! and the Gradle wrapper install to are searched.

use std::path::{Path, PathBuf};

pub struct Toolchain {
    pub sdk: PathBuf,
    pub ndk: PathBuf,
    pub gradle: PathBuf,
    pub java_home: Option<PathBuf>,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn var(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The Android SDK: `ANDROID_HOME`, `ANDROID_SDK_ROOT`, or Android
/// Studio's default location.
pub fn sdk() -> Option<PathBuf> {
    var("ANDROID_HOME")
        .or_else(|| var("ANDROID_SDK_ROOT"))
        .or_else(|| {
            let home = home()?;
            [home.join("Library/Android/sdk"), home.join("Android/Sdk")]
                .into_iter()
                .find(|path| path.is_dir())
        })
        .filter(|path| path.is_dir())
}

/// Version numbers of a directory name such as `29.0.14206865`, for
/// ordering.
fn version(name: &str) -> Vec<u64> {
    name.split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// The newest directory below `dir` whose name `accept` takes.
fn newest(dir: &Path, accept: impl Fn(&str) -> bool) -> Option<PathBuf> {
    let mut found: Vec<(Vec<u64>, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            accept(&name).then(|| (version(&name), entry.path()))
        })
        .collect();
    found.sort();
    found.pop().map(|(_, path)| path)
}

/// The NDK: `ANDROID_NDK_HOME`, `ANDROID_NDK_ROOT`, or the newest one the
/// SDK manager installed.
pub fn ndk(sdk: &Path) -> Option<PathBuf> {
    var("ANDROID_NDK_HOME")
        .or_else(|| var("ANDROID_NDK_ROOT"))
        .or_else(|| newest(&sdk.join("ndk"), |_| true))
        .filter(|path| path.is_dir())
}

/// The NDK's prebuilt toolchain for this computer.
pub fn ndk_bin(ndk: &Path) -> PathBuf {
    let host = match std::env::consts::OS {
        "macos" => "darwin-x86_64",
        "windows" => "windows-x86_64",
        _ => "linux-x86_64",
    };
    ndk.join("toolchains/llvm/prebuilt").join(host).join("bin")
}

/// Gradle 9.4 or newer, which the Android Gradle plugin needs:
/// `BERN_GRADLE`, `gradle` on the path, or the newest the Gradle wrapper
/// downloaded.
pub fn gradle() -> Option<PathBuf> {
    if let Some(path) = var("BERN_GRADLE") {
        return Some(path);
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("gradle");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let dists = home()?.join(".gradle/wrapper/dists");
    let mut found: Vec<(Vec<u64>, PathBuf)> = Vec::new();
    for dist in std::fs::read_dir(&dists).ok()?.filter_map(Result::ok) {
        for hash in std::fs::read_dir(dist.path())
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            for install in std::fs::read_dir(hash.path())
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
            {
                let name = install.file_name().to_string_lossy().into_owned();
                let number = version(&name);
                let usable = name.starts_with("gradle-")
                    && (number.first().is_some_and(|major| *major > 9)
                        || (number.first() == Some(&9)
                            && number.get(1).is_some_and(|minor| *minor >= 4)));
                let program = install.path().join("bin/gradle");
                if usable && program.is_file() {
                    found.push((number, program));
                }
            }
        }
    }
    found.sort();
    found.pop().map(|(_, path)| path)
}

/// A JDK for Gradle: `BERN_JAVA_HOME`, Android Studio's bundled runtime,
/// or `JAVA_HOME`.
pub fn java_home() -> Option<PathBuf> {
    var("BERN_JAVA_HOME")
        .or_else(|| {
            [
                PathBuf::from("/Applications/Android Studio.app/Contents/jbr/Contents/Home"),
                PathBuf::from("/opt/android-studio/jbr"),
            ]
            .into_iter()
            .find(|path| path.join("bin/java").is_file())
        })
        .or_else(|| var("JAVA_HOME"))
}

/// The Android tools, or what is missing.
pub fn find() -> Result<Toolchain, String> {
    let sdk = sdk()
        .ok_or("the Android SDK is not installed; install Android Studio or set ANDROID_HOME")?;
    let ndk = ndk(&sdk).ok_or(
        "the Android NDK is not installed; add it in Android Studio's SDK Manager (SDK Tools → NDK) or set ANDROID_NDK_HOME",
    )?;
    let gradle = gradle().ok_or(
        "Gradle 9.4 or newer is not installed; install it or set BERN_GRADLE to its `gradle` program",
    )?;
    Ok(Toolchain {
        sdk,
        ndk,
        gradle,
        java_home: java_home(),
    })
}

/// What `bern doctor` reports: the tools found, or what is missing.
pub fn describe() -> Result<String, String> {
    let tools = find()?;
    let name = |path: &Path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let gradle = tools
        .gradle
        .ancestors()
        .nth(2)
        .map(name)
        .unwrap_or_else(|| tools.gradle.display().to_string());
    Ok(format!("Android SDK, NDK {}, {gradle}", name(&tools.ndk)))
}

/// `adb` of the SDK, or the one on the path.
pub fn adb() -> PathBuf {
    sdk()
        .map(|sdk| sdk.join("platform-tools/adb"))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("adb"))
}
