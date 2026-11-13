//! Wear OS is a separate package and UI target, sharing Android's engine
//! bridge and build tools, but using Compose for Wear OS Material 3.
use crate::{ComposeAndroid, component_names};
use bern_ir::{Application, Node};
use bern_target_api::{
    BuildContext, BuildPlan, Diagnostic, GeneratedFile, Signing, Target, TargetConfig,
    TargetDescriptor,
};

pub struct ComposeWearos;

impl Target for ComposeWearos {
    fn build_hosts(&self) -> &'static [&'static str] {
        ComposeAndroid.build_hosts()
    }

    fn descriptor(&self) -> TargetDescriptor {
        let mut descriptor = ComposeAndroid.descriptor();
        descriptor.id = "compose-wearos";
        descriptor.platforms = &["wearos"];
        // Wear OS provides no WebView. Static SVGs are lowered to images
        // before capability checks, as on other targets without a WebView.
        static COMPONENTS: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
        descriptor.components = COMPONENTS.get_or_init(|| {
            component_names()
                .iter()
                .copied()
                .filter(|name| !matches!(*name, "WebPage" | "Svg"))
                .collect()
        });
        descriptor
    }

    fn check(&self, app: &Application, config: &TargetConfig) -> Vec<Diagnostic> {
        let app = bern_target_api::svg::prepare(app, config);
        let mut diagnostics = bern_target_api::check_common(self, &app);
        app.walk(&mut |node, _| {
            if let Node::Element(element) = node
                && element.kind == "Adaptive"
                && !element.children.iter().any(|child| matches!(child, Node::Element(layout) if layout.keyword("size") == Some("watch")))
            {
                diagnostics.push(Diagnostic::error("Wear OS requires an explicit `Layout(size: watch)` in every reachable `Adaptive`; a phone layout is not a watch layout").code("B401").at(element.source.clone()));
            }
        });
        diagnostics.extend(bern_target_api::icon_set::check(
            &app,
            "the Material icon set",
            crate::icons::has,
        ));
        let languages = bern_target_api::texts::framework_languages();
        for language in &app.languages.supported {
            if !languages.contains(&language.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "the Wear OS runtime has no `{language}` translation of its own texts"
                )));
            }
        }
        for channel in &config.channels {
            if channel != "ipc" {
                diagnostics.push(Diagnostic::error(format!(
                    "Wear OS has no `{channel}` channel"
                )));
            }
        }
        diagnostics
    }

    fn generate(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
        ComposeAndroid.generate(app, context)
    }

    fn plan(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        ComposeAndroid.plan(app, context)
    }

    fn package(
        &self,
        app: &Application,
        context: &BuildContext,
        signing: &Signing,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        ComposeAndroid.package(app, context, signing)
    }
}
