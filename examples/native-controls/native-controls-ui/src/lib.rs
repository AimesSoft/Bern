//! UI declaration for the native controls showcase.
//!
//! Each `NativeView` references co-located native implementation files sitting
//! right beside this `lib.rs` file (`meter.*` and `calendar.*`), while keeping
//! all typed values, `tr!` localized strings, CLI control entries and fallback
//! controls inside the Bern declaration.

use bern::prelude::*;
use native_controls_protocol::native_controls;

#[bern::session]
pub struct HoverDraft {
    pub hover_level: Option<f64>,
    pub date_draft: String,
}

#[component]
pub fn app(
    app: native_controls::Client,
    #[session] draft: HoverDraft,
    windows: AppWindows,
) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(
                id: main,
                title: tr! { "zh-Hans" => "原生控件示例", "en" => "Native Controls" },
                initial: open,
                width: 680,
                height: 620,
            ) {
                ScrollView {
                    Column(gap: 20, padding: 24) {
                        Row(gap: 12, align: center) {
                            Text(
                                content: tr! { "zh-Hans" => "各平台原生控件", "en" => "Platform-Native Controls" },
                                style: title2,
                            )
                            Spacer
                            LanguagePicker(
                                id: language,
                                label: tr! { "zh-Hans" => "语言", "en" => "Language" },
                                label_hidden: true,
                            )
                        }

                        Row(gap: 16, align: center) {
                            Text(
                                content: tr! {
                                    "zh-Hans" => "电平 {v}",
                                    "en" => "Level {v}";
                                    v: app.level
                                },
                                style: headline,
                            )
                            Text(
                                content: tr! {
                                    "zh-Hans" => "日期 {d}",
                                    "en" => "Date {d}";
                                    d: app.selected_date
                                },
                                emphasis: secondary,
                            )
                            if let Some(at) = draft.hover_level {
                                Text(
                                    content: tr! {
                                        "zh-Hans" => "悬停 {at}",
                                        "en" => "Hover {at}";
                                        at: at
                                    },
                                    color: accent,
                                )
                            }
                        }

                        Divider

                        NativeView(
                            id: meter,
                            label: tr! { "zh-Hans" => "原生电平仪表", "en" => "Native Level Meter" },
                            swiftui: "meter.swiftui.swift",
                            winui3: "meter.winui3.cpp",
                            compose: "meter.compose.kt",
                            gtk: "meter.gtk.rs",
                            gpui: "meter.gpui.rs",
                            web: "meter.web.js",
                        ) {
                            NativeText(
                                name: heading,
                                content: tr! { "zh-Hans" => "原生电平与步进器", "en" => "Native Meter & Stepper" },
                            )
                            NativeText(
                                name: readout,
                                content: tr! {
                                    "zh-Hans" => "{v} / 10",
                                    "en" => "{v} / 10";
                                    v: app.level
                                },
                            )
                            NativeDraft(name: hover, value: draft.hover_level)
                            NativeNumber(
                                id: level,
                                label: tr! { "zh-Hans" => "电平", "en" => "Level" },
                                current: app.level,
                                min: 0,
                                max: 10,
                                step: 1,
                            ) => app.set_level(app.level)
                            NativeBool(
                                id: boosted,
                                label: tr! { "zh-Hans" => "增强模式", "en" => "Boost Mode" },
                                checked: app.boosted,
                            ) => app.set_boosted(app.boosted)
                            NativeAction(
                                id: reset,
                                label: tr! { "zh-Hans" => "重置", "en" => "Reset" },
                            ) => app.reset()
                            Fallback {
                                Column(gap: 10) {
                                    Slider(
                                        id: level_fallback,
                                        label: tr! { "zh-Hans" => "电平", "en" => "Level" },
                                        current: app.level,
                                        hover: draft.hover_level,
                                        min: 0,
                                        max: 10,
                                        step: 1,
                                    ) => app.set_level(app.level)
                                    Row(gap: 12) {
                                        Switch(
                                            id: boosted_fallback,
                                            label: tr! { "zh-Hans" => "增强模式", "en" => "Boost Mode" },
                                            checked: app.boosted,
                                        ) => app.set_boosted(app.boosted)
                                        Button(
                                            id: reset_fallback,
                                            label: tr! { "zh-Hans" => "重置", "en" => "Reset" },
                                        ) => app.reset()
                                    }
                                }
                            }
                        }

                        Divider

                        NativeView(
                            id: calendar,
                            label: tr! { "zh-Hans" => "原生日期选择器", "en" => "Native Calendar Picker" },
                            swiftui: "calendar.swiftui.swift",
                            winui3: "calendar.winui3.cpp",
                            compose: "calendar.compose.kt",
                            gtk: "calendar.gtk.rs",
                            gpui: "calendar.gpui.rs",
                            web: "calendar.web.js",
                        ) {
                            NativeText(
                                name: heading,
                                content: tr! { "zh-Hans" => "原生月历控件", "en" => "Native Calendar Control" },
                            )
                            NativeInput(
                                id: date,
                                label: tr! { "zh-Hans" => "日期", "en" => "Date" },
                                current: app.selected_date,
                            ) => app.set_date(app.selected_date)
                            NativeAction(
                                id: today,
                                label: tr! { "zh-Hans" => "回到今天", "en" => "Today" },
                            ) => app.pick_today()
                            Fallback {
                                Row(gap: 12) {
                                    TextInput(
                                        id: date_fallback,
                                        label: tr! { "zh-Hans" => "日期", "en" => "Date" },
                                        value: draft.date_draft,
                                    ) => app.set_date(draft.date_draft)
                                    Button(
                                        id: today_fallback,
                                        label: tr! { "zh-Hans" => "回到今天", "en" => "Today" },
                                    ) => app.pick_today()
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
