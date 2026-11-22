**English** · [简体中文](../zh-CN/Platforms/Windows.md)

# Windows

Bern compiles the Windows target (`platform = "windows"`) to a native **C++/WinRT** executable linking the Rust engine in-process, targeting **Windows 10 version 2004 (build 19041) or newer** with zero .NET or VC++ redistributable runtime dependencies. Windows targets must be built on Windows.

By default, Bern builds a **dual-frontend executable**: when the system has **Windows App Runtime 2**, it renders with **WinUI 3** (Fluent 2 + Mica backdrop); otherwise, it falls back seamlessly at launch to the inbox **XAML Islands** frontend.

## Prerequisites

- **Windows 10/11 host** (x64 or arm64).
- **Rust 1.99+** targeting the MSVC toolchain (`rustup default stable-msvc`).
- **Visual Studio 2022 or newer** with the **"Desktop development with C++"** workload (Visual Studio Build Tools are sufficient).
- **Windows 11 SDK** (providing `cppwinrt.exe`), installed through the Visual Studio Installer.

Verify your environment before building:

```powershell
bern doctor
```

## Configuration in `Bern.toml`

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"
publisher = "Example Inc"

[app.icon]
default = "assets/icon.png"
windows = "assets/app.ico"   # Optional: PNG or multi-size .ico

[targets.windows]
platform = "windows"
ui = "winui3"
# Default dual frontends; restrict only if you want a single frontend:
# frontends = ["winui3", "xaml-islands"]
```

### Frontend selection (`frontends`)

| `frontends` | Runtime requirement | Behavior |
| --- | --- | --- |
| `["winui3", "xaml-islands"]` *(default)* | None (works out of the box on Windows 10 2004+) | Uses WinUI 3 when Windows App Runtime 2 is available; falls back seamlessly to inbox XAML Islands otherwise. |
| `["winui3"]` | Windows App Runtime 2 | Smaller binary; prompts the user to install Windows App Runtime if absent. |
| `["xaml-islands"]` | None (inbox Windows 10 2004+) | Always uses system XAML Islands without linking WinUI 3 headers. |

## Build, run, and debug

```powershell
# Build the native executable, generated app CLI, and Skill
bern build --target windows

# Build and launch the application
bern run --target windows
```

**First-build note**: On the first build, Bern runs `cppwinrt.exe` to generate C++/WinRT projection headers under `target/bern-winrt/` and builds a precompiled header (about 2.4 GB per build profile). Subsequent incremental builds reuse the precompiled header and typically finish in a few seconds.

Drive the running app via the generated app CLI in `target/bern/windows/bin/`:

```powershell
target\bern\windows\bin\tasks.exe --help
target\bern\windows\bin\tasks.exe window screenshot --output win.png
```

Useful debugging environment variables:

| Variable | Effect |
| --- | --- |
| `BERN_FORCE_ISLANDS=1` | Forces the XAML Islands fallback on machines that already have Windows App Runtime 2 installed. |
| `BERN_APP_RUNTIME=installed\|inbox\|none` | Restricts which Windows App Runtime source startup checks, for testing runtime fallback paths. |
| `BERN_APPEARANCE=light\|dark` | Overrides system light/dark theme. |
| `BERN_FRAME_RATE=1` | Shows WinUI XAML frame-rate counters on every window. |
| `BERN_TRACE=<rules>` | Enables performance tracing (or pass `--trace` to `bern run`). |
| `BERN_SNAPSHOT_DIR=<dir>` | Writes window snapshots into `<dir>`. |

## Platform behavior and adaptations

- **Title bar & sidebar**: The custom Fluent title bar already displays the app icon (from `.ico` bitmaps) and title, so `SidebarHeader(role: title)` is omitted automatically on Windows.
- **System tray & login**: `StatusItem` creates a Windows notification area icon with a native popup menu and keeps the process alive after all windows close. `Switch(login_item: true)` toggles per-user Windows startup registration.
- **Custom media core**: `Video` provides a `SwapChainPanel` and vertical-blank frame clock when a custom `MediaKernel` is registered.
- **WebView**: `WebPage` uses WebView2 when `webview = true` is set on the target.
- **Markdown**: Inline Markdown (`Text(markdown)`) strips markup to plain text on Windows.
- **Not applicable**: `MenuBar` (macOS-only app menu bar) is accepted as a no-op.
- **Native views**: Co-located WinUI 3 / C++ views mount via `NativeView(windows: "...")`. See [Native views](../Topics/Native-Views.md).

## Packaging and signing

```powershell
bern package --target windows
```

`bern package --target windows` compiles a release build and writes two artifacts under `target\bern\dist\`:

- `<app>-<version>-<arch>-setup.exe` — per-user Direct2D LZMS installer (no administrator privileges or UAC prompt required; supports `/S` for silent install and silent uninstall).
- `<app>-<version>-<arch>.zip` — portable archive containing the app, CLI, and Skill.

Without a code-signing certificate, Windows SmartScreen warns users about an unsigned binary. Set either a certificate thumbprint in your Windows certificate store or a `.pfx` file path to sign both the app binaries and the installer with SHA-256 timestamping:

```powershell
# Using a certificate already installed in the current user's store
$env:BERN_WINDOWS_CERTIFICATE = "0123456789ABCDEF..."
bern package --target windows

# Or using a .pfx file
$env:BERN_WINDOWS_CERTIFICATE = "C:\certs\release.pfx"
$env:BERN_WINDOWS_CERTIFICATE_PASSWORD = "your-password"
bern package --target windows
```
