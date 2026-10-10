//! Bundle resources: Info.plist and per-language string tables.

use bern_ir::Application;
use bern_protocol::LocalizedText;
use std::collections::BTreeMap;

/// Strings the SwiftUI runtime shows itself, with built-in translations.
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

fn escape_strings(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// Convert Bern placeholders to positional format specifiers. Texts without
/// parameters are returned unchanged because they are not formatted.
pub fn format_string(text: &str, params: &[String]) -> String {
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
                out.push_str(&format!("%{position}$@"));
            }
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out
}

fn translation<'a>(text: &'a LocalizedText, language: &str, default: &str) -> &'a str {
    text.get(language)
        .or_else(|| text.get(default))
        .or_else(|| text.translations.values().next().map(String::as_str))
        .unwrap_or("")
}

/// `Localizable.strings` contents per language.
pub fn string_tables(app: &Application) -> BTreeMap<String, String> {
    let mut tables = BTreeMap::new();
    for language in &app.languages.supported {
        let mut lines = vec![format!(
            "/* Generated by Bern for {language}. Translations are declared in Rust. */"
        )];
        for text in &app.texts {
            let value = translation(text, language, &app.languages.default);
            lines.push(format!(
                "\"{}\" = \"{}\";",
                escape_strings(&text.key),
                escape_strings(&format_string(value, &text.params))
            ));
        }
        for (key, translations) in FRAMEWORK_TEXTS {
            let value = translations
                .iter()
                .find(|(candidate, _)| candidate == language)
                .or_else(|| {
                    translations
                        .iter()
                        .find(|(candidate, _)| *candidate == "en")
                })
                .map(|(_, value)| *value)
                .unwrap_or("");
            lines.push(format!("\"{key}\" = \"{}\";", escape_strings(value)));
        }
        lines.push(String::new());
        tables.insert(language.clone(), lines.join("\n"));
    }
    tables
}

/// `InfoPlist.strings` contents per language, localizing the app name.
pub fn info_plist_tables(app: &Application) -> BTreeMap<String, String> {
    app.languages
        .supported
        .iter()
        .map(|language| {
            let name = escape_strings(translation(&app.app.name, language, &app.languages.default));
            (
                language.clone(),
                format!("CFBundleName = \"{name}\";\nCFBundleDisplayName = \"{name}\";\n"),
            )
        })
        .collect()
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn info_plist(app: &Application, executable: &str, minimum_system: &str) -> String {
    let name = translation(
        &app.app.name,
        &app.languages.default,
        &app.languages.default,
    );
    let localizations: String = app
        .languages
        .supported
        .iter()
        .map(|language| format!("\n        <string>{}</string>", xml(language)))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>{region}</string>
    <key>CFBundleDisplayName</key>
    <string>{name}</string>
    <key>CFBundleExecutable</key>
    <string>{executable}</string>
    <key>CFBundleIdentifier</key>
    <string>{bundle}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleLocalizations</key>
    <array>{localizations}
    </array>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>LSMinimumSystemVersion</key>
    <string>{minimum_system}</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticTermination</key>
    <false/>
</dict>
</plist>
"#,
        region = xml(&app.languages.default),
        name = xml(name),
        executable = xml(executable),
        bundle = xml(&app.app.bundle_id),
        version = xml(&app.app.version),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_become_positional_specifiers() {
        let params = vec!["count".to_string(), "name".to_string()];
        assert_eq!(
            format_string("{name} 已播放 {count} 集 100%", &params),
            "%2$@ 已播放 %1$@ 集 100%%"
        );
        assert_eq!(format_string("100% {{x}}", &[]), "100% {x}");
        assert_eq!(escape_strings("a\"b\n"), "a\\\"b\\n");
    }
}
