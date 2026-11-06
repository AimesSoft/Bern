//! The installer `bern package` makes: `setup.exe`, a small C++ program
//! with a window of its own and the application's files inside, compressed
//! as one resource (see `templates/installer/`), compiled with the same
//! Visual Studio tools as the application, and signed with it when a
//! certificate is set.

use crate::glue::{architecture, visual_studio_setup};
use crate::product_name;
use bern_ir::Application;
use std::path::{Path, PathBuf};

pub const SETUP_CPP: &str = include_str!("../templates/installer/setup.cpp");
pub const PACK_CPP: &str = include_str!("../templates/installer/pack.cpp");

/// The installer's texts: `{name}` is filled in when it runs. The first
/// language is the fallback.
const TEXTS: &[(&str, [&str; 14])] = &[
    (
        "en",
        [
            "{name} Setup",
            "Install",
            "Update",
            "Reinstall",
            "Installing…",
            "Installed",
            "Open",
            "Uninstall",
            "Uninstalling…",
            "Uninstalled",
            "Done",
            "Try Again",
            "Close {name} first",
            "Could not finish. Check that the disk has room.",
        ],
    ),
    (
        "zh-Hans",
        [
            "{name} 安装程序",
            "安装",
            "更新",
            "重新安装",
            "正在安装…",
            "已安装",
            "打开",
            "卸载",
            "正在卸载…",
            "已卸载",
            "完成",
            "重试",
            "请先关闭 {name}",
            "未能完成，请确认磁盘空间足够",
        ],
    ),
    (
        "zh-Hant",
        [
            "{name} 安裝程式",
            "安裝",
            "更新",
            "重新安裝",
            "正在安裝…",
            "已安裝",
            "開啟",
            "解除安裝",
            "正在解除安裝…",
            "已解除安裝",
            "完成",
            "重試",
            "請先關閉 {name}",
            "未能完成，請確認磁碟空間足夠",
        ],
    ),
    (
        "ja",
        [
            "{name} セットアップ",
            "インストール",
            "アップデート",
            "再インストール",
            "インストール中…",
            "インストールしました",
            "開く",
            "アンインストール",
            "アンインストール中…",
            "アンインストールしました",
            "完了",
            "再試行",
            "{name} を閉じてください",
            "完了できませんでした。ディスクの空き容量を確認してください",
        ],
    ),
];

/// A wide C++ string literal.
fn wide(text: &str) -> String {
    let mut out = String::from("L\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The application's name in `language`, else in its default language.
fn name_in(app: &Application, language: &str) -> String {
    app.app
        .name
        .get(language)
        .or_else(|| app.app.name.get(&app.languages.default))
        .unwrap_or(&app.app.id)
        .to_string()
}

/// Who publishes the application, as installers show it.
pub fn publisher(app: &Application) -> String {
    app.app
        .publisher
        .clone()
        .unwrap_or_else(|| name_in(app, &app.languages.default))
}

/// One file the installer carries: where it is staged and its path in the
/// installed folder.
pub struct Payload {
    pub source: PathBuf,
    pub path: PathBuf,
    pub size: u64,
}

/// The files in `stage`, with their sizes.
pub fn payload(files: &[(PathBuf, PathBuf)]) -> Vec<Payload> {
    files
        .iter()
        .map(|(source, path)| Payload {
            source: source.clone(),
            path: path.clone(),
            size: std::fs::metadata(source)
                .map(|metadata| metadata.len())
                .unwrap_or(0),
        })
        .collect()
}

fn windows(path: &Path) -> String {
    path.display().to_string().replace('/', "\\")
}

/// What the installer tells Windows about the files the application opens:
/// `.ext`, `folder` or `*`, each with the index of its context menu item,
/// -1 for none. Only an extension takes part in “Open with” and the default
/// apps, so the others stand here only with an item.
fn file_types(app: &Application) -> Vec<(String, i32)> {
    let mut types: Vec<(String, i32)> = Vec::new();
    let mut add = |kind: &str, menu: i32| {
        let names: Vec<String> = match kind {
            "any" => vec!["*".to_string()],
            "folder" => vec!["folder".to_string()],
            ext if ext.starts_with('.') => vec![ext.to_string()],
            kind => bern_ir::spec::file_kind_extensions(kind)
                .iter()
                .map(|ext| format!(".{ext}"))
                .collect(),
        };
        for name in names {
            let known = types
                .iter()
                .any(|(type_, item)| *type_ == name && (*item == menu || menu < 0));
            if !known && (menu >= 0 || name.starts_with('.')) {
                types.push((name, menu));
            }
        }
    };
    let mut items = 0;
    for opens in &app.app.opens {
        let menu = if opens.menu.is_empty() {
            -1
        } else {
            items += 1;
            items - 1
        };
        for kind in opens.kinds() {
            add(kind, menu);
        }
    }
    for kind in app.opened_kinds() {
        add(&kind, -1);
    }
    types
}

/// The texts of the context menu items in `language`, in the order
/// [`file_types`] counts them.
fn menu_texts<'a>(app: &'a Application, language: &str) -> Vec<&'a str> {
    app.app
        .opens
        .iter()
        .filter_map(|opens| opens.menu_text(language, &app.languages.default))
        .collect()
}

