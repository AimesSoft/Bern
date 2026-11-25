//! Input controls: `TextInput`, `NumberInput`, `DatePicker`, `TimePicker`,
//! `ColorPicker`, and `ShortcutInput`.

use crate::ir::{
    Align, AriaRole, Axis, Border, BoxNode, ColorValue, Corners, Cursor, Edges, IconNode, Justify,
    Layout, Length, Rgba8, SlotId, StateOverrides, SurfaceNode, TextNode, Typography, VisualStyle,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputKind {
    #[default]
    Text,
    Secure,
    Multiline,
    Search,
}

#[derive(Clone, Debug, Default)]
pub struct TextInputSpec {
    pub label: String,
    pub label_hidden: bool,
    pub kind: InputKind,
    pub height: Option<f32>,
    pub font_size: Option<f32>,
    pub bare: bool,
    pub enabled: bool,
    pub has_text: bool,
}

pub fn text_input(spec: TextInputSpec) -> SurfaceNode {
    let multiline = spec.kind == InputKind::Multiline;
    let search = spec.kind == InputKind::Search;
    let height = spec.height.unwrap_or(if search { 32.0 } else { 36.0 });
    let corner = if search { height.min(36.0) / 2.0 } else { 8.0 };
    let state = if spec.enabled { "rest" } else { "disabled" };
    let fg = ColorValue::Recipe {
        recipe: "field",
        state,
        slot: "foreground",
    };

    let mut box_node = BoxNode::new(if search { "bern-search" } else { "bern-field" })
        .with_role(if search {
            AriaRole::Searchbox
        } else {
            AriaRole::Textbox
        })
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: if multiline {
                Align::Start
            } else {
                Align::Center
            },
            gap: 6.0,
            padding: if multiline {
                Edges::xy(10.0, 8.0)
            } else {
                Edges::xy(10.0, 0.0)
            },
            width: Length::Full,
            height: if multiline {
                Length::Px(88.0)
            } else {
                Length::Auto
            },
            min_height: Some(Length::Px(height)),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: (!spec.bare).then_some(ColorValue::Recipe {
                recipe: "field",
                state,
                slot: "background",
            }),
            foreground: Some(fg),
            border: (!spec.bare).then_some(Border::all(
                1.0,
                ColorValue::Recipe {
                    recipe: "field",
                    state,
                    slot: "border",
                },
            )),
            corners: Some(Corners::all(corner)),
            opacity: (!spec.enabled).then_some(0.6),
            cursor: Some(Cursor::Text),
            typography: Some(Typography {
                size: spec.font_size.unwrap_or(14.0),
                line_height: Some(20.0),
                weight: 400,
                ..Typography::default()
            }),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        });

    if search {
        box_node = box_node.child(SurfaceNode::Icon(IconNode {
            name: "magnifyingglass",
            size: 14.0,
            color: ColorValue::Role("icon.secondary"),
        }));
    }
    box_node = box_node.child(SurfaceNode::Slot(SlotId("editor")));
    if search && spec.has_text {
        box_node = box_node.child(SurfaceNode::Slot(SlotId("clear")));
    }

    if spec.label_hidden {
        return box_node.into_node();
    }

    BoxNode::new("bern-field-group")
        .with_layout(Layout {
            direction: Axis::Vertical,
            gap: 6.0,
            width: Length::Full,
            ..Layout::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: spec.label,
            typography: Typography {
                size: 12.0,
                line_height: Some(16.0),
                weight: 500,
                ..Typography::default()
            },
            color: fg,
            lines: None,
            truncate: false,
            selectable: false,
        }))
        .child(box_node.into_node())
        .into_node()
}

fn stepper_button(part: &'static str, icon: &'static str, enabled: bool) -> SurfaceNode {
    BoxNode::new(part)
        .with_role(AriaRole::Button)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::Center,
            width: Length::Px(28.0),
            height: Length::Px(28.0),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role("surface.panel")),
            foreground: Some(ColorValue::Role("icon.primary")),
            border: Some(Border::all(1.0, ColorValue::Role("control.border"))),
            corners: Some(Corners::all(6.0)),
            opacity: (!enabled).then_some(0.45),
            cursor: Some(if enabled {
                Cursor::Pointer
            } else {
                Cursor::Default
            }),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: enabled.then(|| VisualStyle {
                background: Some(ColorValue::Role("hover.overlay")),
                ..VisualStyle::default()
            }),
            ..StateOverrides::default()
        })
        .child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: 12.0,
            color: ColorValue::Role("icon.primary"),
        }))
        .into_node()
}

