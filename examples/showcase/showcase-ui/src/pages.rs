//! Detail pages of the main window.

use crate::{AlertState, AppWindows, MainDraft, Players, Route};
use bern::prelude::*;
use showcase_protocol::showcase::{self, Action, ItemKind};

/// Text styles, labels and text bound to engine state.
#[component]
pub fn text_page(app: showcase::Client) -> impl View {
    ui! {
        Form {
            Section(title: tr! { "zh-Hans" => "字体层级", "en" => "Text Styles" }) {
                Text(content: tr! { "zh-Hans" => "大标题", "en" => "Large Title" }, style: large_title)
                Text(content: tr! { "zh-Hans" => "标题", "en" => "Title" }, style: title)
                Text(content: tr! { "zh-Hans" => "二级标题", "en" => "Title 2" }, style: title2)
                Text(content: tr! { "zh-Hans" => "三级标题", "en" => "Title 3" }, style: title3)
                Text(content: tr! { "zh-Hans" => "重点文字", "en" => "Headline" }, style: headline)
                Text(content: tr! { "zh-Hans" => "正文用于主要内容，长度不限，会自动换行。", "en" => "Body text carries the main content and wraps as needed." })
                Text(content: tr! { "zh-Hans" => "脚注说明次要信息", "en" => "Footnotes explain secondary details" }, style: footnote, emphasis: secondary)
                Text(content: tr! { "zh-Hans" => "注释", "en" => "Caption" }, style: caption, emphasis: secondary)
            }
            Section(title: tr! { "zh-Hans" => "标签", "en" => "Labels" }) {
                Label(title: tr! { "zh-Hans" => "文稿", "en" => "Documents" }, icon: "doc")
                Label(title: tr! { "zh-Hans" => "照片", "en" => "Photos" }, icon: "photo")
                Label(title: tr! { "zh-Hans" => "下载", "en" => "Downloads" }, icon: "arrow.down.circle")
            }
            Section(title: tr! { "zh-Hans" => "可选文字、徽章与头像", "en" => "Selectable Text, Badges & Avatars" }) {
                Text(
                    content: tr! { "zh-Hans" => "这段文字开启了 selectable，可以直接选中并复制。", "en" => "This paragraph sets selectable: true so you can highlight and copy it." },
                    selectable: true
                )
                Flow(gap: 8) {
                    Badge(content: tr! { "zh-Hans" => "默认", "en" => "Neutral" })
                    Badge(content: tr! { "zh-Hans" => "新版本", "en" => "Release" }, tone: accent, icon: "star")
                    Badge(content: tr! { "zh-Hans" => "已同步", "en" => "Synced" }, tone: success, icon: "checkmark.circle")
                    Badge(content: tr! { "zh-Hans" => "待检查", "en" => "Review" }, tone: warning, icon: "exclamationmark.triangle")
                    Badge(content: tr! { "zh-Hans" => "离线", "en" => "Offline" }, tone: error)
                }
                Row(gap: 12, align: center) {
                    Avatar(source: "images/bern.png", label: tr! { "zh-Hans" => "Bern 徽标", "en" => "Bern Mark" }, size: 40)
                    Avatar(label: tr! { "zh-Hans" => "Ada Lovelace", "en" => "Ada Lovelace" }, size: 40)
                    Avatar(label: tr! { "zh-Hans" => "林微音", "en" => "Lin Weiyin" }, size: 40, shape: rounded)
                    Avatar(label: tr! { "zh-Hans" => "Grace Hopper", "en" => "Grace Hopper" }, size: 32)
                }
            }
            Section(
                title: tr! { "zh-Hans" => "来自引擎的文字", "en" => "Text from the Engine" },
                footer: tr! { "zh-Hans" => "这些文字绑定 Rust 引擎的状态，状态变化后自动更新。", "en" => "These lines are bound to the Rust engine state and update automatically." }
            ) {
                Text(content: tr! { "zh-Hans" => "按钮共按下 {count} 次", "en" => "Buttons pressed {count} times"; count: app.activations })
                if let Some(action) = app.last_action {
                    Text(content: tr! { "zh-Hans" => "最近操作：{action}", "en" => "Last action: {action}"; action: action })
                } else {
                    Text(content: tr! { "zh-Hans" => "还没有操作", "en" => "No actions yet" }, emphasis: secondary)
                }
            }
        }
    }
}

