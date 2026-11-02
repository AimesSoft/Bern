**English** · [简体中文](../zh-CN/Reference/Component-Properties.md)

# Built-in component property table

Updated: 2026-10-27. This table is exported from the current public definitions in [bern-ir::spec](../../crates/bern-ir/src/spec.rs) and covers 78 built-in components. **Bold properties are required**, the rest are optional; these are declaration rules. For what each target implements see the [platform capability table](Platforms.md), and for concrete usage see [component usage](UI.md).

## Property types

| Type | Declaration meaning |
| --- | --- |
| `Id` | Stable identifier or string |
| `DisplayText` | tr! text or a displayable state expression |
| `LocalizedText` | tr! static localized text |
| `Icon` | Icon name string (SF Symbols naming). Apple targets can use any SF Symbol; other targets map the name to the platform's own icon, and all of them draw at least the names in the Bern icon set (`bern_target_api::icon_set::NAMES`); using a name the target does not have is reported as an error by the check |
| `ImageResource` | Image resource; Image also supports engine image state |
| `SvgResource` | Declared bundled SVG resource |
| `Number` | Build-time number |
| `Length` | Logical length number or constrained numeric expression |
| `NumberValue` | Read-only numeric expression |
| `TextValue` | Read-only text expression |
| `Bool` | Boolean expression |
| `Flag` | Build-time true / false |
| `Shortcut` | Keyboard shortcut string literal, or a text binding that the shortcut follows |
| `FileKinds` | File kind string |
| `FileField` | Writable MediaSource session field |
| `PositionField` | Writable scroll position session field |
| `TextField` | Writable text session field |
| `KeysField` | Writable `Vec<String>` or `Vec<i64>` session field that stores row keys |
| `BoolField` | Writable boolean session field |
| `NumberField` | Writable floating-point session field |
| `PickerField` | Writable enum session field whose variants carry labels |
| `SelectionField` | Writable navigation selection enum field |
| `SelectionValue` | Enum value matching the navigation selection field |
| `Items` | Record list binding |
| `KeyField` | Name of the stable key field of list records |
| `PathField` | Writable route list session field |
| `Route` | Typed route value |
| `WindowId` | Window kind identifier |
| `TypeName` | Generated window handle type name |
| `SlotName` | Module slot name |
| `StyleVariant` | style! style variant name |
| `Transition` | Transition declaration or name |
| `MotionToken` | Motion token name |
| `HeroName` | Static name of a shared element |
| `KeyValue` | Read-only text, numeric or enum identity value |
| `Watched` | Read-only boolean, number, text or enum, or optional values and lists of them: the value that `Watch` follows |
| `ColorRole` | Semantic color role |
| `NumberList` | Read-only number list |
| `PatternName` | style! pattern name |
| `MediaSource` | Media source binding, optional or non-optional |
| `Player` | A player in the app's player catalog |
| `Query` | Typed query call |
| `Feature` | An entry point in the owning feature contract |

Fixed enums list the allowed keywords directly. The table does not repeat every cross-property constraint: Switch / Picker / Slider must specify either a draft source or a controlled source, and a controlled source must also provide an action; `appear_delay` must be used together with `appear`; the specific errors are checked by the compiler. Resources, semantic colors, localization, feature coverage and types are validated further as well.

## Components

