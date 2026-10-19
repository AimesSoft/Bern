# 参与贡献

## 提交前的检查

CI（[`.github/workflows/ci.yml`](.github/workflows/ci.yml)）在每个拉取请求上运行下面这些检查，提交前在本地跑一遍：

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node --test crates/bern-svg/tests/*.test.mjs crates/bern-backend-web/tests/*.test.mjs
```

GTK 运行时 `crates/bern-gtk` 不是工作区成员，要单独检查。它需要 GTK 4.14、libadwaita 1.5 和 WebKitGTK 6 的开发包（Debian 与 Ubuntu 上是 `libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev`）：

```sh
cd crates/bern-gtk
cargo fmt --check
cargo clippy --all-features --all-targets -- -D warnings
cargo test --all-features
```

装了 GTK 4 和 libadwaita 时，`bern-backend-gtk` 的测试还会对生成的 Linux 应用做类型检查。

## 测试

- 测试要在任何一台电脑上得到同样的结果。需要 Xcode、Android SDK 或 Windows SDK 的部分，在缺少工具链时只检查能检查的内容，不能因此失败。只需要工具路径的地方，像 `crates/bern-cli/tests/svg_targets.rs` 那样在测试里给一套空的工具目录。
- 生成器的改动，最好用测试检查生成的代码；能在本机编译生成结果的目标，测试会顺带编译它。

## 运行时文案

各运行时自己显示的文字只在 [`crates/bern-target-api/src/texts.rs`](crates/bern-target-api/src/texts.rs) 的 `FRAMEWORK_TEXTS` 里声明一次，并且要写全每一种框架语言。测试会检查每种语言都有译文，也会检查运行时和生成器用到的每个 `bern.*` 键都已声明。

## 提交信息

沿用仓库已有的写法：

```text
<type>(<scope>): <用英文小写写成的一句摘要>

<正文：为什么改、改了什么行为，必要时写清没有改什么>
```

- `type` 用 `feat`、`fix`、`refactor`、`test`、`docs`、`ci` 或 `style`；`scope` 是改动所在的部分，例如 `swiftui`、`gtk`、`compiler`、`targets`，跨度大时可以省略。
- 摘要说明这次提交做了什么，读者不打开 diff 也能看懂。`1`、`macos适配` 这样的信息说明不了改了什么。
- 一次提交只做一件事；格式化、重构和行为改动分开提交。

## 文档

组件、属性或平台能力变化时，按 [Docs/README.md 的更新规则](Docs/README.md#更新规则) 同步参考文档与验收记录。
