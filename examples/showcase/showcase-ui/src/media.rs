//! The media page: a video whose player belongs to the page and follows
//! engine state, and the application's preview player.

use crate::Players;
use bern::prelude::*;
use showcase_protocol::showcase::{self, Clip};

/// Video declared both ways: from a source binding and from a player.
#[component]
pub fn media_page(app: showcase::Client, players: Players) -> impl View {
    ui! {
        Form {
            Section(
                title: tr! { "zh-Hans" => "精选片段", "en" => "Featured Clip" },
                footer: tr! { "zh-Hans" => "这段视频的播放器属于本页面，片源跟随引擎状态；换片后，旧片源迟到的事件会被忽略。离开本页面会释放这个播放器。", "en" => "This video's player belongs to the page and follows engine state; after a change, late reports from the previous clip are ignored. Leaving the page releases the player." }
            ) {
                Video(id: featured, source: app.featured_source, label: tr! { "zh-Hans" => "精选片段", "en" => "Featured clip" })
                Row(gap: 8) {
                    Text(content: tr! { "zh-Hans" => "片段：{clip}", "en" => "Clip: {clip}"; clip: app.featured }, emphasis: secondary)
                    Spacer
                    Button(id: pattern, label: tr! { "zh-Hans" => "测试图", "en" => "Test Pattern" }) => app.feature(Clip::Pattern)
                    Button(id: bars, label: tr! { "zh-Hans" => "色条", "en" => "Color Bars" }) => app.feature(Clip::Bars)
                    Button(id: damaged, label: tr! { "zh-Hans" => "损坏的文件", "en" => "Damaged File" }) => app.feature(Clip::Damaged)
                }
            }
            Section(
                title: tr! { "zh-Hans" => "预览播放器", "en" => "Preview Player" },
                footer: tr! { "zh-Hans" => "应用级播放器：在“列表与详情”中打开项目；离开页面或关闭窗口后仍会继续播放。", "en" => "An application player: open items from List & Detail. It keeps playing when you leave the page or close the window." }
            ) {
                Video(id: preview, player: players.preview, label: tr! { "zh-Hans" => "预览播放器", "en" => "Preview player" })
            }
        }
    }
}