/// Every button style, each bound to a registered command.
#[component]
pub fn buttons_page(
    app: showcase::Client,
    windows: AppWindows,
    #[session] draft: MainDraft,
) -> impl View {
    ui! {
        Form {
            Section(
                title: tr! { "zh-Hans" => "按钮样式", "en" => "Button Styles" },
                footer: tr! { "zh-Hans" => "每个按钮都绑定已登记的引擎命令。", "en" => "Every button is bound to a registered engine command." }
            ) {
                Row(gap: 12) {
                    Button(id: primary, label: tr! { "zh-Hans" => "主要", "en" => "Primary" }, prominence: prominent) => app.record(Action::Primary)
                    Button(id: standard, label: tr! { "zh-Hans" => "普通", "en" => "Standard" }) => app.record(Action::Standard)
                    Button(id: plain, label: tr! { "zh-Hans" => "无边框", "en" => "Borderless" }, prominence: plain) => app.record(Action::Plain)
                    Button(id: delete, label: tr! { "zh-Hans" => "删除", "en" => "Delete" }, role: destructive) => app.record(Action::Delete)
                }
            }
            Section(title: tr! { "zh-Hans" => "图标按钮与提示", "en" => "Icon Buttons & Tooltips" }) {
                Row(gap: 16) {
                    Tooltip(content: tr! { "zh-Hans" => "添加到收藏夹", "en" => "Add to favorites" }) {
                        IconButton(id: favorite, icon: "star", label: tr! { "zh-Hans" => "收藏", "en" => "Favorite" }) => app.record(Action::Icon)
                    }
                    Tooltip(content: tr! { "zh-Hans" => "分享当前内容", "en" => "Share current item" }) {
                        IconButton(id: share, icon: "square.and.arrow.up", label: tr! { "zh-Hans" => "分享", "en" => "Share" }) => app.record(Action::Icon)
                    }
                    IconButton(id: trash, icon: "trash", label: tr! { "zh-Hans" => "移到废纸篓", "en" => "Move to Trash" }) => app.record(Action::Icon)
                }
            }
            Section(title: tr! { "zh-Hans" => "横幅与确认框", "en" => "Banners & Alerts" }) {
                if draft.banner_shown {
                    Banner(
                        id: sample_banner,
                        severity: warning,
                        title: tr! { "zh-Hans" => "本地变更尚未保存", "en" => "Unsaved Local Changes" },
                        content: tr! { "zh-Hans" => "可先记录一次操作，或暂时关闭此提示。", "en" => "Record an action or dismiss this notice for now." }
                    ) {
                        Button(id: banner_save, label: tr! { "zh-Hans" => "记录操作", "en" => "Record" }, prominence: prominent) => app.record(Action::Primary)
                    } => draft.banner_shown.set(false)
                } else {
                    Button(id: restore_banner, label: tr! { "zh-Hans" => "重新显示横幅", "en" => "Show Banner Again" }) => draft.banner_shown.set(true)
                }
                if draft.alert_state == AlertState::Confirmed {
                    Banner(
                        severity: success,
                        content: tr! { "zh-Hans" => "已在确认对话框中完成操作。", "en" => "Action confirmed in the alert dialog." }
                    )
                }
                Row(gap: 12) {
                    Button(id: open_alert, label: tr! { "zh-Hans" => "打开确认对话框…", "en" => "Show Confirmation Alert…" }, role: destructive) => draft.alert_state.set(AlertState::Open)
                }
                if draft.alert_state == AlertState::Open {
                    Alert(
                        title: tr! { "zh-Hans" => "确认删除此项？", "en" => "Delete This Item?" },
                        message: tr! { "zh-Hans" => "确认后会关闭对话框并显示完成横幅。", "en" => "Confirming closes the dialog and displays a completion banner." }
                    ) {
                        AlertAction(id: alert_cancel, label: tr! { "zh-Hans" => "取消", "en" => "Cancel" }, role: cancel) => draft.alert_state.set(AlertState::Idle)
                        AlertAction(id: alert_confirm, label: tr! { "zh-Hans" => "删除", "en" => "Delete" }, role: destructive) => draft.alert_state.set(AlertState::Confirmed)
                    } => draft.alert_state.set(AlertState::Idle)
                }
            }
            Section(title: tr! { "zh-Hans" => "状态与窗口", "en" => "States and Windows" }) {
                Button(id: unavailable, label: tr! { "zh-Hans" => "不可用的按钮", "en" => "Unavailable Button" }, enabled: false) => app.record(Action::Standard)
                Button(id: open_settings, label: tr! { "zh-Hans" => "打开设置…", "en" => "Open Settings…" }, icon: "gearshape") => windows.settings.open()
                if !window.tv && !window.watch {
                    Button(
                        id: export_report,
                        label: tr! { "zh-Hans" => "导出报告…", "en" => "Export Report…" },
                        icon: "square.and.arrow.up",
                        save: draft.save_path,
                        suggested_name: tr! { "zh-Hans" => "report.txt", "en" => "report.txt" }
                    ) => app.record(Action::Standard)
                }
            }
            Section(title: tr! { "zh-Hans" => "最近操作", "en" => "Last Action" }) {
                if let Some(action) = app.last_action {
                    Text(content: action)
                } else {
                    Text(content: tr! { "zh-Hans" => "还没有操作", "en" => "No actions yet" }, emphasis: secondary)
                }
            }
            Toolbar {
                Button(id: toolbar_add, label: tr! { "zh-Hans" => "工具栏按钮", "en" => "Toolbar Button" }, icon: "plus") => app.record(Action::Toolbar)
            }
        }
    }
}

