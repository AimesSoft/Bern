//! Built-in component specifications.
//!
//! This table is the single definition of which components exist, which
//! properties they accept and how their children and actions are shaped.
//! The macros, the compiler and every target read it, so a component is
//! described once and never inferred from one backend's behaviour.

/// How a property value is written and checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    /// Stable control identity: an identifier or a text literal.
    Id,
    /// One of a fixed set of keywords.
    Keyword(&'static [&'static str]),
    /// `tr!` text or a displayable binding (text, int, float, labelled enum).
    DisplayText,
    /// Only `tr!` text; used where data would make no sense (window titles).
    LocalizedText,
    /// Icon name literal from the Bern icon set (SF Symbols naming).
    Icon,
    /// Numeric literal in logical units.
    Number,
    /// Boolean restricted expression.
    Bool,
    /// Writable session text field.
    TextField,
    /// Writable session bool field.
    BoolField,
    /// Writable session float field.
    NumberField,
    /// Writable session enum field whose variants all carry labels.
    PickerField,
    /// Writable session enum field selecting among child items.
    SelectionField,
    /// Value matching the enclosing selection field's enum.
    SelectionValue,
    /// List binding of records feeding a row template.
    Items,
    /// Field name of the row record used as stable key.
    KeyField,
    /// Writable session list of a route enum.
    PathField,
    /// Route value for the enclosing navigation path.
    Route,
    /// Window kind identifier.
    WindowId,
    /// Name of the generated window handle type.
    TypeName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropSpec {
    pub name: &'static str,
    pub kind: PropKind,
    pub required: bool,
}

