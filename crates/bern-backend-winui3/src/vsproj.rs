//! Standalone Visual Studio project (`.sln` + `.vcxproj` + `.vcxproj.filters`)
//! ejection for Windows (`winui3 + windows`).
//!
//! Organizes C++ sources into `src/catalog/`, `src/ui/views/`,
//! `src/ui/windows/`, `src/native/`, and `src/runtime/`, bundles all resources
//! and icons under `resources/`, and wires either:
//! - `EjectEngine::Source`: an MSBuild `BuildRustEngine` target that runs
//!   `scripts\build-rust.cmd` against the self-contained offline Rust workspace
//!   in `rust/`; or
//! - `EjectEngine::Prebuilt`: a pre-compiled `lib\<arch>\bern_app.lib` so
//!   Visual Studio builds and debugs with F5 without Rust installed.
//!
//! Windows App SDK packages and C++/WinRT headers are prepared by
//! `scripts\prepare-sdk.cmd` using Windows' built-in `curl.exe`, `tar.exe`,
//! and `cppwinrt.exe` (no PowerShell).

use crate::cpp::Generator;
use crate::glue::{self, Frontends, PACKAGES};
use crate::{RUNTIME, alternate_ico, app_ico, executable, product_name, status_icos};
use bern_ir::Application;
use bern_target_api::eject::{
    EjectEngine, EjectOptions, EjectedProject, clean_generated_file, copy_declared_resources,
    readme, sanitized_context,
};
use bern_target_api::{BuildContext, BuildStep, Diagnostic, GeneratedFile, icon, skill};
use std::path::{Path, PathBuf};

/// Deterministic uppercase GUID formatted as `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`.
fn guid(seed: &str) -> String {
    let mut h1: u64 = 0xcbf29ce484222325;
    let mut h2: u64 = 0x100000001b3u64.wrapping_mul(0x84222325cbf29ce4);
    for b in seed.as_bytes() {
        h1 ^= u64::from(*b);
        h1 = h1.wrapping_mul(0x100000001b3);
        h2 ^= u64::from(*b).wrapping_mul(31);
        h2 = h2.wrapping_mul(0x100000001b3);
    }
    let d1 = (h1 >> 32) as u32;
    let d2 = ((h1 >> 16) & 0xffff) as u16;
    let d3 = ((h1 & 0x0fff) as u16) | 0x4000;
    let d4 = (((h2 >> 48) & 0x3fff) as u16) | 0x8000;
    let d5 = h2 & 0x0000_ffff_ffff_ffff;
    format!("{{{d1:08X}-{d2:04X}-{d3:04X}-{d4:04X}-{d5:012X}}}")
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn win_rel(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

fn package_version(id: &str) -> &'static str {
    PACKAGES
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, version)| *version)
        .expect("Windows App SDK package version")
}

fn generate_sln(product: &str, project_guid: &str) -> String {
    let cpp_type_guid = "{8BC9CEB8-8B4A-11D0-8D11-00A0C91BC942}";
    let sln_guid = guid(&format!("sln:{product}"));
    format!(
        r#"Microsoft Visual Studio Solution File, Format Version 12.00
# Visual Studio Version 17
VisualStudioVersion = 17.0.31903.59
MinimumVisualStudioVersion = 10.0.40219.1
Project("{cpp_type_guid}") = "{product}", "{product}.vcxproj", "{project_guid}"
EndProject
Global
	GlobalSection(SolutionConfigurationPlatforms) = preSolution
		Debug|ARM64 = Debug|ARM64
		Debug|x64 = Debug|x64
		Release|ARM64 = Release|ARM64
		Release|x64 = Release|x64
	EndGlobalSection
	GlobalSection(ProjectConfigurationPlatforms) = postSolution
		{project_guid}.Debug|ARM64.ActiveCfg = Debug|ARM64
		{project_guid}.Debug|ARM64.Build.0 = Debug|ARM64
		{project_guid}.Debug|x64.ActiveCfg = Debug|x64
		{project_guid}.Debug|x64.Build.0 = Debug|x64
		{project_guid}.Release|ARM64.ActiveCfg = Release|ARM64
		{project_guid}.Release|ARM64.Build.0 = Release|ARM64
		{project_guid}.Release|x64.ActiveCfg = Release|x64
		{project_guid}.Release|x64.Build.0 = Release|x64
	EndGlobalSection
	GlobalSection(SolutionProperties) = preSolution
		HideSolutionNode = FALSE
	EndGlobalSection
	GlobalSection(ExtensibilityGlobals) = postSolution
		SolutionGuid = {sln_guid}
	EndGlobalSection
EndGlobal
"#
    )
    .replace('\n', "\r\n")
}