/// Switches, sliders and pickers bound to session drafts.
#[component]
pub fn controls_page(app: showcase::Client, #[session] draft: MainDraft) -> impl View {
    ui! {
        Form {
            Section(title: tr! { "zh-Hans" => "开关", "en" => "Switches" }) {
                Switch(id: notifications, label: tr! { "zh-Hans" => "通知", "en" => "Notifications" }, value: draft.notifications)
                Switch(id: sync, label: tr! { "zh-Hans" => "同步", "en" => "Sync" }, value: draft.sync)
                Switch(id: autosave, label: tr! { "zh-Hans" => "自动存储", "en" => "Save automatically" }, value: draft.autosave, style: checkbox)
            }
            Section(
                title: tr! { "zh-Hans" => "滑块", "en" => "Sliders" },
                footer: tr! { "zh-Hans" => "拖动时只更新本窗口的预览，松开后提交到引擎。已提交的音量：{volume}", "en" => "Dragging only updates this window's preview; releasing commits to the engine. Committed volume: {volume}"; volume: app.volume }
            ) {
                Slider(id: volume, label: tr! { "zh-Hans" => "音量", "en" => "Volume" }, value: draft.volume, min: 0, max: 100, min_icon: "speaker.fill", max_icon: "speaker.wave.3.fill") => app.set_volume(draft.volume)
                Slider(id: brightness, label: tr! { "zh-Hans" => "亮度", "en" => "Brightness" }, value: draft.brightness, min: 0, max: 1, min_icon: "sun.min", max_icon: "sun.max.fill")
            }
            Section(title: tr! { "zh-Hans" => "选择", "en" => "Choices" }) {
                Picker(id: filter, label: tr! { "zh-Hans" => "显示", "en" => "Show" }, value: draft.filter, style: segmented)
                Picker(id: density, label: tr! { "zh-Hans" => "密度", "en" => "Density" }, value: draft.density, style: menu)
                Picker(id: density_chips, label: tr! { "zh-Hans" => "密度", "en" => "Density" }, value: draft.density, style: chips)
                Picker(
                    id: item_combo,
                    label: tr! { "zh-Hans" => "动态检索", "en" => "Dynamic Search" },
                    value: draft.selected_item,
                    items: app.items,
                    key: id,
                    item_label: kind,
                    style: combobox,
                    prompt: tr! { "zh-Hans" => "输入关键词筛选…", "en" => "Filter items…" }
                )
            }
            Section(title: tr! { "zh-Hans" => "折叠面板", "en" => "Disclosure" }) {
                Disclosure(
                    id: advanced_options,
                    label: tr! { "zh-Hans" => "高级选项", "en" => "Advanced Options" },
                    icon: "gearshape",
                    expanded: draft.advanced_open
                ) {
                    Switch(id: adv_sync, label: tr! { "zh-Hans" => "后台同步", "en" => "Background Sync" }, value: draft.sync)
                    Text(content: tr! { "zh-Hans" => "次要设置收纳在折叠区域内，按需展开。", "en" => "Secondary settings stay tucked away until expanded." }, style: footnote, emphasis: secondary)
                }
            }
            Section(
                title: tr! { "zh-Hans" => "工具条", "en" => "Tool Strip" },
                footer: tr! { "zh-Hans" => "分段控件靠左，选项标签靠右；窄的时候标签换到下一行。", "en" => "The segmented control on the left, the chips on the right; when narrow, the chips wrap onto the next line." }
            ) {
                Flow(gap: 10) {
                    Picker(id: strip_density, label: tr! { "zh-Hans" => "密度", "en" => "Density" }, value: draft.density, style: segmented, label_hidden: true)
                    Spacer
                    Picker(id: strip_filter, label: tr! { "zh-Hans" => "显示", "en" => "Show" }, value: draft.filter, style: chips, label_hidden: true)
                }
            }
            Toolbar {
                Picker(id: toolbar_filter, label: tr! { "zh-Hans" => "显示", "en" => "Show" }, value: draft.filter, style: segmented)
            }
        }
    }
}

