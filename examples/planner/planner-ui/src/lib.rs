//! UI of the planner shell. The main window's sidebar lists the shell's
//! own overview and every page the modules add to the `main` page slot;
//! the overview gathers what the modules place in the `summary` slot.

use bern::prelude::*;
use planner_protocol::planner;
use tags_protocol::tags;

// The planner's own look on every platform: sidebar items grow under the
// pointer and fill with the accent color when selected; buttons sink when
// pressed and spring back.
style! {
    motion {
        jelly: spring(period: 0.42, damping: 0.42),
    }

    transition {
        pop_in: pop(from: 0.4, jelly),
    }

    pattern {
        gingham: check(band(accent, 6, 0.22), gap(6), band(accent, 1, 0.4), gap(5), twill: true),
    }

    SidebarItem {
        corner: 8,
        hover: background(fill.subtle) + scale(1.03, snappy),
        pressed: scale(0.96, press),
        selected: fill(accent),
        settle: jelly,
    }

    Button {
        pressed: scale(0.94, press),
        settle: jelly,
    }

    Button.hero {
        background: accent,
        corner: 10,
        size: large,
        hover: scale(1.03, snappy),
        pressed: scale(x: 0.97, y: 0.9, press),
        settle: jelly,
    }
}

/// Pages of the main window. Modules add theirs to the `main` slot.
#[bern::value(pages = "main")]
pub enum MainPage {
    #[label(tr! { "zh-Hans" => "概览", "en" => "Overview" })]
    Overview,
    #[label(tr! { "zh-Hans" => "归档", "en" => "Archive" })]
    Archive,
}

/// Drafts of the main window.
#[bern::session]
pub struct PlannerDraft {
    pub page: MainPage,
    /// Whether the overview shows the title large.
    pub large_title: bool,
    /// Whether the window hides its title bar once the pointer rests.
    pub focus: bool,
    /// Whether the sidebar lists the archive.
    pub show_archive: bool,
}

#[component]
pub fn app(
    app: planner::Client,
    tags: tags::Client,
    windows: AppWindows,
    #[session] draft: PlannerDraft,
) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "zh-Hans" => "计划", "en" => "Planner" }, initial: open, width: 720, height: 480, chrome: unified, idle_after: 3, immersive: draft.focus && window.idle) {
                main_window(app: app, tags: tags)
            }
        }
    }
}

/// Sidebar of pages and the selected page.
#[component]
pub fn main_window(
    app: planner::Client,
    tags: tags::Client,
    #[session] draft: PlannerDraft,
) -> impl View {
    ui! {
        SplitView {
            Sidebar(id: pages, selection: draft.page, fallback: MainPage::Overview) {
                SidebarItem(value: MainPage::Overview, label: tr! { "zh-Hans" => "概览", "en" => "Overview" }, icon: "square.grid.2x2")
                if draft.show_archive {
                    SidebarItem(value: MainPage::Archive, label: tr! { "zh-Hans" => "归档", "en" => "Archive" }, icon: "archivebox")
                }
                Pages(slot: main)
            }
            Detail {
                match draft.page {
                    MainPage::Overview => { overview(app: app, tags: tags) }
                    MainPage::Archive => {
                        Column(gap: 12, padding: 16) {
                            Text(content: tr! { "zh-Hans" => "归档是空的", "en" => "The archive is empty" }, style: title)
                            Switch(id: keep_archive, label: tr! { "zh-Hans" => "在侧栏显示归档", "en" => "Show the archive in the sidebar" }, value: draft.show_archive)
                        }
                    }
                    _ => { PageContent(slot: main, selected: draft.page) }
                }
            }
        }
    }
}

/// The plan's title, the tags module's labels and the modules' summaries.
#[component]
pub fn overview(
    app: planner::Client,
    tags: tags::Client,
    #[session] draft: PlannerDraft,
) -> impl View {
    ui! {
        Column(gap: 12, padding: 16) {
            if draft.large_title {
                Text(content: app.title, style: large_title, hero: title)
            } else {
                Text(content: app.title, style: title, hero: title)
            }
            Text(content: tr! { "zh-Hans" => "最近的进度", "en" => "Recent progress" }, style: headline, mark: accent)
            Canvas(height: 96, label: tr! { "zh-Hans" => "最近每天的进度", "en" => "Progress of recent days" }) {
                Shape(kind: rounded, corner: 12, fill: surface.panel)
                Shape(kind: rounded, x: 6, y: 6, width: 88, height: 84, corner: 8, stroke: control.border, stroke_width: 1.5, dash: 4.5, dash_gap: 3)
                Pattern(name: gingham, opacity: 0.8)
                Series(values: app.activity, kind: area, fill: accent, opacity: 0.25)
                Series(values: app.activity, stroke: accent, stroke_width: 2.5)
                Shape(kind: capsule, x: 12, y: 12, width: 36, height: 8, fill: status.success)
            }
            Button(id: record, label: tr! { "zh-Hans" => "记一笔", "en" => "Record a day" }) => app.record(7)
            Switch(id: large_title, label: tr! { "zh-Hans" => "大标题", "en" => "Large title" }, value: draft.large_title)
            Switch(id: show_archive, label: tr! { "zh-Hans" => "在侧栏显示归档", "en" => "Show the archive in the sidebar" }, value: draft.show_archive)
            Switch(id: focus, label: tr! { "zh-Hans" => "专注模式：静置 3 秒后隐藏标题栏", "en" => "Focus: hide the title bar after 3 s at rest" }, value: draft.focus)
            // The tags module is optional: its labels and button only show
            // while it runs.
            if app.modules.tags {
                ForEach(items: tags.labels, key: name) |tag| {
                    Text(content: tag.name, appear: pop_in, disappear: fade)
                }
                Button(id: tag_work, label: tr! { "zh-Hans" => "加上“工作”标签", "en" => "Tag as Work" }) => tags.tag("work")
            }
            Button(id: rename, label: tr! { "zh-Hans" => "改名为“本周”", "en" => "Rename to This Week" }, variant: hero) => app.rename("本周")
            Slot(id: summary)
        }
    }
}
