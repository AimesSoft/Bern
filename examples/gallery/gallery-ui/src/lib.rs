//! UI of the gallery: a sidebar of pages with the account and an action at
//! its foot, a search field in the toolbar whose results drop below it, and
//! a wall of cards. Each card is a button showing its pattern; the star
//! beside it shows while the pointer is over the card. The about window's
//! tabs show the about page as its symbol alone.

use bern::prelude::*;
use gallery_protocol::gallery::{self, Accent, Appearance, NoticeKind};

style! {
    accents {
        crimson: "#FF2E55",
        ultramarine: "#2A51D4",
        neon: color(light: "#00B86B", dark: "#07FF94"),
    }

    pattern {
        gingham: check(band(accent, 6, 0.22), gap(6), band(accent, 1, 0.4), gap(5), twill: true),
    }

    transition {
        rise: slide(bottom, 16, smooth),
    }

    // Captions read from across a room on a television.
    text {
        caption: font(tv: 14),
    }

    Button.card {
        corner: 12,
        hover: scale(1.03, snappy) + shadow(14, y: 6, opacity: 0.18),
        pressed: scale(0.97, press),
        settle: jelly,
    }
}

#[bern::value]
pub enum Page {
    #[label(tr! { "zh-Hans" => "图案", "en" => "Patterns" })]
    Patterns,
    #[label(tr! { "zh-Hans" => "收藏", "en" => "Favorites" })]
    Favorites,
    #[label(tr! { "zh-Hans" => "画框", "en" => "Frame" })]
    Frame,
    #[label(tr! { "zh-Hans" => "胶片", "en" => "Film" })]
    Film,
    #[label(tr! { "zh-Hans" => "表格", "en" => "Table" })]
    Table,
}

#[bern::session]
pub struct GalleryDraft {
    pub page: Page,
    pub query: String,
    /// Whether the details panel is open.
    pub details: bool,
    /// Smallest width of a card.
    #[initial(180.0)]
    pub card_size: f64,
    /// How large the frame's caption is.
    #[initial(13.0)]
    pub caption_size: f64,
    /// The video chosen or dropped last.
    pub file: MediaSource,
    /// The picture chosen for the poster.
    pub picture: MediaSource,
    /// Whether a video is held above the frame.
    pub dragging: bool,
    /// The pattern at the center of the film.
    #[initial(3)]
    pub centered: i64,
    /// A level set by dragging up and down.
    #[initial(0.5)]
    pub level: f64,
    /// The frame's volume, set in its popover.
    #[initial(0.7)]
    pub volume: f64,
    /// The patterns chosen in the table.
    pub chosen: Vec<i64>,
}

/// The windows, and the menus of the menu bar where the platform has one:
/// its items toggle the main window's details and choose the accent.
#[component]
pub fn app(app: gallery::Client, windows: AppWindows, #[session] draft: GalleryDraft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main, accent: app.accent, appearance: app.appearance) {
            Window(id: main, title: tr! { "zh-Hans" => "图案集", "en" => "Gallery" }, initial: open, width: 900, height: 600, idle_after: 3) {
                main_window(app: app, windows: windows)
            }
            Window(id: about, title: tr! { "zh-Hans" => "关于图案集", "en" => "About Gallery" }, role: dialog, width: 360, height: 360) {
                about(app: app, windows: windows)
            }
            MenuBar {
                CommandMenu(role: file) {
                    CommandItem(id: menu_open, label: tr! { "zh-Hans" => "打开视频…", "en" => "Open Video…" }, pick: draft.file, files: "video") => app.play(draft.file)
                }
                CommandMenu(label: tr! { "zh-Hans" => "图案", "en" => "Patterns" }) {
                    CommandItem(id: menu_details, label: tr! { "zh-Hans" => "详情", "en" => "Details" }, checked: draft.details) => draft.details.toggle()
                    CommandItem(id: menu_close_details, label: tr! { "zh-Hans" => "关闭详情", "en" => "Close the Details" }, enabled: draft.details) => draft.details.set(false)
                    CommandItem(id: menu_clear, label: tr! { "zh-Hans" => "清除收藏", "en" => "Clear Favorites" }) => app.clear_favorites()
                    Divider
                    CommandMenu(label: tr! { "zh-Hans" => "强调色", "en" => "Accent" }) {
                        CommandItem(id: menu_crimson, label: tr! { "zh-Hans" => "绯红", "en" => "Crimson" }, checked: app.accent == Accent::Crimson) => app.set_accent(Accent::Crimson)
                        CommandItem(id: menu_ultramarine, label: tr! { "zh-Hans" => "群青", "en" => "Ultramarine" }, checked: app.accent == Accent::Ultramarine) => app.set_accent(Accent::Ultramarine)
                        CommandItem(id: menu_neon, label: tr! { "zh-Hans" => "荧绿", "en" => "Neon" }, checked: app.accent == Accent::Neon) => app.set_accent(Accent::Neon)
                    }
                }
                // The about window in place of the system's about panel.
                CommandMenu(role: about) {
                    CommandItem(id: menu_about, label: tr! { "zh-Hans" => "关于图案集", "en" => "About Gallery" }) => windows.about.open()
                }
            }
        }
    }
}