/// Text fields with prompts, secure entry and multi-line notes.
#[component]
pub fn inputs_page(app: showcase::Client, #[session] draft: MainDraft) -> impl View {
    ui! {
        Form {
            Section(title: tr! { "zh-Hans" => "文本", "en" => "Text" }) {
                TextInput(id: name, label: tr! { "zh-Hans" => "名字", "en" => "Name" }, value: draft.name, prompt: tr! { "zh-Hans" => "你的名字", "en" => "Your name" }) => app.record(Action::Submit)
                TextInput(id: password, label: tr! { "zh-Hans" => "密码", "en" => "Password" }, value: draft.password, kind: secure, prompt: tr! { "zh-Hans" => "必填", "en" => "Required" })
                TextInput(id: notes, label: tr! { "zh-Hans" => "备注", "en" => "Notes" }, value: draft.notes, kind: multiline, prompt: tr! { "zh-Hans" => "写点什么", "en" => "Write something" })
            }
            Section(title: tr! { "zh-Hans" => "数字输入", "en" => "Number Input" }) {
                NumberInput(id: port, label: tr! { "zh-Hans" => "端口号", "en" => "Port" }, value: draft.port, min: 1, max: 65535, step: 1, precision: 0)
                NumberInput(id: ratio, label: tr! { "zh-Hans" => "缩放倍率", "en" => "Scale Factor" }, value: draft.ratio, min: 0.25, max: 4.0, step: 0.25, precision: 2)
                Text(content: tr! { "zh-Hans" => "当前端口：{port}，倍率：{ratio}", "en" => "Port: {port}, scale: {ratio}"; port: draft.port, ratio: draft.ratio }, style: footnote, emphasis: secondary)
            }
            Section(title: tr! { "zh-Hans" => "日期与时间", "en" => "Date & Time" }) {
                DatePicker(id: schedule_date, label: tr! { "zh-Hans" => "计划日期", "en" => "Scheduled Date" }, value: draft.scheduled_date)
                TimePicker(id: meeting_time, label: tr! { "zh-Hans" => "提醒时间", "en" => "Reminder Time" }, value: draft.meeting_time, min: 480, max: 1080, step: 15)
                Text(
                    content: tr! { "zh-Hans" => "已选日期：{date, date, y-MM-dd}，分钟数：{mins}", "en" => "Selected date: {date, date, y-MM-dd}, minutes: {mins}"; date: draft.scheduled_date, mins: draft.meeting_time },
                    style: footnote,
                    emphasis: secondary
                )
            }
            Section(title: tr! { "zh-Hans" => "预览", "en" => "Preview" }) {
                if draft.name == "" {
                    Text(content: tr! { "zh-Hans" => "还没有输入名字", "en" => "No name entered yet" }, emphasis: secondary)
                } else {
                    Text(content: tr! { "zh-Hans" => "你好，{name}", "en" => "Hello, {name}"; name: draft.name })
                }
            }
        }
    }
}

