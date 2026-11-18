use bern::prelude::*;

#[bern::session]
pub struct Draft {
    pub text: String,
    pub settings: bool,
}

#[component]
pub fn app(app: gallery::Client, windows: AppWindows) -> impl View {
    ui! { WindowSet(handle: AppWindows, primary: main) {
        Window(id: main, initial: open, title: tr! { "en" => "Search", "zh-Hans" => "搜索" }) {
            page(app: app)
        }
    } }
}

#[component]
pub fn page(app: gallery::Client, #[session] draft: Draft) -> impl View {
    ui! { Column {
        Switch(id: settings, label: tr! { "en" => "Settings", "zh-Hans" => "设置" }, value: draft.settings)
        Query(id: results, query: app.find(draft.text), presented: !draft.settings) |rows| {
            ForEach(items: rows, key: id) |item| { Text(content: item.name) }
        }
        Rail(anchor: center) {
            Query(id: placed_results, query: app.find(draft.text), at: 0.5,
                  presented: !draft.settings) |rows| {
                ForEach(items: rows, key: id) |item| { Text(content: item.name) }
            }
        }
        Row(align: baseline, gap: 8, reflow: smooth) { forwarded(app: app) }
        Accordion(count: 1, selected: 0, collapsed_width: 24) { forwarded(app: app) }
    } }
}

#[component]
pub fn forwarded(app: gallery::Client) -> impl View {
    ui! { query_body(app: app) }
}

#[component]
pub fn query_body(app: gallery::Client, #[session] draft: Draft) -> impl View {
    ui! { Query(id: root_results, query: app.find(draft.text), presented: !draft.settings) |rows| {
        QueryStatusHeader { browse_header() }
        ScrollView {
            browse_header()
            ForEach(items: rows, key: id) |item| { Text(content: item.name) }
        }
    } }
}

#[component]
pub fn browse_header(#[session] draft: Draft) -> impl View {
    ui! { Switch(id: view, label: tr! { "en" => "Settings", "zh-Hans" => "设置" }, value: draft.settings) }
}
