//! `bern run`: start the application a target just built.

use bern_target_api::{Run, SimulatedSystem};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

/// Start the built application and wait until it ends. `trace` carries
/// performance trace rules to the app: as `BERN_TRACE` for programs and
/// simulators, as an intent extra on Android, in the address of a page.
pub fn start(run: &Run, port: Option<u16>, trace: Option<&str>) -> Result<ExitCode, String> {
    validate_port(run, port)?;
    match run {
        Run::Program(program) => {
            eprintln!("    \x1b[1;32mRunning\x1b[0m {}", program.display());
            let mut command = Command::new(program);
            if let Some(rules) = trace {
                command.env("BERN_TRACE", rules);
            }
            let status = command
                .status()
                .map_err(|error| format!("cannot start {}: {error}", program.display()))?;
            Ok(exit_code(status.code()))
        }
        Run::Page(root) => serve(root, port, trace),
        Run::PageHost(program) => host(program, trace),
        Run::Simulator {
            app,
            bundle_id,
            system,
        } => simulator(app, bundle_id, *system, trace),
        Run::Android {
            apk,
            package,
            adb,
            forward,
        } => android(apk, package, adb, forward.as_ref(), trace),
    }
}

/// The address fragment that turns on tracing in a page.
fn page_trace(rules: &str) -> String {
    let mut fragment = String::from("#bern-trace=");
    for byte in rules.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~:,*/".contains(&byte) {
            fragment.push(byte as char);
        } else {
            fragment.push_str(&format!("%{byte:02X}"));
        }
    }
    fragment
}

/// `text` quoted for the device's shell, which `adb shell` hands a single
/// command line.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Reject a page-only option before building or starting a native host.
pub fn validate_port(run: &Run, port: Option<u16>) -> Result<(), String> {
    if port.is_some() && !matches!(run, Run::Page(_)) {
        return Err("--port is only available for browser targets with web_mode = \"app\"".into());
    }
    Ok(())
}

