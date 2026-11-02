**English** · [简体中文](../zh-CN/Implementation/Module-Builds.md)

# Building by module

`bern build`, `run`, `package`, `check` and `ir` support `--exclude-module`. Without it, every module declared in `Bern.toml` is compiled. The argument takes module IDs and can be repeated or comma-separated:

```sh
bern build --target android --exclude-module downloads
bern check --target android --exclude-module downloads,sync
bern run --target macos --exclude-module downloads --exclude-module sync
bern ir --exclude-module downloads
```

Exclusion takes effect before module sources are loaded. An excluded module's protocol, engine, UI, pages, slot contributions and `app.modules.<id>` runtime switch never enter the app. `optional` and `enabled` still control whether **compiled modules** are enabled at runtime.

Module IDs must exist. If a kept module's `depends` points to an excluded module, the build reports an error and lists the dependency chain, for example `sync -> anime`. The caller needs to also exclude the modules that depend on it, or keep that dependency. Bern does not add exclusions automatically.

## Direct references in the UI and in Rust

The UI uses `build.modules.<id>`. The compiler and the Rust macros share the pruning logic, which folds conditions and removes branches before type checking:

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

Runtime conditions can be combined with it, for example `build.modules.anime && app.features.anime`. `if … { … } else { … }` inside condition expressions is folded too. An unknown module ID reports an error.

In Rust, use the `bern-module-<id>` Cargo features; Bern declarations also support `#[cfg]` and `#[cfg_attr]` on these features, including `not`, `any` and `all`:

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

When forwarding an optional component parameter, give the corresponding argument the same `#[cfg]`:

```rust
search(#[cfg(feature = "bern-module-anime")] anime: anime)
```

Page enum variants provided by a module do not exist once that module is excluded, and the matching `match` arms can also use `#[cfg(feature = "bern-module-anime")]`. A protocol command or event listener can take `#[cfg]` on the whole item; if the command must stay and only the listener should go away with the module, use `#[cfg_attr(feature = "bern-module-anime", on(anime::play))]`. Rust engine implementations that get compiled need to use the same feature.

## Cargo dependencies

Projects without cross-module Rust references need no extra Cargo features: the generated workspace only references the protocol/core crates of the kept modules.

When the shell references a module directly, make that dependency optional and declare the matching feature:

```toml
[features]
default = ["storage", "bern-module-downloads"]
storage = []
bern-module-downloads = ["dep:downloads-protocol", "app-protocol/bern-module-downloads"]

[dependencies]
downloads-protocol = { path = "../modules/downloads/protocol", optional = true }
app-protocol = { path = "../app-protocol", default-features = false }
```

Forwarded features must also be declared in `app-protocol`. The generator reads the `bern-module-*` features declared by dependency packages, turns off those packages' default features, and explicitly enables the kept modules and the other default features. `dep:foo` or `foo/bar` entries in `default` should first be wrapped in plain named features (for example `base = ["dep:foo"]`, `default = ["base", "bern-module-downloads"]`). Module features should be listed directly in `default`; another default feature that indirectly enables an excluded module gets a dependency chain error.

Cross-crate dependency edges should set `default-features = false` and forward the needed modules explicitly through features, so that Cargo's feature unification does not turn modules back on. Plain unconditional Cargo dependencies are still compiled by Cargo, so cross-module dependencies should be made optional as in the example above, with the calls wrapped in `#[cfg]`.

## Build outputs and repeated builds

The default output is still `target/bern/<target>/`. Pruned builds go to `target/bern/<target>/without-<sorted-module-ids>/`, with multiple IDs joined by `+`. The `modules.json` in that directory records the module selection of this build.

Bern passes `BERN_BUILD_ROOT` / `BERN_EXCLUDE_MODULES`, scoped to the app directory, to every build step, and writes `.cargo/config.toml` into the generated Cargo workspace. Existing backend configuration is kept. The Rust macros record the selection in rustc's dependency info through `option_env!`; switching to another combination or going back to the default build regenerates the corresponding declarations.

To recompile generated Rust glue on its own, run Cargo in the directory that holds that glue's Cargo.toml, so that it reads its `.cargo/config.toml`. When plain Cargo runs directly in the source workspace, the Bern macros use the full module configuration by default; pruned builds go through the Bern CLI.
