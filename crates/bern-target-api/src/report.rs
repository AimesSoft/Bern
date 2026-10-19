//! The feature contract report of a platform.

use crate::Diagnostic;
use bern_ir::Application;

/// A feature contract report for one platform: the result of every
/// feature in every applicable size, and the diagnostics they raise.
#[derive(Clone, Debug, Default)]
pub struct FeatureReport {
    pub diagnostics: Vec<Diagnostic>,
    /// `(contract, feature, size, result)` rows, in declaration order.
    pub rows: Vec<(String, String, bern_ir::LayoutSize, bern_ir::FeatureResult)>,
}

impl FeatureReport {
    /// One line per row, for terminals.
    pub fn lines(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|(contract, feature, size, result)| {
                let outcome = match result {
                    bern_ir::FeatureResult::Covered { entries } => {
                        format!("covered by {}", entries.join(", "))
                    }
                    bern_ir::FeatureResult::Guaranteed { entries, reason } => format!(
                        "NOT PROVEN, manually guaranteed ({reason}); conditional entries {}",
                        entries.join(", ")
                    ),
                    bern_ir::FeatureResult::Exempt { reason } => format!("exempt: {reason}"),
                    bern_ir::FeatureResult::Missing => "MISSING".into(),
                    bern_ir::FeatureResult::Unproven { entries } => {
                        format!("UNPROVEN: only conditional entries {}", entries.join(", "))
                    }
                };
                format!("{contract}::{feature} [{}] {outcome}", size.name())
            })
            .collect()
    }
}

/// Evaluate every feature contract for `platform`: exemptions apply only
/// to their platform; missing and unproven features are errors (B201,
/// B203), manual guarantees warnings (B206) and exemptions notes (B205).
pub fn feature_report(app: &Application, platform: &str) -> FeatureReport {
    let mut report = FeatureReport::default();
    for contract in &app.contracts {
        for feature in &contract.features {
            for (size, result) in feature.results(platform) {
                let name = format!("{}::{}", contract.name, feature.name);
                let diagnostic = match &result {
                    bern_ir::FeatureResult::Covered { .. } => None,
                    bern_ir::FeatureResult::Missing => Some(
                        Diagnostic::error(format!(
                            "Feature missing in adaptive layout\nContract: {}\nFeature: {}\n{}: missing",
                            contract.name,
                            feature.name,
                            size.name()
                        ))
                        .code("B201"),
                    ),
                    bern_ir::FeatureResult::Unproven { entries } => Some(
                        Diagnostic::error(format!(
                            "`{name}` is reached in `{}` only under conditions the framework cannot prove ({}); add an unconditional entry, or `#[guaranteed(\"why\")]`",
                            size.name(),
                            entries.join(", ")
                        ))
                        .code("B203"),
                    ),
                    bern_ir::FeatureResult::Exempt { reason } => Some(
                        Diagnostic::note(format!(
                            "`{name}` is exempt in `{}` on {platform}: {reason}",
                            size.name()
                        ))
                        .code("B205"),
                    ),
                    bern_ir::FeatureResult::Guaranteed { reason, .. } => Some(
                        Diagnostic::warning(format!(
                            "`{name}` in `{}` is not proven; accepted by a manual guarantee: {reason}",
                            size.name()
                        ))
                        .code("B206"),
                    ),
                };
                if let Some(diagnostic) = diagnostic {
                    report
                        .diagnostics
                        .push(diagnostic.at(feature.source.clone()));
                }
                report
                    .rows
                    .push((contract.name.clone(), feature.name.clone(), size, result));
            }
        }
    }
    report
}
