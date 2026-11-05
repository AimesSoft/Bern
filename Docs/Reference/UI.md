**English** · [简体中文](../zh-CN/Reference/UI.md)

# Current component usage

Updated: 2026-10-28. This document describes the interfaces that can be declared today; for the complete properties, required items, child nodes and action rules see the [component property table](Component-Properties.md), and for what each backend accepts see the [platform capability table](Platforms.md). Projects that can be built directly are in `examples/hello`, `examples/showcase`, `examples/planner` and `examples/gallery`; the snippets below omit the enclosing component, protocol and state definitions.

## Declarations and data sources

Declare the tree with `ui!` inside a `#[component]` function. Static user-visible text uses `tr!`, engine state is read-only, `draft` session fields can be updated by standard controls, and business changes run through typed commands bound with `=>`. `id` is the stable identity for control entry points, and `label` also serves as the accessible name. `label_hidden: true` hides the visible text, but a label must still be given; the controls that currently support it are TextInput, Switch, Slider and Picker.

Lengths usually use logical units; `Length` accepts a number or a restricted numeric expression. `Number` accepts only static numbers, and `Flag` accepts only build-time booleans. Conditions, lists and state references are still checked by the compiler; component features a target does not support are rejected by the capability check, and platform coverage cannot be judged from the component name alone.

## Text, content and formatting

Every piece of text in the interface is either text declared with `tr!` or data declared as content, and the two are told apart at compile time:

- Text fields of protocol and UI types and text parameters of components are marked `#[content]`, meaning they are data (titles, file names, notes the user wrote). Only such text can be shown directly (`Text(content: app.title)`) or embedded in a sentence as a `tr!` argument (`{title}`). Showing unmarked text, or writing a literal as a `tr!` argument, reports B105: fixed words go into the translation. Session fields are entered by the user and count as content.
- The engine does not assemble sentences. It gives numbers, `CalendarDate` values and enums, and the interface writes them in the reader's language with `tr!` or labeled enums.

```rust
#[value]
pub enum Notice {
    #[label(tr! { "zh-Hans" => "已收藏 {name}", "en" => "{name} is a favorite" })]
    Favorite {
        #[content]
        name: String,
    },
    #[label(tr! { "zh-Hans" => "{n} 个文件", "en" => "{n, plural, one {# file} other {# files}}" })]
    Files { n: i64 },
}

#[component]
pub fn heading(#[content] title: String) -> impl View {
    ui! { Text(content: title, style: headline) }
}
```

`tr!` and enum labels use the same message format (a subset of ICU MessageFormat), which every backend writes out with the same semantics in the session's current language:

| Syntax | Writes out | Arguments |
| --- | --- | --- |
| `{name}` | Text as is, numbers in Bern's notation, enums as their label | `#[content]` text, integers, decimals, labeled enums |
| `{n, plural, =0 {…} one {# 项} other {# 项}}` | Picks the branch by the number's plural category in that language, with exact `=N` matches taking precedence; `#` in a branch is the number | Integers, decimals |
| `{d, date, EEE, MMM d}` | Date: `y`/`yy` year, `M`/`MM` month, `MMM`/`MMMM` month name, `d`/`dd` day, `EEE`/`EEEE` weekday; other characters as is | `CalendarDate` |
| `{t, duration, m:ss}` | Seconds: `h`, `m`, `s`, doubled letters pad with zeros; the largest unit written holds the larger parts (under `m:ss`, 125 seconds is `2:05` and 3725 seconds is `62:05`) | Integers, decimals |

- `{{` and `}}` are literal braces; inside a plural branch `}` always ends the branch.
- Each language must write out the plural categories it uses: Chinese, Japanese, Korean and the like have only `other`, while English and others need `one` and `other`; a missing branch reports B104. A mismatched argument type (text for a plural, a number for a date, a date written directly as `{d}`) also reports B104.
- The placeholders of an enum label are that variant's fields; do not write `; name: value`. Text fields must be `#[content]`; the type of an enum field must have a label on every variant.
- Month and weekday names are framework text, built in for Simplified Chinese, Traditional Chinese, English and Japanese; other languages use English.
- `CalendarDate { year, month, day }` is in `bern::prelude` and represents only a date, without time of day or time zone. `Text(content: date)` is rejected; the format must be written out with `tr!`.
- These formats and the labels of displayed fields are implemented by SwiftUI, the Web, GTK, Compose and WinUI 3 (target feature `text_formats`): each backend's runtime parses translated text by the same format, and plural branches, dates and durations are written out in the current language.

