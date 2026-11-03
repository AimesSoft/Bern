//! UI of the catalog: a long list and a grid of cards in one scroll view,
//! beside lines that follow a current one as lyrics do, over a picture that
//! covers the window.

use bern::prelude::*;
use catalog_protocol::catalog;

#[bern::session]
pub struct CatalogDraft {
    /// The row the list stands at: setting it scrolls there, built or not.
    #[initial(1)]
    pub at: i64,
    /// The line the lyrics-like list stands at.
    #[initial(1)]
    pub line: i64,
    pub browsing: bool,
}

#[component]
pub fn app(app: catalog::Client, windows: AppWindows, #[session] draft: CatalogDraft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "zh-Hans" => "目录", "en" => "Catalog" }, initial: open, width: 900, height: 640) {
                Stack {
                    // Covers the window whatever its shape, under the rest.
                    Image(source: "media/backdrop.jpg", fit: cover, blur: 40)
                    Canvas {
                        Shape(kind: rect, fill: surface.canvas, opacity: 0.55)
                    }
                    Row(gap: 0) {
                        lines(app: app)
                        Column {
                            ScrollView(position: draft.at) {
                                Column(gap: 12, padding: 24) {
                                    Text(content: tr! { "zh-Hans" => "目录", "en" => "Catalog" }, style: large_title)
                                    Text(content: tr! { "zh-Hans" => "{count} 行", "en" => "{count} rows"; count: app.count }, style: subheadline, emphasis: secondary)
                                    Row(gap: 8, align: center) {
                                        Button(id: grow, label: tr! { "zh-Hans" => "再加一千行", "en" => "1,000 More" }) => app.grow()
                                        Button(id: rename, label: tr! { "zh-Hans" => "改名前十行", "en" => "Rename the First Ten" }) => app.rename()
                                        Button(id: trim, label: tr! { "zh-Hans" => "删掉前十行", "en" => "Remove the First Ten" }) => app.trim()
                                        Button(id: jump, label: tr! { "zh-Hans" => "到第 4000 行", "en" => "To Row 4,000" }) => draft.at.set(4000)
                                        Spacer
                                        Text(content: app.opened, style: caption, emphasis: secondary)
                                    }
                                    // Thousands of rows: only those in sight are built.
                                    Column(gap: 2) {
                                        ForEach(items: app.entries, key: id) |entry| {
                                            Button(id: entry, label: entry.title, disappear: fade) {
                                                Row(gap: 12, padding_x: 8, padding_y: 6, align: center) {
                                                    Text(content: tr! { "zh-Hans" => "{n}", "en" => "{n}"; n: entry.number }, style: caption, emphasis: tertiary)
                                                    Column(gap: 2, align: leading) {
                                                        Text(content: entry.title, lines: 1)
                                                        Text(content: entry.detail, style: caption, emphasis: secondary, lines: 1)
                                                    }
                                                    Spacer
                                                }
                                                ContextMenu {
                                                    MenuItem(id: entry_open, label: tr! { "zh-Hans" => "打开", "en" => "Open" }) => app.open(entry.id)
                                                    MenuItem(id: entry_remove, label: tr! { "zh-Hans" => "删除", "en" => "Remove" }, role: destructive) => app.remove(entry.id)
                                                }
                                            } => app.open(entry.id)
                                        }
                                    }
                                    Text(content: tr! { "zh-Hans" => "卡片", "en" => "Cards" }, style: title)
                                    // Several columns, cards of different heights.
                                    Grid(min_width: 180, gap: 12) {
                                        ForEach(items: app.cards, key: id) |card| {
                                            Column(gap: 6, align: leading, glow: 1.05, glow_opacity: 0.3) {
                                                Canvas(height: 80) {
                                                    Shape(kind: rect, fill: accent, opacity: 0.6)
                                                }
                                                Text(content: card.title, style: headline)
                                                Text(content: card.detail, style: caption, emphasis: secondary, lines: 3)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Lines with the current one a third of the way down, larger and bold, the
/// others blurred by distance; scrolled by hand, none is.
#[component]
pub fn lines(app: catalog::Client, #[session] draft: CatalogDraft) -> impl View {
    ui! {
        Column(width: 320, gap: 0) {
            Row(gap: 8, padding_x: 16, padding_y: 12, align: center) {
                Button(id: step, label: tr! { "zh-Hans" => "下一行", "en" => "Next Line" }) => app.step()
                Spacer
                if app.browsing {
                    Text(content: tr! { "zh-Hans" => "翻看中", "en" => "Browsing" }, style: caption, emphasis: secondary)
                }
            }
            ScrollView(position: draft.line, anchor: 0.35, browsing: draft.browsing, indicators: hidden) {
                Column(gap: 2, padding_x: 8, padding_y: 24) {
                    ForEach(items: app.lines, key: key) |line| {
                        Column(gap: 0, padding_x: 12, padding_y: 8, blur: line.blur, animate: smooth) {
                            Text(content: line.text, style: title3, font_size: if line.current { 24 } else { 19 }, bold: line.current, animate: smooth)
                        }
                    }
                    Column(height: 300) {
                        Spacer
                    }
                }
            }
            Watch(value: app.scroll_tick) => draft.line.set(app.scroll_to)
            Watch(value: draft.browsing) => app.browse(draft.browsing)
        }
    }
}
