//! Reusable presentational components for the showcase, compiled into a
//! versioned descriptor that applications check before using them.

use bern::prelude::*;

/// Secondary text showing how many files an item holds.
#[component]
pub fn item_count(count: i64) -> impl View {
    ui! {
        Text(content: tr! { "zh-Hans" => "{count} 项", "en" => "{count} items"; count: count }, emphasis: secondary)
    }
}
