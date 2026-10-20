//! Capability checks: the parts of the IR a target cannot build or show,
//! as diagnostics pointing back to their declarations.

mod features;

use crate::{Diagnostic, Target, TargetDescriptor};
use bern_ir::{Application, IR_VERSION};

/// Check the parts of the IR every target must agree on: IR version,
/// supported components and window presentation requirements.
pub fn check_common(target: &dyn Target, app: &Application) -> Vec<Diagnostic> {
    let descriptor = target.descriptor();
    let mut diagnostics = Vec::new();
    if !descriptor.supports_ir() || app.ir_version != IR_VERSION {
        diagnostics.push(Diagnostic::error(format!(
            "target `{}` consumes IR versions {:?}, the application uses IR version {}",
            descriptor.id, descriptor.ir_versions, app.ir_version
        )));
    }
    diagnostics.extend(check_components(&descriptor, app));
    diagnostics.extend(check_styles(&descriptor, app));
    diagnostics.extend(check_dynamic_items(&descriptor, app));
    diagnostics.extend(features::check_features(&descriptor, app));
    diagnostics.extend(check_text_marks(&descriptor, app));
    diagnostics.extend(check_window_chrome(&descriptor, app));
    diagnostics.extend(check_motion(&descriptor, app));
    diagnostics.extend(check_media(&descriptor, app));
    diagnostics.extend(check_presentation(&descriptor, app));
    diagnostics
}

/// Elements, in the components and the windows, of kinds the target does
/// not support.
fn check_components(descriptor: &TargetDescriptor, app: &Application) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut visit = |node: &bern_ir::Node| {
        if let bern_ir::Node::Element(element) = node
            && !descriptor.components.contains(&element.kind.as_str())
        {
            diagnostics.push(
                Diagnostic::error(format!(
                    "target `{}` does not support `{}`",
                    descriptor.id, element.kind
                ))
                .at(element.source.clone()),
            );
        }
    };
    app.walk(&mut |node, _| visit(node));
    diagnostics
}

/// Style overrides `descriptor` cannot show on its platforms, unless the
/// rule exempts the platform.
fn check_styles(descriptor: &TargetDescriptor, app: &Application) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for platform in descriptor.platforms {
        for rule in app.style.rules_for(platform) {
            if rule.exempt.iter().any(|exempt| exempt == platform) {
                continue;
            }
            let support = descriptor
                .styles
                .iter()
                .find(|support| support.component == rule.component);
            let effects =
                rule.base
                    .iter()
                    .map(|effect| (None, effect))
                    .chain(rule.states.iter().flat_map(|state| {
                        state
                            .effects
                            .iter()
                            .map(move |effect| (Some(state.state), effect))
                    }));
            for (state, effect) in effects {
                // Lengths in place of written ones are laid out by every
                // target alike.
                if matches!(effect, bern_ir::style::StyleEffect::Metric { .. })
                    || (matches!(effect, bern_ir::style::StyleEffect::Padding { .. })
                        && matches!(rule.component.as_str(), "Row" | "Column"))
                    || support.is_some_and(|support| support.shows(state, effect.name()))
                {
                    continue;
                }
                let place = match state {
                    Some(state) => format!(" in the `{}` state", state.name()),
                    None => String::new(),
                };
                diagnostics.push(
                    Diagnostic::error(format!(
                        "target `{}` cannot show `{}` on `{}`{place}; write `#[exempt({platform}, \"…\")]` on the rule to keep {platform}'s own look, or `#[target(…)]` to apply it elsewhere",
                        descriptor.id,
                        effect.name(),
                        rule.component
                    ))
                    .at(rule.source.clone()),
                );
            }
        }
    }
    diagnostics
}

/// Navigation items that come and go, on a target whose items of
/// `Sidebar`, `Tabs` and `Toolbar` cannot.
fn check_dynamic_items(descriptor: &TargetDescriptor, app: &Application) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if !descriptor.dynamic_items {
        let mut dynamic = None;
        app.walk(&mut |node, _| {
            if let bern_ir::Node::Element(element) = node
                && bern_ir::spec::DYNAMIC_ITEMS.contains(&element.kind.as_str())
                && dynamic.is_none()
                && has_dynamic_items(&element.children)
            {
                dynamic = Some(element);
            }
        });
        if let Some(element) = dynamic {
            diagnostics.push(
                Diagnostic::error(format!(
                    "target `{}` cannot build `{}` items that come and go with `if`, `match` or `ForEach`, including pages of modules that can be switched off",
                    descriptor.id, element.kind
                ))
                .at(element.source.clone()),
            );
        }
    }
    diagnostics
}

/// Whether navigation items sit in conditions or repetitions, also inside
/// a sidebar's sections.
fn has_dynamic_items(nodes: &[bern_ir::Node]) -> bool {
    nodes.iter().any(|node| match node {
        bern_ir::Node::Element(element) if element.kind == "ForEach" => true,
        bern_ir::Node::Element(element) if element.kind == "Section" => {
            has_dynamic_items(&element.children)
        }
        bern_ir::Node::Element(_) | bern_ir::Node::Component(_) => false,
        bern_ir::Node::If(_) | bern_ir::Node::IfLet(_) | bern_ir::Node::Match(_) => true,
    })
}

