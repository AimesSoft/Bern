//! Per-language string tables for the page, from the application's
//! declared texts and the runtime's own texts.

use bern_ir::Application;
use serde_json::{Map, Value};

/// Texts the Web runtime shows itself, with built-in translations.
pub const FRAMEWORK_TEXTS: &[(&str, &[(&str, &str)])] = &[
    (
        "bern.alert.failed",
        &[
            ("zh-Hans", "无法完成操作"),
            ("zh-Hant", "無法完成操作"),
            ("en", "The action couldn’t be completed"),
            ("ja", "操作を完了できませんでした"),
        ],
    ),
    (
        "bern.alert.ok",
        &[
            ("zh-Hans", "好"),
            ("zh-Hant", "好"),
            ("en", "OK"),
            ("ja", "OK"),
        ],
    ),
    (
        "bern.startup.failed",
        &[
            ("zh-Hans", "应用未能启动"),
            ("zh-Hant", "App 無法啟動"),
            ("en", "The app couldn’t start"),
            ("ja", "アプリを起動できませんでした"),
        ],
    ),
    (
        "bern.query.loading",
        &[
            ("zh-Hans", "正在载入…"),
            ("zh-Hant", "正在載入…"),
            ("en", "Loading…"),
            ("ja", "読み込み中…"),
        ],
    ),
    (
        "bern.query.failed",
        &[
            ("zh-Hans", "无法载入"),
            ("zh-Hant", "無法載入"),
            ("en", "Couldn’t Load"),
            ("ja", "読み込めませんでした"),
        ],
    ),
    (
        "bern.query.retry",
        &[
            ("zh-Hans", "重试"),
            ("zh-Hant", "重試"),
            ("en", "Try Again"),
            ("ja", "再試行"),
        ],
    ),
    (
        "bern.search.clear",
        &[
            ("zh-Hans", "清除"),
            ("zh-Hant", "清除"),
            ("en", "Clear"),
            ("ja", "消去"),
        ],
    ),
    (
        "bern.navigation.back",
        &[
            ("zh-Hans", "返回"),
            ("zh-Hant", "返回"),
            ("en", "Back"),
            ("ja", "戻る"),
        ],
    ),
    (
        "bern.window.close",
        &[
            ("zh-Hans", "关闭"),
            ("zh-Hant", "關閉"),
            ("en", "Close"),
            ("ja", "閉じる"),
        ],
    ),
    (
        "bern.window.minimize",
        &[
            ("zh-Hans", "最小化"),
            ("zh-Hant", "最小化"),
            ("en", "Minimize"),
            ("ja", "しまう"),
        ],
    ),
    (
        "bern.window.maximize",
        &[
            ("zh-Hans", "最大化"),
            ("zh-Hant", "最大化"),
            ("en", "Maximize"),
            ("ja", "拡大"),
        ],
    ),
    (
        "bern.window.restore",
        &[
            ("zh-Hans", "恢复"),
            ("zh-Hant", "恢復"),
            ("en", "Restore"),
            ("ja", "元のサイズに戻す"),
        ],
    ),
    (
        "bern.window.switcher",
        &[
            ("zh-Hans", "窗口"),
            ("zh-Hant", "視窗"),
            ("en", "Windows"),
            ("ja", "ウインドウ"),
        ],
    ),
    (
        "bern.window.settings",
        &[
            ("zh-Hans", "设置"),
            ("zh-Hant", "設定"),
            ("en", "Settings"),
            ("ja", "設定"),
        ],
    ),
    (
        "bern.link.lost",
        &[
            ("zh-Hans", "与引擎的连接已断开，正在重新连接…"),
            ("zh-Hant", "與引擎的連線已中斷，正在重新連線…"),
            ("en", "Lost the connection to the engine. Reconnecting…"),
            ("ja", "エンジンとの接続が切れました。再接続しています…"),
        ],
    ),
    (
        "bern.link.restarted",
        &[
            ("zh-Hans", "引擎已重新启动，页面将重新载入。"),
            ("zh-Hant", "引擎已重新啟動，頁面將重新載入。"),
            ("en", "The engine restarted. Reloading the page."),
            ("ja", "エンジンが再起動しました。ページを再読み込みします。"),
        ],
    ),
    (
        "bern.link.failed",
        &[
            ("zh-Hans", "无法连接到引擎"),
            ("zh-Hant", "無法連線到引擎"),
            ("en", "Can’t Connect to the Engine"),
            ("ja", "エンジンに接続できません"),
        ],
    ),
    (
        "bern.media.video",
        &[
            ("zh-Hans", "视频"),
            ("zh-Hant", "影片"),
            ("en", "Video"),
            ("ja", "ビデオ"),
        ],
    ),
    (
        "bern.media.play",
        &[
            ("zh-Hans", "播放"),
            ("zh-Hant", "播放"),
            ("en", "Play"),
            ("ja", "再生"),
        ],
    ),
    (
        "bern.media.pause",
        &[
            ("zh-Hans", "暂停"),
            ("zh-Hant", "暫停"),
            ("en", "Pause"),
            ("ja", "一時停止"),
        ],
    ),
    (
        "bern.media.mute",
        &[
            ("zh-Hans", "静音"),
            ("zh-Hant", "靜音"),
            ("en", "Mute"),
            ("ja", "消音"),
        ],
    ),
    (
        "bern.media.unmute",
        &[
            ("zh-Hans", "取消静音"),
            ("zh-Hant", "取消靜音"),
            ("en", "Unmute"),
            ("ja", "消音を解除"),
        ],
    ),
    (
        "bern.media.position",
        &[
            ("zh-Hans", "播放位置"),
            ("zh-Hant", "播放位置"),
            ("en", "Playback Position"),
            ("ja", "再生位置"),
        ],
    ),
    (
        "bern.media.volume",
        &[
            ("zh-Hans", "音量"),
            ("zh-Hant", "音量"),
            ("en", "Volume"),
            ("ja", "音量"),
        ],
    ),
    (
        "bern.media.live",
        &[
            ("zh-Hans", "直播"),
            ("zh-Hant", "直播"),
            ("en", "Live"),
            ("ja", "ライブ"),
        ],
    ),
    (
        "bern.media.empty",
        &[
            ("zh-Hans", "没有视频"),
            ("zh-Hant", "沒有影片"),
            ("en", "No Video"),
            ("ja", "ビデオがありません"),
        ],
    ),
    (
        "bern.media.failed",
        &[
            ("zh-Hans", "无法播放"),
            ("zh-Hant", "無法播放"),
            ("en", "Can’t Play"),
            ("ja", "再生できません"),
        ],
    ),
    (
        "bern.media.unavailable",
        &[
            ("zh-Hans", "播放器不可用"),
            ("zh-Hant", "播放器無法使用"),
            ("en", "Player Unavailable"),
            ("ja", "プレーヤーを使用できません"),
        ],
    ),
    (
        "bern.close.save",
        &[
            ("zh-Hans", "存储"),
            ("zh-Hant", "儲存"),
            ("en", "Save"),
            ("ja", "保存"),
        ],
    ),
    (
        "bern.close.dont_save",
        &[
            ("zh-Hans", "不存储"),
            ("zh-Hant", "不儲存"),
            ("en", "Don’t Save"),
            ("ja", "保存しない"),
        ],
    ),
    (
        "bern.close.discard",
        &[
            ("zh-Hans", "放弃更改"),
            ("zh-Hant", "放棄變更"),
            ("en", "Discard Changes"),
            ("ja", "変更を破棄"),
        ],
    ),
    (
        "bern.close.cancel",
        &[
            ("zh-Hans", "取消"),
            ("zh-Hant", "取消"),
            ("en", "Cancel"),
            ("ja", "キャンセル"),
        ],
    ),
    (
        "bern.close.save_detail",
        &[
            ("zh-Hans", "如果不存储，你的更改将会丢失。"),
            ("zh-Hant", "如果不儲存，你的變更將會遺失。"),
            ("en", "Your changes will be lost if you don’t save them."),
            ("ja", "保存しないと、変更は失われます。"),
        ],
    ),
    (
        "bern.close.discard_detail",
        &[
            ("zh-Hans", "关闭窗口会放弃这些更改。"),
            ("zh-Hant", "關閉視窗會放棄這些變更。"),
            ("en", "Closing the window discards these changes."),
            ("ja", "ウインドウを閉じると、これらの変更は破棄されます。"),
        ],
    ),
];

