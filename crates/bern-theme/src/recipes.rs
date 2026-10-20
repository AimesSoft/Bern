//! Control recipes: which token fills each slot of a control in each
//! state. Recipes name roles, never colors, so one table serves every mode,
//! seed and contrast policy. `transparent` means the slot is not painted.

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
        // The label identifies the button; its outline is decorative.
        states: &[
            state(
                "rest",
                &[
                    ("background", "surface.canvas"),
                    ("foreground", "text.primary"),
                    ("border", "separator"),
                ],
            ),
            state(
                "hover",
                &[
                    ("background", "button.standard.hover"),
                    ("foreground", "text.primary"),
                    ("border", "separator"),
                ],
            ),
            state(
                "pressed",
                &[
                    ("background", "button.standard.pressed"),
                    ("foreground", "text.primary"),
                    ("border", "separator"),
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
                &[("background", "transparent"), ("foreground", "accent")],
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
        // An outlined track when off, a filled one when on.
        states: &[
            state(
                "off",
                &[
                    ("track", "transparent"),
                    ("border", "control.border"),
                    ("thumb", "control.border"),
                ],
            ),
            state(
                "off.hover",
                &[
                    ("track", "transparent"),
                    ("border", "text.secondary"),
                    ("thumb", "text.secondary"),
                ],
            ),
            state(
                "on",
                &[
                    ("track", "accent"),
                    ("border", "transparent"),
                    ("thumb", "on_accent"),
                ],
            ),
            state(
                "on.hover",
                &[
                    ("track", "button.prominent.hover"),
                    ("border", "transparent"),
                    ("thumb", "on_accent"),
                ],
            ),
            FOCUS,
            state(
                "disabled",
                &[
                    ("track", "transparent"),
                    ("border", "disabled.foreground"),
                    ("thumb", "disabled.foreground"),
                ],
            ),
        ],
    },
    Recipe {
        name: "slider",
        // The thumb is an accent dot inside a ring of the thumb color.
        states: &[
            state(
                "rest",
                &[
                    ("fill", "accent"),
                    ("track", "control.track"),
                    ("thumb", "control.thumb"),
                    ("thumb_border", "control.border"),
                    ("thumb_center", "accent"),
                ],
            ),
            state(
                "hover",
                &[
                    ("fill", "accent"),
                    ("track", "control.track"),
                    ("thumb", "control.thumb"),
                    ("thumb_border", "control.border"),
                    ("thumb_center", "button.prominent.hover"),
                ],
            ),
            state(
                "active",
                &[
                    ("fill", "accent"),
                    ("track", "control.track"),
                    ("thumb", "control.thumb"),
                    ("thumb_border", "control.border"),
                    ("thumb_center", "button.prominent.pressed"),
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
                    ("thumb_center", "disabled.foreground"),
                ],
            ),
        ],
    },
    Recipe {
        name: "media_scrubber",
        // Neutral progress, so it does not compete with the picture.
        states: &[
            state(
                "rest",
                &[
                    ("fill", "media.played"),
                    ("track", "media.track"),
                    ("thumb", "control.thumb"),
                    ("thumb_border", "control.border"),
                    ("shadow", "scrim"),
                ],
            ),
            FOCUS,
            state(
                "disabled",
                &[
                    ("fill", "disabled.foreground"),
                    ("track", "media.track"),
                    ("thumb", "disabled.surface"),
                    ("thumb_border", "disabled.foreground"),
                    ("shadow", "transparent"),
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
                    ("background", "surface.canvas"),
                    ("foreground", "text.primary"),
                    ("border", "separator"),
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
        name: "chip",
        // A pill per option; the chosen one is inverted, as text on the
        // page would be.
        states: &[
            state(
                "rest",
                &[
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
                    ("background", "text.primary"),
                    ("foreground", "surface.canvas"),
                    ("border", "transparent"),
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
        name: "nav_item",
        // A selected item is filled with the accent, its text and icon in
        // `on_accent`; there is no indicator bar.
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
                &[("background", "accent"), ("foreground", "on_accent")],
            ),
            state(
                "selected.hover",
                &[
                    ("background", "button.prominent.hover"),
                    ("foreground", "on_accent"),
                ],
            ),
            FOCUS,
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
                    ("background", "transparent"),
                    ("foreground", "text.primary"),
                ],
            ),
            state(
                "selected",
                &[
                    ("background", "transparent"),
                    ("foreground", "accent.text"),
                    ("indicator", "accent"),
                ],
            ),
            // Tabs set vertically, as a wide dialog's navigation, select
            // as navigation items do.
            state(
                "selected.vertical",
                &[("background", "accent"), ("foreground", "on_accent")],
            ),
            FOCUS,
        ],
    },
    Recipe {
        name: "menu",
        // A popup list; the trigger uses `field`.
        states: &[
            state(
                "popup",
                &[
                    ("background", "surface.raised"),
                    ("border", "separator"),
                    ("shadow", "scrim"),
                ],
            ),
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
                "selected",
                &[
                    ("background", "transparent"),
                    ("foreground", "text.primary"),
                    ("check", "accent.text"),
                ],
            ),
            FOCUS,
        ],
    },
    Recipe {
        name: "dialog",
        states: &[state(
            "open",
            &[
                ("background", "surface.raised"),
                ("foreground", "text.primary"),
                ("border", "separator"),
                ("scrim", "scrim"),
            ],
        )],
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
