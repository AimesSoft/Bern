//! Per-language string tables of the application: its declared texts, its
//! name and the runtime's own texts.

use bern_ir::Application;

/// Texts the GTK runtime shows itself, with built-in translations.
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
        "bern.startup.failed",
        &[
            ("zh-Hans", "应用未能启动"),
            ("zh-Hant", "App 無法啟動"),
            ("en", "The app couldn’t start"),
            ("ja", "アプリを起動できませんでした"),
        ],
    ),
    (
        "bern.split.more",
        &[
            ("zh-Hans", "更多选项"),
            ("zh-Hant", "更多選項"),
            ("en", "More Options"),
            ("ja", "その他のオプション"),
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
        "bern.navigation.back",
        &[
            ("zh-Hans", "返回"),
            ("zh-Hant", "返回"),
            ("en", "Back"),
            ("ja", "戻る"),
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

pub fn framework_languages() -> Vec<&'static str> {
    FRAMEWORK_TEXTS[0]
        .1
        .iter()
        .map(|(language, _)| *language)
        .collect()
}

/// `(language, [(key, text)])` for every supported language. Texts keep
/// Bern's `{name}` placeholders; the runtime fills them by name.
pub fn tables(app: &Application) -> Vec<(String, Vec<(String, String)>)> {
    app.languages
        .supported
        .iter()
        .map(|language| {
            let mut table: Vec<(String, String)> = app
                .texts
                .iter()
                .map(|text| {
                    let value = text
                        .get(language)
                        .or_else(|| text.get(&app.languages.default))
                        .unwrap_or("");
                    (text.key.clone(), value.to_string())
                })
                .collect();
            let name = app
                .app
                .name
                .get(language)
                .or_else(|| app.app.name.get(&app.languages.default))
                .unwrap_or(&app.app.id);
            table.push(("app.name".into(), name.to_string()));
            for (key, translations) in FRAMEWORK_TEXTS {
                let value = translations
                    .iter()
                    .find(|(candidate, _)| candidate == language)
                    .map(|(_, value)| *value)
                    .unwrap_or("");
                table.push(((*key).into(), value.into()));
            }
            (language.clone(), table)
        })
        .collect()
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
