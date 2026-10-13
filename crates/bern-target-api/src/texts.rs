//! Texts every native runtime shows itself, and the conversion of Bern's
//! named placeholders to a platform's positional format specifiers.

use bern_protocol::LocalizedText;

/// Strings the native runtimes show themselves, with built-in translations.
/// `%@` stands for the one argument; runtimes convert it to their own
/// specifier.
pub const FRAMEWORK_TEXTS: &[(&str, &[(&str, &str)])] = &[
    // iOS shows windows other than the primary as sheets, closed with this
    // button, and offers to open the primary window again once it closed.
    (
        "bern.window.done",
        &[
            ("zh-Hans", "完成"),
            ("zh-Hant", "完成"),
            ("en", "Done"),
            ("ja", "完了"),
        ],
    ),
    (
        "bern.window.open",
        &[
            ("zh-Hans", "打开“%@”"),
            ("zh-Hant", "打開「%@」"),
            ("en", "Open “%@”"),
            ("ja", "“%@”を開く"),
        ],
    ),
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
            ("zh-Hans", "没有媒体"),
            ("zh-Hant", "沒有媒體"),
            ("en", "No Media"),
            ("ja", "メディアがありません"),
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
            ("zh-Hant", "放棄更改"),
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
            ("zh-Hant", "如果不儲存，你的更改將會遺失。"),
            ("en", "Your changes will be lost if you don’t save them."),
            ("ja", "保存しないと、変更内容は失われます。"),
        ],
    ),
    (
        "bern.close.discard_detail",
        &[
            ("zh-Hans", "关闭窗口会放弃这些更改。"),
            ("zh-Hant", "關閉視窗會放棄這些更改。"),
            ("en", "Closing the window discards these changes."),
            ("ja", "ウインドウを閉じると、これらの変更は破棄されます。"),
        ],
    ),
];

/// Languages the runtime's own strings are translated into.
pub fn framework_languages() -> Vec<&'static str> {
    FRAMEWORK_TEXTS[0]
        .1
        .iter()
        .map(|(language, _)| *language)
        .collect()
}

/// Convert Bern placeholders to positional format specifiers made by
/// `specifier` from the 1-based position, escaping `%` as `%%`. Texts
/// without parameters are returned unchanged because they are not
/// formatted.
pub fn positional(text: &str, params: &[String], specifier: impl Fn(usize) -> String) -> String {
    if params.is_empty() {
        return text.replace("{{", "{").replace("}}", "}");
    }
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut name = String::new();
                for next in chars.by_ref() {
                    if next == '}' {
                        break;
                    }
                    name.push(next);
                }
                let position = params.iter().position(|param| *param == name).unwrap_or(0) + 1;
                out.push_str(&specifier(position));
            }
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out
}

/// The text in `language`, else in the default language, else any.
pub fn translation<'a>(text: &'a LocalizedText, language: &str, default: &str) -> &'a str {
    text.get(language)
        .or_else(|| text.get(default))
        .or_else(|| text.translations.values().next().map(String::as_str))
        .unwrap_or("")
}

/// A framework text in `language`, else in English.
pub fn framework(translations: &[(&str, &'static str)], language: &str) -> &'static str {
    translations
        .iter()
        .find(|(candidate, _)| *candidate == language)
        .or_else(|| {
            translations
                .iter()
                .find(|(candidate, _)| *candidate == "en")
        })
        .map(|(_, value)| *value)
        .unwrap_or("")
}