/// The pages of the about window.
#[bern::value]
pub enum AboutPage {
    #[label(tr! { "zh-Hans" => "关于", "en" => "About" })]
    About,
    #[label(tr! { "zh-Hans" => "快捷键", "en" => "Shortcuts" })]
    Shortcuts,
}

#[bern::session]
pub struct AboutDraft {
    pub page: AboutPage,
}

/// The about window: its first tab shows only its symbol, beside a tab
/// that shows only its name, with the system's “open at login” where the
/// platform has one.
#[component]
pub fn about(app: gallery::Client, windows: AppWindows, #[session] draft: AboutDraft) -> impl View {
    ui! {
        Tabs(id: about_pages, selection: draft.page) {
            Tab(value: AboutPage::About, label: tr! { "zh-Hans" => "关于", "en" => "About" }, icon: "info.circle", display: icon) {
                Column(gap: 12, padding: 24, align: center) {
                    Text(content: tr! { "zh-Hans" => "图案集", "en" => "Gallery" }, style: title)
                    Text(content: tr! { "zh-Hans" => "一面会浮起的图案墙。", "en" => "A wall of patterns that lift." }, emphasis: secondary)
                    Text(content: tr! { "zh-Hans" => "预览版", "en" => "Preview" }, style: footnote, color: accent)
                    Button(id: done, label: tr! { "zh-Hans" => "好", "en" => "OK" }, prominence: prominent, default_focus: true) => windows.about.close()
                }
            }
            Tab(value: AboutPage::Shortcuts, label: tr! { "zh-Hans" => "快捷键", "en" => "Shortcuts" }) {
                Column(gap: 8, padding: 24) {
                    Text(content: tr! { "zh-Hans" => "⌘O（Ctrl+O）打开视频", "en" => "⌘O (Ctrl+O) opens a video" })
                    Text(content: tr! { "zh-Hans" => "⌘K（Ctrl+K）搜索图案", "en" => "⌘K (Ctrl+K) searches the patterns" })
                    // The details' keys are the user's to choose.
                    Form {
                        Section {
                            ShortcutInput(id: details_key, label: tr! { "zh-Hans" => "显示详情", "en" => "Show the Details" }, current: app.details_key) => app.set_details_key(app.details_key)
                        }
                        Section {
                            Switch(id: open_at_login, label: tr! { "zh-Hans" => "登录时打开", "en" => "Open at Login" }, login_item: true)
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn main_window(
    app: gallery::Client,
    windows: AppWindows,
    #[session] draft: GalleryDraft,
) -> impl View {
    ui! {
        SplitView {
            Sidebar(id: pages, selection: draft.page, fallback: Page::Patterns) {
                SidebarItem(value: Page::Patterns, label: tr! { "zh-Hans" => "图案", "en" => "Patterns" }, icon: "square.grid.2x2")
                SidebarItem(value: Page::Favorites, label: tr! { "zh-Hans" => "收藏", "en" => "Favorites" }, icon: "star")
                SidebarItem(value: Page::Frame, label: tr! { "zh-Hans" => "画框", "en" => "Frame" }, icon: "play.rectangle")
                SidebarItem(value: Page::Film, label: tr! { "zh-Hans" => "胶片", "en" => "Film" }, icon: "film")
                SidebarItem(value: Page::Table, label: tr! { "zh-Hans" => "表格", "en" => "Table" }, icon: "list.bullet.rectangle")
                SidebarFooter {
                    Row(gap: 8, align: center) {
                        Label(title: tr! { "zh-Hans" => "本地用户", "en" => "Local user" }, icon: "person.crop.circle")
                        Spacer
                        IconButton(id: about, icon: "info.circle", label: tr! { "zh-Hans" => "关于", "en" => "About" }) => windows.about.open()
                        IconButton(id: clear_favorites, icon: "star.slash", label: tr! { "zh-Hans" => "清除收藏", "en" => "Clear Favorites" }) => app.clear_favorites()
                    }
                }
            }
            Detail {
                Toolbar {
                    IconButton(id: open_file, icon: "folder", label: tr! { "zh-Hans" => "打开视频", "en" => "Open Video" }, pick: draft.file, files: "video", shortcut: "cmd+o") => app.play(draft.file)
                    Menu(id: look, label: tr! { "zh-Hans" => "外观", "en" => "Look" }, icon: "paintpalette") {
                        MenuItem(id: crimson, label: tr! { "zh-Hans" => "绯红", "en" => "Crimson" }) => app.set_accent(Accent::Crimson)
                        MenuItem(id: ultramarine, label: tr! { "zh-Hans" => "群青", "en" => "Ultramarine" }) => app.set_accent(Accent::Ultramarine)
                        MenuItem(id: neon, label: tr! { "zh-Hans" => "荧绿", "en" => "Neon" }) => app.set_accent(Accent::Neon)
                        Divider
                        MenuItem(id: follow_system, label: tr! { "zh-Hans" => "跟随系统", "en" => "Follow the System" }) => app.set_appearance(Appearance::System)
                        MenuItem(id: light, label: tr! { "zh-Hans" => "浅色", "en" => "Light" }) => app.set_appearance(Appearance::Light)
                        MenuItem(id: dark, label: tr! { "zh-Hans" => "深色", "en" => "Dark" }) => app.set_appearance(Appearance::Dark)
                        Divider
                        MenuItem(id: choose_poster, label: tr! { "zh-Hans" => "换海报…", "en" => "Choose Poster…" }, pick: draft.picture, files: "picture") => app.set_poster(draft.picture)
                    }
                    Divider
                    Switch(id: show_details, label: tr! { "zh-Hans" => "详情", "en" => "Details" }, value: draft.details, style: button, icon: "sidebar.right", shortcut: app.details_key)
                    // The patterns show as soon as the field is in use.
                    Search(id: find, value: draft.query, prompt: tr! { "zh-Hans" => "搜索图案", "en" => "Search Patterns" }, shortcut: "cmd+k", suggest: true) {
                        Query(
                            id: results,
                            query: app.find(draft.query),
                            loading: tr! { "zh-Hans" => "正在搜索…", "en" => "Searching…" },
                            empty: tr! { "zh-Hans" => "没有匹配的图案", "en" => "No Matching Patterns" },
                            failed: tr! { "zh-Hans" => "无法搜索", "en" => "Search Failed" },
                            icon: "magnifyingglass"
                        ) |found| {
                            ForEach(items: found, key: id) |swatch| {
                                Button(id: result, label: swatch.name) {
                                    Label(title: swatch.name, icon: "square.grid.3x3")
                                } => app.open(swatch.id)
                            }
                        }
                    }
                }
                Stack(align: bottom) {
                    match draft.page {
                        Page::Patterns => { wall(app: app, swatches: app.swatches) }
                        Page::Favorites => { wall(app: app, swatches: app.favorites) }
                        Page::Frame => { frame(app: app) }
                        Page::Film => { film(app: app) }
                        Page::Table => { table_page(app: app) }
                    }
                    // Escape (a television's Back) closes the details.
                    if draft.details {
                        Shortcut(key: "escape") => draft.details.set(false)
                    }
                    if let Some(notice) = app.notice {
                        Column(padding: 24) {
                            match notice.kind {
                                NoticeKind::Favorite => {
                                    Toast(content: tr! { "zh-Hans" => "★ 已收藏 {name}", "en" => "★ {name} is a favorite"; name: notice.name }, key: app.notice_id)
                                }
                                NoticeKind::NoLongerFavorite => {
                                    Toast(content: tr! { "zh-Hans" => "☆ 已取消收藏 {name}", "en" => "☆ {name} is no longer a favorite"; name: notice.name }, key: app.notice_id)
                                }
                                NoticeKind::Playing => {
                                    Toast(content: tr! { "zh-Hans" => "▶ {name}", "en" => "▶ {name}"; name: notice.name }, key: app.notice_id)
                                }
                                NoticeKind::PlayingFile => {
                                    Toast(content: tr! { "zh-Hans" => "▶ {name} · {container} · {kilobytes} KB", "en" => "▶ {name} · {container} · {kilobytes} KB"; name: notice.name, container: notice.container, kilobytes: notice.kilobytes }, key: app.notice_id)
                                }
                            }
                        }
                    }
                }
                Inspector(presented: draft.details, width: 280) {
                    Row(gap: 8, align: center) {
                        Text(content: tr! { "zh-Hans" => "详情", "en" => "Details" }, style: headline)
                        Spacer
                        IconButton(id: close_details, icon: "xmark", label: tr! { "zh-Hans" => "关闭", "en" => "Close" }) => draft.details.set(false)
                    }
                    Stack(align: bottom_leading) {
                        Canvas(height: 180, label: tr! { "zh-Hans" => "海报的颜色", "en" => "The poster's color" }) {
                            Shape(kind: rect, fill_from: app.poster, fade: down)
                        }
                        Image(source: app.poster, height: 150, label: tr! { "zh-Hans" => "海报", "en" => "Poster" })
                    }
                    // Keys that type into the search, as an on-screen
                    // keyboard's do.
                    Row(gap: 8, align: center) {
                        Button(id: search_check, label: tr! { "zh-Hans" => "搜索 check", "en" => "Search “check”" }) => draft.query.push_str("check")
                        Button(id: search_delete, label: tr! { "zh-Hans" => "删去一个字", "en" => "Delete a Letter" }) => draft.query.pop()
                    }
                    if app.opened > 0 {
                        Text(content: app.opened_name, style: title)
                        Canvas(height: 160, label: app.opened_name) {
                            Shape(kind: rounded, corner: 12, fill: surface.panel)
                            Pattern(name: gingham)
                        }
                    } else {
                        Text(content: tr! { "zh-Hans" => "打开一个图案查看详情", "en" => "Open a pattern to see it here" }, emphasis: secondary)
                    }
                }
            }
        }
    }
}

/// Cards of `swatches`; the slider sets how wide a card is at least, and
/// the cards slide to their new places.
#[component]
pub fn wall(
    app: gallery::Client,
    swatches: Vec<gallery::Swatch>,
    #[session] draft: GalleryDraft,
) -> impl View {
    ui! {
        Column(gap: 0) {
            Row(gap: 12, padding: 16, align: center) {
                Spacer
                Slider(id: card_size, label: tr! { "zh-Hans" => "卡片大小", "en" => "Card size" }, value: draft.card_size, min: 100, max: 320, min_icon: "square.grid.3x3", max_icon: "square.grid.2x2")
            }
            Pinch(id: pinch_cards, label: tr! { "zh-Hans" => "缩放卡片", "en" => "Scale the cards" }, value: draft.card_size, min: 100, max: 320) {
            ScrollView {
                Grid(min_width: draft.card_size, gap: 20, padding: if window.tv { 64 } else if window.large { 48 } else if window.expanded { 32 } else if window.medium { 24 } else { 16 }, animate: smooth) {
                    ForEach(items: swatches, key: id, stagger: 0.04) |swatch| {
                        card(app: app, swatch: swatch)
                    }
                }
            }
            }
        }
    }
}

/// Patterns on a film: the one nearest the middle is largest, scrolling
/// comes to rest with one in the middle, and the buttons scroll to the ends.
/// Below, a level follows dragging up and down.
#[component]
pub fn film(app: gallery::Client, #[session] draft: GalleryDraft) -> impl View {
    ui! {
        Column(gap: 16) {
            ScrollView(axis: horizontal, snap: center, position: draft.centered, arrival: nudge, indicators: hidden) {
                Row(gap: 30, padding: 24) {
                    ForEach(items: app.swatches, key: id) |swatch| {
                        Column(gap: 8, width: 240, scroll_scale: 1.12, scroll_opacity: 0.6) {
                            Canvas(height: 135, label: swatch.name) {
                                Shape(kind: rounded, corner: 16, fill: surface.panel)
                                Pattern(name: gingham)
                            }
                            Text(content: swatch.name, style: headline)
                        }
                    }
                }
            }
            Column(gap: 6, padding: 24) {
                Stack(align: center) {
                    Rail {
                        Canvas(from: 0, to: 1, height: 2) {
                            Shape(kind: capsule, fill: control.track)
                        }
                    }
                    Rail {
                        ForEach(items: app.swatches, key: id) |swatch| {
                            Canvas(width: 2, height: 10, at: swatch.at) {
                                Shape(kind: capsule, fill: text.secondary)
                            }
                        }
                        Canvas(width: 3, height: 24, at: draft.level, animate: smooth) {
                            Shape(kind: capsule, fill: accent)
                        }
                    }
                }
                Rail(anchor: start) {
                    ForEach(items: app.swatches, key: id) |swatch| {
                        Text(content: swatch.name, style: caption, emphasis: secondary, at: swatch.at)
                    }
                }
            }
            Adaptive(id: film_controls) {
                Layout(size: compact) {
                    Row(gap: 8, padding: 16, align: center) {
                        IconButton(id: to_first, icon: "chevron.left", label: tr! { "zh-Hans" => "第一个", "en" => "First" }) => draft.centered.set(1)
                        IconButton(id: to_last, icon: "chevron.right", label: tr! { "zh-Hans" => "最后一个", "en" => "Last" }) => draft.centered.set(6)
                        Spacer
                    }
                }
                Layout(size: medium) {
                    Row(gap: 12, padding: 16, align: center) {
                        Button(id: to_first, label: tr! { "zh-Hans" => "第一个", "en" => "First" }) => draft.centered.set(1)
                        Button(id: to_last, label: tr! { "zh-Hans" => "最后一个", "en" => "Last" }) => draft.centered.set(6)
                        Spacer
                    }
                }
                Layout(size: expanded) {
                    Row(gap: 12, padding: 16, align: center) {
                        Button(id: to_first, label: tr! { "zh-Hans" => "第一个", "en" => "First" }) => draft.centered.set(1)
                        Button(id: to_last, label: tr! { "zh-Hans" => "最后一个", "en" => "Last" }) => draft.centered.set(6)
                        Spacer
                        Text(content: tr! { "zh-Hans" => "滚动后停在最近的一格", "en" => "Scrolling comes to rest on the nearest one" }, emphasis: secondary)
                    }
                }
                // On a television the remote's focus starts on 「第一个」.
                Layout(size: tv) {
                    Row(gap: 12, padding: 16, align: center) {
                        Button(id: to_first, label: tr! { "zh-Hans" => "第一个", "en" => "First" }, default_focus: true) => draft.centered.set(1)
                        Button(id: to_last, label: tr! { "zh-Hans" => "最后一个", "en" => "Last" }) => draft.centered.set(6)
                        Spacer
                    }
                }
            }
            Swipe(id: level, label: tr! { "zh-Hans" => "上下拖动调整", "en" => "Drag up or down" }, value: draft.level, min: 0, max: 1) {
                Stack(align: center) {
                    Canvas(height: 120) {
                        Shape(kind: rounded, corner: 16, fill: surface.panel)
                    }
                    Rail {
                        Canvas(from: 0, to: draft.level) {
                            Shape(kind: rounded, corner: 16, fill: fill.subtle)
                        }
                    }
                    Column(gap: 8, padding: 24) {
                        Text(content: tr! { "zh-Hans" => "在这里上下拖动", "en" => "Drag up or down here" }, emphasis: secondary)
                        Progress(value: draft.level)
                    }
                }
            }
            Spacer
        }
    }
}

/// The clip in a frame: while the pointer moves the frame steps back from
/// the edges and rounds its corners; three seconds at rest and it fills the
/// page. Always dark, whatever the window's appearance.
#[component]
pub fn frame(app: gallery::Client, #[session] draft: GalleryDraft) -> impl View {
    ui! {
        Drop(id: drop_clip, label: tr! { "zh-Hans" => "拖入视频", "en" => "Drop a Video" }, value: draft.file, files: "video", over: draft.dragging, opens: true) {
        Stack(appearance: dark, nudge: draft.dragging) {
            Canvas(label: tr! { "zh-Hans" => "画框背景", "en" => "Frame background" }) {
                Shape(kind: rect, fill: surface.canvas)
            }
            Column(gap: 12, padding: if window.idle { 0 } else { 40 }, animate: hero, align: center) {
                Video(id: clip, source: app.clip, controls: none, corner: if window.idle { 0 } else { 20 }, animate: hero, label: tr! { "zh-Hans" => "片段", "en" => "Clip" })
                if !window.idle {
                    Row(gap: 12, align: center) {
                        SplitButton(id: caption, label: tr! { "zh-Hans" => "说明", "en" => "Caption" }, icon: "captions.bubble", checked: app.caption) {
                            Text(content: tr! { "zh-Hans" => "说明文字", "en" => "Caption text" }, style: headline)
                            Slider(id: caption_size, label: tr! { "zh-Hans" => "字号", "en" => "Size" }, value: draft.caption_size, min: 11, max: 20)
                        } => app.set_caption(app.caption)
                        Menu(id: caption_sizes, label: tr! { "zh-Hans" => "字号", "en" => "Size" }, icon: "textformat.size") {
                            MenuItem(id: small, label: tr! { "zh-Hans" => "小", "en" => "Small" }) => draft.caption_size.set(12)
                            MenuItem(id: large, label: tr! { "zh-Hans" => "大", "en" => "Large" }) => draft.caption_size.set(18)
                        }
                        PopoverButton(id: volume, label: tr! { "zh-Hans" => "音量", "en" => "Volume" }, icon: "speaker.wave.3.fill") {
                            Text(content: tr! { "zh-Hans" => "音量", "en" => "Volume" }, style: headline)
                            Slider(id: volume_level, label: tr! { "zh-Hans" => "音量", "en" => "Volume" }, value: draft.volume, min: 0, max: 1)
                        }
                        if app.caption {
                            Text(content: tr! { "zh-Hans" => "静置三秒，画面铺满", "en" => "Rest for three seconds to fill the page" }, emphasis: secondary, appear: fade, disappear: fade)
                        }
                    }
                }
            }
            if draft.dragging {
                Canvas(appear: fade, disappear: fade) {
                    Shape(kind: rounded, corner: 20, stroke: accent, stroke_width: 4)
                }
            }
        }
        } => app.play(draft.file)
    }
}

/// One pattern: the card opens it, the star toggles it as a favorite, and
/// the name sits below, outside the card, so only the card lifts.
#[component]
pub fn card(app: gallery::Client, swatch: gallery::Swatch) -> impl View {
    ui! {
        Column(gap: 8, appear: rise) {
            Stack(align: top_trailing) {
                Button(id: open_swatch, label: swatch.name, variant: card) {
                    Canvas(height: 120, label: swatch.name) {
                        Shape(kind: rect, fill: surface.panel)
                        Pattern(name: gingham)
                    }
                } => app.open(swatch.id)
                IconButton(id: favorite, icon: "star", label: tr! { "zh-Hans" => "收藏", "en" => "Favorite" }, reveal: hover) => app.toggle_favorite(swatch.id)
            }
            Text(content: swatch.name, style: headline)
        }
    }
}

/// The patterns in a table: several can be chosen, the row menu acts on
/// them, choosing opens the first, and the name's heading sorts. The field
/// above renames the open pattern; the button opens the pattern whose name
/// is on the clipboard.
#[component]
pub fn table_page(app: gallery::Client, #[session] draft: GalleryDraft) -> impl View {
    ui! {
        Column(gap: 12, padding: 24) {
            Row(gap: 8, align: center) {
                TextInput(id: rename, label: tr! { "zh-Hans" => "名称", "en" => "Name" }, label_hidden: true, current: app.opened_name, prompt: tr! { "zh-Hans" => "打开的图案", "en" => "The open pattern" }) => app.rename(app.opened, app.opened_name)
                Button(id: paste_open, label: tr! { "zh-Hans" => "打开剪贴板里的图案", "en" => "Open the Pattern on the Clipboard" }, paste: draft.query) => app.open_named(draft.query)
            }
            Table(id: swatch_table, label: tr! { "zh-Hans" => "图案", "en" => "Patterns" }, items: app.swatches, key: id, selection: draft.chosen) |swatch| {
                TableColumn(title: tr! { "zh-Hans" => "名称", "en" => "Name" }, width: 200, sorted: app.by_name, descending: app.descending) {
                    Text(content: swatch.name, lines: 1)
                } => app.sort_by_name()
                TableColumn(title: tr! { "zh-Hans" => "收藏", "en" => "Favorite" }, width: 80) {
                    if swatch.favorite {
                        Label(title: tr! { "zh-Hans" => "收藏", "en" => "Favorite" }, icon: "star.fill", display: icon)
                    }
                }
                TableColumn(title: tr! { "zh-Hans" => "位置", "en" => "Place" }) {
                    Text(content: tr! { "zh-Hans" => "{at}", "en" => "{at}"; at: swatch.at }, emphasis: secondary)
                }
                TableMenu {
                    MenuItem(id: table_favorite, label: tr! { "zh-Hans" => "收藏或取消收藏", "en" => "Favorite or Not" }, icon: "star") => app.toggle_favorites(draft.chosen)
                    Divider
                    MenuItem(id: table_copy, label: tr! { "zh-Hans" => "复制名称", "en" => "Copy the Names" }) => app.copy_names(draft.chosen)
                }
            } => app.open_chosen(draft.chosen)
            Watch(value: draft.chosen) => app.open_chosen(draft.chosen)
        }
    }
}
