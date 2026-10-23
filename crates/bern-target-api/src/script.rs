//! Build-time selection and shared sources of the JavaScript host.
use crate::BuildContext;
pub fn dependency(context: &BuildContext) -> String {
    let Some(backend) = context.target.javascript_backend() else {
        return String::new();
    };
    format!(
        "bern-script = {{ {}, features = [{}] }}\n",
        context.framework.dependency("bern-script"),
        if backend == "boa" { "\"boa\"" } else { "" }
    )
}
pub fn webview(context: &BuildContext) -> bool {
    context.target.javascript_backend() == Some("webview")
}
pub const RUNTIME: &str = include_str!("../../bern-script/src/runtime.js");
pub fn html(complete: &str) -> String {
    format!(
        "<!doctype html><meta charset=\"utf-8\"><body><script>function bernScriptComplete(value){{{complete}}}\n{RUNTIME}</script>"
    )
}
pub const HEADER: &str = "\nvoid bern_script_start(void);\nvoid bern_script_stop(void);\nBernBuffer bern_script_poll(double now_ms);\nvoid bern_script_complete(const uint8_t *bytes, size_t len);\n";

/// Add only the selected host to generated sources and dependencies.
pub fn configure(files: &mut [crate::GeneratedFile], context: &BuildContext) {
    let Some(backend) = context.target.javascript_backend() else {
        return;
    };
    for file in files {
        let path = file.path.to_string_lossy();
        if path.ends_with("Cargo.toml") {
            let source = String::from_utf8_lossy(&file.contents);
            file.contents = source
                .replacen(
                    "[dependencies]\n",
                    &format!("[dependencies]\n{}", dependency(context)),
                    1,
                )
                .into_bytes();
        }
        if backend == "boa" {
            continue;
        }
        let mut source = String::from_utf8_lossy(&file.contents).into_owned();
        if path.ends_with("/bern.h") {
            source.push_str(HEADER);
        } else if path.ends_with("BernRuntime.swift") {
            source = source.replace("        platformWillStart()", "        BernScriptHost.shared.start()\n        platformWillStart()")
                .replace("        if let handle { bern_app_stop(handle) }", "        BernScriptHost.shared.stop()\n        if let handle { bern_app_stop(handle) }")
                .replace("            startupError = String(decoding:", "            BernScriptHost.shared.stop()\n            startupError = String(decoding:");
            let html = html("window.webkit.messageHandlers.bernScript.postMessage(value)");
            source.push_str(&include_str!("../templates/script/host.swift").replace(
                "/*bern:script-html*/",
                &serde_json::to_string(&html).unwrap(),
            ));
        } else if path.ends_with("BernRuntime.kt") {
            source = source.replace("object BernNative {", "object BernNative {\n    external fun scriptStart()\n    external fun scriptStop()\n    external fun scriptPoll(nowMs: Double): ByteArray\n    external fun scriptComplete(bytes: ByteArray)")
                .replace("        val reader = BernReader(BernNative.start(", "        BernScriptHost.start(context)\n        val reader = BernReader(BernNative.start(")
                .replace("            startupError = reader.text()", "            BernScriptHost.stop()\n            startupError = reader.text()");
            let html = html("BernScriptBridge.complete(value)");
            source.push_str(&include_str!("../templates/script/host.kt").replace(
                "/*bern:script-html*/",
                &serde_json::to_string(&html).unwrap().replace('$', "\\$"),
            ));
        } else if path.ends_with("bern_base.h") {
            source = source.replace(
                "void bern_buffer_free(BernBuffer buffer);",
                &format!("void bern_buffer_free(BernBuffer buffer);{HEADER}"),
            );
        } else if path.ends_with("bern_pch.h") {
            source.push_str("\n#include <WebView2.h>\n#include <wrl.h>\n#pragma comment(lib, \"WebView2LoaderStatic.lib\")\n");
        } else if path.ends_with("bern_base.cpp") {
            source = source
                .replace(
                    "namespace bern {",
                    "namespace bern::Scripts { void Start(); void Stop(); }\nnamespace bern {",
                )
                .replace(
                    "    catalog = &app;",
                    "    Scripts::Start();\n    catalog = &app;",
                )
                .replace(
                    "    auto app = handle;",
                    "    Scripts::Stop();\n    auto app = handle;",
                )
                .replace(
                    "    if (!app_handle) {",
                    "    if (!app_handle) {\n        Scripts::Stop();",
                );
            let html = html("window.chrome.webview.postMessage(value)");
            source.push_str(&include_str!("../templates/script/host.cpp").replace(
                "/*bern:script-html*/",
                &format!("R\"BERNJS({html})BERNJS\""),
            ));
        } else if path.ends_with("bern.js") && backend == "browser" {
            source = source.replace(
                "  start(fingerprint, resources) {",
                "  start(fingerprint, resources) {\n    this.stopScripts = startScriptHost(this);",
            );
            source = source.replace(
                "      throw new Error(message);",
                "      this.stopScripts?.();\n      throw new Error(message);",
            );
            source.push_str(include_str!("../templates/script/host.js"));
        } else {
            continue;
        }
        file.contents = source.into_bytes();
    }
}
