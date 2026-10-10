//! Control recipes: which token fills each slot of a control in each
//! state. Recipes name roles, never colors, so one table serves every mode,
//! seed and contrast policy. `transparent` means the slot is not painted;
//! an icon button paints no container in any state.

/// The slots of one control state, as `(slot, token)`.
#[derive(Debug)]
pub struct RecipeState {
    pub state: &'static str,
    pub slots: &'static [(&'static str, &'static str)],
}

#[derive(Debug)]
pub struct Recipe {
    pub name: &'static str,
    pub states: &'static [RecipeState],
}

impl Recipe {
    pub fn state(&self, name: &str) -> Option<&RecipeState> {
        self.states.iter().find(|state| state.state == name)
    }
}

const fn state(state: &'static str, slots: &'static [(&'static str, &'static str)]) -> RecipeState {
    RecipeState { state, slots }
}

/// The focus ring every interactive control shares.
const FOCUS: RecipeState = state("focus", &[("ring", "focus.indicator")]);

pub const RECIPES: &[Recipe] = &[
    Recipe {
        name: "button.standard",
        states: &[
            state(
                "rest",
                &[
                    ("background", "surface.raised"),
                    ("foreground", "text.primary"),
                    ("border", "control.border"),
                ],
            ),
            state(
                "hover",
                &[
                    ("background", "button.standard.hover"),
                    ("foreground", "text.primary"),
                    ("border", "control.border"),
                ],
            ),
            state(
                "pressed",
                &[
                    ("background", "button.standard.pressed"),
                    ("foreground", "text.primary"),
                    ("border", "control.border"),
                ],
            ),
            FOCUS,
            state(
                "disabled",
                &[
                    ("background", "disabled.surface"),
                    ("foreground", "disabled.foreground"),
                    ("border", "transparent"),
                ],
            ),
        ],
    },
    Recipe {
        name: "button.prominent",
        states: &[
            state(
                "rest",
                &[("background", "accent"), ("foreground", "on_accent")],
            ),
            state(
                "hover",
                &[
                    ("background", "button.prominent.hover"),
                    ("foreground", "on_accent"),
                ],
            ),
            state(
                "pressed",
                &[
                    ("background", "button.prominent.pressed"),
                    ("foreground", "on_accent"),
                ],
            ),
            FOCUS,
            state(
                "disabled",
                &[
                    ("background", "disabled.surface"),
                    ("foreground", "disabled.foreground"),
                ],
            ),
        ],
    },
    Recipe {
        name: "button.plain",
        states: &[
            state(
                "rest",
                &[("background", "transparent"), ("foreground", "accent.text")],
            ),
            state(
                "hover",
                &[
                    ("background", "hover.overlay"),
                    ("foreground", "accent.text"),
                ],
            ),
            state(
                "pressed",
                &[
                    ("background", "pressed.overlay"),
                    ("foreground", "accent.text"),
                ],
            ),
            FOCUS,
            state(
                "disabled",
                &[
                    ("background", "transparent"),
                    ("foreground", "disabled.foreground"),
                ],
            ),
        ],
    },
    Recipe {
        name: "button.destructive",
        states: &[
            state(
                "rest",
                &[
                    ("background", "status.error.surface"),
                    ("foreground", "on_error"),
                ],
            ),
            state(
                "hover",
                &[
                    ("background", "button.destructive.hover"),
                    ("foreground", "on_error"),
                ],
            ),
            state(
                "pressed",
                &[
                    ("background", "button.destructive.pressed"),
                    ("foreground", "on_error"),
                ],
            ),
            FOCUS,
            state(
                "disabled",
                &[
                    ("background", "disabled.surface"),
                    ("foreground", "disabled.foreground"),
                ],
            ),
        ],
    },
    Recipe {
        name: "icon_button",
        states: &[
            state(
                "rest",
                &[
                    ("background", "transparent"),
                    ("foreground", "icon.secondary"),
                ],
            ),
            state(
                "hover",
                &[
                    ("background", "transparent"),
                    ("foreground", "icon.primary"),
                ],
            ),
            state(
                "pressed",
                &[("background", "transparent"), ("foreground", "accent.text")],
            ),
            state(
                "selected",
                &[("background", "transparent"), ("foreground", "accent.text")],
            ),
            FOCUS,
            state(
                "disabled",
                &[
                    ("background", "transparent"),
                    ("foreground", "disabled.foreground"),
                ],
            ),
        ],
    },
    Recipe {
        name: "field",
        states: &[
            state(
                "rest",
                &[
                    ("background", "surface.raised"),
                    ("foreground", "text.primary"),
                    ("placeholder", "text.placeholder"),
                    ("border", "control.border"),
                ],
            ),
            state(
                "focus",
                &[
                    ("background", "surface.raised"),
                    ("foreground", "text.primary"),
                    ("border", "accent"),
                    ("ring", "focus.indicator"),
                ],
            ),
            state(
                "error",
                &[
                    ("background", "surface.raised"),
                    ("foreground", "text.primary"),
                    ("border", "status.error"),
                ],
            ),
            state(
                "disabled",
                &[
                    ("background", "disabled.surface"),
                    ("foreground", "disabled.foreground"),
                    ("border", "transparent"),
                ],
            ),
        ],
    },
    Recipe {
        name: "switch",
        states: &[
            state(
                "off",
                &[("track", "control.track"), ("thumb", "control.thumb")],
            ),
            state("on", &[("track", "accent"), ("thumb", "control.thumb")]),
            FOCUS,
            state(
                "disabled",
                &[
                    ("track", "disabled.surface"),
                    ("thumb", "disabled.foreground"),
                ],
            ),
        ],
    },
    Recipe {
        name: "slider",
        states: &[
            state(
                "rest",
                &[
                    ("fill", "accent"),
                    ("track", "control.track"),
                    ("thumb", "control.thumb"),
                    ("thumb_border", "control.border"),
                ],
            ),
            FOCUS,
            state(
                "disabled",
                &[
                    ("fill", "disabled.foreground"),
                    ("track", "disabled.surface"),
                    ("thumb", "disabled.surface"),
                    ("thumb_border", "disabled.foreground"),
                ],
            ),
        ],
    },
    Recipe {
        name: "segmented",
        states: &[
            state(
                "rest",
                &[
                    ("track", "surface.panel"),
                    ("background", "transparent"),
                    ("foreground", "text.secondary"),
                    ("border", "control.border"),
                ],
            ),
            state(
                "hover",
                &[
                    ("background", "hover.overlay"),
                    ("foreground", "text.primary"),
                ],
            ),
            state(
                "selected",
                &[
                    ("background", "surface.raised"),
                    ("foreground", "text.primary"),
                    ("border", "control.border"),
                ],
            ),
            FOCUS,
            state(
                "disabled",
                &[
                    ("background", "transparent"),
                    ("foreground", "disabled.foreground"),
                ],
            ),
        ],
    },
    Recipe {
        name: "row",
        states: &[
            state(
                "rest",
                &[
                    ("background", "transparent"),
                    ("foreground", "text.primary"),
                ],
            ),
            state(
                "hover",
                &[
                    ("background", "hover.overlay"),
                    ("foreground", "text.primary"),
                ],
            ),
            state(
                "pressed",
                &[
                    ("background", "pressed.overlay"),
                    ("foreground", "text.primary"),
                ],
            ),
            state(
                "selected",
                &[
                    ("background", "selection.background"),
                    ("foreground", "selection.foreground"),
                ],
            ),
            FOCUS,
        ],
    },
    Recipe {
        name: "tab",
        states: &[
            state(
                "rest",
                &[
                    ("background", "transparent"),
                    ("foreground", "text.secondary"),
                ],
            ),
            state(
                "hover",
                &[
                    ("background", "hover.overlay"),
                    ("foreground", "text.primary"),
                ],
            ),
            state(
                "selected",
                &[
                    ("background", "hover.overlay"),
                    ("foreground", "accent.text"),
                ],
            ),
            FOCUS,
        ],
    },
    Recipe {
        name: "window.chrome",
        states: &[
            state(
                "active",
                &[
                    ("background", "surface.raised"),
                    ("foreground", "text.primary"),
                    ("border", "separator"),
                ],
            ),
            state(
                "inactive",
                &[
                    ("background", "surface.raised"),
                    ("foreground", "text.secondary"),
                    ("border", "separator"),
                ],
            ),
        ],
    },
];
