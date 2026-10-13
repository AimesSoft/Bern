//! Style overrides on Windows: what the target shows. The generator
//! describes each rule to the runtime as a `B::StyleDef` (see `cpp.rs`);
//! buttons take it through their lightweight styling resources, corner,
//! padding and size, sidebar items through the navigation view's, and
//! scale, offset and opacity move on the runtime's springs.

use bern_ir::style::StyleState::{Disabled, Focus, Hover, Pressed, Selected};
use bern_target_api::StyleSupport;

const BUTTON_STATES: &[&str] = &[
    "background",
    "foreground",
    "border",
    "shadow",
    "scale",
    "offset",
    "opacity",
    "system",
    "none",
];
const ROW_STATES: &[&str] = &[
    "background",
    "foreground",
    "scale",
    "offset",
    "opacity",
    "system",
    "none",
];
const BUTTON_BASE: &[&str] = &[
    "background",
    "foreground",
    "border",
    "shadow",
    "corner",
    "padding",
    "size",
];

/// What WinUI 3 and the system XAML show. A selected sidebar item's
/// `indicator` is not listed: the navigation view draws its own pill on the
/// leading edge only.
pub const SUPPORT: &[StyleSupport] = &[
    StyleSupport {
        component: "Button",
        base: BUTTON_BASE,
        states: &[
            (Hover, BUTTON_STATES),
            (Pressed, BUTTON_STATES),
            (Focus, &["system", "none"]),
            (Disabled, BUTTON_STATES),
        ],
    },
    StyleSupport {
        component: "IconButton",
        base: BUTTON_BASE,
        states: &[
            (Hover, BUTTON_STATES),
            (Pressed, BUTTON_STATES),
            (Focus, &["system", "none"]),
            (Disabled, BUTTON_STATES),
        ],
    },
    StyleSupport {
        component: "SidebarItem",
        base: &["background", "foreground", "corner", "padding"],
        states: &[
            (Hover, ROW_STATES),
            (Pressed, ROW_STATES),
            (
                Selected,
                &[
                    "fill",
                    "background",
                    "foreground",
                    "scale",
                    "offset",
                    "opacity",
                    "system",
                    "none",
                ],
            ),
        ],
    },
];
