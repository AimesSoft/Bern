//! Compile one page declaration and inspect the UI source each backend emits,
//! not the (always present) support functions in its runtime.
use std::path::Path;

fn ordered(backend: &str, mut source: &str, sequence: &[&str]) {
    for &marker in sequence {
        (_, source) = source
            .split_once(marker)
            .unwrap_or_else(|| panic!("{backend}: missing or misplaced `{marker}`"));
    }
}

/// Only inspect this declaration; another query elsewhere must not satisfy it.
fn declaration<'a>(source: &'a str, start: &str) -> &'a str {
    source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing {start}"))
        .1
        .split_once("\n}")
        .expect("end of declaration")
        .0
}

#[test]
fn owning_a_query_does_not_depend_on_presenting_it() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut config =
        bern_compiler::config::load(&here.join("../../examples/gallery/Bern.toml")).unwrap();
    config.ui_sources[0].path = here.join("tests/fixtures/query_page.rs");
    config.ui_sources[0].display = "query_page.rs".into();
    let app = bern_compiler::compile(&config).unwrap().app;

    let swift = bern_backend_swiftui::Generator::new(&app, bern_backend_swiftui::Platform::Macos)
        .generate(0);
    ordered(
        "SwiftUI page",
        declaration(&swift, "struct AppView_page: View"),
        &[
            "BernQueryScope(",
            "presented:",
            "settings",
            "BernQueryView(phase: __bernPhase, retry: __bernRetry,",
            "BernAccordion(",
            "AppView_forwarded(__bernCellCorner: ",
        ],
    );
    ordered(
        "SwiftUI forwarding",
        declaration(&swift, "struct AppView_forwarded: View"),
        &["AppView_query_body(__bernCellCorner: __bernCellCorner)"],
    );
    ordered(
        "SwiftUI root",
        declaration(&swift, "struct AppView_query_body: View"),
        &[
            "BernQueryScope(",
            "presented:",
            "BernAccordionCell(corner: __bernCellCorner)",
            "BernQueryStatusLayout(active: __bernPhase.needsStatusHeader) {",
            "AppView_browse_header(",
            "} content: {",
            "BernQueryView(phase:",
            ") { rows in",
            "AppView_browse_header(",
        ],
    );

    let compose = bern_backend_compose::Generator::new(&app).generate("dev.bern.test", 0);
    ordered(
        "Compose page",
        declaration(&compose, "fun AppView_page("),
        &[
            "BernQueryScope(",
            "settings",
            "BernQueryView(__bernQuery,",
            "AppView_forwarded(__bernRootModifier = Modifier.alignByBaseline())",
        ],
    );
    ordered(
        "Compose forwarding",
        declaration(&compose, "fun AppView_forwarded("),
        &["AppView_query_body(__bernRootModifier = __bernRootModifier)"],
    );
    ordered(
        "Compose root",
        declaration(&compose, "fun AppView_query_body("),
        &[
            "BernQueryScope(",
            "modifier = __bernRootModifier",
            "BernQueryStatusLayout(__bernQuery.phase.needsStatusHeader, header = {",
            "AppView_browse_header(",
            "}) {",
            "BernQueryView(__bernQuery,",
            ") { rows ->",
            "AppView_browse_header(",
        ],
    );

    let gtk = bern_backend_gtk::Generator::new(&app).generate(0, "x");
    ordered(
        "GTK page",
        declaration(&gtk, "fn c_page(p: &b::Parent, c: &b::Ctx) {"),
        &[
            "let draw = b::own_query(c,",
            "b::present_when(p, c,",
            ", draw);",
            "let draw = b::own_query(c,",
            "placed: true",
            "b::Place(p, c,",
            "b::present_when(p, c,",
        ],
    );
    ordered(
        "GTK header and success builders",
        declaration(&gtk, "fn c_query_body(p: &b::Parent, c: &b::Ctx) {"),
        &[
            "let status_header = b::build(",
            "c_browse_header(",
            "let draw = b::own_query(c,",
            "status_header: Some(status_header)",
            "b::build(",
            "c_browse_header(",
            "b::present_when(",
        ],
    );
    let winui = bern_backend_winui3::Generator::new(&app).interface();
    ordered(
        "WinUI page",
        declaration(
            &winui,
            "static void c_page(Parent const& p, Ctx const& c) {",
        ),
        &[
            "auto draw = B::OwnQuery(c,",
            "B::PresentWhen(p, c,",
            ", draw);",
            "auto draw = B::OwnQuery(c,",
            ".grouped = true",
            "B::Place(p, c,",
            "B::PresentWhen(p, c,",
        ],
    );
    ordered(
        "WinUI header and success builders",
        declaration(
            &winui,
            "static void c_query_body(Parent const& p, Ctx const& c) {",
        ),
        &[
            "auto statusHeader = ",
            "c_browse_header(",
            "auto draw = B::OwnQuery(c,",
            ".statusHeader = statusHeader",
            "c_browse_header(",
            "B::PresentWhen(",
        ],
    );
    let web = bern_backend_web::Generator::new(&app).generate(
        0,
        &serde_json::json!({}),
        "test.wasm",
        "app",
        "1",
    );
    ordered(
        "Web page",
        declaration(&web, "function c_page(p, ctx, a) {"),
        &[
            "const draw = B.ownQuery(ctx,",
            "B.presentWhen(p, ctx,",
            ", draw);",
            "const draw = B.ownQuery(ctx,",
            "B.presentWhen(p, ctx,",
            "F.Place(p, ctx,",
        ],
    );

    ordered(
        "Web success and header builders",
        declaration(&web, "function c_query_body(p, ctx, a) {"),
        &[
            "const draw = B.ownQuery(ctx,",
            "(p, ctx, l_rows) => {",
            "c_browse_header(",
            "}, (p, ctx) => {",
            "c_browse_header(",
            "B.presentWhen(",
        ],
    );

    let gpui = bern_backend_gpui::Generator::new(&app, "macos").generate(0, "{}");
    ordered(
        "GPUI page",
        declaration(&gpui, "pub fn c_page("),
        &[
            "v.extend(b::own_query(c, \"results\",",
            ".filter(|_| rt::truthy(",
            ".map(|bern_owned_query|",
            "bern_owned_query.present(c,",
            "b::rail(c,",
            "v.extend(b::own_query(c, \"placed_results\",",
            ".filter(|_| rt::truthy(",
            ".map(|bern_owned_query| (",
            "bern_owned_query.present(c,",
            "b::Placement::At(",
            "if let Some(child) = c_forwarded(",
            "v.push(b::reflowed(c,",
            "M_SMOOTH, child)",
        ],
    );
    ordered(
        "GPUI forwarding",
        declaration(&gpui, "pub fn c_forwarded("),
        &[
            "-> Option<b::El>",
            "if let Some(child) = c_query_body(",
            "v.push(child)",
            "(!children.is_empty()).then(",
        ],
    );
    ordered(
        "GPUI root",
        declaration(&gpui, "pub fn c_query_body("),
        &[
            "-> Option<b::El>",
            "b::own_query(c, \"root_results\",",
            ".filter(|_| rt::truthy(",
            ".map(|bern_owned_query|",
            "bern_owned_query.present_with_header(c,",
            "|c: &b::Ctx|",
            "c_browse_header(",
            "|c: &b::Ctx, data: V|",
            "with_local(\"rows\", data)",
            "c_browse_header(",
        ],
    );
    ordered(
        "GPUI window",
        declaration(&gpui, "fn window_main("),
        &["-> b::El", ".unwrap_or_else(|| b::column("],
    );
}