/// `setup_info.h`: what the installer installs and how it talks.
pub fn setup_info(app: &Application, executable: &str, files: &[Payload]) -> String {
    let default_name = name_in(app, &app.languages.default);
    // A file name: characters Windows refuses are left out.
    let shortcut: String = default_name
        .chars()
        .filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
        .collect();
    let size: u64 = files.iter().map(|file| file.size).sum();
    let mut out = String::from(
        "// Generated by Bern. Do not edit.\n#pragma once\n\n#include <windows.h>\n\n\
         struct Texts {\n    wchar_t const* language;\n    wchar_t const* name;\n    wchar_t const* title;\n    \
         wchar_t const* install;\n    wchar_t const* update;\n    wchar_t const* reinstall;\n    \
         wchar_t const* installing;\n    wchar_t const* installed;\n    wchar_t const* open;\n    \
         wchar_t const* uninstall;\n    wchar_t const* uninstalling;\n    wchar_t const* uninstalled;\n    \
         wchar_t const* done;\n    wchar_t const* retry;\n    wchar_t const* close_first;\n    \
         wchar_t const* failed;\n    wchar_t const* menus[kMenus];\n};\n\n\
         struct FileType {\n    wchar_t const* type;\n    int menu;\n};\n\n",
    );
    // An array needs a length: without items it holds one empty text.
    let items = menu_texts(app, &app.languages.default).len();
    out = out.replace(
        "struct Texts {",
        &format!(
            "inline constexpr int kMenus = {};\n\nstruct Texts {{",
            items.max(1)
        ),
    );
    for (name, value) in [
        ("kKey", app.app.bundle_id.clone()),
        ("kFolder", product_name(app)),
        ("kShortcut", shortcut),
        ("kDisplayName", default_name),
        ("kExecutable", format!("{executable}.exe")),
        ("kVersion", app.app.version.clone()),
        ("kPublisher", publisher(app)),
    ] {
        out.push_str(&format!(
            "inline constexpr wchar_t {name}[] = {};\n",
            wide(&value)
        ));
    }
    out.push_str(&format!(
        "inline constexpr DWORD kSizeKB = {};\n\ninline constexpr Texts kTexts[] = {{\n",
        size.div_ceil(1024)
    ));
    for (language, texts) in TEXTS {
        let mut fields = vec![wide(language), wide(&name_in(app, language))];
        fields.extend(texts.iter().map(|text| wide(text)));
        let menus: Vec<String> = menu_texts(app, language).into_iter().map(wide).collect();
        fields.push(format!("{{{}}}", menus.join(", ")));
        out.push_str(&format!("    {{{}}},\n", fields.join(", ")));
    }
    out.push_str(
        "};\n\n// The last entry ends the list.\ninline constexpr FileType kFileTypes[] = {\n",
    );
    for (name, menu) in file_types(app) {
        out.push_str(&format!("    {{{}, {menu}}},\n", wide(&name)));
    }
    out.push_str("    {nullptr, -1},\n};\n\ninline constexpr wchar_t const* kFiles[] = {\n");
    for file in files {
        out.push_str(&format!("    {},\n", wide(&windows(&file.path))));
    }
    out.push_str("};\n");
    out
}

/// A string in a resource script.
fn rc_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\"\""))
}