fn labeled_row(label: String, label_hidden: bool, control: SurfaceNode) -> SurfaceNode {
    if label_hidden {
        return control;
    }
    BoxNode::new("bern-row")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::SpaceBetween,
            gap: 12.0,
            width: Length::Full,
            ..Layout::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: label,
            typography: Typography {
                size: 13.5,
                line_height: Some(20.0),
                weight: 400,
                ..Typography::default()
            },
            color: ColorValue::Role("text.primary"),
            lines: None,
            truncate: false,
            selectable: false,
        }))
        .child(control)
        .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct NumberInputSpec {
    pub label: String,
    pub label_hidden: bool,
    pub formatted: String,
    pub can_decrement: bool,
    pub can_increment: bool,
    pub enabled: bool,
}

pub fn number_input(spec: NumberInputSpec) -> SurfaceNode {
    let readout = BoxNode::new("bern-number-input")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::End,
            min_width: Some(Length::Px(72.0)),
            height: Length::Px(28.0),
            padding: Edges::xy(10.0, 0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role("surface.raised")),
            foreground: Some(ColorValue::Role(if spec.enabled {
                "text.primary"
            } else {
                "disabled.foreground"
            })),
            border: Some(Border::all(1.0, ColorValue::Role("control.border"))),
            corners: Some(Corners::all(6.0)),
            typography: Some(Typography {
                size: 13.0,
                line_height: Some(18.0),
                weight: 400,
                tabular_nums: true,
                ..Typography::default()
            }),
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: spec.formatted,
            typography: Typography {
                size: 13.0,
                line_height: Some(18.0),
                weight: 400,
                tabular_nums: true,
                ..Typography::default()
            },
            color: ColorValue::Role(if spec.enabled {
                "text.primary"
            } else {
                "disabled.foreground"
            }),
            lines: Some(1),
            truncate: false,
            selectable: false,
        }))
        .into_node();

    let group = BoxNode::new("bern-number-group")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 4.0,
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .child(stepper_button(
            "bern-number-step",
            "minus",
            spec.can_decrement,
        ))
        .child(readout)
        .child(stepper_button(
            "bern-number-step",
            "plus",
            spec.can_increment,
        ))
        .into_node();

    labeled_row(spec.label, spec.label_hidden, group)
}

#[derive(Clone, Debug, Default)]
pub struct DatePickerSpec {
    pub label: String,
    pub label_hidden: bool,
    pub formatted: String,
    pub enabled: bool,
}

pub fn date_picker(spec: DatePickerSpec) -> SurfaceNode {
    let readout = BoxNode::new("bern-date-input")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 6.0,
            height: Length::Px(28.0),
            padding: Edges::xy(10.0, 0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role("surface.raised")),
            foreground: Some(ColorValue::Role("text.primary")),
            border: Some(Border::all(1.0, ColorValue::Role("control.border"))),
            corners: Some(Corners::all(6.0)),
            typography: Some(Typography {
                size: 13.0,
                line_height: Some(18.0),
                weight: 400,
                tabular_nums: true,
                ..Typography::default()
            }),
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Icon(IconNode {
            name: "calendar",
            size: 13.0,
            color: ColorValue::Role("icon.secondary"),
        }))
        .child(SurfaceNode::Text(TextNode {
            content: spec.formatted,
            typography: Typography {
                size: 13.0,
                line_height: Some(18.0),
                weight: 400,
                tabular_nums: true,
                ..Typography::default()
            },
            color: ColorValue::Role("text.primary"),
            lines: Some(1),
            truncate: false,
            selectable: false,
        }))
        .into_node();

    let group = BoxNode::new("bern-date-group")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 4.0,
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .child(stepper_button(
            "bern-number-step",
            "chevron.left",
            spec.enabled,
        ))
        .child(readout)
        .child(stepper_button(
            "bern-number-step",
            "chevron.right",
            spec.enabled,
        ))
        .into_node();

    labeled_row(spec.label, spec.label_hidden, group)
}

#[derive(Clone, Debug, Default)]
pub struct TimePickerSpec {
    pub label: String,
    pub label_hidden: bool,
    pub formatted: String,
    pub enabled: bool,
}