| Component | Properties (bold = required) | Children, action and placement |
| --- | --- | --- |
| `WindowSet` | **`handle: TypeName`**<br>**`primary: WindowId`**<br>`accent: KeyValue`<br>`palette: KeyValue`<br>`appearance: KeyValue`<br>`icon: KeyValue` | Contains only ["Window", "MenuBar", "StatusItem"]; no action; root node only |
| `MenuBar` | None | Contains only ["CommandMenu"]; no action; directly inside ["WindowSet"], at most one; shown only in the macOS menu bar |
| `StatusItem` | **`label: LocalizedText`**<br>`icon: Icon`<br>`image: SvgResource`<br>`login_item: Flag` | Contains only ["CommandItem", "CommandMenu", "Divider"], which can be inside `if`, `match`, `ForEach`; no action; directly inside ["WindowSet"], at most one; the menu does not read or write drafts or window fields, and menu items do not take `pick` or `shortcut`; either `icon` or `image`: `icon` is shown only on macOS, while Windows and Linux show the app icon; `image` is a monochrome SVG that all three desktop platforms show in white or black according to the appearance of the bar it sits in; not shown on iOS, tvOS, Android or the Web |
| `CommandMenu` | `label: LocalizedText`<br>`role: app / file / edit / view / window / help` | Contains only ["CommandItem", "CommandMenu", "Divider"], which can be inside `if`, `match`, `ForEach`; no action; directly inside ["MenuBar", "CommandMenu", "StatusItem"]; either `label` or `role`, and `role` is used only for menus on the menu bar |
| `CommandItem` | **`id: Id`**<br>**`label: DisplayText`**<br>`shortcut: Shortcut`<br>`enabled: Bool`<br>`checked: Bool`<br>`pick: FileField`<br>`files: FileKinds`<br>`paste: TextField` | No children; must bind an action; directly inside ["CommandMenu", "StatusItem"]; the drafts of menu bar items belong to the primary window; either `pick` or `paste` |
| `Window` | **`id: WindowId`**<br>**`title: LocalizedText`**<br>`initial: open / closed`<br>`presentation: auto / virtual / floating / native_required`<br>`role: standard / settings / dialog`<br>`width: Number`<br>`height: Number`<br>`min_width: Number`<br>`min_height: Number`<br>`confirm_close: Bool`<br>`close_message: LocalizedText`<br>`chrome: standard / unified`<br>`immersive: Bool`<br>`landscape: Bool`<br>`idle_after: Number`<br>`idle_enabled: Bool` | Exactly one child; action optional; directly inside ["WindowSet"] |
| `Column` | `variant: StyleVariant`<br>`gap: Length`<br>`padding: Length`<br>`padding_x: Length`<br>`padding_y: Length`<br>`width: Length`<br>`height: Length`<br>`align: leading / center / trailing` | May contain children; no action |
| `FocusRegion` | None | One content view; during text editing, the first pointer action outside the region only removes focus and consumes the whole click, keeping the draft; controls inside the region and the next click work normally |
| `Row` | `variant: StyleVariant`<br>`gap: Length`<br>`padding: Length`<br>`padding_x: Length`<br>`padding_y: Length`<br>`width: Length`<br>`height: Length`<br>`align: top / center / bottom / baseline` | May contain children; no action |
| `Accordion` | **`count: NumberValue`**<br>**`selected: NumberValue`**<br>**`collapsed_width: Length`**<br>`height: Length`<br>`gap: Length`<br>`corner: Length` | May contain children; no action; when height is omitted, it uses the remaining available height |
| `Grid` | **`min_width: Length`**<br>`gap: Length`<br>`row_gap: Length`<br>`padding: Length`<br>`padding_x: Length`<br>`padding_y: Length`<br>`cell_ratio: Number`<br>`cell_extra: Length`<br>`masonry: Flag` | May contain children; no action |
| `Stack` | `variant: StyleVariant`<br>`enabled: Bool`<br>`align: top_leading / top / top_trailing / leading / center / trailing / bottom_leading / bottom / bottom_trailing`<br>`width: Length`<br>`height: Length`<br>`under_bars: Flag` | May contain children; no action |
| `Marquee` | **`duration: Length`**<br>`phase: Number`<br>`paused: Bool`<br>`opacity: NumberValue` | May contain children; no action |
| `ScaleBox` | **`width: Length`**<br>**`height: Length`** | May contain children; no action |
| `Positioned` | **`x: Length`**<br>**`y: Length`**<br>**`width: Length`**<br>**`height: Length`**<br>`bleed: Bool` | May contain children; no action; `bleed` lets a layer in a `ScaleBox` scene extend to the edge of the box on the canvas edges it reaches |
| `Modal` | `width: Length`<br>`height: Length`<br>`cover: compact` | May contain children; no action; without `height` it is as tall as its content (a `ScrollView` inside it is as tall as the content it scrolls), and it stops at the window edge when the window cannot fit it |
| `Flow` | `gap: Length` | May contain children; no action; the children in each line are vertically centered, and `Spacer` takes the rest of the width of its line |
| `Timer` | **`interval: Number`**<br>`enabled: Bool` | No children; must bind an action |
| `Watch` | **`value: Watched`** | No children; must bind an action; runs on mount and when the value changes |
| `Shortcut` | **`key: Shortcut`**<br>`enabled: Bool` | No children; must bind an action |
| `PickerOption` | **`value: KeyValue`**<br>**`label: DisplayText`** | No children; no action; directly inside ["Picker"] |
| `Rail` | `axis: horizontal / vertical`<br>`anchor: start / center / end`<br>`overhang: Length` | May contain children; no action |
| `Spacer` | No own properties | No children; no action |
| `WindowButtons` | No own properties | No children; no action; on macOS it takes up the space of the traffic lights, and on other platforms it takes no space |
| `Divider` | No own properties | No children; no action |
| `ScrollView` | `axis: vertical / horizontal`<br>`snap: start / center`<br>`position: PositionField`<br>`arrival: nudge`<br>`indicators: automatic / hidden / mouse`<br>`open_at: Number`<br>`open_center: NumberValue` | May contain children; no action |
| `SplitView` | No own properties | Contains only ["Sidebar", "Detail"]; no action |
| `Sidebar` | **`selection: SelectionField`**<br>`width: Length`<br>`id: Id`<br>`fallback: SelectionValue` | Contains only ["Search", "Section", "SidebarItem", "Pages", "SidebarHeader", "SidebarFooter"]; no action; directly inside ["SplitView"] |
| `SidebarHeader` | `role: content / title` | May contain children; no action; directly inside ["Sidebar"] |
| `SidebarFooter` | No own properties | May contain children; no action; directly inside ["Sidebar"] |
| `Detail` | `titled: Bool` | May contain children; no action; directly inside ["SplitView"] |
| `SidebarItem` | **`value: SelectionValue`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`badge: NumberValue`<br>`variant: StyleVariant` | No children; no action; directly inside ["Sidebar", "Section"] |
| `Section` | `title: DisplayText`<br>`footer: DisplayText`<br>`style: grouped / plain`<br>`padding_x: Length`<br>`padding_y: Length` | May contain children; no action |
| `Slot` | **`id: SlotName`** | No children; no action |
| `Pages` | **`slot: SlotName`** | No children; no action; directly inside ["Sidebar", "Section", "Tabs"]. Inside `Tabs`, each page expands into a `Tab` whose content is that page's component |
| `PageContent` | **`slot: SlotName`**<br>**`selected: SelectionField`** | No children; no action |
| `Form` | `title: DisplayText`<br>`variant: StyleVariant`<br>`gap: Length`<br>`row_height: Length`<br>`padding_x: Length`<br>`padding_y: Length` | May contain children; no action |
| `Navigation` | **`path: PathField`**<br>`id: Id` | May contain children; no action |
| `Destination` | No own properties | May contain children; no action; directly inside ["Navigation"] |
| `NavLink` | **`id: Id`**<br>**`route: Route`** | May contain children; no action |
| `ControlGroup` | `label: DisplayText`<br>`height: Length` | May contain children; no action |
| `Tabs` | **`selection: SelectionField`**<br>`id: Id`<br>`fallback: SelectionValue` | Contains only ["Tab", "Pages", "TabAccessory"]; no action. The selected tab takes the app's accent color |
| `TabAccessory` | `shown: Bool` | May contain children; no action; directly inside ["Tabs"], at most one, and not placed in `if` or `ForEach`. A persistent strip above all the pages, for example a mini player for what is playing; it collapses when `shown` is false |
| `Tab` | **`value: SelectionValue`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`display: title_and_icon / icon` | May contain children; no action; directly inside ["Tabs"]. `display: icon` shows only the icon (`icon` must be written), and `label` is still the accessible name |
| `SplitButton` | **`id: Id`**<br>**`label: DisplayText`**<br>`value: BoolField`<br>`checked: Bool`<br>`icon: Icon`<br>`shape: capsule`<br>`height: Length`<br>`more_label: DisplayText`<br>`shortcut: Shortcut` | May contain children; either `value` or `checked`; `checked` requires an action, which runs on toggle |
| `PopoverButton` | `panel_appearance: inherit / window / surface`<br>**`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`presented: BoolField`<br>`open: Bool`<br>`shortcut: Shortcut` | May contain children; `open` must bind an action and is mutually exclusive with `presented`; no action when `open` is not used |
| `Inspector` | **`presented: BoolField`**<br>`width: Number` | May contain children; no action |
| `Toast` | **`content: DisplayText`**<br>`icon: Icon`<br>`key: KeyValue`<br>`duration: Number` | No children; no action |
| `Pinch` | **`id: Id`**<br>**`label: DisplayText`**<br>**`value: NumberField`**<br>**`min: Number`**<br>**`max: Number`**<br>`feature: Feature` | Exactly one child; action optional |
| `Swipe` | **`id: Id`**<br>**`label: DisplayText`**<br>**`value: NumberField`**<br>**`min: Number`**<br>**`max: Number`**<br>`axis: vertical / horizontal`<br>`distance: Number`<br>`feature: Feature` | Exactly one child; action optional |
| `Drop` | **`id: Id`**<br>**`label: DisplayText`**<br>**`value: FileField`**<br>`files: FileKinds`<br>`over: BoolField`<br>`opens: Flag`<br>`feature: Feature` | Exactly one child; must bind an action |
| `Toolbar` | No own properties | Contains only ["Button", "IconButton", "Picker", "Menu", "Search", "Switch", "SplitButton", "PopoverButton", "Divider", "Spacer", "Text"]; no action |
| `Adaptive` | `id: Id` | Contains only ["Layout"]; no action |
| `Layout` | **`size: compact / medium / expanded / large / tv`** | May contain children; no action; directly inside ["Adaptive"] |
| `Menu` | **`id: Id`**<br>**`label: DisplayText`**<br>`hide_disabled: Flag`<br>`icon: Icon`<br>`style: button / icon`<br>`shape: capsule`<br>`height: Length` | Contains only ["MenuItem", "Divider"]; no action |
| `MenuItem` | **`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`role: normal / destructive`<br>`enabled: Bool`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`pick: FileField`<br>`files: FileKinds`<br>`paste: TextField` | No children; must bind an action; directly inside ["Menu", "TableMenu"]; either `pick` or `paste` |
| `LanguagePicker` | **`id: Id`**<br>**`label: DisplayText`** | No children; no action |
| `List` | `items: Items`<br>`key: KeyField` | May contain children; no action |
| `ForEach` | **`items: Items`**<br>**`key: KeyField`** | May contain children; no action |
| `Table` | **`id: Id`**<br>**`label: DisplayText`**<br>**`items: Items`**<br>**`key: KeyField`**<br>`selection: KeysField` | Contains only ["TableColumn", "TableMenu"], at least one `TableColumn` and at most one `TableMenu`; action optional, it is the primary action on the selected rows (double-click or Enter, or tapping a row on a touch screen); `selection` is a `Vec<String>` or `Vec<i64>` draft that stores the keys of the selected rows in row order. Uses the `\|item\|` row template |
| `TableColumn` | **`title: DisplayText`**<br>`width: Number`<br>`sorted: Bool`<br>`descending: Bool` | Exactly one child, which is the content of this column's cell in each row; action optional, runs when the header is clicked; directly inside ["Table"]; `sorted` and `descending` only show the sort indicator, and the order of the rows is decided by the engine |
| `TableMenu` | None | Contains only ["MenuItem", "Divider"]; no action; directly inside ["Table"]; opens on right-click or long press on a row, and if that row is not among the selected rows, it first selects only that row |
| `PreviewList` | **`items: Items`**<br>**`key: KeyField`**<br>**`item_height: Number`**<br>`gap: Number`<br>`more_height: Number` | Creates the first few items that fit completely in the available height; when items remain, leaves room for `+N`, with a default height of 22; does not scroll and does not create hidden items. Uses the `\|item\|` row template |
| `Text` | **`content: DisplayText`**<br>`style: large_title / title / title2 / title3 / headline / subheadline / body / callout / footnote / caption / caption2 / control / code`<br>`emphasis: primary / secondary / tertiary`<br>`color: accent / status.success / status.warning / status.error`<br>`mark: ColorRole`<br>`lines: Number`<br>`font_size: Length`<br>`line_height: Length`<br>`markdown: Flag` | No children; no action; either `color` or `emphasis` |
| `Canvas` | `width: Length`<br>`height: Length`<br>`label: DisplayText` | Contains only ["Shape", "Series", "Pattern"]; no action |
| `Shape` | **`kind: rect / rounded / capsule / ellipse`**<br>`x: Number`<br>`y: Number`<br>`width: Number`<br>`height: Number`<br>`corner: Number`<br>`fill: ColorRole`<br>`fill_from: ImageResource`<br>`fade: up / down / left / right`<br>`stroke: ColorRole`<br>`stroke_width: Number`<br>`dash: Number`<br>`dash_gap: Number`<br>`opacity: Number` | No children; no action; directly inside ["Canvas"] |
| `Series` | **`values: NumberList`**<br>`kind: area / line`<br>`max: NumberValue`<br>`fill: ColorRole`<br>`stroke: ColorRole`<br>`stroke_width: Number`<br>`opacity: Number` | No children; no action; directly inside ["Canvas"] |
| `Pattern` | **`name: PatternName`**<br>`opacity: Number` | No children; no action; directly inside ["Canvas"] |
| `Label` | **`title: DisplayText`**<br>**`icon: Icon`**<br>`style: large_title / title / title2 / title3 / headline / subheadline / body / callout / footnote / caption / caption2 / control / code`<br>`font_size: Number`<br>`display: title_and_icon / icon` | No children; no action |
| `Svg` | **`source: SvgResource`**<br>`static_source: SvgResource`<br>**`width: Length`**<br>**`height: Length`**<br>`label: DisplayText`<br>`fit: contain / cover / fill`<br>`playing: Bool`<br>`speed: NumberValue`<br>`restart: NumberValue`<br>`interactive: Flag`<br>`data: DisplayText`<br>`external_links: Flag` | No children; no action |
| `Image` | **`source: ImageResource`**<br>`label: DisplayText`<br>`width: Length`<br>`height: Length`<br>`fit: contain / cover / fill`<br>`corner: Length`<br>`rendering: original / template`<br>`aspect: Number` | No children; no action |
| `Progress` | `value: NumberValue`<br>`total: Number`<br>`label: DisplayText` | No children; no action |
| `Button` | **`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`shape: capsule / chip`<br>`height: Length`<br>`width: fill`<br>`role: normal / destructive / cancel`<br>`prominence: standard / prominent / plain`<br>`enabled: Bool`<br>`variant: StyleVariant`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`pick: FileField`<br>`files: FileKinds`<br>`default_focus: Flag`<br>`clears: TextField`<br>`paste: TextField` | May contain children; must bind an action; `paste` first puts the text on the clipboard into that draft, and the action runs only when there is text; either `pick` or `paste` |
| `IconButton` | **`id: Id`**<br>**`icon: Icon`**<br>`font_size: Number`<br>`height: Length`<br>**`label: DisplayText`**<br>`prominence: standard / prominent / plain`<br>`enabled: Bool`<br>`variant: StyleVariant`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`pick: FileField`<br>`files: FileKinds`<br>`paste: TextField`<br>`default_focus: Flag` | No children; must bind an action; either `pick` or `paste` |
| `TextInput` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>`value: TextField`<br>`current: TextValue`<br>`prompt: DisplayText`<br>`kind: plain / secure / multiline`<br>`submit: clear`<br>`clear_key: NumberValue`<br>`feature: Feature` | No children; action optional; either `value` or `current`; with `value` the action runs on submit, and `submit: clear` clears the draft and gives up focus after the action; with `current` the action runs on every edit, the same expression in the action's arguments is the text typed, and while editing the engine's text does not overwrite the user's input; SwiftUI's `clear_key` clears when the number changes, for use with asynchronous success acknowledgements |
| `Search` | **`id: Id`**<br>**`value: TextField`**<br>`prompt: DisplayText`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`style: automatic / palette`<br>`panel_prompt: DisplayText`<br>`width: Number`<br>`height: Number`<br>`font_size: Number`<br>`suggest: Flag` | May contain children; action optional |
| `SearchItem` | **`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`detail: DisplayText`<br>`hint: DisplayText`<br>`navigation: Bool`<br>`pick: FileField`<br>`files: FileKinds`<br>`feature: Feature` | May contain children; must bind an action |
| `Switch` | **`id: Id`**<br>**`label: DisplayText`**<br>`value: BoolField`<br>`checked: Bool`<br>`label_hidden: Flag`<br>`shape: capsule / chip`<br>`height: Length`<br>`style: switch / checkbox / button`<br>`icon: Icon`<br>`shortcut: Shortcut`<br>`login_item: Flag` | No children; action optional; `login_item: true` shows and changes the system login item, and is written without `value`, `checked` or an action |
| `ShortcutInput` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>**`current: Shortcut`** (state, cannot be a literal)<br>`prompt: DisplayText`<br>`empty: DisplayText`<br>`enabled: Bool`<br>`feature: Feature` | No children; must bind an action, and in the action the expression identical to `current` is replaced with the chosen shortcut |
| `Slider` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>`value: NumberField`<br>`current: NumberValue`<br>`continuous: Flag` (only with `current`)<br>**`min: Number`**<br>**`max: Number`**<br>`step: Number`<br>`min_icon: Icon`<br>`max_icon: Icon`<br>`feature: Feature` | No children; action optional |
| `Video` | **`id: Id`**<br>`source: MediaSource`<br>`player: Player`<br>`label: DisplayText`<br>`controls: standard / none`<br>`fit: contain / cover / fill`<br>`sizing: standard / fill`<br>`corner: Length` | No children; no action. A video with `controls: none` shows only the picture and can be the content of a `Button`. `sizing: fill` makes the playback surface fill a parent container that has a definite size, and `fit` only controls how the video content is placed in the surface; danmaku drawn by the media core can use the letterbox bars. The default `standard` keeps the original layout. When the app registers a media core, the core draws into the surface the view gives it; see the [design document](../Design.md#app-provided-media-cores) |
| `Query` | **`id: Id`**<br>**`query: Query`**<br>`loading: DisplayText`<br>`empty: DisplayText`<br>`failed: DisplayText`<br>`icon: Icon` | May contain children; no action |
| `Picker` | **`id: Id`**<br>**`label: DisplayText`**<br>`value: PickerField`<br>`selected: KeyValue`<br>`label_hidden: Flag`<br>`shape: capsule`<br>`height: Length`<br>`style: automatic / menu / segmented / radio / inline / chips` | Contains only ["PickerOption"]; action optional; `chips` does not accept `shape` / `height` |

When `Grid` is given `cell_ratio`, every row has the same height: column width ÷ `cell_ratio` + `cell_extra` (for example a 2 : 3 poster plus the height of two lines of text: `cell_ratio: 0.6667, cell_extra: 54`); items are laid out from the top of their cell, and taller content extends into the row gap below, unlike the default, which sets the row height from the tallest item.

With `masonry`, `Grid` becomes a masonry layout: items are laid out at their own heights, and each item goes into the column that is currently shortest (the leftmost one when columns are equally short), which suits cards of varying heights. SwiftUI and Compose use a custom layout, and the Web uses one-pixel-high grid rows so that each item spans its own height; GTK and Windows for now lay it out row by row as an ordinary grid.

`Text(style: code)` uses the platform's own monospaced font, slightly smaller than body text, for code, commands and file contents: the `.monospaced` design in SwiftUI, `FontFamily.Monospace` in Compose, the `ui-monospace` family on the Web, the `monospace` style class in GTK, and Cascadia Mono on Windows (Consolas when it is not available). The `text` block of `style!` can set its font size and weight like any other style.

The `static_source` of `Svg` is the frame at the end of the animation, and builds without a WebView show it directly; `data` is a piece of text handed to the document, which the document's script reads in `state.data` of `window.bernSvg.update(state)`, and it is handed over again when it changes; with `external_links: true`, https links opened in the document are handed to the system browser, and the document itself never leaves the bundled resources. See [Animated SVG](../Implementation/Animated-SVG.md).

With `markdown`, the content of `Text` is inline Markdown: `**bold**`, `*italic*` or `_italic_`, `` `code` ``, `~~strikethrough~~` and `[link](url)`, and a backslash makes the next marker display literally. SwiftUI hands it to the system's `AttributedString(markdown:)`, Compose writes it as an `AnnotatedString` with styles and `LinkAnnotation`, the Web generates elements (not treated as HTML; links accept only web and email addresses), and GTK converts it to Pango markup; Windows for now strips the markup and shows only the text. A whole document (headings, lists, tables and so on) is split into blocks by the app itself, and each block is shown as such text.

`Search` placed in a `Toolbar` or a `Sidebar` is taken over by them; placed in a page, it is the platform's own search field (`UISearchTextField` on iOS, `NSSearchField` on macOS, `input[type=search]` on the Web), its children are the results listed below the field when the field has focus or contains text, and `width` / `height` / `font_size` are the size and font size of the field itself (for the search panel in a toolbar, they are the panel's size).