/// Install the APK on the one Android device `adb` reaches, start it, and
/// show its log until the app's process ends.
fn android(
    apk: &Path,
    package: &str,
    adb: &Path,
    forward: Option<&(std::path::PathBuf, String)>,
    trace: Option<&str>,
) -> Result<ExitCode, String> {
    let run = |args: &[&str]| {
        Command::new(adb)
            .args(args)
            .output()
            .map_err(|error| format!("cannot run {}: {error}", adb.display()))
    };
    let devices = run(&["devices"])?;
    let listed: Vec<String> = String::from_utf8_lossy(&devices.stdout)
        .lines()
        .skip(1)
        .filter_map(|line| line.strip_suffix("\tdevice").map(str::to_string))
        .collect();
    let device = match listed.as_slice() {
        [device] => device.clone(),
        [] => {
            return Err(
                "no Android device is connected; connect one with USB debugging on, or start an emulator"
                    .into(),
            );
        }
        _ => {
            return Err(format!(
                "several Android devices are connected ({}); keep one",
                listed.join(", ")
            ));
        }
    };
    eprintln!(
        "    \x1b[1;32mInstalling\x1b[0m {} on {device}",
        apk.display()
    );
    // A release build's baseline profile goes with it where the system
    // takes its form (Android 12 and later): the app is then compiled
    // ahead of time at install, instead of running in the JIT until the
    // system compiles it in the background.
    let apk_path = apk.display().to_string();
    let profile = apk.with_extension("dm");
    let sdk = run(&["shell", "getprop", "ro.build.version.sdk"])
        .ok()
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u32>()
                .ok()
        })
        .unwrap_or(0);
    let mut installed = None;
    if sdk >= 31 && profile.is_file() {
        let profile_path = profile.display().to_string();
        let together = run(&["install-multiple", "-r", &apk_path, &profile_path])?;
        if together.status.success() {
            installed = Some(together);
        } else {
            eprintln!(
                "warning: the device did not take the baseline profile ({}); installing the APK alone",
                String::from_utf8_lossy(&together.stdout).trim()
            );
        }
    }
    let installed = match installed {
        Some(installed) => installed,
        None => run(&["install", "-r", &apk_path])?,
    };
    if !installed.status.success() {
        return Err(format!(
            "cannot install {}: {}",
            apk.display(),
            String::from_utf8_lossy(&installed.stdout).trim()
        ));
    }
    eprintln!("    \x1b[1;32mRunning\x1b[0m {package} on {device}");
    run(&["shell", "am", "force-stop", package])?;
    let activity = format!("{package}/.MainActivity");
    let mut start = vec!["shell", "am", "start", "-W", "-n", &activity];
    // Android apps have no environment to set; the activity hands the
    // extra to the runtime.
    let quoted = trace.map(shell_quote);
    if let Some(rules) = &quoted {
        start.extend(["--es", "bern.trace", rules.as_str()]);
    }
    let started = run(&start)?;
    if !started.status.success() {
        return Err(format!("cannot start {package}"));
    }
    let pid = || {
        run(&["shell", "pidof", "-s", package])
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|pid| !pid.is_empty())
    };
    let Some(process) = pid() else {
        return Err(format!("{package} ended right after it started"));
    };
    // The application's command line tools on this computer reach the
    // app's control channel through a forwarded socket.
    if let Some((local, remote)) = forward {
        if let Some(dir) = local.parent() {
            let _ = std::fs::create_dir_all(dir);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            }
        }
        // A forward of an earlier run would keep its socket file only
        // until the file is removed; start over.
        let local_spec = format!("localfilesystem:{}", local.display());
        let _ = run(&["forward", "--remove", &local_spec]);
        let _ = std::fs::remove_file(local);
        let forwarded = run(&["forward", &local_spec, &format!("localabstract:{remote}")])?;
        if !forwarded.status.success() {
            eprintln!(
                "warning: cannot forward the control channel: {}",
                String::from_utf8_lossy(&forwarded.stderr).trim()
            );
        }
    }
    let mut log = Command::new(adb)
        .args(["logcat", "-v", "brief", &format!("--pid={process}")])
        .spawn()
        .map_err(|error| format!("cannot read the device log: {error}"))?;
    while pid().as_deref() == Some(process.as_str()) {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    let _ = log.kill();
    let _ = log.wait();
    Ok(ExitCode::SUCCESS)
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
fn serve(root: &Path, port: Option<u16>, trace: Option<&str>) -> Result<ExitCode, String> {
    let bridge = root.join("serve-bridge.py");
    if bridge.is_file() {
        let mut command = Command::new("python3");
        command
            .arg("-u")
            .arg(&bridge)
            .arg("--directory")
            .arg(root)
            .arg("--port")
            .arg(port.unwrap_or(0).to_string());
        if let Some(rules) = trace {
            command.arg("--trace").arg(rules);
        }
        let mut child = command
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start browser command bridge (Python 3 required): {e}"))?;
        for line in BufReader::new(child.stdout.take().expect("piped")).lines() {
            let Ok(line) = line else { break };
            println!("{line}");
            if let Some(url) = line.strip_prefix("open ") {
                open_browser(url.trim());
            }
        }
        return child
            .wait()
            .map(|status| exit_code(status.code()))
            .map_err(|e| format!("cannot wait for browser command bridge: {e}"));
    }
    let server = bern_transport_ws::FileServer::start(root.to_path_buf(), port.unwrap_or(0))
        .map_err(|error| format!("cannot serve {}: {error}", root.display()))?;
    let url = server.url() + &trace.map(page_trace).unwrap_or_default();
    eprintln!("    \x1b[1;32mServing\x1b[0m {} at {url}", root.display());
    eprintln!("             press Ctrl-C to stop");
    open_browser(&url);
    loop {
        std::thread::park();
    }
}

/// Start an engine host, open the page it serves, and wait for it. A
/// trace runs in the host, which starts the page's side when it connects.
fn host(program: &Path, trace: Option<&str>) -> Result<ExitCode, String> {
    eprintln!("    \x1b[1;32mRunning\x1b[0m {}", program.display());
    let mut command = Command::new(program);
    if let Some(rules) = trace {
        command.env("BERN_TRACE", rules);
    }
    let mut child = command
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

/// Install the app into the booted simulator of `system` (iOS or tvOS),
/// start it with its output on this terminal, and wait until it exits. The
/// app's IPC channel keeps its socket in this Mac's temporary directory,
/// where the application's command line tools on the Mac look; `BERN_*`
/// variables reach the app as well.
fn simulator(
    app: &Path,
    bundle_id: &str,
    system: SimulatedSystem,
    trace: Option<&str>,
) -> Result<ExitCode, String> {
    let (udid, name) = booted_simulator(system)?;
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
    if let Some(rules) = trace {
        launch.env("SIMCTL_CHILD_BERN_TRACE", rules);
    }
    let status = launch
        .status()
        .map_err(|error| format!("cannot run simctl: {error}"))?;
    Ok(exit_code(status.code()))
}

/// The UDID and name of a booted simulator of `system`.
fn booted_simulator(system: SimulatedSystem) -> Result<(String, String), String> {
    let missing = format!(
        "no {} simulator is running; start one with `open -a Simulator` or `xcrun simctl boot <device>`",
        system.name()
    );
    let runtime = format!(".{}-", system.name());
    let output = Command::new("xcrun")
        .args(["simctl", "list", "devices", "booted", "--json"])
        .output()
        .map_err(|error| format!("cannot run simctl: {error}"))?;
    let list: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|_| missing.clone())?;
    list["devices"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(name, _)| name.contains(&runtime))
        .flat_map(|(_, devices)| devices.as_array().into_iter().flatten())
        .find(|device| device["state"] == "Booted")
        .and_then(|device| {
            Some((
                device["udid"].as_str()?.to_string(),
                device["name"].as_str()?.to_string(),
            ))
        })
        .ok_or(missing)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_rules_reach_pages_and_device_shells_intact() {
        assert_eq!(
            page_trace("engine,command:library.*,file:/tmp/a b.json"),
            "#bern-trace=engine,command:library.*,file:/tmp/a%20b.json"
        );
        assert_eq!(shell_quote("engine,ui"), "'engine,ui'");
        assert_eq!(shell_quote("file:/tmp/it's"), "'file:/tmp/it'\\''s'");
    }

    #[test]
    fn fixed_port_only_applies_to_pages() {
        assert!(validate_port(&Run::Page("page".into()), Some(4173)).is_ok());
        for run in [Run::Program("app".into()), Run::PageHost("host".into())] {
            assert!(validate_port(&run, None).is_ok());
            assert!(
                validate_port(&run, Some(4173))
                    .unwrap_err()
                    .contains("web_mode = \"app\"")
            );
            // Invalid options fail before the nonexistent program is started.
            assert!(
                start(&run, Some(4173), None)
                    .unwrap_err()
                    .contains("--port")
            );
        }
    }
}
