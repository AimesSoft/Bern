//! Texts every native runtime shows itself, the per-language tables of
//! runtimes that fill Bern's named placeholders, and the conversion of
//! those placeholders to a platform's positional format specifiers.

use bern_ir::Application;
use bern_protocol::LocalizedText;

/// Strings the native runtimes show themselves, with built-in translations.
/// Every runtime receives all of them. An argument is `%@` in the texts the
/// SwiftUI and Compose runtimes format, which convert it to their own
/// specifier, and a `{name}` placeholder in the texts the GTK, WinUI 3 and
/// Web runtimes fill by name.
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
        "bern.window.reopen",
        &[
            ("zh-Hans", "打开“{title}”"),
            ("zh-Hant", "打開「{title}」"),
            ("en", "Open {title}"),
            ("ja", "「{title}」を開く"),
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
        "bern.runtime.missing",
        &[
            (
                "zh-Hans",
                "此应用需要 Windows App Runtime 2，这台电脑上还没有。\n\n要现在用浏览器下载微软的安装程序吗？安装完成后请重新打开此应用。",
            ),
            (
                "zh-Hant",
                "此 App 需要 Windows App Runtime 2，這台電腦上還沒有。\n\n要現在以瀏覽器下載 Microsoft 的安裝程式嗎？安裝完成後請重新開啟此 App。",
            ),
            (
                "en",
                "This app needs the Windows App Runtime 2, which this computer doesn’t have yet.\n\nDownload Microsoft’s installer in your browser now? Open the app again once it’s installed.",
            ),
            (
                "ja",
                "このアプリには Windows App Runtime 2 が必要ですが、このコンピューターにはまだありません。\n\nMicrosoft のインストーラーを今すぐブラウザーでダウンロードしますか？インストール後にアプリをもう一度開いてください。",
            ),
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
/// without parameters are not formatted, so only their braces are
/// unescaped.
pub fn positional(text: &str, params: &[String], specifier: impl Fn(usize) -> String) -> String {
    if params.is_empty() {
        return unescape_braces(text);
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

/// A text without parameters as it is shown: `{{` and `}}` stand for one
/// brace, and nothing is filled in at run time.
pub fn unescape_braces(text: &str) -> String {
    text.replace("{{", "{").replace("}}", "}")
}

/// `(language, [(key, text)])` for every supported language, for the
/// runtimes that fill Bern's placeholders by name: the declared texts, the
/// application's name as `app.name`, the framework texts, and the weekday
/// and month names, which fall back to English in a language Bern has none
/// for. Texts with parameters keep their format and escapes for the
/// runtime; the others are stored as they are shown.
pub fn tables(app: &Application) -> Vec<(String, Vec<(String, String)>)> {
    let default = &app.languages.default;
    app.languages
        .supported
        .iter()
        .map(|language| {
            let mut table: Vec<(String, String)> = app
                .texts
                .iter()
                .map(|text| {
                    let value = translation(text, language, default);
                    let value = if text.params.is_empty() {
                        unescape_braces(value)
                    } else {
                        value.to_string()
                    };
                    (text.key.clone(), value)
                })
                .collect();
            let name = app
                .app
                .name
                .get(language)
                .or_else(|| app.app.name.get(default))
                .unwrap_or(&app.app.id);
            table.push(("app.name".into(), name.to_string()));
            for (key, translations) in FRAMEWORK_TEXTS {
                table.push(((*key).into(), framework(translations, language).into()));
            }
            for (key, translations) in bern_protocol::format::date_names() {
                let value = translations
                    .iter()
                    .find(|(candidate, _)| candidate == language)
                    .or_else(|| {
                        translations
                            .iter()
                            .find(|(candidate, _)| *candidate == "en")
                    })
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                table.push((key, value));
            }
            (language.clone(), table)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bern_ir::{AppInfo, Languages, Theme, ThemeMode, WindowCatalog};
    use bern_protocol::{ProtocolDescriptor, TypeTable};
    use std::path::Path;

    fn text(key: &str, value: &str, params: &[&str]) -> LocalizedText {
        LocalizedText {
            key: key.into(),
            translations: [("en".to_string(), value.to_string())].into(),
            params: params.iter().map(|param| param.to_string()).collect(),
        }
    }

    fn app(texts: Vec<LocalizedText>) -> Application {
        Application {
            ir_version: bern_ir::IR_VERSION,
            app: AppInfo {
                id: "test".into(),
                bundle_id: "test.app".into(),
                name: LocalizedText::default(),
                version: "1".into(),
                icon: None,
                platform_icons: Default::default(),
                publisher: None,
            },
            languages: Languages {
                supported: vec!["en".into()],
                default: "en".into(),
                strict: true,
            },
            theme: Theme {
                mode: ThemeMode::System,
                brand_seed: None,
            },
            protocol: ProtocolDescriptor {
                descriptor_version: 1,
                namespace: "test".into(),
                version: 1,
                types: TypeTable::default(),
                state: vec![],
                commands: vec![],
                events: vec![],
                modules: vec![],
            },
            types: TypeTable::default(),
            sessions: vec![],
            windows: WindowCatalog {
                handle: "Windows".into(),
                primary: "main".into(),
                windows: vec![],
                accent: None,
                appearance: None,
            },
            players: vec![],
            components: vec![],
            texts,
            resources: vec![],
            layouts: None,
            contracts: vec![],
            style: Default::default(),
        }
    }

    #[test]
    fn every_framework_text_has_every_framework_language_once() {
        let mut languages = framework_languages();
        languages.sort_unstable();
        let mut keys = Vec::new();
        for (key, translations) in FRAMEWORK_TEXTS {
            let mut found: Vec<_> = translations.iter().map(|(language, _)| *language).collect();
            found.sort_unstable();
            assert_eq!(found, languages, "{key}");
            assert!(!keys.contains(key), "{key} is defined twice");
            keys.push(*key);
        }
    }

    /// Every `"bern.…"` key the runtimes and generators show is defined, so
    /// a new runtime text cannot ship without its translations.
    #[test]
    fn every_text_a_runtime_shows_is_defined() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut dirs = vec![crates.join("bern-gtk/src")];
        for backend in ["swiftui", "compose", "gtk", "winui3", "web"] {
            let backend = crates.join(format!("bern-backend-{backend}"));
            dirs.push(backend.join("src"));
            dirs.push(backend.join("templates"));
        }
        let mut used = Vec::new();
        // Not every backend has templates of its own.
        while let Some(dir) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                let Ok(source) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for (start, _) in source.match_indices("\"bern.") {
                    let key: String = source[start + 1..]
                        .chars()
                        .take_while(|c| c.is_ascii_lowercase() || *c == '_' || *c == '.')
                        .collect();
                    let quoted = source[start + 1 + key.len()..].starts_with(['"', '\\']);
                    if quoted && key.matches('.').count() >= 2 {
                        used.push((key, path.clone()));
                    }
                }
            }
        }
        assert!(
            used.len() > FRAMEWORK_TEXTS.len(),
            "the sources were not found"
        );
        let mut defined: Vec<String> = FRAMEWORK_TEXTS
            .iter()
            .map(|(key, _)| (*key).to_string())
            .collect();
        defined.extend(
            bern_protocol::format::date_names()
                .into_iter()
                .map(|(key, _)| key),
        );
        for (key, path) in used {
            // A key ending in `.` is the start of keys the runtime completes
            // as it runs, such as `bern.date.month.` and the month's number.
            let found = match key.strip_suffix('.') {
                Some(_) => defined.iter().any(|defined| defined.starts_with(&key)),
                None => defined.contains(&key),
            };
            assert!(
                found,
                "{} shows `{key}`, which neither FRAMEWORK_TEXTS nor the date names define",
                path.display()
            );
        }
    }

    #[test]
    fn tables_store_texts_without_parameters_as_shown() {
        let app = app(vec![
            text("plain", "Use {{x}} and }}", &[]),
            text("filled", "{n} of {{all}}", &["n"]),
        ]);
        let tables = tables(&app);
        assert_eq!(tables.len(), 1);
        let (language, table) = &tables[0];
        assert_eq!(language, "en");
        let get = |key: &str| {
            table
                .iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(get("plain"), Some("Use {x} and }"));
        // The runtime fills the placeholders and resolves the escapes.
        assert_eq!(get("filled"), Some("{n} of {{all}}"));
        // Without a translated name, the application id stands in.
        assert_eq!(get("app.name"), Some("test"));
        assert_eq!(get("bern.media.empty"), Some("No Video"));
        assert_eq!(
            table.len(),
            3 + FRAMEWORK_TEXTS.len() + bern_protocol::format::date_names().len()
        );
    }

    #[test]
    fn positional_specifiers_and_escapes() {
        let params = ["name".to_string(), "count".to_string()];
        assert_eq!(
            positional("{count} for {name}, 100% {{sure}}", &params, |n| format!(
                "%{n}$s"
            )),
            "%2$s for %1$s, 100%% {sure}"
        );
        assert_eq!(
            positional("100% {{sure}}", &[], |_| unreachable!()),
            "100% {sure}"
        );
    }
}
