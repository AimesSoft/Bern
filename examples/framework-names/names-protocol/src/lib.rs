//! Protocol whose value types are named like types of the platforms'
//! frameworks and standard libraries. The comment on each type names the
//! ones it shares a name with; the variants of `Content` are named like
//! types too.

#[bern::protocol(namespace = "names", version = 1)]
pub mod names {
    use bern::prelude::*;

    /// SwiftUI's `Layout` protocol.
    #[value]
    pub enum Layout {
        #[label(tr! { "zh-Hans" => "纵列", "en" => "Column" })]
        Column,
        #[label(tr! { "zh-Hans" => "居中", "en" => "Centered" })]
        Centered,
        #[label(tr! { "zh-Hans" => "层叠", "en" => "Layers" })]
        Layers,
    }

    /// SwiftUI's `Color`, Compose's `Color`, WinUI's `Color`.
    #[value]
    pub enum Color {
        #[label(tr! { "zh-Hans" => "红", "en" => "Red" })]
        Red,
        #[label(tr! { "zh-Hans" => "绿", "en" => "Green" })]
        Green,
        #[label(tr! { "zh-Hans" => "蓝", "en" => "Blue" })]
        Blue,
    }

    /// SwiftUI's `View` protocol; `List` is SwiftUI's and Kotlin's.
    #[value]
    pub enum View {
        #[label(tr! { "zh-Hans" => "网格", "en" => "Grid" })]
        Grid,
        #[label(tr! { "zh-Hans" => "列表", "en" => "List" })]
        List,
    }

    /// SwiftUI's and Compose's `Alignment`.
    #[value]
    pub enum Alignment {
        #[label(tr! { "zh-Hans" => "前", "en" => "Leading" })]
        Leading,
        #[label(tr! { "zh-Hans" => "中", "en" => "Center" })]
        Center,
        #[label(tr! { "zh-Hans" => "后", "en" => "Trailing" })]
        Trailing,
    }

    /// Compose's `Modifier`.
    #[value]
    pub enum Modifier {
        Bold,
        Italic,
    }

    /// Swift's `Error` protocol, JavaScript's `Error`.
    #[value]
    pub enum Error {
        Missing,
        Denied,
    }

    /// SwiftUI's `Font`.
    #[value]
    pub enum Font {
        Body,
        Title,
    }

    /// SwiftUI's `Animation`.
    #[value]
    pub enum Animation {
        Spring,
        Linear,
    }

    /// Swift's `Task`.
    #[value]
    pub enum Task {
        Pending,
        Done,
    }

    /// SwiftUI's `Window`, WinUI's `Window`, GTK's `Window`.
    #[value]
    pub enum Window {
        Main,
        Settings,
    }

    /// SwiftUI's `Button` and `Toggle`, WinUI's and GTK's `Button`.
    #[value]
    pub enum Button {
        Primary,
        Plain,
    }

    #[value]
    pub enum Toggle {
        On,
        Off,
    }

    /// SwiftUI's `Scene`.
    #[value]
    pub enum Scene {
        Day,
        Night,
    }

    /// Foundation's `Locale` and `Timer`, Android's `Bundle`.
    #[value]
    pub enum Locale {
        Here,
        There,
    }

    #[value]
    pub enum Timer {
        Running,
        Stopped,
    }

    #[value]
    pub enum Bundle {
        Small,
        Large,
    }

    /// Compose's `Role` and `Arrangement`.
    #[value]
    pub enum Role {
        Owner,
        Guest,
    }

    #[value]
    pub enum Arrangement {
        Spaced,
        Packed,
    }

    /// WinUI's and GTK's `Orientation`, GTK's `Align` and `Widget`.
    #[value]
    pub enum Orientation {
        Horizontal,
        Vertical,
    }

    #[value]
    pub enum Align {
        Start,
        End,
    }

    #[value]
    pub enum Widget {
        Small,
        Large,
    }

    /// The DOM's `Event`, JavaScript's and Kotlin's `Map`, Kotlin's
    /// `Pair`, Foundation's `Notification`.
    #[value]
    pub enum Event {
        Tap,
        Key,
    }

    #[value]
    pub enum Map {
        Street,
        Satellite,
    }

    #[value]
    pub enum Pair {
        One,
        Two,
    }

    #[value]
    pub enum Notification {
        Banner,
        Badge,
    }

    /// SwiftUI's and Compose's `Text`, the DOM's `Text`.
    #[value]
    pub struct Text {
        pub id: i64,
        #[content]
        pub content: String,
        pub size: f64,
    }

    /// SwiftUI's, Compose's, WinUI's and the DOM's `Image`.
    #[value]
    pub struct Image {
        #[content]
        pub name: String,
        pub width: i64,
    }

    /// SwiftUI's `List`, Kotlin's `List`.
    #[value]
    pub struct List {
        pub texts: Vec<Text>,
    }

    /// Foundation's `Data` and `Date`, SwiftUI's `Path`, Compose's `Size`.
    #[value]
    pub struct Data {
        pub length: i64,
    }

    #[value]
    pub struct Date {
        pub day: i64,
    }

    #[value]
    pub struct Path {
        pub length: f64,
    }

    #[value]
    pub struct Size {
        pub width: f64,
        pub height: f64,
    }

    /// Variants named like types. Kotlin declares variants with fields as
    /// classes inside the enum's class, where they meet the types of the
    /// other variants' fields.
    #[value]
    pub enum Content {
        Text { text: Text },
        Image { image: Image },
        List { items: Vec<String> },
        String {
            #[content]
            value: String,
        },
        Boolean { on: bool },
        Long { value: i64 },
        Double { value: f64 },
        Empty,
    }

    #[state]
    pub struct State {
        pub layout: Layout,
        pub color: Color,
        pub view: View,
        pub alignment: Alignment,
        pub modifier: Modifier,
        pub error: Option<Error>,
        pub font: Font,
        pub animation: Animation,
        pub task: Task,
        pub window: Window,
        pub button: Button,
        pub toggle: Toggle,
        pub scene: Scene,
        pub locale: Locale,
        pub timer: Timer,
        pub bundle: Bundle,
        pub role: Role,
        pub arrangement: Arrangement,
        pub orientation: Orientation,
        pub align: Align,
        pub widget: Widget,
        pub event: Event,
        pub map: Map,
        pub pair: Pair,
        pub notification: Notification,
        pub text: Text,
        pub image: Image,
        pub list: List,
        pub data: Data,
        pub date: Date,
        pub path: Path,
        pub size: Size,
        pub content: Content,
    }

    #[commands]
    pub trait Commands {
        /// Lay the page out another way.
        fn set_layout(&mut self, layout: Layout) -> Result<(), CommandError>;
        /// Choose a color.
        fn paint(&mut self, color: Color) -> Result<(), CommandError>;
        /// Show other content.
        fn show(&mut self, content: Content) -> Result<(), CommandError>;
        /// Add a line to the list.
        fn add_text(&mut self, text: Text) -> Result<(), CommandError>;
        /// Record an error, or clear it.
        fn fail(&mut self, error: Option<Error>) -> Result<(), CommandError>;
    }
}