/// Batch helper invoked by MSBuild before C++ compilation to fetch Windows App
/// SDK packages (via built-in `curl.exe` + `tar.exe`) and generate C++/WinRT
/// headers via the installed Windows SDK's `cppwinrt.exe`.
fn prepare_sdk_script() -> String {
    let foundation = package_version("microsoft.windowsappsdk.foundation");
    let winui = package_version("microsoft.windowsappsdk.winui");
    let ix = package_version("microsoft.windowsappsdk.interactiveexperiences");
    let webview2 = package_version("microsoft.web.webview2");
    format!(
        r#"@echo off
rem Prepare Windows App SDK packages and C++/WinRT headers into .sdk-cache\.
rem Uses Windows' built-in curl.exe, tar.exe and cppwinrt.exe. Edit freely.
setlocal EnableExtensions

set "CACHE=%~1"
set "SDK_BIN=%~2"
set "SDK_VER=%~3"
if "%CACHE%"=="" set "CACHE=%~dp0..\.sdk-cache"
if not exist "%CACHE%" mkdir "%CACHE%"

set "PACKAGES=%NUGET_PACKAGES%"
if not defined PACKAGES set "PACKAGES=%USERPROFILE%\.nuget\packages"

call :ensure_pkg microsoft.windowsappsdk.foundation {foundation} foundation || exit /b 1
call :ensure_pkg microsoft.windowsappsdk.winui {winui} winui || exit /b 1
call :ensure_pkg microsoft.windowsappsdk.interactiveexperiences {ix} ix || exit /b 1
call :ensure_pkg microsoft.web.webview2 {webview2} webview2 || exit /b 1

set "WINRT=%CACHE%\winrt"
if exist "%WINRT%\winrt\Microsoft.UI.Xaml.h" exit /b 0

set "CPPWINRT=%SDK_BIN%x64\cppwinrt.exe"
if not exist "%CPPWINRT%" set "CPPWINRT=%SDK_BIN%arm64\cppwinrt.exe"
if not exist "%CPPWINRT%" set "CPPWINRT=%SDK_BIN%x86\cppwinrt.exe"
if not exist "%CPPWINRT%" (
    for /f "delims=" %%p in ('where cppwinrt.exe 2^>nul') do set "CPPWINRT=%%p"
)
if not exist "%CPPWINRT%" (
    echo error: cppwinrt.exe not found in Windows SDK ^(%SDK_BIN%^). Install a Windows 10/11 SDK with C++/WinRT. >&2
    exit /b 1
)

if "%SDK_VER%"=="" set "SDK_VER=local"
echo Generating C++/WinRT headers for Windows SDK %SDK_VER%
"%CPPWINRT%" -input "%SDK_VER%+" -input "%CACHE%\winui\metadata" -input "%CACHE%\ix\metadata\10.0.18362.0" -input "%CACHE%\foundation\metadata" -input "%CACHE%\webview2\lib\Microsoft.Web.WebView2.Core.winmd" -output "%WINRT%" || exit /b 1
exit /b 0

:ensure_pkg
set "PKG_ID=%~1"
set "PKG_VER=%~2"
set "PKG_SLOT=%~3"
set "DEST=%CACHE%\%PKG_SLOT%"
if exist "%DEST%\.version" (
    set /p CUR_VER=<"%DEST%\.version"
) else (
    set "CUR_VER="
)
if "%CUR_VER%"=="%PKG_VER%" exit /b 0
if exist "%DEST%" rmdir /s /q "%DEST%"
if exist "%PACKAGES%\%PKG_ID%\%PKG_VER%\" (
    xcopy /e /i /q /y "%PACKAGES%\%PKG_ID%\%PKG_VER%" "%DEST%" >nul || exit /b 1
    >"%DEST%\.version" echo %PKG_VER%
    exit /b 0
)
echo Fetching %PKG_ID% %PKG_VER% from nuget.org
mkdir "%DEST%" || exit /b 1
curl.exe --fail --location --silent --show-error --output "%DEST%.nupkg" "https://api.nuget.org/v3-flatcontainer/%PKG_ID%/%PKG_VER%/%PKG_ID%.%PKG_VER%.nupkg" || exit /b 1
tar.exe -xf "%DEST%.nupkg" -C "%DEST%" || exit /b 1
del /q "%DEST%.nupkg"
>"%DEST%\.version" echo %PKG_VER%
exit /b 0
"#
    )
    .replace('\n', "\r\n")
}