/// `setup.rc`: the icon, the manifest, the version and the packed files.
pub fn setup_rc(app: &Application, icon: &Path) -> String {
    let name = name_in(app, &app.languages.default);
    let mut parts = app
        .app
        .version
        .split(['.', '-', '+'])
        .map(|part| part.parse::<u16>().unwrap_or(0));
    let [major, minor, patch] = [(); 3].map(|_| parts.next().unwrap_or(0));
    format!(
        r#"// Generated by Bern. Do not edit.
#pragma code_page(65001)
#include <winver.h>

1 ICON {icon}
// The manifest asks for no elevation: without it Windows takes a program
// called setup for an installer that needs administrator rights.
1 24 "setup.manifest"

1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", {publisher}
            VALUE "FileDescription", {description}
            VALUE "FileVersion", {version}
            VALUE "ProductName", {product}
            VALUE "ProductVersion", {version}
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END

1001 RCDATA "payload.bin"
"#,
        icon = rc_string(&windows(icon)),
        publisher = rc_string(&publisher(app)),
        description = rc_string(&format!("{name} Setup")),
        version = rc_string(&app.app.version),
        product = rc_string(&name),
    )
}

pub const SETUP_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>
"#;

/// `package.bat`: signs the staged programs when `BERN_WINDOWS_CERTIFICATE`
/// is set, compresses `staged`, compiles `setup.exe` to `setup`, and signs
/// it. The certificate and its password stay in the environment, never in
/// the script.
pub fn package_script(programs: &[PathBuf], staged: &Path, setup: &Path) -> String {
    let programs = programs
        .iter()
        .map(|program| format!("\"{}\"", windows(program)))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        r#"@echo off
rem Generated by Bern. Do not edit.
rem Makes the installer: signs the staged programs when a certificate is
rem set, compresses them, compiles setup.exe with them inside, signs it.
setlocal EnableExtensions
set "HERE=%~dp0"
set "HERE=%HERE:~0,-1%"
set "SETUP={setup}"

{visual_studio}
if defined BERN_WINDOWS_CERTIFICATE (
    call :sign {programs} || goto failed
)
set "OPTIONS=/nologo /O1 /MT /EHsc /std:c++20 /utf-8 /W3 /DUNICODE /D_UNICODE /DNOMINMAX /DWIN32_LEAN_AND_MEAN"
cl %OPTIONS% /I"%HERE%" /Fo"%HERE%\pack.obj" /Fe"%HERE%\pack.exe" "%HERE%\pack.cpp" cabinet.lib || goto failed
"%HERE%\pack.exe" "{staged}" "%HERE%\payload.bin" || goto failed
rc /nologo /fo "%HERE%\setup.res" "%HERE%\setup.rc" || goto failed
cl %OPTIONS% /c /I"%HERE%" /Fo"%HERE%\setup.obj" "%HERE%\setup.cpp" || goto failed
link /nologo /OUT:"%SETUP%" /SUBSYSTEM:WINDOWS /OPT:REF /OPT:ICF "%HERE%\setup.obj" "%HERE%\setup.res" user32.lib shell32.lib ole32.lib advapi32.lib uuid.lib comctl32.lib d2d1.lib dwrite.lib dwmapi.lib windowscodecs.lib cabinet.lib || goto failed
if defined BERN_WINDOWS_CERTIFICATE (
    call :sign "%SETUP%" || goto failed
)
exit /b 0

rem A .pfx file with its password, or the thumbprint of a certificate in
rem the user's store; RFC 3161 timestamps keep signatures valid after the
rem certificate expires.
:sign
if exist "%BERN_WINDOWS_CERTIFICATE%" (
    signtool sign /q /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f "%BERN_WINDOWS_CERTIFICATE%" /p "%BERN_WINDOWS_CERTIFICATE_PASSWORD%" %* || exit /b 1
) else (
    signtool sign /q /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /sha1 %BERN_WINDOWS_CERTIFICATE% %* || exit /b 1
)
exit /b 0

:no_vs
echo bern: packaging for Windows needs the Visual Studio C++ build tools ^("Desktop development with C++"^).
exit /b 1
:failed
exit /b 1
"#,
        setup = windows(setup),
        staged = windows(staged),
        visual_studio = visual_studio_setup(),
    )
    .replace('\n', "\r\n")
}

