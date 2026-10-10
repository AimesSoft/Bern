//! Protocol of the control showcase: the types, read-only state and
//! commands shared by the engine, the UI and every other entry.

/// Commands and state of the showcase engine.
#[bern::protocol(namespace = "showcase", version = 1)]
pub mod showcase {
    use bern::prelude::*;

    /// A button in the gallery.
    #[value]
    pub enum Action {
        #[label(tr! { "zh-Hans" => "主要按钮", "en" => "Primary button" })]
        Primary,
        #[label(tr! { "zh-Hans" => "普通按钮", "en" => "Standard button" })]
        Standard,
        #[label(tr! { "zh-Hans" => "无边框按钮", "en" => "Borderless button" })]
        Plain,
        #[label(tr! { "zh-Hans" => "删除按钮", "en" => "Delete button" })]
        Delete,
        #[label(tr! { "zh-Hans" => "图标按钮", "en" => "Icon button" })]
        Icon,
        #[label(tr! { "zh-Hans" => "工具栏按钮", "en" => "Toolbar button" })]
        Toolbar,
        #[label(tr! { "zh-Hans" => "提交输入框", "en" => "Submitted text field" })]
        Submit,
    }

    /// Category of a sample library item.
    #[value]
    pub enum ItemKind {
        #[label(tr! { "zh-Hans" => "文稿", "en" => "Documents" })]
        Documents,
        #[label(tr! { "zh-Hans" => "照片", "en" => "Photos" })]
        Photos,
        #[label(tr! { "zh-Hans" => "音乐", "en" => "Music" })]
        Music,
        #[label(tr! { "zh-Hans" => "影片", "en" => "Movies" })]
        Movies,
        #[label(tr! { "zh-Hans" => "下载", "en" => "Downloads" })]
        Downloads,
    }

    /// A sample library entry.
    #[value]
    pub struct Item {
        /// Stable identity, used as the list key.
        pub id: i64,
        pub kind: ItemKind,
        /// Number of files in the item.
        pub count: i64,
        /// Media that plays the item.
        pub clip: MediaSource,
    }

    /// A sample clip of the media page.
    #[value]
    pub enum Clip {
        #[label(tr! { "zh-Hans" => "测试图", "en" => "Test Pattern" })]
        Pattern,
        #[label(tr! { "zh-Hans" => "色条", "en" => "Color Bars" })]
        Bars,
        #[label(tr! { "zh-Hans" => "损坏的文件", "en" => "Damaged File" })]
        Damaged,
    }

    #[state]
    pub struct State {
        /// The most recent button activation.
        pub last_action: Option<Action>,
        /// How many gallery buttons were activated.
        pub activations: i64,
        /// Committed volume between 0 and 100.
        pub volume: f64,
        /// Sample library items.
        pub items: Vec<Item>,
        /// Clip featured on the media page.
        pub featured: Clip,
        /// Source of the featured clip; the media page's video follows it.
        pub featured_source: MediaSource,
        /// Saved display name, edited in the settings window.
        pub display_name: String,
    }

    #[commands]
    pub trait Commands {
        /// Record that a gallery button was activated.
        fn record(&mut self, action: Action) -> Result<(), CommandError>;

        /// Commit the volume slider.
        #[idempotent]
        #[errors("out_of_range")]
        fn set_volume(&mut self, value: f64) -> Result<(), CommandError>;

        /// Add a sample item and return its id.
        fn add_item(&mut self, kind: ItemKind) -> Result<i64, CommandError>;

        /// Remove an item by id.
        #[errors("not_found")]
        fn remove_item(&mut self, id: i64) -> Result<(), CommandError>;

        /// Feature a clip on the media page.
        #[idempotent]
        fn feature(&mut self, clip: Clip) -> Result<(), CommandError>;

        /// Save the display name: 1 to 40 characters, not only spaces.
        #[idempotent]
        #[errors("invalid_name")]
        fn set_display_name(&mut self, name: String) -> Result<(), CommandError>;
    }

    #[queries]
    pub trait Queries {
        /// Library items whose category name contains `text`, in any
        /// language; an empty text lists every item. Broad searches answer
        /// later than narrow ones, as a real index would.
        #[reads(items)]
        #[errors("invalid_search")]
        fn search_library(&self, text: String, answer: Answer<Vec<Item>>);
    }
}