/// Batch helper invoked by MSBuild in `EjectEngine::Source` mode to compile
/// the vendored Rust workspace for the active Visual Studio configuration and
/// platform.
fn build_rust_script() -> String {
    r#"@echo off
rem Build the vendored Rust engine static library for Visual Studio. Edit freely.
setlocal EnableExtensions

set "ROOT=%~1"
set "CONFIG=%~2"
set "PLAT=%~3"
set "OUT_LIB=%~4"

if "%CONFIG%"=="" set "CONFIG=Debug"
if "%PLAT%"=="" set "PLAT=x64"

set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>&1 || (
    echo error: cargo not found on PATH; install Rust from https://rustup.rs >&2
    exit /b 1
)

if /i "%PLAT%"=="ARM64" (
    set "RUST_TARGET=aarch64-pc-windows-msvc"
) else (
    set "RUST_TARGET=x86_64-pc-windows-msvc"
)

set "PROFILE=debug"
set "CARGO_ARGS="
if /i "%CONFIG%"=="Release" (
    set "PROFILE=release"
    set "CARGO_ARGS=--release"
)

set "MANIFEST=%ROOT%rust\glue\Cargo.toml"
set "TARGET_DIR=%ROOT%rust\target"

pushd "%ROOT%rust\glue" || exit /b 1
cargo build --manifest-path "%MANIFEST%" --lib --target "%RUST_TARGET%" --target-dir "%TARGET_DIR%" %CARGO_ARGS% || (popd & exit /b 1)
popd

for %%d in ("%OUT_LIB%") do if not exist "%%~dpd" mkdir "%%~dpd"
copy /y "%TARGET_DIR%\%RUST_TARGET%\%PROFILE%\bern_app.lib" "%OUT_LIB%" >nul || exit /b 1
exit /b 0
"#
    .replace('\n', "\r\n")
}

struct VsProjSpec {
    product: String,
    executable: String,
    project_guid: String,
    frontends: Frontends,
    engine: EjectEngine,
    cpp_files: Vec<PathBuf>,
    header_files: Vec<PathBuf>,
    has_resources_dir: bool,
    has_alternates_dir: bool,
}