/// What kind of children a component accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Children {
    None,
    /// Any nodes.
    Any,
    /// Exactly one node.
    One,
    /// Only the listed components (conditional nodes are not allowed).
    Only(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionRule {
    None,
    Required,
    Optional,
}

/// Where a component may appear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Anywhere,
    /// Only as the root of a component body.
    Root,
    /// Only directly inside one of the listed parents.
    Within(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentSpec {
    pub name: &'static str,
    pub props: &'static [PropSpec],
    pub children: Children,
    pub template: bool,
    pub action: ActionRule,
    pub placement: Placement,
    /// Whether the component is an operable control that needs an `id`.
    pub interactive: bool,
    pub summary: &'static str,
}

impl ComponentSpec {
    pub fn prop(&self, name: &str) -> Option<&'static PropSpec> {
        self.props.iter().find(|prop| prop.name == name)
    }
}

const fn req(name: &'static str, kind: PropKind) -> PropSpec {
    PropSpec {
        name,
        kind,
        required: true,
    }
}

const fn opt(name: &'static str, kind: PropKind) -> PropSpec {
    PropSpec {
        name,
        kind,
        required: false,
    }
}

pub const TEXT_STYLES: &[&str] = &[
    "large_title",
    "title",
    "title2",
    "title3",
    "headline",
    "subheadline",
    "body",
    "callout",
    "footnote",
    "caption",
    "caption2",
];

pub const EMPHASIS: &[&str] = &["primary", "secondary", "tertiary"];

const fn spec(
    name: &'static str,
    props: &'static [PropSpec],
    children: Children,
    summary: &'static str,
) -> ComponentSpec {
    ComponentSpec {
        name,
        props,
        children,
        template: false,
        action: ActionRule::None,
        placement: Placement::Anywhere,
        interactive: false,
        summary,
    }
}

pub const COMPONENTS: &[ComponentSpec] = &[
    ComponentSpec {
        placement: Placement::Root,
        ..spec(
            "WindowSet",
            &[
                req("handle", PropKind::TypeName),
                req("primary", PropKind::WindowId),
            ],
            Children::Only(&["Window"]),
            "Window catalog of a session root",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["WindowSet"]),
        ..spec(
            "Window",
            &[
                req("id", PropKind::WindowId),
                req("title", PropKind::LocalizedText),
                opt("initial", PropKind::Keyword(&["open", "closed"])),
                opt(
                    "presentation",
                    PropKind::Keyword(&["auto", "virtual", "native_required"]),
                ),
                opt("role", PropKind::Keyword(&["standard", "settings"])),
                opt("width", PropKind::Number),
                opt("height", PropKind::Number),
                opt("min_width", PropKind::Number),
                opt("min_height", PropKind::Number),
            ],
            Children::One,
            "Logical window with its own identity and state",
        )
    },
    spec(
        "Column",
        &[
            opt("gap", PropKind::Number),
            opt("padding", PropKind::Number),
            opt(
                "align",
                PropKind::Keyword(&["leading", "center", "trailing"]),
            ),
        ],
        Children::Any,
        "Vertical arrangement",
    ),
    spec(
        "Row",
        &[
            opt("gap", PropKind::Number),
            opt("padding", PropKind::Number),
            opt("align", PropKind::Keyword(&["top", "center", "bottom"])),
        ],
        Children::Any,
        "Horizontal arrangement",
    ),
    spec("Spacer", &[], Children::None, "Flexible space"),
    spec("Divider", &[], Children::None, "Separator line"),
    spec(
        "ScrollView",
        &[opt("axis", PropKind::Keyword(&["vertical", "horizontal"]))],
        Children::Any,
        "Scrolling container",
    ),
    spec(
        "SplitView",
        &[],
        Children::Only(&["Sidebar", "Detail"]),
        "Sidebar and detail columns",
    ),
    ComponentSpec {
        placement: Placement::Within(&["SplitView"]),
        ..spec(
            "Sidebar",
            &[
                req("selection", PropKind::SelectionField),
                opt("id", PropKind::Id),
            ],
            Children::Only(&["Search", "Section", "SidebarItem"]),
            "Sidebar list selecting a destination",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["SplitView"]),
        ..spec("Detail", &[], Children::Any, "Detail column")
    },
    ComponentSpec {
        placement: Placement::Within(&["Sidebar", "Section"]),
        ..spec(
            "SidebarItem",
            &[
                req("value", PropKind::SelectionValue),
                req("label", PropKind::DisplayText),
                opt("icon", PropKind::Icon),
            ],
            Children::None,
            "Selectable sidebar destination",
        )
    },
    spec(
        "Section",
        &[
            opt("title", PropKind::DisplayText),
            opt("footer", PropKind::DisplayText),
        ],
        Children::Any,
        "Titled group inside a form, list or sidebar",
    ),
    spec(
        "Form",
        &[opt("title", PropKind::DisplayText)],
        Children::Any,
        "Grouped form of labelled controls, optionally titling its page",
    ),
    spec(
        "Navigation",
        &[req("path", PropKind::PathField), opt("id", PropKind::Id)],
        Children::Any,
        "Push navigation bound to a typed route path",
    ),
    ComponentSpec {
        template: true,
        placement: Placement::Within(&["Navigation"]),
        ..spec(
            "Destination",
            &[],
            Children::Any,
            "Content shown for a pushed route",
        )
    },
    ComponentSpec {
        interactive: true,
        ..spec(
            "NavLink",
            &[req("id", PropKind::Id), req("route", PropKind::Route)],
            Children::Any,
            "Row that pushes a route",
        )
    },
    spec(
        "Tabs",
        &[
            req("selection", PropKind::SelectionField),
            opt("id", PropKind::Id),
        ],
        Children::Only(&["Tab"]),
        "Sibling pages switched by tabs",
    ),
    ComponentSpec {
        placement: Placement::Within(&["Tabs"]),
        ..spec(
            "Tab",
            &[
                req("value", PropKind::SelectionValue),
                req("label", PropKind::DisplayText),
                opt("icon", PropKind::Icon),
            ],
            Children::Any,
            "One tab page",
        )
    },
    spec(
        "Toolbar",
        &[],
        Children::Only(&["Button", "IconButton", "Picker"]),
        "Window or page level actions",
    ),
    ComponentSpec {
        template: true,
        ..spec(
            "List",
            &[
                opt("items", PropKind::Items),
                opt("key", PropKind::KeyField),
            ],
            Children::Any,
            "List of rows, static or keyed by data",
        )
    },
    ComponentSpec {
        template: true,
        ..spec(
            "ForEach",
            &[
                req("items", PropKind::Items),
                req("key", PropKind::KeyField),
            ],
            Children::Any,
            "Keyed repetition inside any container",
        )
    },
    spec(
        "Text",
        &[
            req("content", PropKind::DisplayText),
            opt("style", PropKind::Keyword(TEXT_STYLES)),
            opt("emphasis", PropKind::Keyword(EMPHASIS)),
        ],
        Children::None,
        "Text with a semantic style",
    ),
    spec(
        "Label",
        &[
            req("title", PropKind::DisplayText),
            req("icon", PropKind::Icon),
            opt("style", PropKind::Keyword(TEXT_STYLES)),
        ],
        Children::None,
        "Icon and title",
    ),
    ComponentSpec {
        action: ActionRule::Required,
        interactive: true,
        ..spec(
            "Button",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("icon", PropKind::Icon),
                opt(
                    "role",
                    PropKind::Keyword(&["normal", "destructive", "cancel"]),
                ),
                opt(
                    "prominence",
                    PropKind::Keyword(&["standard", "prominent", "plain"]),
                ),
                opt("enabled", PropKind::Bool),
            ],
            Children::None,
            "Button bound to a registered command",
        )
    },
    ComponentSpec {
        action: ActionRule::Required,
        interactive: true,
        ..spec(
            "IconButton",
            &[
                req("id", PropKind::Id),
                req("icon", PropKind::Icon),
                req("label", PropKind::DisplayText),
                opt("enabled", PropKind::Bool),
            ],
            Children::None,
            "Icon-only button with an accessible name",
        )
    },
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "TextInput",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                req("value", PropKind::TextField),
                opt("prompt", PropKind::DisplayText),
                opt("kind", PropKind::Keyword(&["plain", "secure", "multiline"])),
            ],
            Children::None,
            "Text editing bound to a session draft; the action runs on submit",
        )
    },
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "Search",
            &[
                req("id", PropKind::Id),
                req("value", PropKind::TextField),
                opt("prompt", PropKind::DisplayText),
            ],
            Children::None,
            "Search text bound to a session draft; the action runs on submit",
        )
    },
    ComponentSpec {
        interactive: true,
        ..spec(
            "Switch",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                req("value", PropKind::BoolField),
                opt("style", PropKind::Keyword(&["switch", "checkbox"])),
            ],
            Children::None,
            "Boolean state",
        )
    },
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "Slider",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                req("value", PropKind::NumberField),
                req("min", PropKind::Number),
                req("max", PropKind::Number),
                opt("step", PropKind::Number),
                opt("min_icon", PropKind::Icon),
                opt("max_icon", PropKind::Icon),
            ],
            Children::None,
            "Range value previewed in the session; the action runs on commit",
        )
    },
    ComponentSpec {
        interactive: true,
        ..spec(
            "Picker",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                req("value", PropKind::PickerField),
                opt(
                    "style",
                    PropKind::Keyword(&["automatic", "menu", "segmented", "radio", "inline"]),
                ),
            ],
            Children::None,
            "Single choice among the labelled variants of an enum",
        )
    },
];

