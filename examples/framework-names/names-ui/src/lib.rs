//! UI of the names example: picks, shows, matches and sends values whose
//! types are named like framework types, and declares one more of them,
//! `Shape` (SwiftUI's `Shape` protocol).

use bern::prelude::*;
use names_protocol::names::{self, Color, Content, Layout};

/// A value type of the UI alone.
#[bern::value]
pub enum Shape {
    #[label(tr! { "zh-Hans" => "圆", "en" => "Circle" })]
    Circle,
    #[label(tr! { "zh-Hans" => "方", "en" => "Square" })]
    Square,
}

/// Choices of one window.
#[bern::session]
pub struct Draft {
    pub layout: Layout,
    pub color: Color,
    pub view: names::View,
    pub alignment: names::Alignment,
    pub shape: Shape,
}

#[component]
pub fn app(app: names::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "zh-Hans" => "重名", "en" => "Names" }, initial: open, width: 640, height: 480) {
                page(app: app)
            }
        }
    }
}

#[component]
pub fn page(app: names::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Form {
            Section(title: tr! { "zh-Hans" => "选择", "en" => "Choices" }) {
                Picker(id: layout, label: tr! { "zh-Hans" => "布局", "en" => "Layout" }, value: draft.layout)
                Picker(id: color, label: tr! { "zh-Hans" => "颜色", "en" => "Color" }, value: draft.color, style: segmented)
                Picker(id: view, label: tr! { "zh-Hans" => "视图", "en" => "View" }, value: draft.view, style: menu)
                Picker(id: alignment, label: tr! { "zh-Hans" => "对齐", "en" => "Alignment" }, value: draft.alignment)
                Picker(id: shape, label: tr! { "zh-Hans" => "形状", "en" => "Shape" }, value: draft.shape)
                Button(id: apply, label: tr! { "zh-Hans" => "应用布局", "en" => "Apply Layout" }) => app.set_layout(draft.layout)
                Button(id: green, label: tr! { "zh-Hans" => "涂成绿色", "en" => "Paint Green" }) => app.paint(Color::Green)
                Button(id: words, label: tr! { "zh-Hans" => "显示文字", "en" => "Show Words" }) => app.show(Content::String { value: "words" })
                Button(id: clear, label: tr! { "zh-Hans" => "清空", "en" => "Clear" }) => app.show(Content::Empty)
            }
            Section(title: tr! { "zh-Hans" => "引擎", "en" => "Engine" }) {
                Text(content: app.color)
                Text(content: app.layout)
                Text(content: app.view)
                swatch(color: app.color)
                caption(text: app.text)
                match app.layout {
                    Layout::Column => { Text(content: tr! { "zh-Hans" => "纵列", "en" => "In a column" }) }
                    Layout::Centered => { Text(content: tr! { "zh-Hans" => "居中", "en" => "Centered" }) }
                    Layout::Layers => { Text(content: tr! { "zh-Hans" => "层叠", "en" => "In layers" }) }
                }
                match app.content {
                    Content::Text { text } => { caption(text: text) }
                    Content::Image { image } => { Text(content: image.name) }
                    Content::String { value } => { Text(content: value) }
                    Content::Long { value } => { Text(content: tr! { "zh-Hans" => "整数 ${value}", "en" => "Whole number ${value}"; value: value }) }
                    _ => { Text(content: tr! { "zh-Hans" => "没有内容", "en" => "Nothing" }) }
                }
                ForEach(items: app.list.texts, key: id) |text| {
                    caption(text: text)
                }
            }
        }
    }
}

/// One line of text from the engine.
#[component]
pub fn caption(text: names::Text) -> impl View {
    ui! {
        Text(content: text.content)
    }
}

/// The name of a color.
#[component]
pub fn swatch(color: Color) -> impl View {
    ui! {
        Text(content: color)
    }
}