/// Pictures from declared resources, layers, progress and a grid of
/// engine items.
#[component]
pub fn layout_page(app: showcase::Client, #[session] draft: MainDraft) -> impl View {
    ui! {
        Form {
            Section(
                title: tr! { "zh-Hans" => "骨架图", "en" => "Skeleton" },
                footer: tr! { "zh-Hans" => "开启后在 120 ms 防闪烁延迟后切换为由真实布局树在编译期自动投影出的骨架图，关闭后真实内容淡入。", "en" => "When enabled, switches after a 120 ms anti-flicker delay to a skeleton projected at compile time from the real layout tree, and fades real content back in once cleared." }
            ) {
                Switch(
                    id: preview_skeleton,
                    label: tr! { "zh-Hans" => "模拟加载状态", "en" => "Simulate Loading" },
                    value: draft.preview_skeleton
                )
                Grid(min_width: 140, gap: 12, skeleton: draft.preview_skeleton, skeleton_count: 6) {
                    ForEach(items: app.items, key: id) |item| {
                        Column(gap: 6) {
                            Image(source: "images/landscape.jpg", aspect: 1.5, fit: cover, corner: 8)
                            match item.kind {
                                ItemKind::Documents => { Label(title: item.kind, icon: "doc") }
                                ItemKind::Photos => { Label(title: item.kind, icon: "photo") }
                                ItemKind::Music => { Label(title: item.kind, icon: "music.note") }
                                ItemKind::Movies => { Label(title: item.kind, icon: "film") }
                                ItemKind::Downloads => { Label(title: item.kind, icon: "arrow.down.circle") }
                            }
                            showcase_widgets::item_count(count: item.count)
                        }
                    }
                }
                Row(gap: 12, align: center) {
                    Skeleton(width: 36, height: 36, shape: circle, tone: media)
                    Column(gap: 6) {
                        Skeleton(fraction: 0.72, max_width: 240, height: 13, corner: 4, tone: primary)
                        Skeleton(fraction: 0.48, max_width: 200, height: 11, corner: 4, tone: secondary)
                    }
                    Spacer
                    Skeleton(width: 56, height: 22, shape: capsule, tone: secondary)
                }
            }
            Section(title: tr! { "zh-Hans" => "空状态", "en" => "Empty State" }) {
                EmptyState(
                    title: tr! { "zh-Hans" => "还没有收藏项目", "en" => "No Saved Items Yet" },
                    message: tr! { "zh-Hans" => "点击下方按钮记录一条新内容，或稍后再回来查看。", "en" => "Record a new item below or check back later." },
                    icon: "tray"
                ) {
                    Button(id: empty_record, label: tr! { "zh-Hans" => "新建记录", "en" => "New Entry" }, prominence: prominent) => app.record(Action::Primary)
                }
            }
            Section(
                title: tr! { "zh-Hans" => "图片", "en" => "Images" },
                footer: tr! { "zh-Hans" => "图片是随包资源。同一张 16:9 的图片放进 120×90 的框：完整显示留出空白，填满会裁掉两边，拉伸会变形。", "en" => "Pictures are bundled resources. The same 16:9 picture in a 120×90 frame: Contain leaves bars, Cover crops the sides, Fill stretches it." }
            ) {
                Grid(min_width: 120, gap: 16) {
                    Column(gap: 6, align: center) {
                        Image(source: "images/landscape.jpg", label: tr! { "zh-Hans" => "曼德博集合，完整显示", "en" => "Mandelbrot set, contained" }, width: 120, height: 90, fit: contain)
                        Text(content: tr! { "zh-Hans" => "完整显示", "en" => "Contain" }, style: caption, emphasis: secondary)
                    }
                    Column(gap: 6, align: center) {
                        Image(source: "images/landscape.jpg", label: tr! { "zh-Hans" => "曼德博集合，填满", "en" => "Mandelbrot set, covering" }, width: 120, height: 90, fit: cover)
                        Text(content: tr! { "zh-Hans" => "填满", "en" => "Cover" }, style: caption, emphasis: secondary)
                    }
                    Column(gap: 6, align: center) {
                        Image(source: "images/landscape.jpg", label: tr! { "zh-Hans" => "曼德博集合，拉伸", "en" => "Mandelbrot set, stretched" }, width: 120, height: 90, fit: fill)
                        Text(content: tr! { "zh-Hans" => "拉伸", "en" => "Fill" }, style: caption, emphasis: secondary)
                    }
                    Column(gap: 6, align: center) {
                        Image(source: "images/bern.png", label: tr! { "zh-Hans" => "Bern 标志", "en" => "The Bern mark" }, width: 64)
                        Text(content: tr! { "zh-Hans" => "只给宽度", "en" => "Width only" }, style: caption, emphasis: secondary)
                    }
                }
            }
            Section(
                title: tr! { "zh-Hans" => "HDR 图片", "en" => "HDR Pictures" },
                footer: tr! { "zh-Hans" => "左半边是 SDR 白，右半边亮四倍。能显示 HDR 的屏幕上右半边更亮；其他屏幕上显示作者给的 SDR 版本或压缩后的高光。", "en" => "The left half is SDR white, the right half four times as bright. On a display that shows HDR the right half is brighter; elsewhere the author's SDR version or compressed highlights show." }
            ) {
                Grid(min_width: 192, gap: 16) {
                    Column(gap: 6, align: center) {
                        Image(source: "images/hdr-gainmap.jpg", label: tr! { "zh-Hans" => "带增益图的 JPEG", "en" => "JPEG with a gain map" }, width: 192, height: 108)
                        Text(content: tr! { "zh-Hans" => "增益图 JPEG", "en" => "Gain map JPEG" }, style: caption, emphasis: secondary)
                    }
                    Column(gap: 6, align: center) {
                        Image(source: "images/hdr-pq.png", label: tr! { "zh-Hans" => "PQ 编码的 PNG", "en" => "PNG in PQ" }, width: 192, height: 108)
                        Text(content: tr! { "zh-Hans" => "PQ PNG", "en" => "PQ PNG" }, style: caption, emphasis: secondary)
                    }
                }
            }
            Section(
                title: tr! { "zh-Hans" => "层叠", "en" => "Layers" },
                footer: tr! { "zh-Hans" => "后写的一层在上面；标志对齐到右上角。", "en" => "Later layers sit on top; the mark aligns to the top trailing corner." }
            ) {
                Stack(align: top_trailing) {
                    Image(source: "images/landscape.jpg", label: tr! { "zh-Hans" => "曼德博集合", "en" => "Mandelbrot set" }, width: 320, height: 180, fit: cover)
                    Column(padding: 8) {
                        Image(source: "images/bern.png", width: 40)
                    }
                }
            }
            Section(
                title: tr! { "zh-Hans" => "进度", "en" => "Progress" },
                footer: tr! { "zh-Hans" => "进度条绑定引擎状态或会话草稿；没有数值时是不确定的进度。", "en" => "Progress bars follow engine state or session drafts; without a value, progress is indeterminate." }
            ) {
                Progress(value: app.volume, total: 100, label: tr! { "zh-Hans" => "已提交的音量", "en" => "Committed volume" })
                Progress(value: draft.brightness, label: tr! { "zh-Hans" => "亮度", "en" => "Brightness" })
                Slider(id: layout_brightness, label: tr! { "zh-Hans" => "亮度", "en" => "Brightness" }, value: draft.brightness, min: 0, max: 1)
                Progress(label: tr! { "zh-Hans" => "正在等待…", "en" => "Waiting…" })
            }
            Section(
                title: tr! { "zh-Hans" => "网格", "en" => "Grid" },
                footer: tr! { "zh-Hans" => "列宽至少 140，窗口变宽时自动增加列数。项目来自引擎状态。", "en" => "Columns are at least 140 wide; widening the window adds columns. Items come from engine state." }
            ) {
                Grid(min_width: 140, gap: 12) {
                    ForEach(items: app.items, key: id) |item| {
                        Column(gap: 4) {
                            match item.kind {
                                ItemKind::Documents => { Label(title: item.kind, icon: "doc") }
                                ItemKind::Photos => { Label(title: item.kind, icon: "photo") }
                                ItemKind::Music => { Label(title: item.kind, icon: "music.note") }
                                ItemKind::Movies => { Label(title: item.kind, icon: "film") }
                                ItemKind::Downloads => { Label(title: item.kind, icon: "arrow.down.circle") }
                            }
                            showcase_widgets::item_count(count: item.count)
                        }
                    }
                }
            }
        }
    }
}