pub fn component(name: &str) -> Option<&'static ComponentSpec> {
    COMPONENTS.iter().find(|spec| spec.name == name)
}

/// One structural problem found by [`check_shape`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeError {
    /// Property the problem refers to, if any.
    pub prop: Option<String>,
    pub message: String,
}

/// The syntactic outline of an element, as seen by any parser.
pub struct ElementShape<'a> {
    pub name: &'a str,
    pub props: Vec<&'a str>,
    pub has_template: bool,
    pub children: Option<Vec<ChildShape<'a>>>,
    pub has_action: bool,
    pub parent: Option<&'a str>,
    pub is_root: bool,
}

pub enum ChildShape<'a> {
    Element(&'a str),
    Other,
}

/// Validate an element against its specification. Shared by the macros
/// (for early, span-accurate errors) and the compiler.
pub fn check_shape(shape: &ElementShape<'_>) -> Vec<ShapeError> {
    let mut errors = Vec::new();
    let Some(spec) = component(shape.name) else {
        let known: Vec<_> = COMPONENTS.iter().map(|spec| spec.name).collect();
        errors.push(ShapeError {
            prop: None,
            message: format!(
                "unknown component `{}`; built-in components are: {}",
                shape.name,
                known.join(", ")
            ),
        });
        return errors;
    };
    for prop in &shape.props {
        if spec.prop(prop).is_none() {
            let known: Vec<_> = spec.props.iter().map(|prop| prop.name).collect();
            errors.push(ShapeError {
                prop: Some(prop.to_string()),
                message: if known.is_empty() {
                    format!("`{}` takes no properties", spec.name)
                } else {
                    format!(
                        "`{}` has no property `{prop}`; expected one of: {}",
                        spec.name,
                        known.join(", ")
                    )
                },
            });
        }
    }
    for prop in spec.props.iter().filter(|prop| prop.required) {
        if !shape.props.contains(&prop.name) {
            errors.push(ShapeError {
                prop: None,
                message: format!("`{}` requires the property `{}`", spec.name, prop.name),
            });
        }
    }
    let list_needs_template = spec.name == "List" && shape.props.contains(&"items");
    if shape.has_template && !spec.template {
        errors.push(ShapeError {
            prop: None,
            message: format!("`{}` does not take a row template `|item|`", spec.name),
        });
    }
    if spec.template && !shape.has_template && (spec.name != "List" || list_needs_template) {
        errors.push(ShapeError {
            prop: None,
            message: format!("`{}` needs a template variable such as `|item|`", spec.name),
        });
    }
    if spec.name == "List" && shape.has_template && !list_needs_template {
        errors.push(ShapeError {
            prop: None,
            message: "a static `List` has no row template; add `items:` and `key:`".into(),
        });
    }
    if spec.name == "List" && shape.props.contains(&"items") != shape.props.contains(&"key") {
        errors.push(ShapeError {
            prop: None,
            message: "`List` needs both `items:` and `key:` for data rows".into(),
        });
    }
    match (spec.action, shape.has_action) {
        (ActionRule::Required, false) => errors.push(ShapeError {
            prop: None,
            message: format!(
                "error[B301]: UI action must bind a registered command; add `=> client.command()` to `{}`",
                spec.name
            ),
        }),
        (ActionRule::None, true) => errors.push(ShapeError {
            prop: None,
            message: format!("`{}` does not take an action", spec.name),
        }),
        _ => {}
    }
    let children = shape.children.as_deref().unwrap_or(&[]);
    match spec.children {
        Children::None if shape.children.is_some() => errors.push(ShapeError {
            prop: None,
            message: format!("`{}` does not take children", spec.name),
        }),
        Children::One if children.len() != 1 => errors.push(ShapeError {
            prop: None,
            message: format!("`{}` takes exactly one child", spec.name),
        }),
        Children::Only(allowed) => {
            for child in children {
                match child {
                    ChildShape::Element(name) if allowed.contains(name) => {}
                    ChildShape::Element(name) => errors.push(ShapeError {
                        prop: None,
                        message: format!(
                            "`{}` cannot contain `{name}`; allowed children: {}",
                            spec.name,
                            allowed.join(", ")
                        ),
                    }),
                    ChildShape::Other => errors.push(ShapeError {
                        prop: None,
                        message: format!(
                            "`{}` only contains {} elements directly",
                            spec.name,
                            allowed.join(", ")
                        ),
                    }),
                }
            }
        }
        _ => {}
    }
    if spec.name == "SplitView" {
        let names: Vec<_> = children
            .iter()
            .map(|child| match child {
                ChildShape::Element(name) => *name,
                ChildShape::Other => "",
            })
            .collect();
        if names != ["Sidebar", "Detail"] {
            errors.push(ShapeError {
                prop: None,
                message: "`SplitView` contains exactly `Sidebar { … }` followed by `Detail { … }`"
                    .into(),
            });
        }
    }
    if spec.name == "Navigation" {
        let destinations = children
            .iter()
            .filter(|child| matches!(child, ChildShape::Element("Destination")))
            .count();
        if destinations != 1 {
            errors.push(ShapeError {
                prop: None,
                message: "`Navigation` contains exactly one `Destination |route| { … }`".into(),
            });
        }
    }
    match spec.placement {
        Placement::Root if !shape.is_root => errors.push(ShapeError {
            prop: None,
            message: format!("`{}` must be the root of a component", spec.name),
        }),
        Placement::Within(parents) if !shape.parent.is_some_and(|p| parents.contains(&p)) => errors
            .push(ShapeError {
                prop: None,
                message: format!(
                    "`{}` may only appear directly inside {}",
                    spec.name,
                    parents.join(" or ")
                ),
            }),
        _ => {}
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape<'a>(name: &'a str, props: Vec<&'a str>, has_action: bool) -> ElementShape<'a> {
        ElementShape {
            name,
            props,
            has_template: false,
            children: None,
            has_action,
            parent: Some("Column"),
            is_root: false,
        }
    }

    #[test]
    fn buttons_without_actions_fail_with_b301() {
        let errors = check_shape(&shape("Button", vec!["id", "label"], false));
        assert!(errors.iter().any(|error| error.message.contains("B301")));
        assert!(check_shape(&shape("Button", vec!["id", "label"], true)).is_empty());
    }

    #[test]
    fn unknown_components_and_props_are_reported() {
        assert!(!check_shape(&shape("Marquee", vec![], false)).is_empty());
        let errors = check_shape(&shape("Text", vec!["content", "color"], false));
        assert_eq!(errors[0].prop.as_deref(), Some("color"));
    }

    #[test]
    fn interactive_components_require_ids() {
        for spec in COMPONENTS.iter().filter(|spec| spec.interactive) {
            assert!(
                spec.prop("id").is_some_and(|prop| prop.required),
                "{} needs a required id",
                spec.name
            );
        }
    }

    #[test]
    fn placement_rules_apply() {
        let mut item = shape("SidebarItem", vec!["value", "label"], false);
        assert!(!check_shape(&item).is_empty());
        item.parent = Some("Section");
        assert!(check_shape(&item).is_empty());
    }
}
