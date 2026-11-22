[English](../../Platforms/Android.md) · **简体中文**

# Android 与 Wear OS

Bern 将 Android 目标（`platform = "android"`）编译为原生的 **Jetpack Compose（Material 3）** 应用，最低支持 **Android 8.0（API 26）**，覆盖手机、平板与 Android TV；将 Wear OS 目标（`platform = "wearos"`）编译为 **Wear Compose Material 3** 应用，最低支持 **Wear OS 4（API 30）**。两者均可在 macOS、Linux 或 Windows 上构建。

## 环境准备

- **Rust 1.99+**（`rustup update`）。
- **Android Studio** 及 **Android SDK**、**Android NDK**（通过 SDK Manager → SDK Tools 安装）。若安装在非默认路径，请设置 `ANDROID_HOME`（或 `ANDROID_SDK_ROOT`）。
- **Gradle 9.4 或更新版本**位于 `PATH` 中（也可通过 `BERN_GRADLE` 指定路径，通过 `BERN_JAVA_HOME` 指定 JDK）。
- 安装与目标 ABI 匹配的 Rust 编译目标：

```bash
# 默认必需 ABI（arm64-v8a）
rustup target add aarch64-linux-android

# 若在 `abis = [...]` 中声明了更多 ABI，请一并安装
rustup target add armv7-linux-androideabi x86_64-linux-android

bern doctor
```

## `Bern.toml` 配置

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"
cleartext_http = false       # 允许明文 HTTP（自动写入 networkSecurityConfig）
background_audio = false     # 开启前台媒体播放服务
overlays = false             # 启用 SYSTEM_ALERT_WINDOW 悬浮层权限

[app.icon]
default = "assets/icon.png"
android = "assets/android/res"  # 可选：PNG 或 Android res/ mipmap 目录

[targets.android]
platform = "android"
ui = "compose"
abis = ["arm64-v8a"]            # 可选值："arm64-v8a"、"armeabi-v7a"、"x86_64"
android = { network = true, multicast = false, audio_settings = false, all_files_access = false, cxx_runtime = false }

[targets.wearos]
platform = "wearos"
ui = "compose"
android = { standalone = true } # 默认为 true：独立 Wear OS 应用
```

### Android 目标选项（`android = { ... }`）

| 选项 | 适用目标 | 用途 |
| --- | --- | --- |
| `network` | Android / Wear OS | 声明 `INTERNET` 与 `ACCESS_NETWORK_STATE` 权限。 |
| `multicast` | Android / Wear OS | 声明 `CHANGE_WIFI_MULTICAST_STATE`，用于局域网设备发现（mDNS/SSDP）。 |
| `audio_settings` | Android / Wear OS | 声明 `MODIFY_AUDIO_SETTINGS` 权限。 |
| `all_files_access` | Android | 声明 `MANAGE_EXTERNAL_STORAGE`（运行时调用 `bern::platform::ask_all_files` 引导授权）。 |
| `cxx_runtime` | Android / Wear OS | 当原生 C++ 依赖需要时，从 NDK 打包 `libc++_shared.so`。 |
| `standalone` | Wear OS | 写入 `com.google.android.wearable.standalone` 元数据（默认 `true`）。 |

## 构建、运行与调试

```bash
# 检查声明（Wear OS 会强制检查 B401 Layout(size: watch) 覆盖）
bern check --target android
bern check --target wearos

# 构建 APK 与 Gradle 工程
bern build --target android
bern build --target wearos

# 安装并启动到已连接的 USB 真机或模拟器，同时建立 adb IPC 转发
bern run --target android
bern run --target wearos
```

`bern run` 会通过 `adb` 安装并启动应用，并自动建立 `adb forward` 通道，开发机可直接通过配套应用 CLI 操控手机、平板、电视或手表上的运行实例：

```bash
# 操控运行中的 Android 应用
target/bern/android/bin/tasks window screenshot --output android.png

# 单独操控运行中的 Wear OS 应用
target/bern/wearos/bin/tasks --engine wearos window screenshot --output wear.png
```

## 平台行为与适配要点

- **Material 3 与动态取色**：遵循 Material 3 设计语言，自动适配 Android 12+ 系统动态取色与深浅色切换。
- **存储访问框架（`content://`）**：文件选择器返回 SAF `content://` URI。可直接传给 `MediaSource`，或用 `bern::files::list` / `bern::files::walk` 遍历目录——Bern 内部自动桥接 SAF 文件描述符，无需拷贝文件。
- **动态切换应用图标**：`[app.alternate_icons]` 自动生成 `<activity-alias>`，运行时通过 `WindowSet(icon:)` 切换桌面入口图标。
- **横竖屏锁定**：`Window(landscape: true)` 可在手机与平板上将 Activity 锁定为横屏。
- **Android TV**：运行时根据系统 TV 模式或 leanback 特性自动识别，启用 `Layout(size: tv)`、`window.tv`、`font(tv:)`、方向键焦点（`default_focus: true`）与遥控器 `Shortcut`（`escape` 对应返回，`space` 对应播放/暂停，方向键在焦点边缘触发）。
- **Wear OS**：所有可达的 `Adaptive` 块必须包含 `Layout(size: watch)`（`B401`），非手表分支在编译期裁剪。支持旋钮滚动、右滑返回、纯 Rust 动态 `Svg`（`BSVG`）与进程内 `javascript = true`。
- **原生视图**：可通过 `NativeView(android: "...")`（或 `wearos: "..."`）挂载同仓的 Jetpack Compose / Android 视图。详见 [原生视图](../Topics/Native-Views.md)。
- **不适用能力**：`MenuBar`、`StatusItem` 与 `Switch(login_item: true)` 会被安全视作不适用而不报错。

## 打包与签名

```bash
bern package --target android
bern package --target wearos
```

`bern package` 会在 `target/bern/dist/` 下同时生成 Release 版 **`.apk`**（用于直装测试）与 **`.aab`**（用于上架 Google Play）。

未设置签名环境变量时，产物使用调试密钥签名（仅限本机测试，应用商店会拒绝）。发布签名请配置以下环境变量：

```bash
export BERN_ANDROID_KEYSTORE="/path/to/release.jks"
export BERN_ANDROID_KEYSTORE_PASSWORD="store-password"
export BERN_ANDROID_KEY_ALIAS="release"
# 可选：省略时默认与 BERN_ANDROID_KEYSTORE_PASSWORD 相同
export BERN_ANDROID_KEY_PASSWORD="key-password"

bern package --target android
```
