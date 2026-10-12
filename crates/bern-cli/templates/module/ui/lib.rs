//! Interface of the {{id}} module. The application shows these components
//! where it places them: in a page slot, a slot, or by calling them.

use bern::prelude::*;
use {{crate}}_protocol::{{id}};

/// The module's page.
#[component]
pub fn {{id}}_page({{id}}: {{id}}::Client) -> impl View {
    ui! {
        Column(gap: 12, padding: 16) {
            Text(content: tr! { {{text:count}}; count: {{id}}.count })
            Button(id: add_one, label: tr! { {{text:add_one}} }) => {{id}}.add_one()
        }
    }
}
