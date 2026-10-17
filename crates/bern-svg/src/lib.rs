//! Animated SVG documents shared by every Bern presenter.
//!
//! Rendering uses the platform's SVG/DOM engine, preserving CSS, SMIL,
//! filters, masks, text, and document-local JavaScript instead of flattening
//! an animation into bitmaps. Documents are self-contained bundled assets;
//! they have no native command bridge or access to external resources.

pub const RUNTIME: &str = include_str!("runtime.js");
pub const PREFIX: &str = r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; connect-src 'none'; base-uri 'none'; form-action 'none'"><style>html,body{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}body>svg{display:block;width:100%!important;height:100%!important}html{color-scheme:light dark}</style></head><body>"#;

pub fn suffix() -> String {
    format!("<script>{RUNTIME}</script></body></html>")
}

/// Validate an SVG document, preserving its original source and scripts.
/// Reject DTDs/external entities. XML declarations are valid on input, but
/// only the root SVG is embedded into the HTML document.
pub fn root(source: &str) -> Result<&str, String> {
    let doc = roxmltree::Document::parse(source).map_err(|e| format!("invalid SVG: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
    {
        return Err("expected an SVG root in the SVG namespace".into());
    }
    Ok(&source[root.range()])
}

pub fn document(source: &str) -> Result<String, String> {
    Ok(format!("{PREFIX}{}{}", root(source)?, suffix()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_css_smil_and_script() {
        let svg = r#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"><style>@keyframes a{to{opacity:0}}</style><rect><animate attributeName="x" to="20" dur="1s"/></rect><script><![CDATA[document.documentElement.dataset.test = 'ok';]]></script></svg>"#;
        let html = document(svg).unwrap();
        assert!(!html.contains("<?xml"));
        assert!(html.contains("@keyframes a"));
        assert!(html.contains("attributeName=\"x\""));
        assert!(html.contains("dataset.test"));
        assert!(html.contains("default-src 'none'"));
    }

    #[test]
    fn rejects_non_svg_and_external_entities() {
        for source in [
            "<html/>",
            "<svg/>",
            "<svg",
            "<!DOCTYPE svg [<!ENTITY x SYSTEM 'file:///tmp/x'>]><svg xmlns=\"http://www.w3.org/2000/svg\">&x;</svg>",
        ] {
            assert!(document(source).is_err());
        }
    }
}
