**English** · [简体中文](../zh-CN/Guides/Layouts-and-Navigation.md)

# Layouts, Navigation & Feature Contracts

Bern UI declarations live in normal `.rs` files inside your UI crate (`*-ui`). This guide explains how components, window drafts, adaptive size classes, navigation, and compile-time feature contracts work together.

## 1. Components (`#[component]`) and session drafts (`#[bern::session]`)

UI state is split cleanly between **engine state** (shared across every window and CLI client, read through `app: <ns>::Client`) and **window-local drafts** (per-window text input, selected sidebar page, active filter, modal flags, declared with `#[bern::session]`):

```rust
#[bern::session]
pub struct MainDraft {
    pub page: Page,
    pub query: String,
    #[initial(true)]
    pub only_open: bool,
}

#[component]
pub fn app(app: tasks::Client, #[session] draft: MainDraft) -> impl View {
    ui! {
        Form {
            Switch(
                id: only_open,
                label: tr! { "zh-Hans" => "只看未完成", "en" => "Only open" },
                value: draft.only_open
            )
            ForEach(items: app.items, key: id) |task| {
                if !draft.only_open || !task.done {
                    Switch(id: done, label: task.title, checked: task.done, style: checkbox)
                        => app.toggle(task.id)
                }
            }
        }
    }
}
```

Key declaration rules:

- Every actionable control binds either an engine command (`=> app.add(...)`) or a draft/window action (`=> draft.only_open.set(false)`, `=> windows.settings.open()`).
- Controls with `value: draft.<field>` generate UI session commands automatically; controls displaying engine state (`checked:`, `current:`, `selected:`) dispatch an engine command on change.
- Use `if`, `if let`, `match`, and `ForEach(items: ..., key: <id>)` for conditional and keyed list rendering.
- See [Component Guide](../Reference/UI.md) and [Component Properties](../Reference/Component-Properties.md) for every built-in control.

## 2. Navigation containers

Bern maps high-level navigation containers directly to each platform's standard navigation chrome:

- **`SplitView` (`Sidebar` + `Detail`)** — side-by-side sidebar and detail pane on desktop/tablet; collapses to a navigation stack on phones (`compact`).
- **`Tabs` (`Tab` + optional `TabAccessory`)** — tab bar (with optional `bar: floating` pill style on mobile/desktop) and an optional mini-player/status accessory bar.
- **`Navigation` (`NavLink` + `Destination`)** — typed push navigation backed by a `Vec<Route>` draft field, preserving back stacks per window.
- **`Toolbar`, `Menu`, `BottomSheet`, `Modal`, `Alert`** — standard platform action chrome, sheets, and confirmation dialogs.

## 3. Adaptive layouts and breakpoints

Configure width breakpoints once in `Bern.toml`:

```toml
[layouts]
compact_below = 600
medium_below = 1024
expanded_below = 1440
```

Inside `ui!`, use `Adaptive` to provide distinct layouts for each size tier (`compact`, `medium`, `expanded`, `large`), plus dedicated `watch` (watchOS / Wear OS) and `tv` (tvOS / Android TV) branches:

```rust
Adaptive(id: controls) {
    Layout(size: expanded) {
        Row(gap: 8) {
            Button(id: prev, label: tr! { "zh-Hans" => "上一项", "en" => "Previous" }) => app.prev()
            Button(id: next, label: tr! { "zh-Hans" => "下一项", "en" => "Next" }) => app.next()
        }
    }
    Layout(size: compact) {
        Menu(id: actions, label: tr! { "zh-Hans" => "操作", "en" => "Actions" }) {
            MenuItem(id: prev, label: tr! { "zh-Hans" => "上一项", "en" => "Previous" }) => app.prev()
            MenuItem(id: next, label: tr! { "zh-Hans" => "下一项", "en" => "Next" }) => app.next()
        }
    }
}
```

You can also branch on runtime or compile-time window flags such as `window.compact`, `window.expanded`, `window.tv`, `window.watch`, `window.short`, `window.keyboard`, `window.touch`, and `window.webview`. On `tvos`, `watchos`, and `wearos`, unreachable branches are pruned at compile time before capability checking.

## 4. Compile-time feature contracts (`#[bern::features]`)

When a screen has multiple adaptive layouts, it is easy to add a button on desktop and forget to expose it on phone or watch. Declare a feature contract to turn missing entries into compile errors:

```rust
#[bern::features(protocol = media)]
pub enum MediaFeature {
    #[commands(select_clip)]
    #[sizes(compact, expanded)]
    Clip,
}

#[component(features = MediaFeature)]
pub fn media_controls(app: media::Client) -> impl View {
    ui! {
        Adaptive(id: clip_choice) {
            Layout(size: expanded) {
                Button(id: bars, label: tr! { "zh-Hans" => "色条", "en" => "Color Bars" }, feature: Clip)
                    => app.select_clip(Clip::Bars)
            }
            Layout(size: compact) {
                Menu(id: clip_menu, label: tr! { "zh-Hans" => "换片", "en" => "Change Clip" }) {
                    MenuItem(id: bars, label: tr! { "zh-Hans" => "色条", "en" => "Color Bars" }, feature: Clip)
                        => app.select_clip(Clip::Bars)
                }
            }
        }
    }
}
```

Diagnostic codes enforced by `bern check`:

| Code | Meaning |
| --- | --- |
| `B201` | A feature in the contract has no reachable UI entry in a required layout size |
| `B202` | An interactive entry inside a contracted component is not registered to a feature |
| `B203` | An entry appears only inside a conditional branch and cannot be proven always reachable |
| `B204` | An entry invokes a command that does not belong to the feature it declares |
| `B401` | A watch target (`watchos` / `wearos`) reaches an `Adaptive` missing `Layout(size: watch)` |

Every build writes a machine-readable `features.json` coverage report into `target/bern/<target>/`.