/// Languages the runtime's own texts are translated into.
pub fn framework_languages() -> Vec<&'static str> {
    FRAMEWORK_TEXTS[0]
        .1
        .iter()
        .map(|(language, _)| *language)
        .collect()
}

/// `{ language: { key: text } }` for every supported language. Texts keep
/// Bern's `{name}` placeholders; the runtime fills them by name.
pub fn tables(app: &Application) -> Value {
    let mut tables = Map::new();
    for language in &app.languages.supported {
        let mut table = Map::new();
        for text in &app.texts {
            let value = text
                .get(language)
                .or_else(|| text.get(&app.languages.default))
                .unwrap_or("");
            table.insert(text.key.clone(), Value::String(value.to_string()));
        }
        table.insert(
            "app.name".into(),
            Value::String(
                app.app
                    .name
                    .get(language)
                    .or_else(|| app.app.name.get(&app.languages.default))
                    .unwrap_or(&app.app.id)
                    .to_string(),
            ),
        );
        for (key, translations) in FRAMEWORK_TEXTS {
            let value = translations
                .iter()
                .find(|(candidate, _)| candidate == language)
                .map(|(_, value)| *value)
                .unwrap_or("");
            table.insert((*key).into(), Value::String(value.into()));
        }
        tables.insert(language.clone(), Value::Object(table));
    }
    Value::Object(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_framework_text_has_every_framework_language() {
        let languages = framework_languages();
        for (key, translations) in FRAMEWORK_TEXTS {
            let mut found: Vec<_> = translations.iter().map(|(language, _)| *language).collect();
            found.sort_unstable();
            let mut expected = languages.clone();
            expected.sort_unstable();
            assert_eq!(found, expected, "{key}");
        }
    }
}
