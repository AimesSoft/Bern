//! The generated Kotlin file as a whole, and the activity that hosts
//! the windows.

use super::values::{generate_engine, generate_enum, generate_record, generate_sessions};
use super::{Generator, Out};
use crate::names::{double, member, string};
use bern_ir::WindowRole;

impl Generator<'_> {
    fn emit_header(&self, out: &mut Out, package: &str, banner: &str) {
        out.line(banner);
        out.line("@file:Suppress(\"RedundantSuppression\", \"UNUSED_VARIABLE\", \"UNUSED_ANONYMOUS_PARAMETER\", \"unused\", \"NAME_SHADOWING\")");
        out.line("");
        out.line(format!("package {package}"));
        out.line("");
        for import in if self.watch { WEAR_IMPORTS } else { IMPORTS } {
            out.line(format!("import {import}"));
        }
        out.line("");
    }

    fn emit_styles(&self, out: &mut Out) -> bool {
        let app = self.app;
        let mut wrote = false;
        let styles = crate::style::used(app, self.platform());
        if !styles.is_empty() {
            wrote = true;
            out.line("// MARK: - Styles");
            out.line("");
            out.line("/// The style overrides of `style!` on this platform.");
            out.open("object BernStyles {");
            for style in &styles {
                out.line(crate::style::declaration(app, style));
            }
            out.close("}");
            out.line("");
        }
        if !app.style.patterns.is_empty() {
            wrote = true;
            out.line("/// The patterns of `style!`.");
            out.open("object BernPatterns {");
            for pattern in &app.style.patterns {
                let bands = pattern
                    .bands
                    .iter()
                    .map(|band| {
                        format!(
                            "BernBand({}, {}, {})",
                            band.color
                                .as_ref()
                                .map(crate::style::color)
                                .unwrap_or_else(|| "null".into()),
                            double(band.width),
                            double(band.opacity)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.line(format!(
                    "val {} = BernPatternDef(listOf({bands}), {})",
                    member(&pattern.name),
                    pattern.twill
                ));
            }
            out.close("}");
            out.line("");
        }
        if !app.style.accents.is_empty() {
            wrote = true;
            out.line("/// The accents of `style!`, by the enum variant that chooses one.");
            out.open("object BernAccents {");
            out.open("fun color(name: String): BernColor? = when (name.replace(\"_\", \"\").lowercase()) {");
            for accent in &app.style.accents {
                out.line(format!(
                    "{} -> {}",
                    string(&accent.name.replace('_', "").to_lowercase()),
                    crate::style::color(&accent.color)
                ));
            }
            out.line("else -> null");
            out.close("}");
            out.close("}");
            out.line("");
        }
        wrote
    }

    fn emit_activity(&self, out: &mut Out, fingerprint: u64) {
        let app = self.app;
        let kinds: Vec<String> = app
            .windows
            .windows
            .iter()
            .map(|window| {
                let dialog = if window.role == WindowRole::Dialog {
                    format!(
                        ", isDialog = true, width = {}, height = {}",
                        window.width.map(double).unwrap_or_else(|| "null".into()),
                        window.height.map(double).unwrap_or_else(|| "null".into())
                    )
                } else {
                    String::new()
                };
                let overlay = window
                    .overlay
                    .as_deref()
                    .map(string)
                    .unwrap_or_else(|| "null".into());
                format!(
                    "BernWindowKind({}, {}, {}, isUtility = {}, overlay = {overlay}{dialog})",
                    string(&window.id),
                    string(&window.title.key),
                    window.role == WindowRole::Settings,
                    window.role == WindowRole::Utility
                )
            })
            .collect();
        let initial: Vec<String> = app
            .windows
            .windows
            .iter()
            .filter(|window| {
                window.initial == bern_ir::WindowInitial::Open && window.id != app.windows.primary
            })
            .map(|window| string(&window.id))
            .collect();
        let languages: Vec<String> = app.languages.supported.iter().map(|l| string(l)).collect();
        out.line("// MARK: - Activity");
        out.line("");
        out.line("/// The application's one activity: it starts the engine and shows the");
        out.line("/// primary window with the windows open over it.");
        out.open("class MainActivity : ComponentActivity() {");
        if self.multicast {
            out.line(
                "private var bernMulticastLock: android.net.wifi.WifiManager.MulticastLock? = null",
            );
            out.open("override fun onStart() {");
            out.line("super.onStart()");
            out.open("try {");
            out.line("val wifi = applicationContext.getSystemService(android.content.Context.WIFI_SERVICE) as? android.net.wifi.WifiManager");
            out.open("val lock = bernMulticastLock ?: wifi?.createMulticastLock(\"${packageName}:bern-discovery\")?.apply {");
            out.line("setReferenceCounted(false)");
            out.line("bernMulticastLock = this");
            out.close("}");
            out.line("if (lock != null && !lock.isHeld) lock.acquire()");
            out.close("} catch (error: RuntimeException) {");
            out.indent += 1;
            out.line(
                "android.util.Log.w(\"Bern\", \"Cannot enable Wi-Fi multicast reception\", error)",
            );
            out.close("}");
            out.close("}");
            out.open("override fun onStop() {");
            out.open("try {");
            out.line("val lock = bernMulticastLock");
            out.line("if (lock != null && lock.isHeld) lock.release()");
            out.close("} catch (error: RuntimeException) {");
            out.indent += 1;
            out.line(
                "android.util.Log.w(\"Bern\", \"Cannot release Wi-Fi multicast lock\", error)",
            );
            out.close("} finally {");
            out.indent += 1;
            out.line("super.onStop()");
            out.close("}");
            out.close("}");
        }
        out.open("override fun onCreate(savedInstanceState: Bundle?) {");
        out.line("super.onCreate(savedInstanceState)");
        out.line("enableEdgeToEdge()");
        if self.watch {
            out.line("BernHost.watch = true");
        }
        out.open("BernHost.start(");
        out.line("applicationContext,");
        out.line(format!("{}L,", fingerprint as i64));
        out.line(format!("listOf({}),", languages.join(", ")));
        out.line(format!("{},", string(&app.languages.default)));
        out.line(format!("listOf({}),", kinds.join(", ")));
        out.line(format!("{},", string(&app.windows.primary)));
        out.close(") { field, reader -> BernEngine.bernApply(field, reader) }");
        out.line("BernFiles.register(this)");
        out.line("BernClipboard.register(this)");
        out.line("BernHost.readKeyboard(resources.configuration)");
        if !app.style.text_weights.is_empty() {
            let styles: Vec<String> = app
                .style
                .text_weights
                .iter()
                .map(|entry| {
                    let size = entry.size.map(double).unwrap_or_else(|| "null".into());
                    let size = match entry.tv_size {
                        Some(tv) => format!("if (BernHost.television) {} else {size}", double(tv)),
                        None => size,
                    };
                    format!(
                        "{} to Pair({}, {size})",
                        string(&entry.style),
                        string(&entry.weight),
                    )
                })
                .collect();
            out.line(format!(
                "BernTypography.styles = mapOf({})",
                styles.join(", ")
            ));
        }
        if let Some(layouts) = &app.layouts {
            let or_null = |value: Option<f64>| value.map(double).unwrap_or_else(|| "null".into());
            out.line(format!(
                "BernHost.layouts = BernLayouts({}, {}, {})",
                double(layouts.compact_below),
                or_null(layouts.medium_below),
                or_null(layouts.expanded_below)
            ));
        }
        out.open("setContent {");
        out.open("BernTheme {");
        out.open("val renderWindow: @Composable (String) -> Unit = { kind ->");
        out.open("when (kind) {");
        for (index, window) in app.windows.windows.iter().enumerate() {
            out.line(format!("{} -> BernWindow{index}()", string(&window.id)));
        }
        out.close("}");
        out.close("}");
        out.line("BernHost.overlayRenderer = renderWindow");
        out.line(format!(
            "LaunchedEffect(Unit) {{ BernHost.openInitial(listOf({})) }}",
            initial.join(", ")
        ));
        out.line("BernVirtualHost(renderWindow)");
        out.close("}");
        out.close("}");
        out.close("}");
        out.line("");
        out.line("/// A keyboard connecting or going away, for `window.keyboard`.");
        out.open(
            "override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {",
        );
        out.line("super.onConfigurationChanged(newConfig)");
        out.line("BernHost.readKeyboard(newConfig)");
        out.close("}");
        out.line("");
        out.line("/// A file another app opens with this one while it runs.");
        out.open("override fun onNewIntent(intent: android.content.Intent) {");
        out.line("super.onNewIntent(intent)");
        out.line("BernFiles.opened(intent)");
        out.close("}");
        out.line("");
        out.line("/// Keyboard shortcuts come first, whether or not a control has focus:");
        out.line("/// Compose sends keys only to the focused control and its parents.");
        out.line("/// On a television the remote's arrows move the focus first, and a");
        out.line("/// shortcut takes one only where the focus moves nowhere.");
        out.open("override fun dispatchKeyEvent(event: android.view.KeyEvent): Boolean {");
        out.line("BernShortcuts.pressed(event)");
        out.line("val key = androidx.compose.ui.input.key.KeyEvent(event)");
        out.line("if (BernHost.television && BernShortcuts.isArrow(event)) return super.dispatchKeyEvent(event) || BernShortcuts.handle(key)");
        out.line("return BernShortcuts.handle(key) || super.dispatchKeyEvent(event)");
        out.close("}");
        out.close("}");
    }

    /// Generate the ejected, multi-file Kotlin source tree (`Models.kt`,
    /// `Styles.kt`, `AppState.kt`, `views/<Component>.kt`,
    /// `windows/<Window>Window.kt`, and `MainActivity.kt`).
    pub fn generate_split(&self, package: &str, fingerprint: u64) -> Vec<(String, String)> {
        *self.package.borrow_mut() = package.to_string();
        let app = self.app;
        let mut files = Vec::new();
        let banner = "// Ejected from Bern — edit freely.";

        if !app.types.enums.is_empty() || !app.types.records.is_empty() {
            let mut out = Out::default();
            self.emit_header(&mut out, package, banner);
            for def in &app.types.enums {
                generate_enum(&mut out, def);
            }
            for def in &app.types.records {
                generate_record(&mut out, def);
            }
            files.push(("Models.kt".into(), out.finish()));
        }

        {
            let mut out = Out::default();
            self.emit_header(&mut out, package, banner);
            if self.emit_styles(&mut out) {
                files.push(("Styles.kt".into(), out.finish()));
            }
        }

        {
            let mut out = Out::default();
            self.emit_header(&mut out, package, banner);
            out.line("// MARK: - Engine state");
            out.line("");
            generate_engine(&mut out, app);
            out.line("// MARK: - Session state");
            out.line("");
            generate_sessions(&mut out, app);
            files.push(("AppState.kt".into(), out.finish()));
        }

        let mut used_basenames: std::collections::BTreeSet<String> =
            ["models", "styles", "appstate", "mainactivity"]
                .into_iter()
                .map(str::to_string)
                .collect();
        let take_unique = |used: &mut std::collections::BTreeSet<String>, base: String| -> String {
            let mut candidate = base.clone();
            let mut suffix = 2usize;
            while !used.insert(candidate.to_ascii_lowercase()) {
                candidate = format!("{base}{suffix}");
                suffix += 1;
            }
            candidate
        };

        for component in &app.components {
            let stem = take_unique(
                &mut used_basenames,
                bern_target_api::eject::pascal_filename(&component.name),
            );
            let mut out = Out::default();
            self.emit_header(&mut out, package, banner);
            self.component(&mut out, component);
            files.push((format!("views/{stem}.kt"), out.finish()));
        }

        for (index, window) in app.windows.windows.iter().enumerate() {
            let base = format!(
                "{}Window",
                bern_target_api::eject::pascal_filename(&window.id)
            );
            let stem = if used_basenames.contains(&base.to_ascii_lowercase()) {
                take_unique(
                    &mut used_basenames,
                    format!(
                        "{}Scene",
                        bern_target_api::eject::pascal_filename(&window.id)
                    ),
                )
            } else {
                take_unique(&mut used_basenames, base)
            };
            let mut out = Out::default();
            self.emit_header(&mut out, package, banner);
            self.window(&mut out, index, window);
            files.push((format!("windows/{stem}.kt"), out.finish()));
        }

        {
            let mut out = Out::default();
            self.emit_header(&mut out, package, banner);
            self.emit_activity(&mut out, fingerprint);
            files.push(("MainActivity.kt".into(), out.finish()));
        }

        files
    }

    pub fn generate(&self, package: &str, fingerprint: u64) -> String {
        *self.package.borrow_mut() = package.to_string();
        let app = self.app;
        let mut out = Out::default();
        self.emit_header(
            &mut out,
            package,
            "// Generated by Bern from the Rust declarations. Do not edit.",
        );
        out.line("// MARK: - Values");
        out.line("");
        for def in &app.types.enums {
            generate_enum(&mut out, def);
        }
        for def in &app.types.records {
            generate_record(&mut out, def);
        }
        self.emit_styles(&mut out);
        out.line("// MARK: - Engine state");
        out.line("");
        generate_engine(&mut out, app);
        out.line("// MARK: - Session state");
        out.line("");
        generate_sessions(&mut out, app);
        out.line("// MARK: - Components");
        out.line("");
        for component in &app.components {
            self.component(&mut out, component);
        }
        out.line("// MARK: - Windows");
        out.line("");
        for (index, window) in app.windows.windows.iter().enumerate() {
            self.window(&mut out, index, window);
        }
        self.emit_activity(&mut out, fingerprint);
        out.finish()
    }
}

const IMPORTS: &[&str] = &[
    "androidx.compose.animation.AnimatedContent",
    "androidx.compose.animation.AnimatedVisibility",
    "androidx.compose.animation.SizeTransform",
    "androidx.compose.animation.togetherWith",
    "androidx.compose.foundation.Canvas",
    "androidx.compose.ui.graphics.Color",
    "androidx.compose.ui.semantics.Role",
    "androidx.compose.ui.semantics.clearAndSetSemantics",
    "androidx.compose.ui.semantics.contentDescription",
    "androidx.compose.ui.semantics.role",
    "androidx.compose.ui.semantics.semantics",
    "android.os.Bundle",
    "androidx.activity.ComponentActivity",
    "androidx.activity.compose.setContent",
    "androidx.activity.enableEdgeToEdge",
    "androidx.compose.foundation.horizontalScroll",
    "androidx.compose.foundation.layout.Arrangement",
    "androidx.compose.foundation.layout.Box",
    "androidx.compose.foundation.layout.BoxWithConstraints",
    "androidx.compose.foundation.background",
    "androidx.compose.foundation.shape.RoundedCornerShape",
    "androidx.compose.ui.draw.clipToBounds",
    "androidx.compose.foundation.layout.Column",
    "androidx.compose.foundation.layout.Row",
    "androidx.compose.foundation.layout.Spacer",
    "androidx.compose.foundation.layout.fillMaxHeight",
    "androidx.compose.foundation.layout.fillMaxSize",
    "androidx.compose.foundation.layout.fillMaxWidth",
    "androidx.compose.foundation.layout.padding",
    "androidx.compose.foundation.layout.size",
    "androidx.compose.foundation.layout.width",
    "androidx.compose.foundation.layout.height",
    "androidx.compose.foundation.layout.wrapContentHeight",
    "androidx.compose.foundation.lazy.LazyColumn",
    "androidx.compose.foundation.lazy.items",
    "androidx.compose.foundation.rememberScrollState",
    "androidx.compose.foundation.verticalScroll",
    "androidx.compose.material3.HorizontalDivider",
    "androidx.compose.material3.MaterialTheme",
    "androidx.compose.material3.Text",
    "androidx.compose.material3.TextButton",
    "androidx.compose.material3.VerticalDivider",
    "androidx.compose.runtime.Composable",
    "androidx.compose.runtime.CompositionLocalProvider",
    "androidx.compose.runtime.LaunchedEffect",
    "androidx.compose.runtime.getValue",
    "androidx.compose.runtime.key",
    "androidx.compose.runtime.mutableStateOf",
    "androidx.compose.runtime.remember",
    "androidx.compose.runtime.saveable.rememberSaveable",
    "androidx.compose.runtime.setValue",
    "androidx.compose.ui.Alignment",
    "androidx.compose.ui.Modifier",
    "androidx.compose.ui.layout.ContentScale",
    "androidx.compose.ui.platform.testTag",
    "androidx.compose.ui.platform.LocalContext",
    "androidx.compose.ui.state.ToggleableState",
    "androidx.compose.ui.unit.dp",
    "dev.bern.runtime.*",
];

const WEAR_IMPORTS: &[&str] = &[
    "androidx.compose.animation.AnimatedContent",
    "androidx.compose.animation.AnimatedVisibility",
    "androidx.compose.animation.SizeTransform",
    "androidx.compose.animation.togetherWith",
    "androidx.compose.foundation.Canvas",
    "androidx.compose.ui.graphics.Color",
    "androidx.compose.ui.semantics.Role",
    "androidx.compose.ui.semantics.clearAndSetSemantics",
    "androidx.compose.ui.semantics.contentDescription",
    "androidx.compose.ui.semantics.role",
    "androidx.compose.ui.semantics.semantics",
    "android.os.Bundle",
    "androidx.activity.ComponentActivity",
    "androidx.activity.compose.setContent",
    "androidx.activity.enableEdgeToEdge",
    "androidx.compose.foundation.horizontalScroll",
    "androidx.compose.foundation.layout.Arrangement",
    "androidx.compose.foundation.layout.Box",
    "androidx.compose.foundation.layout.BoxWithConstraints",
    "androidx.compose.foundation.background",
    "androidx.compose.foundation.shape.RoundedCornerShape",
    "androidx.compose.ui.draw.clipToBounds",
    "androidx.compose.foundation.layout.Column",
    "androidx.compose.foundation.layout.Row",
    "androidx.compose.foundation.layout.Spacer",
    "androidx.compose.foundation.layout.fillMaxHeight",
    "androidx.compose.foundation.layout.fillMaxSize",
    "androidx.compose.foundation.layout.fillMaxWidth",
    "androidx.compose.foundation.layout.padding",
    "androidx.compose.foundation.layout.size",
    "androidx.compose.foundation.layout.width",
    "androidx.compose.foundation.layout.height",
    "androidx.compose.foundation.layout.wrapContentHeight",
    "androidx.compose.foundation.lazy.LazyColumn",
    "androidx.compose.foundation.lazy.items",
    "androidx.wear.compose.foundation.lazy.items",
    "androidx.compose.foundation.rememberScrollState",
    "androidx.compose.foundation.verticalScroll",
    "androidx.wear.compose.material3.MaterialTheme",
    "androidx.wear.compose.material3.Text",
    "androidx.compose.runtime.Composable",
    "androidx.compose.runtime.CompositionLocalProvider",
    "androidx.compose.runtime.LaunchedEffect",
    "androidx.compose.runtime.getValue",
    "androidx.compose.runtime.key",
    "androidx.compose.runtime.mutableStateOf",
    "androidx.compose.runtime.remember",
    "androidx.compose.runtime.saveable.rememberSaveable",
    "androidx.compose.runtime.setValue",
    "androidx.compose.ui.Alignment",
    "androidx.compose.ui.Modifier",
    "androidx.compose.ui.layout.ContentScale",
    "androidx.compose.ui.platform.testTag",
    "androidx.compose.ui.platform.LocalContext",
    "androidx.compose.ui.state.ToggleableState",
    "androidx.compose.ui.unit.dp",
    "dev.bern.runtime.*",
];