/// Keyed engine list, filtered by the sidebar search through an
/// asynchronous engine query, with push navigation to a detail page.
#[component]
pub fn library_page(
    app: showcase::Client,
    players: Players,
    #[session] draft: MainDraft,
) -> impl View {
    ui! {
        Navigation(id: library, path: draft.path) {
            Form {
                Section(
                    title: tr! { "zh-Hans" => "资料库", "en" => "Library" },
                    footer: tr! { "zh-Hans" => "列表来自引擎查询：侧边栏搜索按类别名称筛选，中英文均可；每次输入都是新的请求，只显示最新一次的结果。行按项目编号保持身份。", "en" => "Rows come from an engine query: the sidebar search filters them by category name in either language. Every edit is a new request and only the newest answer is shown. Rows keep their identity by item number." }
                ) {
                    Query(
                        id: library_results,
                        query: app.search_library(draft.query),
                        loading: tr! { "zh-Hans" => "正在搜索…", "en" => "Searching…" },
                        empty: tr! { "zh-Hans" => "没有与“{query}”匹配的项目", "en" => "No Items Match “{query}”"; query: draft.query },
                        failed: tr! { "zh-Hans" => "无法搜索", "en" => "Search Failed" },
                        icon: "magnifyingglass"
                    ) |items| {
                        ForEach(items: items, key: id) |item| {
                            NavLink(id: item_link, route: Route::Item { id: item.id, clip: item.clip }) {
                                Row(gap: 8) {
                                    match item.kind {
                                        ItemKind::Documents => { Label(title: item.kind, icon: "doc") }
                                        ItemKind::Photos => { Label(title: item.kind, icon: "photo") }
                                        ItemKind::Music => { Label(title: item.kind, icon: "music.note") }
                                        ItemKind::Movies => { Label(title: item.kind, icon: "film") }
                                        ItemKind::Downloads => { Label(title: item.kind, icon: "arrow.down.circle") }
                                    }
                                    Spacer
                                    showcase_widgets::item_count(count: item.count)
                                }
                            }
                        }
                    }
                }
            }
            Toolbar {
                Button(id: add_item, label: tr! { "zh-Hans" => "添加项目", "en" => "Add Item" }, icon: "plus") => app.add_item(ItemKind::Downloads)
            }
            Destination |route| {
                match route {
                    Route::Item { id, clip } => { item_detail(app: app, players: players, id: id, clip: clip) }
                }
            }
        }
    }
}

