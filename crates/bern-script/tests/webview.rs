#[cfg(target_os = "macos")]
#[test]
fn wkwebview_executes_scripts() {
    let runtime = bern_script::HOST_RUNTIME;
    let html = format!(
        "<!doctype html><body><script>function bernScriptComplete(value){{window.webkit.messageHandlers.bernScript.postMessage(value)}}\n{runtime}</script>"
    );
    let host = include_str!("../../bern-target-api/templates/script/host.swift").replace(
        "/*bern:script-html*/",
        &serde_json::to_string(&html).unwrap(),
    );
    let source = include_str!("webview_smoke.swift").replace("/*bern:host*/", &host);
    let dir = std::env::temp_dir().join(format!("bern-script-webview-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.swift"), source).unwrap();
    let build = std::process::Command::new("xcrun")
        .args(["swiftc", "-swift-version", "6", "-parse-as-library"])
        .arg(dir.join("main.swift"))
        .arg("-o")
        .arg(dir.join("test"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = std::process::Command::new(dir.join("test"))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
