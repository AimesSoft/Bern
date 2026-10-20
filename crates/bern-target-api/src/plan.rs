//! Build plans: the steps a target asks the build driver to run, how
//! `bern run` starts the product, and helpers that make steps and place
//! packages.

use crate::BuildContext;
use std::path::PathBuf;

/// One step the build driver executes for a target.
#[derive(Clone, Debug)]
pub enum BuildStep {
    Run {
        description: String,
        program: String,
        args: Vec<String>,
        cwd: PathBuf,
        env: Vec<(String, String)>,
    },
    CreateDir {
        path: PathBuf,
    },
    Copy {
        from: PathBuf,
        to: PathBuf,
    },
    RemoveIfExists {
        path: PathBuf,
    },
    /// Write a file, creating its directory.
    Write {
        path: PathBuf,
        contents: Vec<u8>,
    },
    /// Make `dir` hold exactly `files` plus the paths in `keep`, which
    /// other steps own (for example generated localizations). Files are
    /// copied by content, without extended attributes; a destination whose
    /// size and modification time match its source is left alone. Anything
    /// else below `dir` is removed, so files dropped from a declaration do
    /// not linger in the product.
    Mirror {
        dir: PathBuf,
        /// Source file and destination relative to `dir`.
        files: Vec<(PathBuf, PathBuf)>,
        /// Paths relative to `dir` to leave in place.
        keep: Vec<PathBuf>,
    },
}

#[derive(Clone, Debug, Default)]
pub struct BuildPlan {
    pub steps: Vec<BuildStep>,
    /// Final artifacts, for reporting.
    pub products: Vec<PathBuf>,
    /// How `bern run` starts the built application.
    pub run: Option<Run>,
    /// What the person building should know about the products, printed
    /// after the steps; for example that a package is not signed.
    pub notes: Vec<String>,
}

/// How `bern run` starts a built application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
    /// Start the program and wait until it exits.
    Program(PathBuf),
    /// Serve the directory on the loopback interface and open its
    /// `index.html` in the browser.
    Page(PathBuf),
    /// Start the program, which serves its own page and prints the address
    /// on a line `open <address>`; open that address in the browser and
    /// wait until the program exits.
    PageHost(PathBuf),
    /// Install the app bundle into a booted simulator of `system`, start it
    /// there and wait until it exits.
    Simulator {
        app: PathBuf,
        bundle_id: String,
        system: SimulatedSystem,
    },
    /// Install the APK on the Android device `adb` reaches, start its
    /// activity and show its log until it exits.
    Android {
        apk: PathBuf,
        package: String,
        adb: PathBuf,
        /// The socket on this computer and the app's abstract socket that
        /// `adb forward` joins, for the control channel.
        forward: Option<(PathBuf, String)>,
    },
}

/// The system a simulator runs, as its runtimes name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulatedSystem {
    Ios,
    Tvos,
}

impl SimulatedSystem {
    /// The system's name in simulator runtimes and in messages: `iOS`,
    /// `tvOS`.
    pub fn name(self) -> &'static str {
        match self {
            SimulatedSystem::Ios => "iOS",
            SimulatedSystem::Tvos => "tvOS",
        }
    }
}

/// Where packages go: `target/bern/dist/` of the workspace.
pub fn dist_dir(context: &BuildContext) -> PathBuf {
    context
        .workspace_dir
        .join("target")
        .join("bern")
        .join("dist")
}

/// The build machine's architecture as packages name it: `arm64` or
/// `x64` (`x86_64` on macOS, where that is the usual name).
pub fn package_architecture() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        (_, "aarch64") => "arm64",
        ("macos", _) => "x86_64",
        _ => "x64",
    }
}

/// A step that archives `dir`, with `dir` itself as the top folder, into
/// `stem` plus the extension the build machine's own tools make: `.zip`
/// on macOS (`ditto`) and Windows (`tar`), `.tar.gz` on Linux (`tar`).
pub fn archive(dir: &std::path::Path, stem: &std::path::Path) -> (BuildStep, PathBuf) {
    let parent = dir.parent().unwrap_or(dir).to_path_buf();
    let name = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let with = |extension: &str| {
        let mut file = stem.as_os_str().to_owned();
        file.push(extension);
        PathBuf::from(file)
    };
    let (program, args, out) = match std::env::consts::OS {
        "macos" => {
            let out = with(".zip");
            let args = vec![
                "-c".to_string(),
                "-k".into(),
                // Plain files: no resource forks, extended attributes or
                // quarantine flags (`__MACOSX/._*` entries elsewhere).
                "--norsrc".into(),
                "--noextattr".into(),
                "--noqtn".into(),
                "--keepParent".into(),
                dir.display().to_string(),
                out.display().to_string(),
            ];
            ("ditto", args, out)
        }
        "windows" => {
            let out = with(".zip");
            let args = vec![
                "-a".to_string(),
                "-c".into(),
                "-f".into(),
                out.display().to_string(),
                "-C".into(),
                parent.display().to_string(),
                name.clone(),
            ];
            ("tar", args, out)
        }
        _ => {
            let out = with(".tar.gz");
            let args = vec![
                "-czf".to_string(),
                out.display().to_string(),
                "-C".into(),
                parent.display().to_string(),
                name.clone(),
            ];
            ("tar", args, out)
        }
    };
    let step = BuildStep::Run {
        description: format!("Archive {name}"),
        program: program.into(),
        args,
        cwd: parent,
        env: vec![],
    };
    (step, out)
}

/// Every file below `dir`, as (path, path relative to `dir`), sorted.
pub fn files_below(dir: &std::path::Path) -> std::io::Result<Vec<(PathBuf, PathBuf)>> {
    fn walk(
        dir: &std::path::Path,
        relative: &std::path::Path,
        out: &mut Vec<(PathBuf, PathBuf)>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir.join(relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                walk(dir, &path, out)?;
            } else {
                out.push((dir.join(&path), path));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, std::path::Path::new(""), &mut out)?;
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archives_use_the_build_machine_s_own_tools() {
        let (step, out) = archive(
            std::path::Path::new("/stage/Hello"),
            std::path::Path::new("/dist/Hello-1.0"),
        );
        let BuildStep::Run { program, args, .. } = step else {
            panic!("a command");
        };
        match std::env::consts::OS {
            "macos" => {
                assert_eq!(
                    (program.as_str(), out),
                    ("ditto", PathBuf::from("/dist/Hello-1.0.zip"))
                );
                assert!(args.contains(&"--noextattr".to_string()));
            }
            "windows" => assert_eq!(program, "tar"),
            _ => assert_eq!(
                (program.as_str(), out),
                ("tar", PathBuf::from("/dist/Hello-1.0.tar.gz"))
            ),
        }
    }
}