Description text in data (a plugin's introduction, a changelog) often carries inline Markdown. `Text(markdown: true)` shows `**bold**`, `*italic*`, `` `code` ``, `~~strikethrough~~` and `[link](url)` in the platform's style, and a backslash makes the next marker show literally; it recognizes only inline markup, and headings, lists, quotes, code blocks and tables are split into blocks by the app, each shown with a suitable component. Code blocks use `style: code`, the platform's monospaced font:

```rust
Column(gap: 10) {
    Text(content: plugin.summary, markdown: true)
    Text(content: plugin.install_command, style: code)
}
```

Windows currently strips the markup and shows only the text; for the other targets' implementations see the [property table](Component-Properties.md).

## Coordinate scenes and scrolling content

`ScaleBox(width:, height:)` defines a fixed logical canvas that is scaled proportionally and centered within the available area the parent container gives, keeping the whole canvas visible. The parent container must provide an available width and height. `Positioned(x:, y:, width:, height:)` positions a layer from the top-left corner of the scene, and coordinates may be negative; position and size can be bound to state and change smoothly together with `animate`. Content outside the `ScaleBox` is clipped.

When the area and the canvas have different aspect ratios, blank space is left on both sides of the canvas (or above and below it). A `Positioned(bleed: true)` layer keeps extending past every canvas edge it reaches, filling out to the edges of the `ScaleBox`: write backgrounds and window-filling pictures as layers that reach the canvas edges and carry `bleed`, and wide windows get no black bars; when the canvas is shown at its design size there is no blank space, exactly the same as without `bleed`. `bleed` can be bound to a boolean expression (for example extending only while the picture fills the window); the extended edges move in and out with the layer's `animate` motion and follow window size changes immediately. It only takes effect on layers in a `ScaleBox` scene; a `Positioned` inside a layer uses its own coordinates and does not extend further. Implemented by: Web, SwiftUI (macOS, iOS, tvOS), Compose, GTK and WinUI 3.

`WindowButtons` takes up the space that the window's own buttons occupy at the leading edge of the content: when the content extends under the title bar, the macOS traffic lights and the spacing after them take 84 points in total; windows on the web, phones, tablets, TVs, Windows and Linux have no buttons there, so it takes no space and produces no spacing in its row.

```rust
ScaleBox(width: 1280, height: 720) {
    Positioned(x: 0, y: 0, width: 1280, height: 720) {
        Image(source: app.poster, width: 1280, height: 720, fit: cover)
    }
    Positioned(x: 48, y: 40, width: 1184, height: 32) {
        Marquee(duration: 12, phase: 0.25, paused: app.paused, opacity: 0.8) {
            Text(content: app.message, font_size: 24)
        }
    }
}
```

The `duration` of `Marquee` is the number of seconds one pass takes to cross the content area from right to left, and the current runtime uses at least 1 second; `phase` is the starting fraction of the cycle, recommended 0–1, default 0. `paused` defaults to false, and `opacity` defaults to 1 and is clamped to 0–1. The clock is managed by the presentation layer and can resume after pausing; under Reduce Motion the content is presented statically. It only expresses looping scrolling content and provides no per-frame callbacks, video clock synchronization or danmaku scheduling engine.

These three components are provided by the `scaled_scene` capability, currently supported by SwiftUI and the Web. SwiftUI applies one overall scale; the Web updates the scale through a ResizeObserver and stops observing on unmount. iOS shares the generation path; no player device interaction acceptance was done in this round.

## Control containers and sizes

`ControlGroup(height: 40)` puts controls with related functions into one shared capsule, keeping each child control's own clicks, menus and popovers. The height defaults to 40; `label` is optional, the Web uses it as the group's accessible name, and SwiftUI currently keeps the child controls' accessible names. SwiftUI uses interactive Liquid Glass from macOS / iOS 26 on and a system material capsule on older systems; the Web uses its own capsule style.

```rust
ControlGroup(height: 40) {
    IconButton(id: subtitles, icon: "captions.bubble",
        label: tr! { "zh-Hans" => "字幕", "en" => "Subtitles" }) => app.toggle_subtitles()
    IconButton(id: fullscreen, icon: "arrow.up.left.and.arrow.down.right",
        label: tr! { "zh-Hans" => "全屏", "en" => "Fullscreen" }) => app.toggle_fullscreen()
}
```

Standalone controls can declare a container and its height with `shape: capsule` and `height` on `Button`, `Menu`, `Picker`, `Switch` and `SplitButton`. The web has no capsule-shaped buttons: on the web they are standard controls with the standard height and an 8-point corner radius. `Button` and `Switch(style: button)` also have `shape: chip`, which makes them a chip: on the web an outlined pill (30 high, inverted when selected or with `prominence: prominent`), on other platforms a 30-point-high capsule, or the written height when `height` is given. Adjacent menus use a consistent shape / height, and related actions can share a ControlGroup; the shape itself does not change the command's semantics. `IconButton(prominence: plain)` is for standalone buttons that need a plain icon, and `font_size` adjusts the icon size; it does not replace the container height. `prominence: prominent` expresses an emphasized button.

A back entry with an icon and text uses one `Button(icon:, label:)`, which lays out and hit-tests the two as one control. Container size belongs to the declaration; content overflow cannot be avoided by removing the container. The default appearance of ordinary controls is still decided by the target platform.

## Engine-controlled controls

| Control | Session draft source | Engine snapshot source | Value the command receives |
| --- | --- | --- | --- |
| Switch | `value: draft.enabled` | `checked: app.enabled` | The boolean the user just chose |
| Picker | `value: draft.appearance` | `selected: app.appearance` | The enum value the user just chose |
| Slider | `value: draft.volume` | `current: app.volume` | The number the user committed |
| TextInput | `value: draft.name` | `current: app.name` | The whole text the user just changed it to |

Each control can choose only one column as its source; it cannot write both or neither. An engine source must provide an action; in the action's arguments, an expression identical to `checked` / `selected` / `current` is replaced with the user's new value, while its subexpressions and other arguments are not replaced. The enum variants of a Picker must have labels.

```rust
Slider(id: volume,
    label: tr! { "zh-Hans" => "音量", "en" => "Volume" },
    label_hidden: true, current: app.volume, min: 0, max: 1, step: 0.01
) => app.set_volume(app.volume)
```

A controlled `TextInput` runs its action on every edit, without waiting for Enter: use it for fields that save as you type, such as renaming and filter conditions. While the field has focus, the user's text stays in the box, and the engine's response does not move the cursor away or swallow what was just typed; once focus leaves, the engine's text is shown. With `value:`, the action runs on submit.

```rust
TextInput(id: rename, label: tr! { "zh-Hans" => "名称", "en" => "Name" }, current: app.opened_name) => app.rename(app.opened, app.opened_name)
```

A controlled Slider shows a local preview while dragging by default and commits when the drag ends; `current:` together with `continuous: true` commits during the drag (for immediate settings such as volume, opacity and font size), coalesced to at most 30 times per second on Apple platforms, with the last value committed immediately on release; engine snapshots do not take back the slider position during an edit. `PickerOption(value:, label:)` declares options explicitly and can insert live counts into the label. Controlled values are supported by SwiftUI and the Web through `controlled_values`, and explicit options are additionally checked under `modal_flow_timer`.

`Picker(style: chips)` has one chip per option, with the selected one emphasized: on Apple platforms they are system glass capsule buttons (the design's 30-point chip, the selected one in the theme color, a material capsule before 26), on Compose Material `FilterChip`, on WinUI 3 `ToggleButton`, on GTK `GtkToggleButton`s in one group, and on the Web outlined pill buttons (the selected one inverted). When it has no visible label (`label_hidden: true`) and sits directly in a `Flow`, each chip is a child of that Flow and they wrap one by one when it is narrow; elsewhere they wrap within their own row. Below is a toolbar: the segmented control on the left, the chips on the right, and chips that do not fit wrap to the next line.

```rust
Flow(gap: 10) {
    Picker(id: tab, label: tr! { "zh-Hans" => "插件", "en" => "Plugins" }, selected: app.tab, style: segmented, label_hidden: true) => app.set_tab(app.tab)
    Spacer
    Picker(id: category, label: tr! { "zh-Hans" => "分类", "en" => "Category" }, selected: app.category, style: chips, label_hidden: true) => app.set_category(app.category)
}
```

## Images, typography and shared elements

| Declaration | Purpose and limits |
| --- | --- |
| `Image(aspect: 0.6666666667, fit: cover)` | Keeps 2:3 as the container width changes, so the frame does not jump before and after loading; the ratio must be positive |
| `Image(corner:)` | Clips the image to rounded corners and can be bound to a length; a named corner radius from `style!` also works, for example `corner: poster` |
| `Image(rendering: template)` | Renders a template image in the current foreground color; original color images use original |
| `Grid(row_gap:)` | Sets the row spacing separately; when omitted, gap is used |
| `Grid(min_width: 300, gap: 12, masonry: true)` | Masonry layout: cards of different heights each keep their own height and go into the currently shortest column; GTK and Windows lay them out row by row as an ordinary grid |
| `Row { Text(…) Spacer Switch(…) }` | The leading text takes the place of the `Spacer` and wraps within the remaining width, while the switch keeps its own width, like a row in the system settings; SwiftUI and Compose lay it out this way |
| `Text(font_size:, lines:)` | The font size can be bound to a length, and lines sets the maximum number of lines; a dynamic font size requires scaled_scene |
| `Row(align: baseline)` | Aligns by the baseline of the first line of text |
| `padding_x` / `padding_y` | Set horizontal and vertical padding separately on the layout containers that support them; see the property table |

Shared elements use `hero: poster, hero_key: item.id`: the static hero name distinguishes the purpose, and the dynamic key distinguishes the specific item. Both ends must be in the same window with matching keys; do not keep several visible elements with the same key at once. `hero_key` does not replace the stable key of ForEach.

On macOS, Image uses a pixel overlay above the window, and the Web uses a copy of the image in viewport coordinates, handling coordinate changes across the sidebar and ScaleBox; the temporary picture accepts no clicks, does not enter the accessibility tree, and does not fly under Reduce Motion. macOS / Web have passed acceptance for an image expanding from the stage, shrinking into the sidebar and restoring. Other views on iOS and macOS still use matched geometry; passing Video's continuous picture across split-view columns was outside the scope of this image acceptance.

## Same-window onboarding, sidebar and search

SwiftUI targets can set the split-view background colors in `style!` with `Sidebar { background: color(light: "#FAFAFA", dark: "#171717") }` and `Detail { background: color(light: "#F2F2F2", dark: "#1E1E1E") }`, adding `#[target(macos, ios, tvos)]` to the rules. The background is drawn directly on the native split view and extends behind the window bar, while the content still respects the title bar's safe area, with no need for an extra `Stack` wrapper. The system background of the sidebar list is hidden, and the top and bottom bars use the same background.

| Interface | Current semantics |
| --- | --- |
| `WebPage(url:, cookies_file:, cookie_domain:, user_agent:, close_on:, label:) => action` | A page of the web filling its place, in a session of its own that goes with it. A press on what the CSS selector `close_on` finds runs the action, before the page's own handlers: a page made for another app's web view can close with its own button. Schemes other than the web's are not followed. `cookies_file` names a file holding `name=value; name=value`, read when it appears and set on `cookie_domain`: sign-in cookies stay out of the state, where the command line could read them. A browser cannot set them on another site's frame. Not available on tvOS, which has no web engine |
| `Modal(width:, height:)` | A centered in-window panel whose size is constrained by the available window area, with a background scrim; the body can scroll and the footer can be placed separately. Without `height` it is as tall as its content: the body's `ScrollView` takes the height of the content it scrolls, and when that exceeds the window the panel stops at the window edge and the body scrolls. The background subtree is disabled with `Stack(enabled: false)` |
| `Flow(gap:)` | Wraps from left to right by the content's intrinsic size; the children in each row are centered vertically, and a `Spacer` takes no width when wrapping and, once laid out, fills the rest of its row |
| `Timer(interval:, enabled:) => command()` | Submits the command periodically while mounted and enabled; cancelled when unmounted or disabled; the interval is in seconds. It cannot be used to drive per-frame drawing |
| `Watch(value:) => command()` | Runs the action once when mounted, then once each time the value changes; an equal new value does not count as a change. Use it to tell the engine about drafts that live only in the interface: the selected rows of a table, the page selected in the sidebar, the text of a search box. The value is a boolean, number, text or enum, or an optional or list of them. Written inside an `if`, it runs once at the moment the condition becomes true |
| `Text(color:)` | Draws text in the accent color or a status color: `accent`, `status.success`, `status.warning`, `status.error`, each platform using its own text colors; mutually exclusive with `emphasis`. `mark` is shading under the text and does not change the text color |
| `Accordion(count:, selected:, collapsed_width:, height:, gap:, corner:)` | One column is expanded and the others keep a fixed collapsed width; selected starts at 0, and animate drives the column widths and positions; height can be omitted, in which case it fills the remaining height of the parent layout and should be placed in a container with a height constraint (such as a Column in a window) |
| `Sidebar(width:)` | Fixed column width. `SidebarHeader` is pinned above the list and `SidebarFooter` below it. `SidebarHeader(role: title)` means the header only names the window with the app's icon and name: platforms whose title bar already shows the app name (both Windows frontends and Linux GTK) do not show it, and other platforms show it as usual |
| `SidebarItem(badge:)` | Shows a positive count; the badge in a module's page list can be bound to a module state field |
| `Search(style: palette)` | Opens a command palette from a toolbar entry; `panel_prompt`, `width` and `height` set the prompt and the panel size |
| `SearchItem(id:, label:) => command()` | A panel result, which can carry an icon, detail, hint or custom child content; `navigation: false` excludes it from arrow-key selection, while it can still be clicked and focused with Tab |

The search panel supports arrow keys, Enter, Esc, outside clicks and focus restoration after closing, and is currently available only on macOS and the Web. The other extensions in the table are provided by SwiftUI and the Web; the app's layout and platform choices must still pass the target capability check.

## Motion and the idle state

- `appear` / `disappear` control structural changes; `animate` controls supported length changes, and `reflow` makes neighboring elements give way. Motion names come from `style!`.
- `corner { card: 22 }` in `style!` gives a corner radius a name, and `#[target(windows)] corner { card: 8 }` gives that platform its own value; style rules, the `corner` of `Image` / `Video` / `Shape` / `Accordion` and the `if` expressions in it can all use the name, which is replaced with the target platform's value at build time.
- `grow(edge: bottom, bouncy)` expands with one edge fixed; `appear_delay` must go with appear, the delay is non-negative and can be added to stagger. Ordinary state updates do not replay the entrance, and unmounting cancels the wait.
- `nudge` responds to a boolean trigger; `nudge_key` lets different event values trigger the hint, `nudge_scale` sets the amplitude, and `nudge_motion` sets how it returns. A container's `reduce_motion` is combined with the system Reduce Motion preference.
- While `spin` holds, the element rotates around its center at a constant speed, one turn every `spin_period` seconds (default 1, negative for counterclockwise); when it does not hold, the element stops at its current angle, and it does not turn under Reduce Motion. GPUI only rotates an `Image` from resources.
- `glow: multiple` lights up a white glow where the pointer is on the element, with a radius of this multiple of the element's shorter side and a center opacity of `glow_opacity` (default 0.4); it lights up while the mouse hovers, and on touch screens while a finger is held down. It looks at whether the pointer is within the element's bounds, so a title laid over a cover does not block the glow. `blur: radius` blurs the element's picture without going beyond its bounds and corner radius. Neither affects layout or taps; GPUI does not support them, and the check warns.
- `Window(idle_after:, idle_enabled:, immersive:)` controls the idle state and immersive mode. idle_after is in seconds, and `window.idle` is read-only session state. idle_enabled defaults to true; turning it off clears the idle state and stops detection, and turning it on restarts the timer. SwiftUI and the Web consume this switch (capability `idle_control`); other targets do not consume the property yet, `bern check` warns for them, and idle detection is always on there.

macOS keeps the control layer while text is being edited or a menu or child window is in use; the Web restarts the timer only when the enabled state changes, and playback progress snapshot updates do not prevent idle hiding. UIKit shares the start/stop logic; player device interaction has not passed acceptance yet.

## Persistence and local preview

`bern::storage::Store::host()` uses a small data store installed by the host and isolated by the app's bundle ID. macOS / iOS use Application Support, Windows uses APPDATA, and Linux uses XDG_CONFIG_HOME / ~/.config; a Web app uses same-origin localStorage, and WebUI uses the native engine host. Keys are limited to simple file names; a missing value is different from an empty value, and read and write failures return errors. `Store::directory` is for native tests with a given directory; pure engine tests without a host do not persist anything.

```sh
bern run --target web_app --port 4173
```

Target names are defined by the project's Bern.toml. `--port` is only for browser apps, in the range 1–65535; when omitted, a free port is used. A fixed port keeps the same origin, so that a port change during development does not read a different localStorage. Browser permissions, storage quotas and clearing site data affect persistence.

## TV, keyboard shortcuts and initial focus

- `Layout(size: tv)` is the layout on TVs, at most one per `Adaptive`, and does not affect the required and fallback rules of the other sizes; expressions can read `window.tv`. tvOS keeps only what the TV shows at build time, so capabilities in the phone and desktop layouts that TVs lack (such as `pick:`) do not make the tvOS build fail.
- `Shortcut(key:, enabled:) => action` is a keyboard shortcut without a control; keys are written the same way as a control's `shortcut`. On TVs, `escape`, `space` and the arrow keys are the remote's back, play/pause and touch surface directions (directions only arrive when focus has nowhere to move). On Android phones and tablets the system Back, its button or gesture, runs the `escape` shortcut active last before it leaves the app: a page or layer that closes on Escape closes on Back too.
- `default_focus: true` on `Button` / `IconButton` makes focus start from it when the interface appears; on TVs every page should give one.
- `window.keyboard` is whether a physical keyboard is connected: always true on computers, and on phones and tablets it follows the keyboard being connected and disconnected. Content that is only useful with a keyboard (such as keyboard shortcut settings) is written inside `if window.keyboard { … }`, so it does not appear on phones without a keyboard.
- `window.app_version` is `version` of `[app]` in `Bern.toml`, the one every package carries: `Text(content: tr! { "en" => "Version {version}"; version: window.app_version })`. It is decided when the app is built, so it is the same on every target. A component library belongs to no application and takes it as a parameter.
- macOS windows hide the system focus ring throughout, including on buttons, text fields, sliders, toolbars and popover panels; keyboard focus, text editing and keyboard shortcuts still work normally.
- The `text` block of `style!` can give TVs their own font size: `body: font(size: 13, tv: 16)`.
- Lengths that an element writes itself can also be left to styles to give per platform, with the element choosing a style through `variant`: `gap`, `padding`, `padding_x`, `padding_y`, `width`, `height` of `Row` and `Column`; `width`, `height` of `Stack`; `gap`, `padding_x`, `padding_y`, `row_height` of `Form`; and `font_size`, `height` of `IconButton`. `width: fit` and `height: fit` remove the written size, and the element takes the size of its content. When building for a platform, the rules that apply to it (those without `#[target]` first, those with it after) replace the values the element writes, so where the design has different sizes on different platforms, the interface does not need to check the platform:

```rust
style! {
    Row.brand { gap: 5, padding_x: 12 }
    #[target(browser)]
    Row.brand { gap: 10, padding_x: 0 }
    Row.menu_box { width: 92 }
    #[target(browser)]
    Row.menu_box { width: fit }
}
```

  An element that does not have that length reports an error, for example `Button.x { gap: 4 }`; lengths are not less than 0, and `font_size` is greater than 0.
- Keys such as those of an on-screen keyboard use text session actions: `=> draft.query.push_str("A")`, `=> draft.query.pop()`; a search page that wants to list results before anything is typed writes `Search(suggest: true)`.
- `Pages(slot:, all: true)` in `Tabs` also shows the pages declared `tab = false`, for a TV that has no room-saving button for them. `Tab(display: icon)` makes a tab show only its icon, for example a search page that is just a magnifying glass in a TV tab bar; `icon` is required, and `label` is still the name the screen reader reads (and the hover tooltip on desktops). The keywords are the same as for `Label(display:)`, defaulting to `title_and_icon`. It is not limited to TVs; all six targets generate it.

```rust
Adaptive(id: shell) {
    Layout(size: compact) { phone_home(app: app) }
    Layout(size: expanded) { desktop_home(app: app) }
    Layout(size: tv) { tv_home(app: app) }
}

Stack {
    Button(id: resume, label: tr! { "zh-Hans" => "继续播放", "en" => "Resume" }, default_focus: true) => app.resume()
    if app.details_open {
        Shortcut(key: "escape") => app.close_details()
    }
}

Tabs(id: tv_tabs, selection: draft.page) {
    Tab(value: Page::Search, label: tr! { "zh-Hans" => "搜索", "en" => "Search" }, icon: "magnifyingglass", display: icon) { search(app: app) }
    Tab(value: Page::Home, label: tr! { "zh-Hans" => "首页", "en" => "Home" }) { tv_home(app: app) }
}
```

For the scope of each target see the [platform capability table](Platforms.md); for tvOS focus, remote control and scaled scenes see the [tvOS implementation notes](../Implementation/tvOS-SwiftUI.md).

## Menu bar and rebindable keyboard shortcuts

- `shortcut:` and `Shortcut(key:)` can be bound to text, and the keyboard shortcut follows it: `shortcut: app.keys.open`. The text is read in keyboard shortcut notation (`cmd+shift+o`); empty text, or text that cannot be read as a shortcut, means there is no shortcut. The engine normalizes the keys the user chose with `bern::shortcut::parse_shortcut`, and uses `bern::shortcut::same_keys` to tell whether two key strings are the same set of keys.
- `ShortcutInput(id:, label:, current: app.keys.x) => app.set_key(app.keys.x)` is a button for rebinding: it shows the current shortcut (`empty` when there is none), and after it is pressed it waits for a new key combination (showing `prompt`); Esc leaves it unchanged and Delete pressed on its own clears it; while it is waiting, key presses do not trigger other shortcuts. Conflicts, saving and restoring defaults are handled by the app's engine.
- `MenuBar` in a `WindowSet` is the app's menus: `CommandMenu(label:)` is a menu of the app's own, `CommandMenu(role: file)` and the like add items to the system menu of the same name, a nested `CommandMenu` is a submenu, and `Divider` groups items; a `CommandItem` has an action, `shortcut`, `enabled` (grayed out), `checked` (check mark) and `pick`. The drafts that menu items read and write belong to the main window and are declared with a `#[session]` parameter on the root component.
- Only macOS shows the menu bar; other targets accept the declaration but do not show it (capability `menu_bar` not applicable, no warning). `window.menu_bar` is known at build time and is true on macOS; use it to show settings that only exist because of the menu only on the Mac.

```rust
WindowSet(handle: AppWindows, primary: main) {
    Window(id: main, title: tr! { "zh-Hans" => "主窗口", "en" => "Main" }, initial: open) { page(app: app) }
    MenuBar {
        CommandMenu(role: file) {
            CommandItem(id: open, label: tr! { "zh-Hans" => "打开…", "en" => "Open…" }, shortcut: app.keys.open, pick: draft.file, files: "video") => app.open(draft.file)
        }
        CommandMenu(label: tr! { "zh-Hans" => "播放", "en" => "Playback" }) {
            CommandItem(id: play, label: tr! { "zh-Hans" => "播放 / 暂停", "en" => "Play / Pause" }, shortcut: app.keys.play, enabled: app.has_video) => app.toggle_pause()
            Divider
            CommandMenu(label: tr! { "zh-Hans" => "倍速", "en" => "Speed" }) {
                CommandItem(id: double, label: tr! { "zh-Hans" => "2×", "en" => "2×" }, checked: app.speed == Speed::Double) => app.set_speed(Speed::Double)
            }
        }
    }
}
```

For the scope of each target see the [platform capability table](Platforms.md); for semantics and implementation progress see [keyboard shortcuts and the menu bar in the design document](../Design.md#menu-bar).

## Status bar items (tray)

A `StatusItem` in a `WindowSet` is the app's item in the system status area: the notification area of the Windows taskbar, the status area of Linux desktops and the right side of the macOS menu bar. When it is declared, closing the last window does not quit the app; the app stays there.

- `shown` makes that a choice: while it is false the item is not there and closing the last window quits the app, on macOS too, and while it is true the app stays as above. It reads engine state and follows it while the app runs, for a setting such as what closing the window does.

- `label` is the tooltip and the accessible name. The icon is one of two: `icon` is a Bern icon name, for macOS only, where the menu bar draws this symbol in the system color, while Windows and Linux show the app icon; `image` is a monochrome SVG in `[resources]` (only its shape counts, not its color, and cut-outs are the hollowed-out parts), shown on all three desktop platforms, white on a dark bar and black on a light bar, following the system as it switches.
- The first menu item opens the main window and the last one quits, with the declared `CommandItem`, `CommandMenu` and `Divider` in between, which can be written inside `if`, `if let`, `match` and `ForEach`. On Windows and Linux, clicking the icon opens the main window and right-clicking shows the menu; on macOS, clicking shows the menu.
- The menu belongs to no window: it only reads engine state and runs commands and window actions, does not read or write drafts or read window fields, and its items carry no `pick` or `shortcut`.
- `login_item: true` adds "Open at Login" before "Quit": once the user checks it, the system starts the app at login, and it appears only in the status area without opening a window.
- The same switch in a settings window is written `Switch(id:, label:, login_item: true)`: it shows and changes the system login item and is the same item as "Open at Login" in the menu, with no `value`, `checked` or action. Platforms without login items (iOS, tvOS, Android, the web) do not show it (capability `login_switch` not applicable).
- Phones, TVs and the web have no such place; they accept the declaration without showing it (capability `status_item` not applicable).

```rust
StatusItem(icon: "sun.max.fill", label: tr! { "zh-Hans" => "显示器调节", "en" => "Monitor Dial" }, login_item: true) {
    CommandItem(id: dim, label: tr! { "zh-Hans" => "调暗", "en" => "Dim" }, enabled: app.brightness > 10) => app.set_brightness(10)
    Divider
    CommandMenu(label: tr! { "zh-Hans" => "预设", "en" => "Presets" }) {
        ForEach(items: app.presets, key: id) |preset| {
            CommandItem(id: preset, label: preset.name, checked: preset.id == app.preset) => app.apply(preset.id)
        }
    }
}
```

For semantics and each platform's implementation see [status bar items in the design document](../Design.md#status-bar-item).

## Tables

`Table` lays out a list of records in columns, one row per item, and `key` is the row's identity: as rows are added, removed and reordered with state, each keeps its own controls. Each `TableColumn` has a header `title`, and its children are this column's cell in each row, written with the table's row variable.

- `selection: draft.chosen` is a `Vec<String>` or `Vec<i64>` draft that stores the keys of the selected rows in row order. On desktops a click selects one row, and Shift and the platform's toggle key (Command on macOS, Ctrl on other platforms) select several; on Android every row has a checkbox in front, and the checkbox in the header selects all or none.
- The table's action is the primary action for the selected rows: double-click or Enter; on Android, tapping a row selects only that row and runs the action.
- A `TableColumn`'s action runs when its header is clicked, so the engine can change the sort order; `sorted` and `descending` only show the sort indicator on the header, and the order of the rows is the order of `items`.
- `TableMenu` is the row menu, opened by right-clicking (the keyboard's Menu key or Shift+F10) or long-pressing a row; when that row is not among the selected rows, it first becomes the only selected row, so menu items that read `selection` get exactly the rows to act on.

```rust
Table(id: servers, label: tr! { "zh-Hans" => "服务器", "en" => "Servers" }, items: app.servers, key: id, selection: draft.selected) |server| {
    TableColumn(title: tr! { "zh-Hans" => "别名", "en" => "Name" }, width: 260, sorted: app.sort == SortKey::Name, descending: app.descending) {
        Text(content: server.name, lines: 1)
    } => app.sort_by(SortKey::Name)
    TableColumn(title: tr! { "zh-Hans" => "延迟", "en" => "Delay" }) {
        Text(content: tr! { "zh-Hans" => "{n} ms", "en" => "{n} ms"; n: server.delay })
    }
    TableMenu {
        MenuItem(id: copy_links, label: tr! { "zh-Hans" => "复制分享链接", "en" => "Copy Share Links" }) => app.copy_links(draft.selected)
        Divider
        MenuItem(id: remove, label: tr! { "zh-Hans" => "删除", "en" => "Delete" }, role: destructive) => app.remove(draft.selected)
    }
} => app.use_chosen(draft.selected)
```

On macOS and iOS it is SwiftUI's `Table`, on GTK a `GtkColumnView`, on WinUI 3 a header plus a `ListView` (Windows has no system table), on the web a `<table>`, and on Android it looks like a Material data table; tvOS has no tables and shows a column of row buttons.

## Clipboard

- `paste: draft.text` on `Button`, `IconButton`, `MenuItem` and `CommandItem` first puts the text from the clipboard into that draft and then runs the action; when the clipboard holds no text, it does neither. Mutually exclusive with `pick`.
- The engine puts text on the clipboard with `bern::platform::copy_text(text)`; it can be called from any thread, and the UI thread completes it.

```rust
Button(id: import, label: tr! { "zh-Hans" => "从剪贴板导入", "en" => "Import from the Clipboard" }, paste: draft.pasted) => app.import_links(draft.pasted)
```

## Photos

`[app].photos = true` lets the engine save a picture where the system keeps the user's photos with `bern::platform::save_image(path)`, a PNG or a JPEG, for other apps to pick from. iOS asks the user the first time, with the reason in each supported language; Android 10 and later need no permission. The call waits for the system's answer, so the engine makes it on a worker thread; `refused` means the user did not allow it, and other platforms answer `platform_unavailable`.

## Progress rings

`Progress(style: ring)` draws how far `value` is of `total` as an arc in the accent from the top, clockwise, with round ends, `width` across (24 unless given) and `stroke_width` thick (3 unless given); at nothing yet it is a dot at the top. It takes no input, so it can lie over a button in a `Stack`, ringing the button's own symbol:

```rust
Stack {
    IconButton(id: download, icon: "arrow.down.circle", font_size: 18, prominence: plain, label: tr! { "zh-Hans" => "下载", "en" => "Download" }) => app.download()
    if app.downloading {
        Progress(value: app.download_progress, label: tr! { "zh-Hans" => "下载进度", "en" => "Download Progress" }, style: ring, width: 19.3, stroke_width: 3)
    }
}
```

## Other interfaces

Animated SVG is a standalone component used on demand; for its interface, resource declarations, platform dependencies and lifecycle see the [dedicated document](../Implementation/Animated-SVG.md). For the full semantics of the video player, queries, modules, themes and the command protocol, continue to see the [design document](../Design.md) and the corresponding implementation notes.

### Hiding unavailable menu items

`Menu(hide_disabled: true)` hides unavailable items according to each `MenuItem`'s `enabled` condition and shows them again when the condition is restored. When this option is omitted, disabled items are still shown grayed out. SwiftUI, the Web, Compose, GTK and WinUI 3 all support it.


### Input region

`FocusRegion` wraps a content view (usually a `Row` holding a text field and a send button). While a text field inside it is being edited, the first pointer action that lands outside the region only cancels the editing focus, consuming that press and the following release, keeping the draft and not submitting it. Buttons inside the region work as usual, and the next outside click works as usual. Keyboard Tab navigation is not affected. Each platform backend handles the events, so the app does not need to intercept them by control ID or by platform.

```rust
FocusRegion {
    Row {
        TextInput(id: message, label: tr! { "en" => "Message" }, value: draft.text)
        Button(id: send, label: tr! { "en" => "Send" }) => app.send(draft.text)
    }
}
```

### Runtime semantic palette

`WindowSet(palette: app.palette)` binds a text field whose content is a JSON object mapping semantic roles to light and dark `#RRGGBB` values, for example `{"accent":{"light":"#006A60","dark":"#4FDAC8"},"surface.canvas":{"light":"#F4FBF8","dark":"#101815"}}`. The palette can be replaced at runtime, and roles that are not written keep the platform default. Both appearances must be given, and foreground and background must be readable as pairs. The `accent` pair also tints native controls. SwiftUI (macOS, iOS, tvOS), GTK and WinUI 3 support `runtime_palette`; on other targets, using this property fails the build. Without a palette, the existing `accent:` and `appearance:` work as before.
