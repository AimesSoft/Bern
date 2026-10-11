//! The interface of {{name}}: one window with a counter. Every string is
//! written in every language next to where it is shown, and every button
//! runs a command from the protocol.

use bern::prelude::*;
use {{id}}_protocol::{{id}};

#[component]
pub fn app(app: {{id}}::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { {{text:title}} }, initial: open, width: 480, height: 320) {
                Column(gap: 16, padding: 24, align: center) {
                    Text(content: tr! { {{text:greeting}} }, style: title)
                    Text(content: tr! { {{text:count}}; count: app.count })
                    Row(gap: 12) {
                        Button(id: add_one, label: tr! { {{text:add_one}} }, prominence: prominent) => app.add_one()
                        Button(id: reset, label: tr! { {{text:reset}} }) => app.reset()
                    }
                }
            }
        }
    }
}