/// The installer's file name.
pub fn setup_name(app: &Application) -> String {
    format!(
        "{}-{}-windows-{}-setup.exe",
        product_name(app),
        app.app.version,
        architecture()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello() -> Application {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello/Bern.toml");
        let config = bern_compiler::config::load(&manifest).unwrap();
        bern_compiler::compile(&config).unwrap().app
    }

    #[test]
    fn the_installer_carries_every_file_and_speaks_four_languages() {
        let app = hello();
        let files = vec![
            Payload {
                source: PathBuf::from(r"C:\stage\Hello\hello.exe"),
                path: PathBuf::from("hello.exe"),
                size: 3000,
            },
            Payload {
                source: PathBuf::from(r"C:\stage\Hello\bin\hello.exe"),
                path: PathBuf::from("bin/hello.exe"),
                size: 2000,
            },
        ];
        let info = setup_info(&app, "hello", &files);
        for expected in [
            r#"kKey[] = L"dev.bern.hello";"#,
            r#"kFolder[] = L"Hello";"#,
            r#"kExecutable[] = L"hello.exe";"#,
            "kSizeKB = 5;",
            r#"L"hello.exe","#,
            r#"L"bin\\hello.exe","#,
            r#"L"zh-Hans", L"你好""#,
            r#"L"ja", L"Hello""#,
        ] {
            assert!(info.contains(expected), "missing `{expected}` in\n{info}");
        }
        // English comes first: the fallback for other languages.
        let first = info.find("L\"en\"").unwrap();
        assert!(info.find("L\"zh-Hans\"").unwrap() > first);

        let script = setup_rc(&app, Path::new(r"C:\out\app\app.ico"));
        assert!(
            script.contains(r#"1 ICON "C:\\out\\app\\app.ico""#),
            "{script}"
        );
        assert!(script.contains(r#"1001 RCDATA "payload.bin""#));
        assert!(script.contains(r#"1 24 "setup.manifest""#));
        assert!(SETUP_MANIFEST.contains(r#"level="asInvoker""#));
    }

    #[test]
    fn every_language_has_every_text() {
        let fields = TEXTS[0].1.len();
        assert!(TEXTS.iter().all(|(_, texts)| texts.len() == fields));
        let header = setup_info(&hello(), "hello", &[]);
        // Each text in `Texts`, with the language and the name before them.
        assert_eq!(
            header.matches("wchar_t const* ").count(),
            fields + 2 + 3,
            "and the menus, a file type and kFiles"
        );
    }

    #[test]
    fn the_installer_knows_what_the_application_opens() {
        let opens = |files: &str, menu: &str| bern_ir::OpensDef {
            files: files.to_string(),
            menu: [("en".to_string(), menu.to_string())]
                .into_iter()
                .filter(|_| !menu.is_empty())
                .collect(),
        };
        let mut app = hello();
        app.languages.default = "en".to_string();
        let plain = setup_info(&app, "hello", &[]);
        assert!(plain.contains("kMenus = 1;"), "{plain}");
        assert!(plain.contains("kFileTypes[] = {\n    {nullptr, -1},"), "{plain}");
        app.app.opens = vec![
            opens("audio folder", "Play"),
            opens(".lrc", ""),
            opens("any", "Inspect"),
        ];
        let info = setup_info(&app, "hello", &[]);
        for expected in [
            "kMenus = 2;",
            r#"{L".mp3", 0},"#,
            r#"{L"folder", 0},"#,
            r#"{L".lrc", -1},"#,
            r#"{L"*", 1},"#,
            r#"{L"Play", L"Inspect"}},"#,
        ] {
            assert!(info.contains(expected), "{expected} in {info}");
        }
    }

    #[test]
    fn the_files_are_compressed_after_they_are_signed() {
        let script = package_script(
            &[PathBuf::from(r"C:\stage\Hello\hello.exe")],
            Path::new(r"C:\stage\Hello"),
            Path::new(r"C:\dist\Hello-setup.exe"),
        );
        let signed = script.find("call :sign \"C:\\stage").unwrap();
        let packed = script.find(r#"pack.exe" "C:\stage\Hello""#).unwrap();
        let compiled = script.find(r#"rc /nologo"#).unwrap();
        assert!(signed < packed && packed < compiled, "{script}");
    }

    #[test]
    fn signing_stays_in_the_environment() {
        let script = package_script(
            &[PathBuf::from(r"C:\stage\Hello\hello.exe")],
            Path::new(r"C:\stage\Hello"),
            Path::new(r"C:\dist\Hello-setup.exe"),
        );
        assert!(script.contains("if defined BERN_WINDOWS_CERTIFICATE"));
        assert!(script.contains(r#"call :sign "C:\stage\Hello\hello.exe""#));
        assert!(script.contains("%BERN_WINDOWS_CERTIFICATE_PASSWORD%"));
        assert!(script.contains("\r\n"), "CRLF lines");
    }
}
