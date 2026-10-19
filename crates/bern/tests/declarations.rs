//! Declarations that must compile, and declarations that must not.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn declarations() {
    // trybuild compiles every case in one generated project; the macros
    // look for `Bern.toml` in its directory and the ones above it.
    let config = trybuild_project().join("Bern.toml");
    let _ = std::fs::remove_file(&config);
    {
        // Without a project configuration, each `tr!` is checked on its own.
        let cases = trybuild::TestCases::new();
        cases.pass("tests/ui/pass_*.rs");
        cases.compile_fail("tests/ui/fail_*.rs");
    }
    // With one, every `tr!` and label translates exactly the languages it
    // lists, and the macros say so before the build tool runs.
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "[languages]\nsupported = [\"en\", \"ja\"]\n").unwrap();
    let _config = Removed(config);
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/languages/pass_*.rs");
    cases.compile_fail("tests/ui/languages/fail_*.rs");
}

/// The project trybuild generates: `tests/trybuild/<package>` in the
/// target directory `cargo metadata` names, as trybuild finds it.
fn trybuild_project() -> PathBuf {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version=1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let target = metadata["target_directory"]
        .as_str()
        .expect("the target directory");
    PathBuf::from(target)
        .join("tests/trybuild")
        .join(env!("CARGO_PKG_NAME"))
}

/// A file removed when the test ends, also when it fails.
struct Removed(PathBuf);

impl Drop for Removed {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