fn generate_vcxproj(spec: &VsProjSpec) -> String {
    let winui = u8::from(spec.frontends.winui);
    let islands = u8::from(spec.frontends.islands);
    let bootstrap_lib = if spec.frontends.winui {
        "$(ProjectDir).sdk-cache\\foundation\\lib\\native\\$(BernArch)\\Microsoft.WindowsAppRuntime.Bootstrap.lib;"
    } else {
        ""
    };
    let engine_lib = match spec.engine {
        EjectEngine::Source => "$(IntDir)bern_app.lib;",
        EjectEngine::Prebuilt => "$(ProjectDir)lib\\$(BernArch)\\bern_app.lib;",
    };
    let rust_target = if spec.engine == EjectEngine::Source {
        r#"  <Target Name="BuildRustEngine" BeforeTargets="Link">
    <Exec Command="call &quot;$(ProjectDir)scripts\build-rust.cmd&quot; &quot;$(ProjectDir)&quot; &quot;$(Configuration)&quot; &quot;$(Platform)&quot; &quot;$(IntDir)bern_app.lib&quot;" />
  </Target>
"#
    } else {
        ""
    };

    let mut post_build_cmds = Vec::new();
    if spec.frontends.winui {
        post_build_cmds.push(
            "copy /y \"$(ProjectDir).sdk-cache\\foundation\\runtimes\\win-$(BernArch)\\native\\Microsoft.WindowsAppRuntime.Bootstrap.dll\" \"$(OutDir)\" >nul",
        );
    }
    if spec.has_alternates_dir {
        post_build_cmds.push(
            "xcopy /i /q /y \"$(ProjectDir)resources\\alternates\\*.ico\" \"$(OutDir)\" >nul",
        );
    }
    if spec.has_resources_dir {
        post_build_cmds
            .push("xcopy /e /i /q /y \"$(ProjectDir)resources\\assets\" \"$(OutDir)\" >nul");
    }
    let post_build_block = if post_build_cmds.is_empty() {
        String::new()
    } else {
        format!(
            "    <PostBuildEvent>\n      <Command>{}</Command>\n    </PostBuildEvent>\n",
            xml_escape(&post_build_cmds.join("\r\n"))
        )
    };

    let mut compile_items = String::new();
    for path in &spec.cpp_files {
        let rel = win_rel(path);
        if path.file_name().is_some_and(|name| name == "bern_pch.cpp") {
            compile_items.push_str(&format!(
                "    <ClCompile Include=\"{rel}\">\n      <PrecompiledHeader>Create</PrecompiledHeader>\n    </ClCompile>\n"
            ));
        } else {
            compile_items.push_str(&format!("    <ClCompile Include=\"{rel}\" />\n"));
        }
    }
    let mut header_items = String::new();
    for path in &spec.header_files {
        header_items.push_str(&format!(
            "    <ClInclude Include=\"{}\" />\n",
            win_rel(path)
        ));
    }

    let mut configs = String::new();
    for (cfg, plat, release) in [
        ("Debug", "x64", false),
        ("Release", "x64", true),
        ("Debug", "ARM64", false),
        ("Release", "ARM64", true),
    ] {
        let opt = if release { "MinSpace" } else { "Disabled" };
        let debug_macro = if release { "NDEBUG;" } else { "" };
        let debug_info = if release { "None" } else { "OldStyle" };
        let link_debug = if release { "false" } else { "true" };
        configs.push_str(&format!(
            r#"  <ItemDefinitionGroup Condition="'$(Configuration)|$(Platform)'=='{cfg}|{plat}'">
    <ClCompile>
      <LanguageStandard>stdcpp20</LanguageStandard>
      <ExceptionHandling>Sync</ExceptionHandling>
      <RuntimeLibrary>MultiThreaded</RuntimeLibrary>
      <Optimization>{opt}</Optimization>
      <DebugInformationFormat>{debug_info}</DebugInformationFormat>
      <FunctionLevelLinking>true</FunctionLevelLinking>
      <MultiProcessorCompilation>true</MultiProcessorCompilation>
      <WarningLevel>Level3</WarningLevel>
      <DisableSpecificWarnings>4505;%(DisableSpecificWarnings)</DisableSpecificWarnings>
      <ConformanceMode>true</ConformanceMode>
      <PrecompiledHeader>Use</PrecompiledHeader>
      <PrecompiledHeaderFile>bern_pch.h</PrecompiledHeaderFile>
      <AdditionalOptions>/bigobj /utf-8 /Gw %(AdditionalOptions)</AdditionalOptions>
      <PreprocessorDefinitions>{debug_macro}UNICODE;_UNICODE;NOMINMAX;WIN32_LEAN_AND_MEAN;BERN_WITH_WINUI={winui};BERN_WITH_ISLANDS={islands};%(PreprocessorDefinitions)</PreprocessorDefinitions>
      <AdditionalIncludeDirectories>$(ProjectDir).sdk-cache\winrt;$(ProjectDir).sdk-cache\webview2\build\native\include;$(ProjectDir).sdk-cache\foundation\include;$(ProjectDir).sdk-cache\winui\include;$(ProjectDir).sdk-cache\ix\include;$(ProjectDir)src;$(ProjectDir)src\ui;$(ProjectDir)src\runtime;%(AdditionalIncludeDirectories)</AdditionalIncludeDirectories>
    </ClCompile>
    <ResourceCompile>
      <AdditionalIncludeDirectories>$(ProjectDir)resources;%(AdditionalIncludeDirectories)</AdditionalIncludeDirectories>
    </ResourceCompile>
    <Manifest>
      <AdditionalManifestFiles>$(ProjectDir)resources\app.manifest;%(AdditionalManifestFiles)</AdditionalManifestFiles>
    </Manifest>
    <Link>
      <SubSystem>Windows</SubSystem>
      <GenerateDebugInformation>{link_debug}</GenerateDebugInformation>
      <EnableCOMDATFolding>true</EnableCOMDATFolding>
      <OptimizeReferences>true</OptimizeReferences>
      <AdditionalLibraryDirectories>$(ProjectDir).sdk-cache\webview2\build\native\$(BernArch);%(AdditionalLibraryDirectories)</AdditionalLibraryDirectories>
      <AdditionalDependencies>{engine_lib}{bootstrap_lib}windowsapp.lib;kernel32.lib;user32.lib;gdi32.lib;ole32.lib;oleaut32.lib;advapi32.lib;shell32.lib;shlwapi.lib;dwmapi.lib;ntdll.lib;userenv.lib;ws2_32.lib;bcrypt.lib;dbghelp.lib;secur32.lib;crypt32.lib;ncrypt.lib;iphlpapi.lib;synchronization.lib;propsys.lib;%(AdditionalDependencies)</AdditionalDependencies>
    </Link>
{post_build_block}  </ItemDefinitionGroup>
"#
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Project DefaultTargets="Build" xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup Label="ProjectConfigurations">
    <ProjectConfiguration Include="Debug|x64">
      <Configuration>Debug</Configuration>
      <Platform>x64</Platform>
    </ProjectConfiguration>
    <ProjectConfiguration Include="Release|x64">
      <Configuration>Release</Configuration>
      <Platform>x64</Platform>
    </ProjectConfiguration>
    <ProjectConfiguration Include="Debug|ARM64">
      <Configuration>Debug</Configuration>
      <Platform>ARM64</Platform>
    </ProjectConfiguration>
    <ProjectConfiguration Include="Release|ARM64">
      <Configuration>Release</Configuration>
      <Platform>ARM64</Platform>
    </ProjectConfiguration>
  </ItemGroup>
  <PropertyGroup Label="Globals">
    <VCProjectVersion>17.0</VCProjectVersion>
    <ProjectGuid>{guid}</ProjectGuid>
    <Keyword>Win32Proj</Keyword>
    <RootNamespace>{product}</RootNamespace>
    <WindowsTargetPlatformVersion>10.0</WindowsTargetPlatformVersion>
  </PropertyGroup>
  <Import Project="$(VCTargetsPath)\Microsoft.Cpp.Default.props" />
  <PropertyGroup Label="Configuration">
    <ConfigurationType>Application</ConfigurationType>
    <PlatformToolset>v143</PlatformToolset>
    <CharacterSet>Unicode</CharacterSet>
  </PropertyGroup>
  <Import Project="$(VCTargetsPath)\Microsoft.Cpp.props" />
  <PropertyGroup>
    <TargetName>{exe}</TargetName>
    <OutDir>$(ProjectDir)build\$(Platform)\$(Configuration)\</OutDir>
    <IntDir>$(ProjectDir)obj\$(Platform)\$(Configuration)\</IntDir>
    <BernArch Condition="'$(Platform)'=='ARM64'">arm64</BernArch>
    <BernArch Condition="'$(Platform)'!='ARM64'">x64</BernArch>
  </PropertyGroup>
{configs}  <ItemGroup>
{compile_items}  </ItemGroup>
  <ItemGroup>
{header_items}  </ItemGroup>
  <ItemGroup>
    <ResourceCompile Include="resources\app.rc" />
    <Manifest Include="resources\app.manifest" />
  </ItemGroup>
  <Import Project="$(VCTargetsPath)\Microsoft.Cpp.targets" />
  <Target Name="PrepareWindowsAppSdk" BeforeTargets="ClCompile">
    <Exec Command="call &quot;$(ProjectDir)scripts\prepare-sdk.cmd&quot; &quot;$(ProjectDir).sdk-cache&quot; &quot;$(WindowsSdkVerBinPath)&quot; &quot;$(TargetPlatformVersion)&quot;" />
  </Target>
{rust_target}</Project>
"#,
        guid = spec.project_guid,
        product = xml_escape(&spec.product),
        exe = xml_escape(&spec.executable),
    )
    .replace('\n', "\r\n")
}

fn generate_vcxproj_filters(cpp_files: &[PathBuf], header_files: &[PathBuf]) -> String {
    let filter_for = |path: &Path| -> &'static str {
        let s = path.to_string_lossy().replace('\\', "/");
        if s.starts_with("src/catalog/") {
            "Catalog"
        } else if s.starts_with("src/ui/views/") {
            "UI\\Views"
        } else if s.starts_with("src/ui/windows/") {
            "UI\\Windows"
        } else if s.starts_with("src/ui/") {
            "UI"
        } else if s.starts_with("src/native/") {
            "Native"
        } else if s.starts_with("src/runtime/") {
            "Runtime"
        } else {
            "App"
        }
    };
    let filters = [
        "App",
        "Catalog",
        "UI",
        "UI\\Views",
        "UI\\Windows",
        "Native",
        "Runtime",
        "Resources",
    ];
    let mut filter_defs = String::new();
    for f in filters {
        filter_defs.push_str(&format!(
            "    <Filter Include=\"{f}\">\n      <UniqueIdentifier>{}</UniqueIdentifier>\n    </Filter>\n",
            guid(&format!("filter:{f}"))
        ));
    }
    let mut compile_items = String::new();
    for path in cpp_files {
        compile_items.push_str(&format!(
            "    <ClCompile Include=\"{}\">\n      <Filter>{}</Filter>\n    </ClCompile>\n",
            win_rel(path),
            filter_for(path)
        ));
    }
    let mut header_items = String::new();
    for path in header_files {
        header_items.push_str(&format!(
            "    <ClInclude Include=\"{}\">\n      <Filter>{}</Filter>\n    </ClInclude>\n",
            win_rel(path),
            filter_for(path)
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Project ToolsVersion="4.0" xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
{filter_defs}  </ItemGroup>
  <ItemGroup>
{compile_items}  </ItemGroup>
  <ItemGroup>
{header_items}  </ItemGroup>
  <ItemGroup>
    <ResourceCompile Include="resources\app.rc">
      <Filter>Resources</Filter>
    </ResourceCompile>
    <Manifest Include="resources\app.manifest">
      <Filter>Resources</Filter>
    </Manifest>
  </ItemGroup>
</Project>
"#
    )
    .replace('\n', "\r\n")
}

/// Eject `winui3 + windows` into a standalone Visual Studio solution.
pub(crate) fn eject_windows(
    app: &Application,
    context: &BuildContext,
    options: &EjectOptions,
) -> Result<EjectedProject, Vec<Diagnostic>> {
    let context = &sanitized_context(context, options);
    let app = &bern_target_api::svg::prepare(app, &context.target);
    bern_target_api::svg::validate(app, context)?;

    let product = product_name(app);
    let exe = executable(app);
    let frontends = Frontends::of(&context.target.frontends);
    let generator = Generator::with_project_dir(app, &context.project_dir);
    let svg = bern_target_api::svg::svg_used(app);
    let webpage = bern_target_api::svg::webpage_used(app);

    let mut files: Vec<GeneratedFile> = Vec::new();
    let mut cpp_files: Vec<PathBuf> = Vec::new();
    let mut header_files: Vec<PathBuf> = Vec::new();

    // 1. Identity & catalog (`fingerprint = 0` relaxes protocol check).
    let app_id_h = PathBuf::from("src/app_id.h");
    header_files.push(app_id_h.clone());
    files.push(GeneratedFile::text(app_id_h, generator.identity()));

    let catalog_cpp = PathBuf::from("src/catalog/app_catalog.cpp");
    cpp_files.push(catalog_cpp.clone());
    files.push(GeneratedFile::text(catalog_cpp, generator.catalog(0)));

    // 2. Split UI interface (`src/ui/app_ui.inl`, `src/ui/styles.inl`,
    //    `src/ui/views/*.inl`, `src/ui/windows/*.inl`, `src/native/*.inl`).
    for (rel, content) in generator.interface_split() {
        let path = PathBuf::from("src").join(rel);
        header_files.push(path.clone());
        files.push(GeneratedFile::text(path, content));
    }

    // 3. C++ runtime under `src/runtime/`.
    let svg_inl = PathBuf::from("src/runtime/bern_ui_svg.inl");
    header_files.push(svg_inl.clone());
    files.push(GeneratedFile::text(
        svg_inl,
        if svg || webpage {
            include_str!("../templates/cpp/bern_ui_svg.inl")
        } else {
            ""
        },
    ));
    for (name, contents) in RUNTIME {
        let path = PathBuf::from("src/runtime").join(name);
        let skip_cpp = (*name == "bern_winui.cpp" && !frontends.winui)
            || (*name == "bern_islands.cpp" && !frontends.islands);
        if skip_cpp {
            continue;
        }
        if name.ends_with(".cpp") {
            cpp_files.push(path.clone());
        } else {
            header_files.push(path.clone());
        }
        let body = if *name == "bern_pch.h" {
            contents
                .replace("/*bern:svg-enabled*/", if svg { "1" } else { "0" })
                .replace("/*bern:webpage-enabled*/", if webpage { "1" } else { "0" })
        } else {
            (*contents).to_string()
        };
        files.push(GeneratedFile::text(path, body));
    }

    // 4. Resources, manifest, version/icon RC, status icons, alternate icons.
    files.push(GeneratedFile::text(
        "resources/app.manifest",
        glue::manifest(app),
    ));
    files.push(GeneratedFile::text(
        "resources/app.rc",
        glue::resource_script(app, &exe),
    ));
    files.push(GeneratedFile::text(
        "resources/fluent-icons-license.txt",
        include_str!("../icons/fluent/LICENSE"),
    ));
    files.push(GeneratedFile {
        path: "resources/app.ico".into(),
        contents: app_ico(app, context)?,
    });
    for status in status_icos(app, context)? {
        let name = status
            .path
            .file_name()
            .expect("status icon file name")
            .to_os_string();
        files.push(GeneratedFile {
            path: PathBuf::from("resources").join(name),
            contents: status.contents,
        });
    }

    let mut has_alternates_dir = false;
    let failed = |message: String| vec![Diagnostic::error(message)];
    for (name, source) in icon::alternates(app, "windows", &context.project_dir).map_err(failed)? {
        let contents = match source {
            icon::Source::Native(path) => std::fs::read(&path).map_err(|error| {
                failed(format!("cannot read the icon {}: {error}", path.display()))
            })?,
            icon::Source::Png(path) => {
                let png = std::fs::read(&path).map_err(|error| {
                    failed(format!("cannot read the icon {}: {error}", path.display()))
                })?;
                let size = icon::png_size(&png).map_or(256, |(w, _)| w);
                icon::ico(&[icon::Image {
                    size,
                    png: png.into(),
                }])
            }
            icon::Source::Logo => continue,
        };
        has_alternates_dir = true;
        files.push(GeneratedFile {
            path: PathBuf::from("resources/alternates").join(alternate_ico(&name)),
            contents,
        });
    }

    let declared_res = copy_declared_resources(app, context, Path::new("resources/assets"))?;
    let svg_res: Vec<GeneratedFile> = bern_target_api::svg::static_files(app, context)?
        .into_iter()
        .map(|f| GeneratedFile {
            path: PathBuf::from("resources/assets").join(f.path),
            contents: f.contents,
        })
        .collect();
    let has_resources_dir = !declared_res.is_empty() || !svg_res.is_empty();
    files.extend(declared_res);
    files.extend(svg_res);

    // 5. SDK prep script + Visual Studio solution and project files.
    files.push(GeneratedFile::text(
        "scripts/prepare-sdk.cmd",
        prepare_sdk_script(),
    ));
    let project_guid = guid(&format!("vcxproj:{product}"));
    files.push(GeneratedFile::text(
        format!("{product}.sln"),
        generate_sln(&product, &project_guid),
    ));
    files.push(GeneratedFile::text(
        format!("{product}.vcxproj"),
        generate_vcxproj(&VsProjSpec {
            product: product.clone(),
            executable: exe.clone(),
            project_guid,
            frontends,
            engine: options.engine,
            cpp_files: cpp_files.clone(),
            header_files: header_files.clone(),
            has_resources_dir,
            has_alternates_dir,
        }),
    ));
    files.push(GeneratedFile::text(
        format!("{product}.vcxproj.filters"),
        generate_vcxproj_filters(&cpp_files, &header_files),
    ));

    // 6. Rust engine integration (`source` or `prebuilt`).
    let mut rust_manifests = Vec::new();
    let mut prebuilt_staging = Vec::new();
    let mut prebuilt_steps = Vec::new();
    let glue_cargo = glue::cargo_toml_for(context, app, options.with_cli);
    let glue_lib = glue::lib_rs(context, app);
    let glue_config = glue::cargo_config();

    match options.engine {
        EjectEngine::Source => {
            files.push(GeneratedFile::text("rust/glue/Cargo.toml", glue_cargo));
            files.push(GeneratedFile::text(
                "rust/glue/.cargo/config.toml",
                glue_config,
            ));
            files.push(GeneratedFile::text("rust/glue/src/lib.rs", glue_lib));
            rust_manifests.push(PathBuf::from("rust/glue/Cargo.toml"));
            if options.with_cli {
                files.push(GeneratedFile::text(
                    "rust/glue/src/cli.rs",
                    glue::cli_rs(context, app),
                ));
            }
            files.push(GeneratedFile::text(
                "scripts/build-rust.cmd",
                build_rust_script(),
            ));
        }
        EjectEngine::Prebuilt => {
            prebuilt_staging.push(GeneratedFile::text("glue/Cargo.toml", glue_cargo));
            prebuilt_staging.push(GeneratedFile::text("glue/.cargo/config.toml", glue_config));
            prebuilt_staging.push(GeneratedFile::text("glue/src/lib.rs", glue_lib));
            if options.with_cli {
                prebuilt_staging.push(GeneratedFile::text(
                    "glue/src/cli.rs",
                    glue::cli_rs(context, app),
                ));
            }
            if let Ok(lock) = std::fs::read(context.workspace_dir.join("Cargo.lock")) {
                prebuilt_staging.push(GeneratedFile {
                    path: "glue/Cargo.lock".into(),
                    contents: lock,
                });
            }
            bern_target_api::script::configure(&mut prebuilt_staging, context);

            let staging = &options.staging_dir;
            let profile = if options.release { "release" } else { "debug" };
            let target_dir = context.workspace_dir.join("target/bern-glue-windows");
            let arch = glue::architecture();
            let mut cargo = vec![
                "build".to_string(),
                "--lib".into(),
                "--manifest-path".into(),
                staging.join("glue/Cargo.toml").display().to_string(),
                "--target-dir".into(),
                target_dir.display().to_string(),
            ];
            if options.release {
                cargo.push("--release".into());
            }
            let lib_dir = context.output_dir.join("lib").join(arch);
            prebuilt_steps.extend([
                BuildStep::Run {
                    description: format!("Build prebuilt Rust engine for Windows ({arch})"),
                    program: "cargo".into(),
                    args: cargo,
                    cwd: staging.join("glue"),
                    env: vec![],
                },
                BuildStep::CreateDir {
                    path: lib_dir.clone(),
                },
                BuildStep::Copy {
                    from: target_dir.join(profile).join("bern_app.lib"),
                    to: lib_dir.join("bern_app.lib"),
                },
            ]);
        }
    }

    if options.with_cli && skill::listens(&context.target.channels) {
        let cli_name = app.app.id.replace('_', "-");
        files.push(GeneratedFile::text(
            skill::path(app),
            skill::markdown(
                app,
                &skill::Place {
                    cli: cli_name,
                    installed: None,
                    connect: None,
                },
            ),
        ));
    }

    bern_target_api::script::configure(&mut files, context);

    let app_title = app.app.name.get(&app.languages.default).unwrap_or(&product);
    let build_help = match options.engine {
        EjectEngine::Source => format!(
            "Open `{product}.sln` in **Visual Studio 2022** (Desktop development with C++) and press **F5**.\n\nMSBuild automatically prepares the Windows App SDK C++/WinRT headers (`scripts\\prepare-sdk.cmd`) and compiles the self-contained Rust workspace in `rust\\` (`scripts\\build-rust.cmd`)."
        ),
        EjectEngine::Prebuilt => format!(
            "Open `{product}.sln` in **Visual Studio 2022** (Desktop development with C++) and press **F5**.\n\nNo Rust toolchain is required: the pre-compiled engine static library is bundled in `lib\\`."
        ),
    };
    let layout_help = "- `src/catalog/app_catalog.cpp` — Localized strings, types, commands, and sessions\n- `src/ui/app_ui.inl` — UI entry point included by active front ends (WinUI 3 / XAML Islands)\n- `src/ui/views/` — One C++ file per UI component\n- `src/ui/windows/` — One C++ file per window\n- `src/runtime/` — Self-contained Bern C++/WinRT runtime\n- `resources/` — Application manifest, `.rc`, icons, and bundled assets";
    files.push(GeneratedFile::text(
        "README.md",
        readme(
            app_title,
            "Windows · WinUI 3 / XAML Islands",
            options.engine,
            &[
                ("Opening & Building", &build_help),
                ("Project Layout", layout_help),
            ],
        ),
    ));

    let files = files.into_iter().map(clean_generated_file).collect();
    Ok(EjectedProject {
        files,
        executables: Vec::new(),
        rust_manifests,
        rust_root: PathBuf::from("rust"),
        prebuilt_staging,
        prebuilt_steps,
        notes: vec![format!(
            "Open `{product}.sln` in Visual Studio 2022 to build and edit."
        )],
    })
}
