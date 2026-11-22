//! The generated Swift file as a whole, and the scenes of the
//! application.

use super::values::{generate_engine, generate_enum, generate_record, generate_sessions};
use super::{Generator, Out, Platform};
use crate::names::{double, string};
use bern_ir::{Node, WindowDef, WindowRole};

impl Generator<'_> {
    fn platform_imports(&self, out: &mut Out) {
        out.line(match self.platform {
            Platform::Macos => "import AppKit",
            Platform::Ios | Platform::Tvos => "import UIKit",
            Platform::Watchos => "import WatchKit",
        });
        out.line("import SwiftUI");
        out.line("");
    }

    fn emit_styles(&self, out: &mut Out) {
        let app = self.app;
        let styles = crate::style::used(app, self.style_platform());
        if !styles.is_empty() {
            out.line("// MARK: - Styles");
            out.line("");
            out.line("/// The style overrides of `style!` on this platform.");
            out.open("enum BernStyles {");
            for style in &styles {
                out.line(crate::style::declaration(app, style));
            }
            out.close("}");
            out.line("");
        }
        out.line("/// The breakpoints of `[layouts]`.");
        out.line(format!(
            "let bernAppBreakpoints = BernBreakpoints(compact: {}, medium: {}, expanded: {})",
            app.layouts
                .map(|l| double(l.compact_below))
                .unwrap_or_else(|| double(0.0)),
            app.layouts
                .and_then(|l| l.medium_below)
                .map(double)
                .unwrap_or_else(|| "nil".into()),
            app.layouts
                .and_then(|l| l.expanded_below)
                .map(double)
                .unwrap_or_else(|| "nil".into())
        ));
        out.line("");
        if !app.style.accents.is_empty() {
            out.line("/// The accents of `style!`, by the enum variant that chooses one.");
            out.open("enum BernAccents {");
            out.open("static func color(_ name: String) -> BernColor? {");
            out.open("switch name.replacingOccurrences(of: \"_\", with: \"\").lowercased() {");
            for accent in &app.style.accents {
                out.line(format!(
                    "case {}: return {}",
                    string(&accent.name.replace('_', "").to_lowercase()),
                    crate::style::color(&accent.color)
                ));
            }
            out.line("default: return nil");
            out.close("}");
            out.close("}");
            out.close("}");
            out.line("");
        }
        if !app.style.patterns.is_empty() {
            out.line("/// The patterns of `style!`.");
            out.open("enum BernPatterns {");
            for pattern in &app.style.patterns {
                let bands = pattern
                    .bands
                    .iter()
                    .map(|band| {
                        format!(
                            "BernBand(color: {}, width: {}, opacity: {})",
                            band.color
                                .as_ref()
                                .map(crate::style::color)
                                .unwrap_or_else(|| "nil".into()),
                            double(band.width),
                            double(band.opacity)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.line(format!(
                    "static let `{}` = BernPatternDef(bands: [{bands}], twill: {})",
                    pattern.name, pattern.twill
                ));
            }
            out.close("}");
            out.line("");
        }
        if self.platform == Platform::Tvos {
            // The body of a scene laid out at the design's size: the
            // application's body for a television, or the design's 16.
            let body = app
                .style
                .text_weights
                .iter()
                .find(|entry| entry.style == "body")
                .and_then(|entry| entry.tv_size.or(entry.size))
                .unwrap_or(16.0);
            out.line("/// The body of a scene laid out at the design's size.");
            out.line(format!("let bernSceneBodySize: CGFloat = {}", double(body)));
            out.line("");
        }
    }

    fn emit_app_entry(&self, out: &mut Out, fingerprint: u64, include_native_types: bool) {
        let app = self.app;
        let mut order: Vec<usize> = (0..app.windows.windows.len()).collect();
        order.sort_by_key(|index| {
            let window = &app.windows.windows[*index];
            (
                window.id != app.windows.primary,
                window.role == WindowRole::Settings,
            )
        });
        let kinds: Vec<String> = app
            .windows
            .windows
            .iter()
            .map(|window| {
                let size = match (window.width, window.height) {
                    (Some(width), Some(height)) => format!(
                        "CGSize(width: {}, height: {})",
                        double(width),
                        double(height)
                    ),
                    _ => "nil".into(),
                };
                let overlay = window
                    .overlay
                    .as_deref()
                    .map(string)
                    .unwrap_or_else(|| "nil".into());
                format!(
                    "BernWindowKind(id: {}, title: {}, isSettings: {}, isDialog: {}, isUtility: {}, overlay: {overlay}, defaultSize: {size})",
                    string(&window.id),
                    string(&window.title.key),
                    window.role == WindowRole::Settings,
                    window.role == WindowRole::Dialog,
                    window.role == WindowRole::Utility
                )
            })
            .collect();
        // Windows that open with the session besides the primary; a macOS
        // settings window opens only on request.
        let initial: Vec<String> = app
            .windows
            .windows
            .iter()
            .filter(|window| {
                window.initial == bern_ir::WindowInitial::Open
                    && window.id != app.windows.primary
                    && (self.platform.uikit()
                        || self.platform == Platform::Watchos
                        || window.role != WindowRole::Settings)
            })
            .map(|window| string(&window.id))
            .collect();
        self.menu_window(out);
        if include_native_types {
            self.native_view_types(out);
        }
        let menu_bar = self.menu_bar(out);
        let status_item = self.status_menu(out);
        let status_follows = status_item
            && app
                .windows
                .status_item
                .as_ref()
                .is_some_and(|item| item.expr("shown").is_some());
        if status_item {
            self.status_scene_type(out);
        }
        out.line("@main");
        out.open("struct BernApplication: App {");
        match self.platform {
            Platform::Macos => {
                out.line(
                    "@NSApplicationDelegateAdaptor(BernAppDelegate.self) private var delegate",
                );
                out.line("");
            }
            // The orientations a window asks of the phone.
            Platform::Ios => {
                out.line("@UIApplicationDelegateAdaptor(BernOrientationDelegate.self) private var delegate");
                out.line("");
            }
            // A television or watch has no orientations to ask for.
            Platform::Tvos | Platform::Watchos => {}
        }
        out.open("init() {");
        out.open(format!(
            "BernHost.shared.start(fingerprint: 0x{fingerprint:016x}, kinds: [{}], primary: {}) {{ field, reader in",
            kinds.join(", "),
            string(&app.windows.primary)
        ));
        out.line("try BernEngine.shared.bernApply(field, &reader)");
        out.close("}");
        if status_item {
            out.line("BernStatusArea.shown = true");
        }
        out.close("}");
        out.line("");
        out.open("var body: some Scene {");
        match self.platform {
            Platform::Macos => {
                self.macos_scenes(out, &order, &initial, menu_bar);
                if status_follows {
                    out.line("BernStatusScene()");
                } else if status_item {
                    self.status_scene(out);
                }
            }
            Platform::Ios => self.ios_scene(out, &initial),
            Platform::Tvos => self.tvos_scene(out, &initial),
            Platform::Watchos => self.watchos_scene(out, &initial),
        }
        out.close("}");
        out.close("}");
    }

    /// Generate the ejected, multi-file Swift source tree organized into
    /// `Models/`, `Styles/`, `Views/`, `Windows/`, and `App/`.
    pub fn generate_split(&self, fingerprint: u64) -> Vec<(String, String)> {
        let app = self.app;
        let mut files = Vec::new();
        let header = |out: &mut Out, title: &str| {
            out.line(format!("// {title}"));
            out.line("// Ejected from Bern — edit freely.");
            out.line("");
            self.platform_imports(out);
        };

        if !app.types.enums.is_empty() || !app.types.records.is_empty() {
            let mut out = Out::default();
            header(&mut out, "Protocol value types");
            for def in &app.types.enums {
                generate_enum(&mut out, def);
            }
            for def in &app.types.records {
                generate_record(&mut out, def);
            }
            files.push(("Models/Values.swift".into(), out.finish()));
        }

        {
            let mut out = Out::default();
            header(&mut out, "Engine and session state");
            out.line("// MARK: - Engine state");
            out.line("");
            generate_engine(&mut out, app);
            out.line("// MARK: - Session state");
            out.line("");
            generate_sessions(&mut out, app);
            files.push(("Models/AppState.swift".into(), out.finish()));
        }

        {
            let mut out = Out::default();
            header(
                &mut out,
                "Styles, accents, patterns, and layout breakpoints",
            );
            self.emit_styles(&mut out);
            files.push(("Styles/AppStyles.swift".into(), out.finish()));
        }

        let mut used_basenames: std::collections::BTreeSet<String> = [
            "values",
            "appstate",
            "appstyles",
            "nativeviews",
            "appentry",
            "bernruntime",
            "bernmedia",
            "bernstyle",
            "bernappkit",
            "bernuikit",
            "bernwatchkit",
        ]
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
            header(&mut out, &format!("View component: {}", component.name));
            self.component(&mut out, component);
            files.push((format!("Views/{stem}.swift"), out.finish()));
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
            header(&mut out, &format!("Window: {}", window.id));
            self.window(&mut out, index, window);
            files.push((format!("Windows/{stem}.swift"), out.finish()));
        }

        {
            let mut native_out = Out::default();
            self.native_view_types(&mut native_out);
            let native_body = native_out.finish();
            if !native_body.trim().is_empty() {
                let mut out = Out::default();
                header(&mut out, "NativeView bridge types");
                out.line(native_body.trim_end());
                files.push(("App/NativeViews.swift".into(), out.finish()));
            }
        }

        {
            let mut out = Out::default();
            header(&mut out, "Application entry point and scenes");
            self.emit_app_entry(&mut out, fingerprint, false);
            files.push(("App/AppEntry.swift".into(), out.finish()));
        }

        files
    }

    pub fn generate(&self, fingerprint: u64) -> String {
        let app = self.app;
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("");
        self.platform_imports(&mut out);
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
        self.emit_app_entry(&mut out, fingerprint, true);
        out.finish()
    }

    /// One SwiftUI scene per window kind: `Window`, or `Settings` for the
    /// settings window. The primary window's scene carries the menu bar's
    /// commands, which hold for the whole application.
    fn macos_scenes(&self, out: &mut Out, order: &[usize], initial: &[String], menu_bar: bool) {
        let app = self.app;
        for &index in order {
            let window = &app.windows.windows[index];
            let title = format!("BernText.string({})", string(&window.title.key));
            if window.role == WindowRole::Dialog {
                continue;
            }
            if window.role == WindowRole::Settings {
                out.open("SwiftUI.Settings {");
                out.line(format!("BernWindow{index}()"));
                out.line("    .environment(BernEngine.shared)");
                out.close("}");
            } else {
                out.open(format!(
                    "SwiftUI.Window(SwiftUI.Text(verbatim: {title}), id: {}) {{",
                    string(&window.id)
                ));
                out.line(format!("BernWindow{index}()"));
                if window.id == app.windows.primary {
                    self.dialogs(out);
                }
                out.line("    .environment(BernEngine.shared)");
                if window.id == app.windows.primary && !initial.is_empty() {
                    out.line(format!(
                        "    .task {{ BernHost.shared.openInitial([{}]) }}",
                        initial.join(", ")
                    ));
                }
                out.close("}");
                if let (Some(width), Some(height)) = (window.width, window.height) {
                    out.line(format!(
                        ".defaultSize(width: {}, height: {})",
                        double(width),
                        double(height)
                    ));
                }
                if window.min_width.is_some() || window.min_height.is_some() {
                    out.line(".windowResizability(.contentMinSize)");
                }
                if menu_bar && window.id == app.windows.primary {
                    out.line(".commands { BernMenuBar() }");
                }
            }
        }
    }

    /// The dialog windows over the primary window, as sheets.
    fn dialogs(&self, out: &mut Out) {
        let dialogs: Vec<(usize, &WindowDef)> = self
            .app
            .windows
            .windows
            .iter()
            .enumerate()
            .filter(|(_, window)| window.role == WindowRole::Dialog)
            .collect();
        if dialogs.is_empty() {
            return;
        }
        out.open("    .modifier(BernDialogs(depth: 0) { kind in");
        out.open("switch kind {");
        for (index, window) in dialogs {
            // A sheet takes its content's size: the declared one, at least.
            let size = match (window.width, window.height) {
                (Some(width), Some(height)) => format!(
                    ".frame(minWidth: {w}, idealWidth: {w}, minHeight: {h}, idealHeight: {h})",
                    w = double(width),
                    h = double(height)
                ),
                (Some(width), None) => {
                    format!(".frame(minWidth: {w}, idealWidth: {w})", w = double(width))
                }
                (None, Some(height)) => format!(
                    ".frame(minHeight: {h}, idealHeight: {h})",
                    h = double(height)
                ),
                (None, None) => String::new(),
            };
            out.line(format!(
                "case {}: BernWindow{index}(){size}",
                string(&window.id)
            ));
        }
        out.line("default: SwiftUI.EmptyView()");
        out.close("}");
        out.close("})");
    }

    /// The scenes of an iOS app. The primary window's scene hosts the
    /// virtual windows: on iPhone every other window is a sheet over the
    /// primary, titled and with a close button unless its content brings
    /// its own navigation; on iPad only settings is. On iPad every other
    /// window has a scene of its own, identified by its kind, so opening it
    /// again brings the open one forward, and titled in a toolbar unless
    /// its content brings its own navigation. A settings window opens with
    /// Command-Comma, as settings do.
    fn ios_scene(&self, out: &mut Out, initial: &[String]) {
        let app = self.app;
        let window_group = |out: &mut Out, window: &WindowDef| {
            out.open(format!(
                "SwiftUI.WindowGroup(SwiftUI.Text(verbatim: BernText.string({})), id: {}, for: String.self) {{ _ in",
                string(&window.title.key),
                string(&window.id)
            ));
        };
        let scene_modifiers = |out: &mut Out, window: &WindowDef| {
            out.close(format!("}} defaultValue: {{ {} }}", string(&window.id)));
            if let (Some(width), Some(height)) = (window.width, window.height) {
                out.line(format!(
                    ".defaultSize(width: {}, height: {})",
                    double(width),
                    double(height)
                ));
            }
            if window.min_width.is_some() || window.min_height.is_some() {
                out.line(".windowResizability(.contentMinSize)");
            }
        };
        let Some(primary) = app
            .windows
            .windows
            .iter()
            .find(|w| w.id == app.windows.primary)
        else {
            return;
        };
        window_group(out, primary);
        out.open(format!(
            "BernVirtualHost(initial: [{}]) {{ kind in",
            initial.join(", ")
        ));
        out.open("switch kind {");
        for (index, window) in app.windows.windows.iter().enumerate() {
            out.line(format!("case {}:", string(&window.id)));
            out.indent += 1;
            if window.id == app.windows.primary || self.has_navigation(&window.content, 0) {
                out.line(format!("BernWindow{index}()"));
            } else {
                out.line(format!(
                    "BernSheetChrome(kind: {}, title: {}) {{ BernWindow{index}() }}",
                    string(&window.id),
                    string(&window.title.key)
                ));
            }
            out.indent -= 1;
        }
        out.line("default:");
        out.line("    SwiftUI.EmptyView()");
        out.close("}");
        out.close("}");
        out.line(".environment(BernEngine.shared)");
        scene_modifiers(out, primary);
        if let Some(settings) = app
            .windows
            .windows
            .iter()
            .find(|window| window.role == WindowRole::Settings)
        {
            out.open(".commands {");
            out.open("SwiftUI.CommandGroup(replacing: .appSettings) {");
            out.line(format!(
                "SwiftUI.Button(BernText.string({}) + \"…\") {{ BernHost.shared.open({}) }}",
                string(&settings.title.key),
                string(&settings.id)
            ));
            out.line("    .keyboardShortcut(\",\")");
            out.close("}");
            out.close("}");
        }
        for (index, window) in app.windows.windows.iter().enumerate() {
            if window.id == app.windows.primary || window.role == WindowRole::Settings {
                continue;
            }
            window_group(out, window);
            if self.has_navigation(&window.content, 0) {
                out.line(format!("BernWindow{index}()"));
            } else {
                out.line(format!(
                    "BernWindowChrome(title: {}) {{ BernWindow{index}() }}",
                    string(&window.title.key)
                ));
            }
            out.line("    .environment(BernEngine.shared)");
            scene_modifiers(out, window);
        }
    }

    /// The scene of a tvOS app: one full-screen window group, the primary
    /// window, with every other window over it, filling the screen, as
    /// tvOS shows its own screens: without a title bar or a Done button,
    /// the remote's Back closes it. A television shows one scene; there
    /// are no window sizes, commands or keyboard shortcuts for it.
    fn tvos_scene(&self, out: &mut Out, initial: &[String]) {
        let app = self.app;
        out.open("SwiftUI.WindowGroup {");
        out.open(format!(
            "BernVirtualHost(initial: [{}]) {{ kind in",
            initial.join(", ")
        ));
        out.open("switch kind {");
        for (index, window) in app.windows.windows.iter().enumerate() {
            out.line(format!("case {}:", string(&window.id)));
            out.line(format!("    BernWindow{index}()"));
        }
        out.line("default:");
        out.line("    SwiftUI.EmptyView()");
        out.close("}");
        out.close("}");
        out.line(".environment(BernEngine.shared)");
        out.close("}");
    }

    /// The scene of a watchOS app: one full-screen window group hosting the
    /// primary window, with secondary windows presented as watch sheets over
    /// it. Sheets without their own navigation get a title and a Done action
    /// so windows with close checks can still ask to close.
    fn watchos_scene(&self, out: &mut Out, initial: &[String]) {
        let app = self.app;
        out.open("SwiftUI.WindowGroup {");
        out.open(format!(
            "BernVirtualHost(initial: [{}]) {{ kind in",
            initial.join(", ")
        ));
        out.open("switch kind {");
        for (index, window) in app.windows.windows.iter().enumerate() {
            out.line(format!("case {}:", string(&window.id)));
            out.indent += 1;
            if window.id == app.windows.primary || self.has_navigation(&window.content, 0) {
                out.line(format!("BernWindow{index}()"));
            } else {
                out.line(format!(
                    "BernSheetChrome(kind: {}, title: {}) {{ BernWindow{index}() }}",
                    string(&window.id),
                    string(&window.title.key)
                ));
            }
            out.indent -= 1;
        }
        out.line("default:");
        out.line("    SwiftUI.EmptyView()");
        out.close("}");
        out.close("}");
        out.line(".environment(BernEngine.shared)");
        out.close("}");
    }

    /// Whether a window's content has a navigation container of its own,
    /// directly or through the components it calls.
    fn has_navigation(&self, node: &Node, depth: usize) -> bool {
        // Components cannot call themselves forever; the depth only guards
        // against a cycle the compiler would report.
        if depth > 64 {
            return false;
        }
        let mut found = false;
        node.walk(&mut |node| match node {
            Node::Element(element)
                if matches!(element.kind.as_str(), "SplitView" | "Navigation") =>
            {
                found = true;
            }
            Node::Component(call) => {
                if let Some(component) = self.app.component(&call.component) {
                    found |= self.has_navigation(&component.body, depth + 1);
                }
            }
            _ => {}
        });
        found
    }
}
