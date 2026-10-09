use bern_webui_demo::{CounterEngine, application};
use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    process::Command,
};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace path")
}

fn checked(command: &mut Command) -> Result<(), Box<dyn Error + Send + Sync>> {
    let program = command.get_program().to_string_lossy().into_owned();
    if !command.status()?.success() {
        return Err(format!("{program} failed").into());
    }
    Ok(())
}

fn build(endpoint: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let app = application();
    app.validate()?;
    let root = workspace();
    let version = Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .map_err(|_| "install wasm-bindgen-cli 0.2.126 before building (see README)")?;
    if !version.status.success()
        || String::from_utf8_lossy(&version.stdout).trim() != "wasm-bindgen 0.2.126"
    {
        return Err("wasm-bindgen CLI must be 0.2.126 to match the Rust runtime dependency".into());
    }
    checked(
        Command::new("cargo")
            .current_dir(&root)
            .args([
                "build",
                "--locked",
                "--release",
                "-p",
                "bern-web-runtime",
                "--target",
                "wasm32-unknown-unknown",
                "--target-dir",
            ])
            .arg(root.join("target")),
    )?;
    let package = root.join("target/bern-runtime-pkg");
    // This directory contains generated bindings only; remove stale generated snippets.
    if package.exists() {
        std::fs::remove_dir_all(&package)?;
    }
    checked(
        Command::new("wasm-bindgen")
            .arg(root.join("target/wasm32-unknown-unknown/release/bern_web_runtime.wasm"))
            .args([
                "--target",
                "web",
                "--out-name",
                "bern_web_runtime",
                "--out-dir",
            ])
            .arg(&package),
    )?;
    let output = root.join("target/bern/web_ui");
    bern_web::export(&app, &output, &package, endpoint)?;
    println!("WebUI 已生成：{}", output.display());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("build") => {
            let endpoint = match args.next().as_deref() {
                Some("--endpoint") => args.next().ok_or("--endpoint needs a WebSocket URL")?,
                None => "/api/ws".into(),
                _ => return Err("usage: build [--endpoint ws://127.0.0.1:3000/api/ws]".into()),
            };
            if args.next().is_some() {
                return Err("unexpected build argument".into());
            }
            build(&endpoint)?;
        }
        Some("serve") => {
            let port: u16 = match args.next().as_deref() {
                Some("--port") => args.next().ok_or("--port needs a number")?.parse()?,
                None => 3000,
                _ => return Err("usage: serve [--port 3000]".into()),
            };
            if args.next().is_some() {
                return Err("unexpected serve argument".into());
            }
            let directory = workspace().join("target/bern/web_ui");
            if !directory.join("build.json").is_file() {
                return Err("run `cargo run -p bern-webui-demo -- build` first".into());
            }
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
            println!("Bern WebUI: http://{address}");
            println!("独立引擎正在运行；按 Ctrl+C 退出。");
            bern_server::serve(CounterEngine::default(), directory, address).await?;
        }
        Some("--help" | "-h") | None => {
            println!(
                "Bern 基础 WebUI\n\n  build [--endpoint URL]   构建 UI-only 静态产物\n  serve [--port 3000]     启动独立引擎与本地静态资源服务\n\n本阶段仅实现 WebUI，不支持 Web 应用构建。"
            );
        }
        _ => return Err("unknown command; use --help".into()),
    }
    Ok(())
}