pub fn time_picker(spec: TimePickerSpec) -> SurfaceNode {
    let readout = BoxNode::new("bern-time-input")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::Center,
            min_width: Some(Length::Px(68.0)),
            height: Length::Px(28.0),
            padding: Edges::xy(10.0, 0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role("surface.raised")),
            foreground: Some(ColorValue::Role("text.primary")),
            border: Some(Border::all(1.0, ColorValue::Role("control.border"))),
            corners: Some(Corners::all(6.0)),
            typography: Some(Typography {
                size: 13.0,
                line_height: Some(18.0),
                weight: 400,
                tabular_nums: true,
                ..Typography::default()
            }),
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: spec.formatted,
            typography: Typography {
                size: 13.0,
                line_height: Some(18.0),
                weight: 400,
                tabular_nums: true,
                ..Typography::default()
            },
            color: ColorValue::Role("text.primary"),
            lines: Some(1),
            truncate: false,
            selectable: false,
        }))
        .into_node();

    let group = BoxNode::new("bern-time-group")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 4.0,
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .child(stepper_button("bern-number-step", "minus", spec.enabled))
        .child(readout)
        .child(stepper_button("bern-number-step", "plus", spec.enabled))
        .into_node();

    labeled_row(spec.label, spec.label_hidden, group)
}

#[derive(Clone, Debug)]
pub struct ColorPickerSpec {
    pub label: String,
    pub label_hidden: bool,
    pub rgb: u32,
    pub enabled: bool,
}

impl Default for ColorPickerSpec {
    fn default() -> Self {
        Self {
            label: String::new(),
            label_hidden: false,
            rgb: 0x2f6fe4,
            enabled: true,
        }
    }
}

pub fn color_picker(spec: ColorPickerSpec) -> SurfaceNode {
    let color = Rgba8::new(
        ((spec.rgb >> 16) & 0xff) as u8,
        ((spec.rgb >> 8) & 0xff) as u8,
        (spec.rgb & 0xff) as u8,
        255,
    );
    let swatch = BoxNode::new("bern-color-picker")
        .with_role(AriaRole::Button)
        .with_layout(Layout {
            width: Length::Px(40.0),
            height: Length::Px(24.0),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Rgba(color)),
            border: Some(Border::all(
                1.0,
                ColorValue::Recipe {
                    recipe: "button.standard",
                    state: "rest",
                    slot: "border",
                },
            )),
            corners: Some(Corners::all(6.0)),
            opacity: (!spec.enabled).then_some(0.5),
            cursor: Some(if spec.enabled {
                Cursor::Pointer
            } else {
                Cursor::Default
            }),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        })
        .into_node();

    labeled_row(spec.label, spec.label_hidden, swatch)
}

#[derive(Clone, Debug, Default)]
pub struct ShortcutInputSpec {
    pub label: String,
    pub label_hidden: bool,
    pub shown: String,
    pub recording: bool,
    pub enabled: bool,
}

pub fn shortcut_input(spec: ShortcutInputSpec) -> SurfaceNode {
    let border = if spec.recording {
        ColorValue::Role("accent")
    } else {
        ColorValue::Recipe {
            recipe: "field",
            state: "rest",
            slot: "border",
        }
    };
    let recorder = BoxNode::new("bern-shortcut-input")
        .with_role(AriaRole::Button)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::Center,
            height: Length::Px(32.0),
            min_width: Some(Length::Px(96.0)),
            padding: Edges::xy(12.0, 0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Recipe {
                recipe: "field",
                state: "rest",
                slot: "background",
            }),
            foreground: Some(ColorValue::Recipe {
                recipe: "field",
                state: "rest",
                slot: "foreground",
            }),
            border: Some(Border::all(1.0, border)),
            corners: Some(Corners::all(8.0)),
            opacity: (!spec.enabled).then_some(0.5),
            cursor: Some(if spec.enabled {
                Cursor::Pointer
            } else {
                Cursor::Default
            }),
            typography: Some(Typography {
                size: 13.0,
                line_height: Some(18.0),
                weight: 500,
                ..Typography::default()
            }),
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: spec.shown,
            typography: Typography {
                size: 13.0,
                line_height: Some(18.0),
                weight: 500,
                ..Typography::default()
            },
            color: ColorValue::Recipe {
                recipe: "field",
                state: "rest",
                slot: "foreground",
            },
            lines: Some(1),
            truncate: false,
            selectable: false,
        }))
        .into_node();

    labeled_row(spec.label, spec.label_hidden, recorder)
}
