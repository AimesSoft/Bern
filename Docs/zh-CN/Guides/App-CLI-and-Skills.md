[English](../../Guides/App-CLI-and-Skills.md) · **简体中文**

# 应用 CLI 与 AI Skills（`SKILL.md`）

因为 Bern 中所有界面交互与业务操作都经过同一套类型化命令目录，只要在目标上开启控制通道，构建产物就会同时附带配套的**应用 CLI** 和供 AI Agent 阅读的 **`SKILL.md`**。

## 1. 开启控制通道

在 `Bern.toml` 中为目标添加 `channels = ["ipc"]`：

```toml
[targets.macos]
platform = "macos"
channels = ["ipc"]
```

构建后（`bern build` / `bern run` / `bern package`），Bern 自动产出：

- **应用 CLI**：位于 `target/bern/<目标>/bin/<程序>`（桌面端发布包内也会随包附带，例如 macOS 在 `Contents/Helpers/<程序>`）。
- **Agent Skill**：位于 `target/bern/<目标>/skills/<程序>/SKILL.md`，完整列出所有内置界面/窗口/媒体命令以及你的协议命令与查询。

## 2. 按语义查询并驱动已挂载控件

请始终使用与运行中应用同一次构建产出的 CLI。人、测试脚本与 AI Agent 都按控件的语义名称操作，而不是脆弱的像素坐标：

```sh
# 列出当前窗口中所有已挂载控件的名称、角色、标签、当前值与可用状态
my_app ui inspect

# 与真实用户点击/输入走完全相同的代码路径
my_app ui activate --target tasks_view.add
my_app ui set-bool --target tasks_view.only_open --value true
my_app ui edit --target tasks_view.new_task --value "发布 v1.0"
my_app ui submit --target tasks_view.new_task
my_app ui select --target main_window.pages --option Settings
my_app ui set-language --language en
```

当控件被禁用、被模态对话框遮挡或未挂载时，CLI 会返回明确的错误码（如 `target_unavailable`、`blocked_by_modal`、`target_ambiguous`），绝不静默吞掉操作。

## 3. 窗口控制与直接截屏

内置窗口命令用于读取真实系统窗口状态、测试关闭确认协议，并直接截取应用窗口画面而无需全屏录屏：

```sh
my_app window list
my_app window open --kind settings
my_app window screenshot --output shot.png

# 触发并响应关闭确认协议
my_app window close --window settings#1
my_app window confirm-close --window settings#1 --confirmation settings#1:c1 --decision Save
```

## 4. 直接调用引擎命令、查询与状态

协议中声明的每条 `#[commands]` 与 `#[queries]` 方法都会自动成为命名空间下的类型化子命令：

```sh
my_app catalog                              # 查看完整命令与查询目录
my_app state                                # 读取引擎实时状态快照
my_app tasks add --title "审阅 PR"          # 执行业务命令
my_app tasks search --text "PR"             # 执行引擎查询
```

## 5. 多实例选择、移动端与 SSH 远端控制

- **同一包标识多端并存**：例如 macOS 与 iOS 模拟器同时运行时，使用 `--engine ios`、`--engine macos`（或 `--engine watchos` / `--engine tvos` / `--engine wearos`）指定目标引擎。
- **自定义 IPC 目录**：为应用与 CLI 同时设置 `BERN_IPC_DIR=/path/to/dir` 可隔离并行测试。Apple 模拟器下 `bern run` 会通过 `SIMCTL_CHILD_BERN_IPC_DIR` 自动透传该目录。
- **Android / Wear OS**：CLI 始终在开发电脑上运行（不在手机或手表端执行），`bern run` 会自动配置 `adb forward`。
- **Web（`web_app` / `web_ui`）**：开启 IPC 的 `web_app` 在 `bern run` 时会启动本地桥连接当前打开的浏览器标签页；发布的静态 Web 包不含本机 CLI 与 `SKILL.md`。
- **经 SSH 远端控制**：直接以拥有控制通道端点的账户在目标机器上执行 CLI，无需额外观开网络监听端口：

```sh
ssh HOST /path/to/my_app state
```

## 6. 进程内脚本（`bern.cli`）

如果需要让应用内的用户脚本或插件在 Rust 侧黑白名单控权下调用同一套命令目录，请参阅[JavaScript 与 `bern.cli`](../Topics/JavaScript.md)。
