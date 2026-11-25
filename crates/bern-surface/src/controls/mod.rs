//! Lowering library for every Bern self-drawn control into [`crate::SurfaceNode`].
//!
//! Each module defines a platform-neutral `*Spec` and lowers it to a canonical
//! box-model tree consumed by both Web (`crate::web`) and GPUI (`bern-gpui`).

pub mod button;
pub mod content;
pub mod data;
pub mod form;
pub mod input;
pub mod navigation;
pub mod overlay;
pub mod picker;
pub mod search;
pub mod slider;
pub mod switch;

pub use button::{
    ButtonMetrics, ButtonProminence, ButtonRole, ButtonShape, ButtonSize, ButtonSpec,
    IconButtonSpec, button, button_metrics, icon_button,
};
pub use content::{
    AvatarSpec, BadgeSpec, BadgeTone, BannerSeverity, BannerSpec, EmptyStateSpec, LabelSpec,
    ProgressRingSpec, ProgressSpec, SkeletonShape, SkeletonSpec, TextEmphasis, TextSpec,
    TextStyleKind, avatar, badge, banner, empty_state, label, progress, progress_ring, skeleton,
    text, text_scale,
};
pub use data::{
    MediaBarSpec, RefreshableSpec, ReorderRowSpec, ReorderableSpec, SplitPaneSpec, TableColumnSpec,
    TableRowSpec, TableSpec, TreeRowSpec, TreeViewSpec, media_bar, refreshable, reorder_row,
    reorderable, split_pane, table, table_column_header, table_row, tree_row, tree_view,
};
pub use form::{
    ControlGroupSpec, DividerSpec, DropAreaSpec, FormSpec, ListSpec, SectionSpec, control_group,
    divider, drop_area, form, list, section,
};
pub use input::{
    ColorPickerSpec, DatePickerSpec, InputKind, NumberInputSpec, ShortcutInputSpec, TextInputSpec,
    TimePickerSpec, color_picker, date_picker, number_input, shortcut_input, text_input,
    time_picker,
};
pub use navigation::{
    DetailSpec, InspectorSpec, NavLinkSpec, NavigationBarSpec, SidebarItemSpec, SidebarSpec,
    SplitViewSpec, TabAccessorySpec, TabItemSpec, TabsSpec, detail, inspector, nav_link,
    navigation_bar, sidebar, sidebar_item, split_view, tab_accessory, tab_item, tabs,
};
pub use overlay::{
    AlertSpec, BottomSheetSpec, DisclosureSpec, MenuItemSpec, MenuPanelSpec, ModalSpec,
    PopoverPanelSpec, SplitButtonSpec, ToastSpec, TooltipSpec, alert, bottom_sheet, disclosure,
    menu_item, menu_panel, modal, popover_panel, split_button, toast, tooltip,
};
pub use picker::{
    ChipOptionSpec, PickerChipsSpec, PickerMenuSpec, PickerSegmentedSpec, SegmentOptionSpec,
    picker_chips, picker_menu, picker_segmented,
};
pub use search::{
    SearchItemSpec, SearchPaletteSpec, SearchSpec, ToolbarSpec, search, search_item,
    search_palette, toolbar,
};
pub use slider::{
    MediaScrubberSpec, SLIDER_HIT_EXTENT, SLIDER_THUMB_LENGTH, SLIDER_THUMB_RADIUS,
    SLIDER_THUMB_SCALE_ACTIVE, SLIDER_THUMB_THICKNESS, SLIDER_TRACK_RADIUS, SLIDER_TRACK_THICKNESS,
    SliderSpec, media_scrubber, slider, slider_thumb,
};
pub use switch::{
    CheckboxSpec, SWITCH_HEIGHT, SWITCH_KNOB, SWITCH_WIDTH, SwitchSpec, ToggleButtonSpec, checkbox,
    switch, toggle_button,
};

/// Exhaustive catalog of every Bern self-drawn control lowered by `bern-surface`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControlKind {
    // Core interactive inputs
    Button,
    IconButton,
    Switch,
    Checkbox,
    ToggleButton,
    Slider,
    MediaScrubber,
    TextInput,
    NumberInput,
    DatePicker,
    TimePicker,
    ColorPicker,
    ShortcutInput,
    // Selection & menus
    PickerMenu,
    PickerSegmented,
    PickerChips,
    MenuPanel,
    MenuItem,
    ControlGroup,
    PopoverPanel,
    SplitButton,
    // Search & chrome
    Search,
    SearchPalette,
    SearchItem,
    Toolbar,
    // Navigation & containers
    SplitView,
    Sidebar,
    SidebarItem,
    Detail,
    NavigationBar,
    NavLink,
    Tabs,
    TabItem,
    TabAccessory,
    Inspector,
    Form,
    Section,
    Divider,
    List,
    DropArea,
    // Feedback & overlays
    Modal,
    Alert,
    BottomSheet,
    Tooltip,
    Toast,
    Disclosure,
    Banner,
    Badge,
    EmptyState,
    Avatar,
    // Content & data
    Text,
    Label,
    Progress,
    ProgressRing,
    Skeleton,
    Table,
    TreeView,
    SplitPane,
    Reorderable,
    Refreshable,
    MediaBar,
}

impl ControlKind {
    pub const ALL: &'static [ControlKind] = &[
        Self::Button,
        Self::IconButton,
        Self::Switch,
        Self::Checkbox,
        Self::ToggleButton,
        Self::Slider,
        Self::MediaScrubber,
        Self::TextInput,
        Self::NumberInput,
        Self::DatePicker,
        Self::TimePicker,
        Self::ColorPicker,
        Self::ShortcutInput,
        Self::PickerMenu,
        Self::PickerSegmented,
        Self::PickerChips,
        Self::MenuPanel,
        Self::MenuItem,
        Self::ControlGroup,
        Self::PopoverPanel,
        Self::SplitButton,
        Self::Search,
        Self::SearchPalette,
        Self::SearchItem,
        Self::Toolbar,
        Self::SplitView,
        Self::Sidebar,
        Self::SidebarItem,
        Self::Detail,
        Self::NavigationBar,
        Self::NavLink,
        Self::Tabs,
        Self::TabItem,
        Self::TabAccessory,
        Self::Inspector,
        Self::Form,
        Self::Section,
        Self::Divider,
        Self::List,
        Self::DropArea,
        Self::Modal,
        Self::Alert,
        Self::BottomSheet,
        Self::Tooltip,
        Self::Toast,
        Self::Disclosure,
        Self::Banner,
        Self::Badge,
        Self::EmptyState,
        Self::Avatar,
        Self::Text,
        Self::Label,
        Self::Progress,
        Self::ProgressRing,
        Self::Skeleton,
        Self::Table,
        Self::TreeView,
        Self::SplitPane,
        Self::Reorderable,
        Self::Refreshable,
        Self::MediaBar,
    ];
}
