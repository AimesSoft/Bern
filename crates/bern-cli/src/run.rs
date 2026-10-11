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
