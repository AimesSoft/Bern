//! Shared animated SVG assets and runtime templates for all presenters.
use crate::{BuildContext, Diagnostic};
use bern_ir::{Application, Node};

pub fn used(app: &Application) -> bool {
    let mut used = false;
    for node in app
        .components
        .iter()
        .map(|c| &c.body)
        .chain(app.windows.windows.iter().map(|w| &w.content))
    {
        node.walk(&mut |node| {
            if matches!(node, Node::Element(e) if e.kind == "Svg") {
                used = true;
            }
        });
    }
    used
}

pub fn validate(app: &Application, context: &BuildContext) -> Result<(), Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for node in app
        .components
        .iter()
        .map(|c| &c.body)
        .chain(app.windows.windows.iter().map(|w| &w.content))
    {
        node.walk(&mut |node| {
            if let Node::Element(e) = node
                && e.kind == "Svg"
                && let Some(name) = e.resource("source")
                && seen.insert(name.to_owned())
            {
                let result = std::fs::read_to_string(context.project_dir.join(name))
                    .map_err(|e| e.to_string())
                    .and_then(|source| bern_svg::root(&source).map(|_| ()));
                if let Err(error) = result {
                    errors.push(
                        Diagnostic::error(format!("SVG `{name}`: {error}")).at(e.source.clone()),
                    );
                }
            }
        });
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Templates use JSON-compatible quoted strings; Kotlin additionally
/// needs dollar signs escaped to preserve JavaScript template literals.
pub fn template(source: &str, kotlin: bool) -> String {
    let quote = |s: &str| {
        let quoted = serde_json::to_string(s).unwrap();
        if kotlin {
            quoted.replace('$', "\\$")
        } else {
            quoted
        }
    };
    source
        .replace("/*bern:svg-prefix*/", &quote(bern_svg::PREFIX))
        .replace("/*bern:svg-suffix*/", &quote(&bern_svg::suffix()))
}

pub fn cpp_template(source: &str) -> String {
    source
        .replace(
            "/*bern:svg-prefix*/",
            &format!("R\"BERNSVG({})BERNSVG\"", bern_svg::PREFIX),
        )
        .replace(
            "/*bern:svg-suffix*/",
            &format!("R\"BERNSVG({})BERNSVG\"", bern_svg::suffix()),
        )
}
