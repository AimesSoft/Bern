//! UI of the hello world: one window, one line.

use bern::prelude::*;
use hello_protocol::hello;

#[component]
pub fn app(app: hello::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "zh-Hans" => "你好", "en" => "Hello" }, initial: open, width: 480, height: 320) {
                Text(content: tr! { "zh-Hans" => "你好，世界！", "en" => "Hello, World!" }, style: title)
            }
        }
    }
}
