use bern::prelude::*;
use svg_demo_protocol::svg_demo;
#[component]
pub fn app(app: svg_demo::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "SVG Animation" }, initial: open, width: 560, height: 500) {
                Column(gap: 18, padding: 24, align: center) {
                    Text(content: tr! { "en" => "CSS · SMIL · JavaScript" }, style: title2)
                    Svg(source: "animation.svg", width: 280, height: 280, label: tr! { "en" => "Animated orbit. Double-click to change its color." }, playing: app.playing, speed: app.speed, restart: app.revision)
                    Row(gap: 12) {
                        Button(id: toggle, label: tr! { "en" => "Play / Pause" }) => app.toggle()
                        Button(id: speed, label: tr! { "en" => "1× / 2×" }) => app.faster()
                        Button(id: restart, label: tr! { "en" => "Restart" }) => app.restart()
                    }
                    Text(content: tr! { "en" => "Double-click the SVG to change its color" }, emphasis: secondary)
                }
            }
        }
    }
}
