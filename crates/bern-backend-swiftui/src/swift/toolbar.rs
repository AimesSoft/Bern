//! Toolbars, side panels, menus and search.

use super::{Ctx, Generator, Out, Platform};
use crate::names::{double, string};
use bern_ir::{Element, Node};

impl Generator<'_> {
    pub(super) fn toolbar(&self, out: &mut Out, children: &[Node], ctx: &Ctx) {
        if children.iter().any(|n| matches!(n, Node::Element(e) if e.kind == "Toolbar" && e.children.iter().any(|n| matches!(n, Node::Element(e) if e.kind == "Search" && e.keyword("style") == Some("palette"))))) {
            out.line(".modifier(BernPaletteDetail())");
        }
        let toolbars: Vec<&Element> = children
            .iter()
            .filter_map(|node| match node {
                Node::Element(element) if element.kind == "Toolbar" => Some(element),
                _ => None,
            })
            .collect();
        let is_search = |node: &Node| matches!(node, Node::Element(item) if item.kind == "Search" && item.keyword("style") != Some("palette"));
        let items: Vec<&Element> = toolbars
            .iter()
            .copied()
            .filter(|toolbar| toolbar.children.iter().any(|node| !is_search(node)))
            .collect();
        if !items.is_empty() {
            out.open(".toolbar {");
            for toolbar in items {
                let children: Vec<Node> = toolbar
                    .children
                    .iter()
                    .filter(|node| !is_search(node))
                    .cloned()
                    .collect();
                self.toolbar_items(out, &children, ctx);
            }
            out.close("}");
        }
        for toolbar in toolbars {
            for node in &toolbar.children {
                if let Node::Element(search) = node
                    && search.kind == "Search"
                    && search.keyword("style") != Some("palette")
                {
                    let placement = match self.platform {
                        Platform::Macos => ".toolbar",
                        Platform::Ios | Platform::Tvos => ".automatic",
                    };
                    self.searchable(out, search, placement, ctx);
                }
            }
        }
    }

    /// The side panel standing in `children`, at the trailing edge of the
    /// container they belong to.
    pub(super) fn inspector(&self, out: &mut Out, children: &[Node], ctx: &Ctx) {
        let Some(panel) = children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "Inspector" => Some(child),
            _ => None,
        }) else {
            return;
        };
        let presented = self.field(panel, "presented").expect("bool field");
        out.open(format!(
            ".inspector(isPresented: {}) {{",
            self.binding_of(&presented)
        ));
        out.open("SwiftUI.VStack(alignment: .leading, spacing: 12) {");
        self.nodes(out, &panel.children, ctx);
        out.close("}");
        out.line(".padding(16)");
        out.line(".frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)");
        if let Some(width) = panel.number("width") {
            out.line(format!(
                ".inspectorColumnWidth(min: {}, ideal: {}, max: {})",
                double(width * 0.8),
                double(width),
                double(width * 1.5)
            ));
        }
        out.close("}");
    }

    /// A search field at `placement`, with its results below it.
    pub(super) fn searchable(&self, out: &mut Out, search: &Element, placement: &str, ctx: &Ctx) {
        let field = self.field(search, "value").expect("search field");
        let prompt = self
            .text_prop(search, "prompt")
            .unwrap_or_else(|| "nil".into());
        let submit = search
            .action
            .as_ref()
            .map(|action| format!(", submit: {{ {} }}", self.action(action)))
            .unwrap_or_default();
        let shortcut = self
            .shortcut(search)
            .map(|shortcut| format!(", shortcut: {shortcut}"))
            .unwrap_or_default();
        out.open(format!(
            ".modifier(BernSearchable(text: {}, placement: {placement}, prompt: {prompt}, target: {}{submit}{shortcut}) {{",
            self.binding_of(&field),
            self.target(search, ctx)
        ));
        if search.children.is_empty() {
            out.line("SwiftUI.EmptyView()");
        } else {
            self.nodes(out, &search.children, ctx);
        }
        out.close("})");
    }

    /// The items of one toolbar. Items before a `Spacer` stand at the
    /// leading end, where the way back is; the rest at the trailing end.
    /// Items between `Divider`s form one group on a shared background,
    /// and a fixed space parts the groups, as the platform's toolbars group
    /// their sections; a picker stands in the center.
    fn toolbar_items(&self, out: &mut Out, children: &[Node], ctx: &Ctx) {
        let split = children
            .iter()
            .position(|node| matches!(node, Node::Element(element) if element.kind == "Spacer"));
        let (leading, trailing) = match split {
            Some(index) => (&children[..index], &children[index + 1..]),
            None => (&children[..0], children),
        };
        if !leading.is_empty() {
            self.toolbar_groups(out, leading, ".navigation", ctx);
        }
        // The trailing end: on macOS `.primaryAction` is the leading one,
        // and without a title the items follow the window's buttons unless
        // a flexible space stands before them.
        let placement = match self.platform {
            Platform::Macos => {
                out.open("if #available(macOS 26.0, *) {");
                out.line("SwiftUI.ToolbarSpacer(.flexible, placement: .automatic)");
                out.close("}");
                ".automatic"
            }
            Platform::Ios | Platform::Tvos => ".primaryAction",
        };
        self.toolbar_groups(out, trailing, placement, ctx);
    }

    fn toolbar_groups(&self, out: &mut Out, children: &[Node], placement: &str, ctx: &Ctx) {
        let mut groups: Vec<Vec<Node>> = vec![Vec::new()];
        for node in children {
            match node {
                Node::Element(element) if element.kind == "Divider" => groups.push(Vec::new()),
                Node::Element(element) if element.kind == "Picker" => {
                    out.open("SwiftUI.ToolbarItem(placement: .principal) {");
                    self.element(out, element, ctx);
                    out.close("}");
                }
                // Words in a toolbar, such as the page a way back leads
                // to: an item of their own, without a background.
                Node::Element(element) if element.kind == "Text" => {
                    groups.push(Vec::new());
                    out.open("if #available(macOS 26.0, iOS 26.0, tvOS 26.0, *) {");
                    out.open(format!("SwiftUI.ToolbarItem(placement: {placement}) {{"));
                    self.element(out, element, ctx);
                    out.close("}");
                    out.line(".sharedBackgroundVisibility(.hidden)");
                    out.close("} else {");
                    out.indent += 1;
                    out.open(format!("SwiftUI.ToolbarItem(placement: {placement}) {{"));
                    self.element(out, element, ctx);
                    out.close("}");
                    out.close("}");
                }
                node => groups.last_mut().expect("a group").push(node.clone()),
            }
        }
        for (index, group) in groups.iter().filter(|group| !group.is_empty()).enumerate() {
            if index > 0 {
                out.open("if #available(macOS 26.0, iOS 26.0, tvOS 26.0, *) {");
                out.line(format!(
                    "SwiftUI.ToolbarSpacer(.fixed, placement: {placement})"
                ));
                out.close("}");
            }
            out.open(format!(
                "SwiftUI.ToolbarItemGroup(placement: {placement}) {{"
            ));
            let group_ctx = Ctx {
                toolbar: true,
                shared: group.len() > 1,
                ..ctx.clone()
            };
            self.nodes(out, group, &group_ctx);
            out.close("}");
        }
    }

    /// A menu of command items. The items are registered on the menu button,
    /// so external entries reach them whether the menu is open or not.
    pub(super) fn menu(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let hide_disabled = element.keyword("hide_disabled") == Some("true");
        let items: Vec<&Element> = element
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(item) => Some(item),
                _ => None,
            })
            .collect();
        out.open("SwiftUI.Menu {");
        for item in &items {
            if item.kind == "Divider" {
                out.line("SwiftUI.Divider()");
                continue;
            }
            let visible = item.expr("enabled").filter(|_| hide_disabled);
            if let Some(visible) = visible {
                out.open(format!("if {} {{", self.expr(visible)));
            }
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let (action, _) = self.pressed(item);
            let role = if item.keyword("role") == Some("destructive") {
                "role: .destructive, "
            } else {
                ""
            };
            out.open(format!("SwiftUI.Button({role}action: {{"));
            out.line(&action);
            out.close("}) {");
            out.indent += 1;
            match item.icon("icon") {
                Some(icon) => out.line(format!(
                    "SwiftUI.Label({item_label}, systemImage: {})",
                    string(icon)
                )),
                None => out.line(format!("SwiftUI.Text(verbatim: {item_label})")),
            }
            out.close("}");
            if let Some(line) = self.shortcut_line(item, Some(&action)) {
                out.line(line);
            }
            if let Some(enabled) = item.expr("enabled") {
                out.line(format!(".disabled(!({}))", self.expr(enabled)));
            }
            if visible.is_some() {
                out.close("}");
            }
        }
        out.close("} label: {");
        out.indent += 1;
        let icon_only = element.keyword("style") == Some("icon");
        let diameter = self
            .length(element, "height")
            .filter(|_| icon_only && !ctx.toolbar);
        let capsule = element.keyword("shape") == Some("capsule") && !ctx.toolbar;
        // iOS draws no arrow on a menu's button; the capsule shows one, as
        // macOS's pull-down does.
        let arrow = capsule && self.platform.uikit();
        if arrow {
            // The design's pull-down (6.3): the symbol in a 16-point box, the
            // arrow in a 12-point one at 60 %, 6 apart, and the arrow's side
            // 11 points from the edge instead of 14.
            out.open("SwiftUI.HStack(spacing: 6) {");
        }
        match element.icon("icon") {
            Some(icon) if arrow => {
                out.line(format!(
                    "SwiftUI.Image(systemName: {}).frame(width: 16, height: 16).accessibilityHidden(true)",
                    string(icon)
                ));
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
            }
            Some(icon) => {
                out.line(format!(
                    "SwiftUI.Label({label}, systemImage: {})",
                    string(icon)
                ));
                if icon_only {
                    out.line(".labelStyle(.iconOnly)");
                    // Size the label before the system draws its circular
                    // button: short symbols such as ellipsis would otherwise
                    // produce a smaller button than symbols such as star.
                    if diameter.is_some() {
                        out.line(".font(.system(size: 17)).frame(width: 18, height: 18)");
                    } else if !ctx.toolbar {
                        out.line(".frame(width: 16, height: 16)");
                    }
                }
            }
            None => out.line(format!("SwiftUI.Text(verbatim: {label})")),
        }
        if arrow {
            out.line("SwiftUI.Image(systemName: \"chevron.down\").font(.system(size: 10, weight: .semibold)).frame(width: 12, height: 12).opacity(0.6)");
            out.close("}");
            out.line(".padding(.trailing, -3)");
        }
        out.close("}");
        if ctx.toolbar && ctx.shared {
            out.line(".menuIndicator(.hidden)");
        }
        // Outside a toolbar a menu is a pull-down button of the platform;
        // an icon-only one is a round icon button without the arrow.
        if !ctx.toolbar {
            if element.keyword("shape") == Some("capsule") {
                // A macOS pull-down's AppKit bezel ignores .buttonStyle(.glass).
                // Keep the native menu, remove that bezel, and let the system
                // glass surface provide the capsule around its label.
                if self.platform == Platform::Macos {
                    out.line(".menuStyle(.borderlessButton)");
                }
                out.line(format!(
                    ".bernCapsuleControl(height: {})",
                    self.length(element, "height")
                        .unwrap_or_else(|| "32.0".into())
                ));
            } else if let Some(diameter) = &diameter {
                out.line(".menuStyle(.button)");
                out.line(".menuIndicator(.hidden)");
                out.line(format!(
                    ".bernCircleControl(diameter: {diameter}, prominent: false)"
                ));
                out.line(format!(".help({label})"));
            } else if icon_only {
                out.line(".menuStyle(.button)");
                out.line(".menuIndicator(.hidden)");
                out.line(".bernButtonLook(.icon)");
                out.line(format!(".help({label})"));
            } else {
                out.line(".menuStyle(.button)");
                out.line(".bernButtonLook(.secondary)");
            }
        }
        out.line(format!(
            ".bernControl({}, role: \"menu\", label: {label}, actions: BernControlActions())",
            self.target(element, ctx)
        ));
        for item in items.iter().filter(|item| item.kind == "MenuItem") {
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let (action, file) = self.pressed(item);
            let enabled = item
                .expr("enabled")
                .map(|expr| self.expr(expr))
                .unwrap_or_else(|| "true".into());
            out.line(format!(
                ".bernControl({}, role: \"button\", label: {item_label}, enabled: {enabled}, actions: BernControlActions(activate: {{ {action} }}{file}))",
                self.target(item, ctx)
            ));
        }
    }

    pub(super) fn search_palette(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("search field");
        let prompt = self
            .text_prop(element, "prompt")
            .unwrap_or_else(|| "\"\"".into());
        let panel_prompt = self
            .text_prop(element, "panel_prompt")
            .unwrap_or_else(|| prompt.clone());
        let shortcut = self
            .shortcut(element)
            .map(|s| format!(", shortcut: {s}"))
            .unwrap_or_default();
        out.open(format!("BernSearchPalette(text: {}, prompt: {prompt}, panelPrompt: {panel_prompt}, target: {}, width: {}, height: {}{shortcut}) {{",
            self.binding_of(&field), self.target(element, ctx), double(element.number("width").unwrap_or(640.0)), double(element.number("height").unwrap_or(480.0))));
        self.nodes(
            out,
            &element.children,
            &Ctx {
                toolbar: false,
                ..ctx.clone()
            },
        );
        out.close("}");
    }

    /// A search field in the page: toolbars and sidebars realize their
    /// own.
    pub(super) fn page_search(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("search field");
        let prompt = self
            .text_prop(element, "prompt")
            .unwrap_or_else(|| "\"\"".into());
        let submit = element
            .action
            .as_ref()
            .map(|action| format!(", submit: {{ {} }}", self.action(action)))
            .unwrap_or_default();
        let size = [
            ("height", "height"),
            ("font_size", "fontSize"),
            ("width", "width"),
        ]
        .iter()
        .filter_map(|(prop, name)| {
            element
                .number(prop)
                .map(|value| format!(", {name}: {}", double(value)))
        })
        .collect::<String>();
        let suggest = if element.keyword("suggest") == Some("true") {
            ", suggest: true"
        } else {
            ""
        };
        out.open(format!(
            "BernSearchField(text: {}, prompt: {prompt}, target: {}{size}{suggest}{submit}) {{",
            self.binding_of(&field),
            self.target(element, ctx)
        ));
        if element.children.is_empty() {
            out.line("SwiftUI.EmptyView()");
        } else {
            self.nodes(
                out,
                &element.children,
                &Ctx {
                    toolbar: false,
                    ..ctx.clone()
                },
            );
        }
        out.close("}");
    }

    pub(super) fn search_item(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let (action, file) = self.pressed(element);
        let navigation = element
            .expr("navigation")
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "true".into());
        out.open(format!("BernSearchItem(target: {}, label: {label}, navigation: {navigation}, actions: BernControlActions(activate: {{ {action} }}{file})) {{", self.target(element, ctx)));
        if element.children.is_empty() {
            out.line(format!(
                "BernSearchRow(label: {label}, icon: {}, detail: {}, hint: {})",
                element
                    .icon("icon")
                    .map(string)
                    .unwrap_or_else(|| "nil".into()),
                self.text_prop(element, "detail")
                    .unwrap_or_else(|| "\"\"".into()),
                self.text_prop(element, "hint")
                    .unwrap_or_else(|| "\"\"".into())
            ));
        } else {
            self.nodes(out, &element.children, ctx);
        }
        out.close("}");
    }
}