`Rail(overhang:)` makes the ticks extend this many points past each end of the track: a child at ratio f stands at f × (length + 2 × overhang) − overhang, like a timeline built with negative margins in CSS. `ScrollView(open_at:)` is the position on opening: the point at ratio f of the content is aligned to the same ratio of the view (0 the start, 0.5 center to center, 1 the end). `ScrollView(open_center:)` is also the position on opening: the point at ratio f of the content stops at the center of the view; near either end, where it cannot reach the center, the view scrolls as far as it can, and the point is still in view. It can bind an expression, which is read only once on opening, and the view does not scroll along when the value changes later; for example, it centers "now" as far as possible when a timeline opens.

The `gap` of `Form` is the distance between groups, `row_height` is the minimum row height, and `padding_x` / `padding_y` are the inner padding of the rows of all groups (a `Section`'s own properties of the same name take precedence); when a form begins with `Section(style: plain)`, it starts at the top of the page, and the rows of that group have no background, margins or separators, serving as a heading that scrolls with the form. The row padding the form gives groups does not apply to plain groups, which use only their own `padding_x` / `padding_y`. The leading plain group can also come from a slot, a component or a condition: during layout the form checks whether the first item is a plain group, and follows when the condition changes, so `if window.compact { Section(style: plain) { … } }` makes the form start at the top only in narrow windows. In `style!`, `Form { background: role }` is the background of the rows, and the form itself lets the page background show through.

Items in a `Row` take their own widths when they fit, and the remaining space is shared equally by `Spacer`s and filling items; when they do not fit, the other items shrink in proportion to their own widths, but not below their own minimum widths. `Spacer` has no minimum length and is the first to shrink to zero when things do not fit, the same on every target. When leading text in a `Row` is immediately followed by a `Spacer` (a `Text` without a width, or a `Column` without `align` or with `align: leading`), the text takes the `Spacer`'s place: the controls after it keep their own widths, and the text wraps within the remaining width, like the label of a system list row; SwiftUI and Compose lay it out this way, and other targets follow the general rule above. When `width` / `height` is given and the content is smaller, `Column` starts from the top and is placed horizontally by `align`, and `Row` starts from the leading edge and is placed vertically by `align`. A `Column` without `align` is the same as `leading`: buttons and other controls, as well as inner `Column` and `Stack`, keep their own widths, while horizontally filling content such as input fields, dividers, `Grid` and `List` spans the whole column; to make a button span the whole column, write `Button(width: fill)`. A `Column` written directly in a window without a width is as wide as the window content area, starts from the top and is placed horizontally by `align`; a `Stack` without a size fills the content area; other body content smaller than the content area is laid out from the top-left corner.

## Common motion and rail properties

The following properties are open only to layout elements for which `ComponentSpec::moves()` is true, and should not be added to nodes that do not accept motion, such as Window, navigation structures and slots. Each backend may have further capability restrictions.

| Property | Type |
| --- | --- |
| `appear` | `Transition` |
| `appear_delay` | `Number` |
| `disappear` | `Transition` |
| `reflow` | `MotionToken` |
| `hero` | `HeroName` |
| `hero_key` | `KeyValue` |
| `reveal` | `hover` |
| `animate` | `MotionToken` |
| `appearance` | `light / dark` |
| `tint` | `ColorRole`: the accent color of this part of the interface changes to that role, and the controls, selection states and links inside it are colored accordingly |
| `scroll_scale` | `Number` |
| `scroll_opacity` | `Number` |
| `scroll_origin_x` | `Number` |
| `scroll_origin_y` | `Number` |
| `nudge` | `Bool` |
| `nudge_key` | `KeyValue` |
| `nudge_scale` | `Number` |
| `reduce_motion` | `Bool` |
| `spin` | `Bool`: when true, rotates at a constant speed around its center; when false, stops at the angle it has at that moment |
| `spin_period` | `Number`: seconds per revolution, default 1, negative for counterclockwise; requires `spin` |
| `stagger` | `Number` |
| `idle_area` | `Number`: the number of seconds the mouse must rest here before the window goes idle; when there are idle areas, the mouse resting elsewhere does not make the window idle |
| `at` | `NumberValue` |
| `from` | `NumberValue` |
| `to` | `NumberValue` |

Current components that do not accept the common properties above: `WindowSet`, `Window`, `Detail`, `SidebarItem`, `Section`, `Slot`, `Pages`, `PageContent`, `Destination`, `Tab`, `Layout`, `MenuItem`.

`scroll_origin_x` and `scroll_origin_y` are the point that stays still when `scroll_scale` enlarges the element, taken as a ratio of the element's width and height from 0 to 1, default 0.5 (the center), like CSS `transform-origin`. `at`, `from` and `to` are used for positioning in a Rail; `stagger` is used only in Column, Row, Grid, Stack, ForEach and List, which support staggering. The existence of a common property does not mean that it is meaningful in every container.

## Popover state and message input


The bubble of `PopoverButton(panel_appearance: window)` follows the window (the app's appearance choice or the system's), while the trigger button keeps the appearance of the region it is in. When omitted, the bubble inherits the region it is in; it also responds to appearance changes while it is open. `panel_appearance: surface` chooses according to the current platform style: with a specified background color it follows the window, and a native material bubble inherits the region it is in, so that a glass bubble above a video keeps its dark look and light text. On macOS without a specified background color, the system bubble material is kept (Liquid Glass on systems that support it). On Windows without a specified background color, it is the system's in-app acrylic, which likewise inherits the appearance of the region it is in and is confined to the window. All targets support `style! { PopoverButton { background: color(light: "#F2F2F2", dark: "#1E1E1E"), foreground: text.primary } }`; the colors apply to the entire bubble surface and its content, not to the trigger button.

`PopoverButton(open: app.panel) { … } => app.set_panel(app.panel)` replaces the identical state expression in the action with the new open/closed value; opening, dismissing by clicking outside and closing with Escape all go through this action. When `open` is omitted, `presented: draft.panel` can still be used, or the component can keep its own state.

On macOS, clicking outside a `TextInput` releases focus and keeps the draft, and does not block the normal action of the clicked control. Clicking inside still positions the cursor and selects text. `submit: clear` clears and releases the keyboard immediately after submitting; an input field that waits for server confirmation should use `clear_key` and increment the number on success, keeping the text on failure. `clear_key` does not clear on first mount; currently only the SwiftUI backend implements its runtime behavior.