/// `Text(mark:)` on a target that does not draw the highlight.
fn check_text_marks(descriptor: &TargetDescriptor, app: &Application) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if !descriptor.text_marks {
        let mut marked = false;
        app.walk(&mut |node, _| {
            if let bern_ir::Node::Element(element) = node
                && element.kind == "Text"
                && element.props.contains_key("mark")
            {
                marked = true;
            }
        });
        if marked {
            diagnostics.push(Diagnostic::warning(format!(
                "target `{}` shows `Text(mark:)` as plain text, without the highlight",
                descriptor.id
            )));
        }
    }
    diagnostics
}

/// `chrome`, `immersive` and `idle_after` on a target whose windows
/// ignore them.
fn check_window_chrome(descriptor: &TargetDescriptor, app: &Application) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if !descriptor.window_chrome {
        for window in &app.windows.windows {
            if window.chrome != bern_ir::WindowChrome::Standard
                || window.immersive.is_some()
                || window.idle_after.is_some()
            {
                diagnostics.push(Diagnostic::warning(format!(
                    "target `{}` keeps window `{}`'s standard title bar: `chrome`, `immersive` and `idle_after` have no effect there, and `window.idle` stays false",
                    descriptor.id, window.id
                )));
            }
        }
    }
    diagnostics
}

/// Motion props the target shows without motion: one warning for each.
fn check_motion(descriptor: &TargetDescriptor, app: &Application) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut still = std::collections::BTreeSet::new();
    app.walk(&mut |node, _| {
        if let bern_ir::Node::Element(element) = node {
            for prop in bern_ir::spec::MOTION_PROPS {
                if prop.name != "hero_key"
                    && prop.name != "reveal"
                    && prop.name != "appearance"
                    && element.props.contains_key(prop.name)
                    && !descriptor.motion.contains(&prop.name)
                {
                    still.insert(prop.name);
                }
            }
        }
    });
    for prop in still {
        let message = if prop == "stagger" {
            format!(
                "target `{}` does not stagger: children appear together",
                descriptor.id
            )
        } else if prop == "nudge" {
            format!(
                "target `{}` does not nudge: the element stays still",
                descriptor.id
            )
        } else if prop.starts_with("scroll_") {
            format!(
                "target `{}` does not follow scrolling with `{prop}`: items keep their look",
                descriptor.id
            )
        } else {
            format!(
                "target `{}` shows `{prop}` without motion: the change happens at once",
                descriptor.id
            )
        };
        diagnostics.push(Diagnostic::warning(message));
    }
    diagnostics
}

/// `Video` and players on a target without a media backend for this
/// build's media contract.
fn check_media(descriptor: &TargetDescriptor, app: &Application) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if app.uses_media() {
        let mut video = None;
        let mut find = |node: &bern_ir::Node| {
            if let bern_ir::Node::Element(element) = node
                && element.kind == "Video"
                && video.is_none()
            {
                video = Some(element.source.clone());
            }
        };
        app.walk(&mut |node, _| find(node));
        let at = |diagnostic: Diagnostic| match &video {
            Some(location) => diagnostic.at(location.clone()),
            None => diagnostic,
        };
        match &descriptor.media {
            None => diagnostics.push(at(Diagnostic::error(format!(
                "target `{}` has no media backend; `Video` and application players cannot be built",
                descriptor.id
            )))),
            Some(media)
                if !media
                    .contract_versions
                    .contains(&bern_media::MEDIA_CONTRACT_VERSION) =>
            {
                diagnostics.push(at(Diagnostic::error(format!(
                    "target `{}` pairs media backend `{}` and presenter `{}` for media contract versions {:?}; this build uses version {}",
                    descriptor.id,
                    media.backend,
                    media.presenter,
                    media.contract_versions,
                    bern_media::MEDIA_CONTRACT_VERSION
                ))));
            }
            Some(_) => {}
        }
    }
    diagnostics
}

/// Window presentations the target cannot provide.
fn check_presentation(descriptor: &TargetDescriptor, app: &Application) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for window in &app.windows.windows {
        match window.presentation {
            bern_ir::Presentation::NativeRequired if !descriptor.windows.native => {
                diagnostics.push(Diagnostic::error(format!(
                    "window `{}` requires native windows, which target `{}` cannot provide",
                    window.id, descriptor.id
                )));
            }
            bern_ir::Presentation::Floating if !descriptor.windows.floating => {
                diagnostics.push(Diagnostic::error(format!(
                    "window `{}` requests floating presentation, which target `{}` cannot provide",
                    window.id, descriptor.id
                )));
            }
            bern_ir::Presentation::Virtual if !descriptor.windows.virtual_windows => {
                diagnostics.push(Diagnostic::error(format!(
                    "window `{}` requests virtual presentation, which target `{}` has not implemented",
                    window.id, descriptor.id
                )));
            }
            _ => {}
        }
    }
    diagnostics
}