/// Detail page for one library item.
#[component]
pub fn item_detail(
    app: showcase::Client,
    players: Players,
    id: i64,
    clip: MediaSource,
    #[session] draft: MainDraft,
) -> impl View {
    ui! {
        Form(title: tr! { "zh-Hans" => "项目 {id}", "en" => "Item {id}"; id: id }) {
            Section(title: tr! { "zh-Hans" => "项目", "en" => "Item" }) {
                Text(content: tr! { "zh-Hans" => "项目编号 {id}", "en" => "Item number {id}"; id: id }, style: title2)
                if draft.query != "" {
                    Text(content: tr! { "zh-Hans" => "搜索：{query}", "en" => "Search: {query}"; query: draft.query }, emphasis: secondary)
                }
            }
            Section(
                title: tr! { "zh-Hans" => "媒体", "en" => "Media" },
                footer: tr! { "zh-Hans" => "在应用级预览播放器中打开；返回列表后仍会继续播放。", "en" => "Opens in the application's preview player, which keeps playing after you go back." }
            ) {
                Video(id: preview, player: players.preview, label: tr! { "zh-Hans" => "预览播放器", "en" => "Preview player" })
                Button(id: open_clip, label: tr! { "zh-Hans" => "在预览播放器中打开", "en" => "Open in Preview Player" }, icon: "play.rectangle") => players.preview.open(clip)
            }
            Section {
                Button(id: remove_item, label: tr! { "zh-Hans" => "删除此项目", "en" => "Delete This Item" }, role: destructive) => app.remove_item(id)
            }
        }
    }
}
