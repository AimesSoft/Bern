# 按模块编译

`bern build`、`run`、`package`、`check` 和 `ir` 支持 `--exclude-module`。不传时编译 `Bern.toml` 声明的全部模块。参数接受模块 ID，可以重复，也可以用逗号分隔：

```sh
bern build --target android --exclude-module downloads
bern check --target android --exclude-module downloads,sync
bern run --target macos --exclude-module downloads --exclude-module sync
bern ir --exclude-module downloads
```

排除在加载模块源码之前生效。被排除模块的协议、引擎、UI、页面、slot 贡献和 `app.modules.<id>` 运行时开关均不进入应用。`optional`、`enabled` 仍控制**已编译模块**的运行时启停。

模块 ID 必须存在。如果保留模块的 `depends` 指向被排除模块，构建报错并列出依赖链，例如 `sync -> anime`。调用者需要一并排除依赖它的模块，或者保留该依赖。Bern 不会自动追加排除项。

## UI 与 Rust 中的直接引用

UI 使用 `build.modules.<id>`。编译器和 Rust 宏共用裁剪逻辑，在类型检查前折叠条件和移除分支：

```rust
#[component]
pub fn content(app: app::Client) -> impl View {
    ui! {
        Column {
            if build.modules.downloads {
                download_button(app: app)
            }
            if !build.modules.anime || !app.modules.anime {
                local_details(app: app)
            }
        }
    }
}
```

运行时条件可以组合，例如 `build.modules.anime && app.features.anime`。条件表达式中的 `if … { … } else { … }` 也会折叠。不认识的模块 ID 会报错。

Rust 中使用 `bern-module-<id>` Cargo feature；Bern 声明也支持这些 feature 的 `#[cfg]` 和 `#[cfg_attr]`，包括 `not`、`any`、`all`：

```rust
#[cfg(feature = "bern-module-anime")]
use anime_protocol::anime;

#[component]
pub fn search(#[cfg(feature = "bern-module-anime")] anime: anime::Client) -> impl View {
    ui! {
        Column {
            if build.modules.anime {
                anime_results(anime: anime)
            }
        }
    }
}
```

转发可选组件参数时，给对应实参加相同的 `#[cfg]`：

```rust
search(#[cfg(feature = "bern-module-anime")] anime: anime)
```

由模块提供的页面枚举项在该模块排除后不存在，`match` 的对应分支也可以使用 `#[cfg(feature = "bern-module-anime")]`。协议命令或事件监听可以整项加 `#[cfg]`；若命令仍需保留、仅监听需要随模块移除，可使用 `#[cfg_attr(feature = "bern-module-anime", on(anime::play))]`。会被编译的 Rust 引擎实现需使用相同 feature。

## Cargo 依赖

没有跨模块 Rust 引用的项目，无需增加 Cargo feature：生成的工作空间只引用保留模块的 protocol/core crate。

外壳直接引用模块时，把该依赖改为 optional，并声明对应 feature：

```toml
[features]
default = ["storage", "bern-module-downloads"]
storage = []
bern-module-downloads = ["dep:downloads-protocol", "app-protocol/bern-module-downloads"]

[dependencies]
downloads-protocol = { path = "../modules/downloads/protocol", optional = true }
app-protocol = { path = "../app-protocol", default-features = false }
```

被转发的 feature 也要在 `app-protocol` 中声明。生成器读取依赖包声明的 `bern-module-*` features，关闭这些包的默认 feature，显式启用保留模块和其他默认 feature。`default` 中的 `dep:foo` 或 `foo/bar` 项应先包装为普通具名 feature（例如 `base = ["dep:foo"]`、`default = ["base", "bern-module-downloads"]`）。模块 feature 应直接列在 `default` 中；其他默认 feature 若间接启用了已排除模块，会得到依赖链错误。

跨 crate 的依赖边应设置 `default-features = false`，通过 feature 显式转发所需模块，避免 Cargo 合并 feature 时重新开启模块。普通无条件 Cargo 依赖仍由 Cargo 编译，因此跨模块依赖应按上例设为 optional，并用 `#[cfg]` 包围调用。

## 构建产物与重复构建

默认输出仍为 `target/bern/<target>/`。裁剪构建输出到 `target/bern/<target>/without-<排序后的模块ID>/`，多个 ID 用 `+` 连接。目录中的 `modules.json` 记录本次模块选择。

Bern 向所有构建步骤传递按应用目录限定的 `BERN_BUILD_ROOT` / `BERN_EXCLUDE_MODULES`，并在生成的 Cargo 工作空间中写入 `.cargo/config.toml`。已有后端配置会保留。Rust 宏通过 `option_env!` 将选择记录到 rustc 的依赖信息中；切换组合、恢复默认构建都会重新生成对应声明。

需要单独重新编译已生成的 Rust glue 时，在该 glue 的 Cargo.toml 所在目录运行 Cargo，以读取其 `.cargo/config.toml`。直接在源工作空间运行普通 Cargo 时，Bern 宏默认使用完整模块配置；裁剪构建以 Bern CLI 为入口。
