//! `bern run`: start the application a target just built.

use bern_target_api::Run;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

/// Start the built application and wait until it ends.
pub fn start(run: &Run) -> Result<ExitCode, String> {
    match run {
        Run::Program(program) => {
            eprintln!("    \x1b[1;32mRunning\x1b[0m {}", program.display());
            let status = Command::new(program)
                .status()
                .map_err(|error| format!("cannot start {}: {error}", program.display()))?;
            Ok(exit_code(status.code()))
        }
        Run::Page(root) => serve(root),
        Run::PageHost(program) => host(program),
        Run::Simulator { app, bundle_id } => simulator(app, bundle_id),
    }
}

fn exit_code(code: Option<i32>) -> ExitCode {
    match code {
        Some(0) => ExitCode::SUCCESS,
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        // Ended by a signal, for example Ctrl-C.
        None => ExitCode::FAILURE,
    }
}

/// Serve a page whose engine runs in the page until Ctrl-C.
fn serve(root: &Path) -> Result<ExitCode, String> {
    let server = bern_transport_ws::FileServer::start(root.to_path_buf(), 0)
        .map_err(|error| format!("cannot serve {}: {error}", root.display()))?;
    let url = server.url();
    eprintln!("    \x1b[1;32mServing\x1b[0m {} at {url}", root.display());
    eprintln!("             press Ctrl-C to stop");
    open_browser(&url);
    loop {
        std::thread::park();
    }
}

/// Start an engine host, open the page it serves, and wait for it.
fn host(program: &Path) -> Result<ExitCode, String> {
    eprintln!("    \x1b[1;32mRunning\x1b[0m {}", program.display());
    let mut child = Command::new(program)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", program.display()))?;
    let stdout = child.stdout.take().expect("piped");
    let mut out = std::io::stdout();
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        let _ = writeln!(out, "{line}");
        if let Some(url) = line.strip_prefix("open ") {
            open_browser(url.trim());
        }
    }
    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for {}: {error}", program.display()))?;
    Ok(exit_code(status.code()))
}

/// Install the app into the booted iOS simulator, start it with its output
/// on this terminal, and wait until it exits. The app's IPC channel keeps
/// its socket in this Mac's temporary directory, where the application's
/// command line tools on the Mac look; `BERN_*` variables reach the app as
/// well.
fn simulator(app: &Path, bundle_id: &str) -> Result<ExitCode, String> {
    let (udid, name) = booted_simulator()?;
    eprintln!(
        "    \x1b[1;32mInstalling\x1b[0m {} on {name}",
        app.display()
    );
    let installed = Command::new("xcrun")
        .args(["simctl", "install", &udid])
        .arg(app)
        .status()
        .map_err(|error| format!("cannot run simctl: {error}"))?;
    if !installed.success() {
        return Err(format!("cannot install {} on {name}", app.display()));
    }
    eprintln!("    \x1b[1;32mRunning\x1b[0m {bundle_id} on {name}");
    let mut launch = Command::new("xcrun");
    launch.args([
        "simctl",
        "launch",
        "--console",
        "--terminate-running-process",
        &udid,
        bundle_id,
    ]);
    // simctl hands `SIMCTL_CHILD_` variables to the app without the prefix.
    let temp = std::env::var_os("BERN_IPC_DIR")
        .or_else(|| std::env::var_os("TMPDIR"))
        .unwrap_or_else(|| "/tmp".into());
    launch.env("SIMCTL_CHILD_BERN_IPC_DIR", temp);
    for (key, value) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("BERN_") {
            let mut child = std::ffi::OsString::from("SIMCTL_CHILD_");
            child.push(&key);
            launch.env(child, value);
        }
    }
    let status = launch
        .status()
        .map_err(|error| format!("cannot run simctl: {error}"))?;
    Ok(exit_code(status.code()))
}

/// The UDID and name of a booted iOS simulator.
fn booted_simulator() -> Result<(String, String), String> {
    let missing = "no iOS simulator is running; start one with `open -a Simulator` or `xcrun simctl boot <device>`";
    let output = Command::new("xcrun")
        .args(["simctl", "list", "devices", "booted", "--json"])
        .output()
        .map_err(|error| format!("cannot run simctl: {error}"))?;
    let list: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|_| missing.to_string())?;
    list["devices"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(runtime, _)| runtime.contains(".iOS-"))
        .flat_map(|(_, devices)| devices.as_array().into_iter().flatten())
        .find(|device| device["state"] == "Booted")
        .and_then(|device| {
            Some((
                device["udid"].as_str()?.to_string(),
                device["name"].as_str()?.to_string(),
            ))
        })
        .ok_or_else(|| missing.to_string())
}

/// Open `url` in the default browser; the address is printed either way.
fn open_browser(url: &str) {
    let mut command = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        let mut command = Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    } else {
        Command::new("xdg-open")
    };
    let opened = command
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !opened {
        eprintln!("             open {url} in a browser");
    }
}
