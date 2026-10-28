//! Where builders put widgets.
//!
//! A parent is a container (a box, a list of rows, the layers of a stack,
//! a rail, or a header's tool area) and a region inside it. Conditional
//! and repeated content gets a region of its own: clearing or reordering a
//! region touches only its widgets, and new widgets land at the region's
//! place among its siblings, as the markers of the Web runtime do.
//!
//! A widget with a disappearing transition stays in its container while
//! the transition plays: it has left its region, so later widgets take
//! their places as if it were gone, and it leaves the container when the
//! transition ends.

use crate::layout::{Layers, Rail, ReflowBox};
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// Plays a widget's disappearing transition, then runs the removal.
pub(crate) type Exit = Rc<dyn Fn(Box<dyn FnOnce()>)>;

thread_local! {
    static EXITS: RefCell<Vec<(glib::WeakRef<gtk::Widget>, Exit)>> = const { RefCell::new(Vec::new()) };
}

/// Play `exit` before `widget` leaves its container.
pub(crate) fn on_exit(widget: &gtk::Widget, exit: Exit) {
    EXITS.with(|exits| {
        let mut exits = exits.borrow_mut();
        exits.retain(|(candidate, _)| candidate.upgrade().is_some());
        exits.push((widget.downgrade(), exit));
    });
}

fn take_exit(widget: &gtk::Widget) -> Option<Exit> {
    EXITS.with(|exits| {
        let mut exits = exits.borrow_mut();
        let index = exits
            .iter()
            .position(|(candidate, _)| candidate.upgrade().as_ref() == Some(widget))?;
        Some(exits.remove(index).1)
    })
}

const LEAVING: &str = "bern-leaving";

/// Marks a container of Bern content inside a widget GTK manages: what
/// fills inside it does not make GTK's widgets around it refit or expand.
const BOUNDARY: &str = "bern-boundary";

/// Make `container` hold Bern content inside a widget GTK manages, such as
/// a cell of a column view: refits stop there and its expanding stays in.
pub(crate) fn bound_content(container: &impl IsA<gtk::Widget>) {
    let container = container.as_ref();
    container.add_css_class(BOUNDARY);
    container.set_hexpand(false);
    container.set_vexpand(false);
}

/// Marks a control that keeps its own width, such as a button.
const OWN_WIDTH: &str = "bern-own-width";

/// Marks `widget` as a control that keeps its own width, such as a button:
/// a column that stretches its children leaves it at its leading edge.
pub(crate) fn keeps_width(widget: &impl IsA<gtk::Widget>) {
    widget.as_ref().add_css_class(OWN_WIDTH);
}

/// The classes that carry a box's alignment across its axis, so that
/// content arriving or leaving later can place the children it is in.
const CROSS: [(gtk::Align, &str); 4] = [
    (gtk::Align::Start, "bern-cross-start"),
    (gtk::Align::Center, "bern-cross-center"),
    (gtk::Align::End, "bern-cross-end"),
    (gtk::Align::BaselineCenter, "bern-cross-baseline"),
];

/// Marks `container` as aligning its children `cross` across its axis.
fn mark_cross(container: &impl IsA<gtk::Widget>, cross: gtk::Align) {
    if let Some((_, class)) = CROSS.iter().find(|(align, _)| *align == cross) {
        container.as_ref().add_css_class(class);
    }
}

/// Whether `container` lays its children along a row, and how it aligns
/// them across, if it is a box that aligns them all one way.
fn cross_of(container: &gtk::Widget) -> Option<(bool, gtk::Align)> {
    let (cross, _) = CROSS
        .iter()
        .find(|(_, class)| container.has_css_class(class))?;
    let horizontal = match container.downcast_ref::<gtk::Box>() {
        Some(container) => container.orientation() == gtk::Orientation::Horizontal,
        None => container.downcast_ref::<ReflowBox>()?.horizontal(),
    };
    Some((horizontal, *cross))
}

/// Content came or went after its containers were placed, such as a branch
/// of a condition or a query's result: each container on the way up that
/// stands in a box aligning its children fills the box across while it
/// holds something that fills, and otherwise stands where the box aligns
/// its children, as `Parent::add` would have placed it.
fn refit(from: &gtk::Widget) {
    let mut widget = from.clone();
    for _ in 0..32 {
        // Content placed in a widget GTK manages, such as a table's cell,
        // refits up to that widget only.
        if widget.has_css_class(BOUNDARY) {
            return;
        }
        let Some(container) = widget.parent() else {
            return;
        };
        if let Some((horizontal, cross)) = cross_of(&container)
            && !leaving(&widget)
        {
            let across = if horizontal {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            };
            let now = if horizontal {
                widget.valign()
            } else {
                widget.halign()
            };
            match refitted(widget.compute_expand(across), now, cross) {
                Some(align) if horizontal => widget.set_valign(align),
                Some(align) => widget.set_halign(align),
                None => {}
            }
        }
        widget = container;
    }
}

/// Where a child of a box that aligns its children `cross` stands once
/// content in it came or went, or `None` to leave it at `now`: across the
/// box while it `fills`, else at `cross`.
fn refitted(fills: bool, now: gtk::Align, cross: gtk::Align) -> Option<gtk::Align> {
    match (fills, now) {
        (true, now) if now == cross => Some(gtk::Align::Fill),
        (false, gtk::Align::Fill) => Some(cross),
        _ => None,
    }
}

/// Whether `widget` fills either way, so that it coming or going may change
/// how the containers around it fill.
fn fills_any(widget: &gtk::Widget) -> bool {
    widget.compute_expand(gtk::Orientation::Horizontal)
        || widget.compute_expand(gtk::Orientation::Vertical)
}

/// The alignment a box gives a new child across its axis, or `None` to
/// leave the child's own: the box's `cross`, except that content that fills,
/// such as a video, a drawing without a size or a scroll view, fills across
/// the box whatever its alignment, and that a control that keeps its own
/// width, such as a button, stays at the leading edge of a window that
/// stretches the rest, as on the other targets.
fn cross_alignment(
    cross: Option<gtk::Align>,
    window: bool,
    horizontal: bool,
    keeps_width: bool,
    fills: bool,
) -> Option<gtk::Align> {
    if fills {
        return None;
    }
    let stretching = !horizontal && cross.map_or(window, |cross| cross == gtk::Align::Fill);
    if stretching && keeps_width {
        Some(gtk::Align::Start)
    } else {
        cross
    }
}

fn leaving(widget: &gtk::Widget) -> bool {
    widget.has_css_class(LEAVING)
}

#[derive(Clone)]
enum Sink {
    Box(gtk::Box),
    List(gtk::ListBox),
    /// The cells of a `Grid`, each at least as wide as the cell.
    Flow(gtk::FlowBox, Rc<std::cell::Cell<i32>>),
    /// The layers of a `Stack`, placed by the alignments.
    Layers(Layers, gtk::Align, gtk::Align),
    /// The children of a `Rail`, placed by their fractions.
    Rail(Rail),
    /// A column or row whose children slide to new places.
    Reflow(ReflowBox),
    /// A container of Bern's own that lays out its direct children.
    Children(gtk::Widget),
    /// Widgets an element made, for a wrapper to take: decorations and
    /// placements put them in their container themselves.
    Capture(Rc<RefCell<Vec<gtk::Widget>>>, bool),
}

impl Sink {
    /// The container widget, unless an element keeps what it adds.
    fn container(&self) -> Option<gtk::Widget> {
        Some(match self {
            Sink::Box(container) => container.clone().upcast(),
            Sink::List(list) => list.clone().upcast(),
            Sink::Flow(flow, _) => flow.clone().upcast(),
            Sink::Layers(layers, _, _) => layers.clone().upcast(),
            Sink::Rail(rail) => rail.clone().upcast(),
            Sink::Reflow(reflow) => reflow.clone().upcast(),
            Sink::Children(container) => container.clone(),
            Sink::Capture(..) => return None,
        })
    }

    /// The widgets of the container, without those leaving it.
    fn children(&self) -> Vec<gtk::Widget> {
        let Some(container) = self.container() else {
            return Vec::new();
        };
        let mut children = Vec::new();
        let mut child = container.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            children.push(current);
        }
        children
    }

    fn remove(&self, widget: &gtk::Widget) {
        match self {
            Sink::List(list) => {
                // `insert` wraps widgets that are not rows in a row.
                match widget.downcast_ref::<gtk::ListBoxRow>() {
                    Some(row) => list.remove(row),
                    None => {
                        if let Some(row) = widget.parent() {
                            list.remove(&row);
                        }
                    }
                }
            }
            Sink::Flow(flow, _) => flow.remove(widget),
            Sink::Layers(layers, _, _) => layers.remove(widget),
            Sink::Rail(rail) => rail.remove(widget),
            Sink::Reflow(reflow) => reflow.remove(widget),
            Sink::Children(container) => {
                if widget.parent().as_ref() == Some(container) {
                    widget.unparent();
                    container.queue_resize();
                }
            }
            Sink::Box(container) => container.remove(widget),
            Sink::Capture(widgets, _) => widgets.borrow_mut().retain(|kept| kept != widget),
        }
    }
}

enum Item {
    Widget(gtk::Widget),
    Region(Rc<Region>),
}

struct Region {
    items: RefCell<Vec<Item>>,
    parent: Option<Weak<Region>>,
}

impl Region {
    fn new(parent: Option<&Rc<Region>>) -> Rc<Self> {
        Rc::new(Self {
            items: RefCell::new(Vec::new()),
            parent: parent.map(Rc::downgrade),
        })
    }

    fn widgets(&self, out: &mut Vec<gtk::Widget>) {
        for item in self.items.borrow().iter() {
            match item {
                Item::Widget(widget) => out.push(widget.clone()),
                Item::Region(region) => region.widgets(out),
            }
        }
    }

    fn count(&self) -> usize {
        self.items
            .borrow()
            .iter()
            .map(|item| match item {
                Item::Widget(_) => 1,
                Item::Region(region) => region.count(),
            })
            .sum()
    }

    /// Widgets of the container before this region's start.
    fn offset(self: &Rc<Self>) -> usize {
        let Some(parent) = self.parent.as_ref().and_then(Weak::upgrade) else {
            return 0;
        };
        let mut before = parent.offset();
        for item in parent.items.borrow().iter() {
            match item {
                Item::Region(region) if Rc::ptr_eq(region, self) => break,
                Item::Region(region) => before += region.count(),
                Item::Widget(_) => before += 1,
            }
        }
        before
    }
}

/// A container and a region inside it.
#[derive(Clone)]
pub struct Parent {
    sink: Sink,
    region: Rc<Region>,
    /// The region this one is in: it must outlive this one for new widgets
    /// to find their place, also after the builder that made it returned.
    /// Regions point to the regions they hold, not back, so nothing cycles.
    _above: Option<Rc<Region>>,
    horizontal: bool,
    /// Alignment across the box's axis for every child, from `align`.
    cross: Option<gtk::Align>,
    /// The window's content, directly or through a condition or a
    /// component: a stack there fills the window.
    window: bool,
}

impl Parent {
    pub(crate) fn of_box(container: &gtk::Box) -> Self {
        Self {
            horizontal: container.orientation() == gtk::Orientation::Horizontal,
            sink: Sink::Box(container.clone()),
            region: Region::new(None),
            _above: None,
            cross: None,
            window: false,
        }
    }

    /// The content of a window.
    pub(crate) fn of_window(container: &gtk::Box) -> Self {
        Self {
            window: true,
            ..Self::of_box(container)
        }
    }

    /// A box whose children all align the same way across its axis.
    pub(crate) fn of_aligned_box(container: &gtk::Box, cross: gtk::Align) -> Self {
        mark_cross(container, cross);
        Self {
            cross: Some(cross),
            ..Self::of_box(container)
        }
    }

    pub(crate) fn of_list(list: &gtk::ListBox) -> Self {
        Self {
            sink: Sink::List(list.clone()),
            region: Region::new(None),
            _above: None,
            horizontal: false,
            cross: None,
            window: false,
        }
    }

    /// Cells of a flow box, `min_width` wide or wider; the width can change
    /// later with [`Parent::set_cell_width`].
    pub(crate) fn of_flow(flow: &gtk::FlowBox, min_width: Rc<std::cell::Cell<i32>>) -> Self {
        Self {
            sink: Sink::Flow(flow.clone(), min_width),
            region: Region::new(None),
            _above: None,
            horizontal: false,
            cross: None,
            window: false,
        }
    }

    /// Layers of a stack, aligned in it.
    pub(crate) fn of_layers(layers: &Layers, halign: gtk::Align, valign: gtk::Align) -> Self {
        Self {
            sink: Sink::Layers(layers.clone(), halign, valign),
            region: Region::new(None),
            _above: None,
            horizontal: false,
            cross: None,
            window: false,
        }
    }

    /// A column or row whose children slide, all aligned the same way
    /// across its axis.
    pub(crate) fn of_reflow(reflow: &ReflowBox, cross: gtk::Align) -> Self {
        mark_cross(reflow, cross);
        Self {
            horizontal: reflow.horizontal(),
            sink: Sink::Reflow(reflow.clone()),
            region: Region::new(None),
            _above: None,
            cross: Some(cross),
            window: false,
        }
    }

    /// The direct children of a container of Bern's own.
    pub(crate) fn of_children(container: &impl IsA<gtk::Widget>, horizontal: bool) -> Self {
        Self {
            horizontal,
            sink: Sink::Children(container.as_ref().clone()),
            region: Region::new(None),
            _above: None,
            cross: None,
            window: false,
        }
    }

    pub(crate) fn of_rail(rail: &Rail) -> Self {
        Self {
            horizontal: !rail.vertical(),
            sink: Sink::Rail(rail.clone()),
            region: Region::new(None),
            _above: None,
            cross: None,
            window: false,
        }
    }

    /// A parent that keeps what an element adds, shaped like `like`: rows
    /// where `like` takes rows.
    pub(crate) fn capture(like: &Parent) -> (Self, Rc<RefCell<Vec<gtk::Widget>>>) {
        let widgets = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                sink: Sink::Capture(Rc::clone(&widgets), like.is_list()),
                region: Region::new(None),
                _above: None,
                horizontal: like.horizontal,
                cross: None,
                window: false,
            },
            widgets,
        )
    }

    /// Whether children are rows of a boxed list, as in form sections.
    pub fn is_list(&self) -> bool {
        matches!(self.sink, Sink::List(_) | Sink::Capture(_, true))
    }

    pub fn is_horizontal(&self) -> bool {
        self.horizontal
    }

    /// Whether children are written directly in a window.
    pub(crate) fn is_window(&self) -> bool {
        self.window
    }

    /// Whether children are items of a `Flow`.
    pub(crate) fn is_flow(&self) -> bool {
        matches!(&self.sink, Sink::Children(container) if container.is::<crate::scene::FlowWidget>())
    }

    /// The rail this parent places on, if any.
    pub(crate) fn rail(&self) -> Option<&Rail> {
        match &self.sink {
            Sink::Rail(rail) => Some(rail),
            _ => None,
        }
    }

    /// The narrowest cell of a grid changed.
    pub(crate) fn set_cell_width(&self, width: i32) {
        if let Sink::Flow(flow, min_width) = &self.sink {
            min_width.set(width);
            let mut child = flow.first_child();
            while let Some(cell) = child {
                cell.set_size_request(width, -1);
                child = cell.next_sibling();
            }
        }
    }

    /// The container widget at `index` among those not leaving, and the
    /// one before that place, leaving or not.
    fn previous_at(&self, index: usize) -> Option<gtk::Widget> {
        let mut previous = None;
        let mut position = 0;
        for child in self.sink.children() {
            if !leaving(&child) {
                if position == index {
                    break;
                }
                position += 1;
            }
            previous = Some(child);
        }
        previous
    }

    fn insert_at(&self, widget: &gtk::Widget, index: usize) {
        match &self.sink {
            Sink::List(list) => {
                // The list counts leaving rows too.
                let mut position = 0;
                let mut seen = 0;
                for child in self.sink.children() {
                    if !leaving(&child) {
                        if seen == index {
                            break;
                        }
                        seen += 1;
                    }
                    position += 1;
                }
                list.insert(widget, position);
            }
            Sink::Flow(flow, min_width) => {
                let mut position = 0;
                let mut seen = 0;
                for child in self.sink.children() {
                    let inner = child
                        .downcast_ref::<gtk::FlowBoxChild>()
                        .and_then(|cell| cell.child());
                    if !inner.as_ref().is_some_and(leaving) {
                        if seen == index {
                            break;
                        }
                        seen += 1;
                    }
                    position += 1;
                }
                flow.insert(widget, position);
                // The cell, not its content, is what the flow box sizes
                // and focuses; content keeps the focus.
                if let Some(cell) = widget.parent() {
                    cell.set_size_request(min_width.get(), -1);
                    cell.set_focusable(false);
                }
            }
            Sink::Layers(layers, halign, valign) => {
                // The stack aligns its layers each time it lays them out,
                // as what they hold may come to fill it later.
                layers.set_alignment(*halign, *valign);
                let previous = self.previous_at(index);
                layers.insert_after(widget, previous.as_ref());
            }
            Sink::Rail(rail) => {
                let previous = self.previous_at(index);
                rail.insert_after(widget, previous.as_ref());
            }
            Sink::Reflow(reflow) => {
                let previous = self.previous_at(index);
                reflow.insert_after(widget, previous.as_ref());
            }
            Sink::Children(container) => {
                let previous = self.previous_at(index);
                if widget.parent().as_ref() == Some(container) {
                    widget.unparent();
                }
                widget.insert_after(container, previous.as_ref());
                container.queue_resize();
            }
            Sink::Box(container) => {
                let previous = self.previous_at(index);
                container.insert_child_after(widget, previous.as_ref());
            }
            Sink::Capture(widgets, _) => {
                let mut widgets = widgets.borrow_mut();
                let index = index.min(widgets.len());
                widgets.insert(index, widget.clone());
            }
        }
    }

    /// Take a widget out of the container, after its disappearing
    /// transition when it has one.
    fn remove_widget(&self, widget: &gtk::Widget) {
        match take_exit(widget) {
            Some(exit) => {
                widget.add_css_class(LEAVING);
                widget.set_can_target(false);
                let sink = self.sink.clone();
                let gone = widget.clone();
                let filled = fills_any(widget);
                exit(Box::new(move || {
                    sink.remove(&gone);
                    if filled && let Some(container) = sink.container() {
                        refit(&container);
                    }
                }));
            }
            None => self.sink.remove(widget),
        }
    }

    /// Add a widget at the end of this region.
    pub fn add(&self, widget: &impl IsA<gtk::Widget>) {
        let widget = widget.clone().upcast::<gtk::Widget>();
        let keeps_width = widget.has_css_class(OWN_WIDTH);
        if self.cross.is_some() || (self.window && keeps_width) {
            let across = if self.horizontal {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            };
            let fills = widget.compute_expand(across);
            match cross_alignment(self.cross, self.window, self.horizontal, keeps_width, fills) {
                Some(align) if self.horizontal => widget.set_valign(align),
                Some(align) => widget.set_halign(align),
                None => {}
            }
        }
        let index = self.region.offset() + self.region.count();
        self.region
            .items
            .borrow_mut()
            .push(Item::Widget(widget.clone()));
        self.insert_at(&widget, index);
        // Content that arrives after its containers were placed, such as a
        // branch of a condition, makes them fill in turn.
        if fills_any(&widget)
            && let Some(container) = self.sink.container()
        {
            refit(&container);
        }
    }

    /// A new empty region at the end of this one.
    pub fn region(&self) -> Parent {
        let region = Region::new(Some(&self.region));
        self.region
            .items
            .borrow_mut()
            .push(Item::Region(Rc::clone(&region)));
        Parent {
            sink: self.sink.clone(),
            region,
            _above: Some(Rc::clone(&self.region)),
            horizontal: self.horizontal,
            cross: self.cross,
            window: self.window,
        }
    }

    /// The widgets of this region, in order.
    pub(crate) fn widgets(&self) -> Vec<gtk::Widget> {
        let mut widgets = Vec::new();
        self.region.widgets(&mut widgets);
        widgets
    }

    /// Remove every widget of this region.
    pub fn clear(&self) {
        let mut widgets = Vec::new();
        self.region.widgets(&mut widgets);
        self.region.items.borrow_mut().clear();
        // Content that filled leaves its containers to their own size.
        let filled = widgets.iter().any(fills_any);
        for widget in &widgets {
            self.remove_widget(widget);
        }
        if filled && let Some(container) = self.sink.container() {
            refit(&container);
        }
    }

    /// Put child regions in a new order, keeping their widgets.
    pub(crate) fn reorder(&self, order: &[Parent]) {
        let mut widgets = Vec::new();
        self.region.widgets(&mut widgets);
        for widget in &widgets {
            self.sink.remove(widget);
        }
        {
            let mut items = self.region.items.borrow_mut();
            items.clear();
            for child in order {
                items.push(Item::Region(Rc::clone(&child.region)));
            }
        }
        let start = self.region.offset();
        let mut widgets = Vec::new();
        self.region.widgets(&mut widgets);
        for (position, widget) in widgets.iter().enumerate() {
            self.insert_at(widget, start + position);
        }
    }

    /// Drop a child region and its widgets.
    pub(crate) fn remove_region(&self, child: &Parent) {
        child.clear();
        self.region.items.borrow_mut().retain(
            |item| !matches!(item, Item::Region(region) if Rc::ptr_eq(region, &child.region)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{cross_alignment, refitted};
    use gtk::Align::{Center, End, Fill, Start};

    #[test]
    fn a_leading_column_fills_only_with_what_fills() {
        // A column without `align` is a leading one: a column, a stack or a
        // button inside keeps its own width at the leading edge; a field, a
        // divider or a filling button fills it.
        assert_eq!(
            cross_alignment(Some(Start), false, false, false, false),
            Some(Start)
        );
        assert_eq!(
            cross_alignment(Some(Start), false, false, true, false),
            Some(Start)
        );
        assert_eq!(
            cross_alignment(Some(Start), false, false, false, true),
            None
        );
        assert_eq!(cross_alignment(Some(Start), false, false, true, true), None);
    }

    #[test]
    fn content_that_comes_or_goes_refits_its_containers() {
        // Something that fills arrived: a container standing at the
        // column's alignment fills it.
        assert_eq!(refitted(true, Start, Start), Some(Fill));
        assert_eq!(refitted(true, Center, Center), Some(Fill));
        assert_eq!(refitted(true, Fill, Start), None);
        // It left: the container goes back to the column's alignment.
        assert_eq!(refitted(false, Fill, Start), Some(Start));
        assert_eq!(refitted(false, Fill, End), Some(End));
        assert_eq!(refitted(false, Start, Start), None);
        // A child placed its own way stays there.
        assert_eq!(refitted(true, Center, Start), None);
        assert_eq!(refitted(false, Center, Start), None);
    }

    #[test]
    fn a_button_keeps_its_width_where_the_rest_stretches() {
        // A box that stretches its children, such as a control group: the
        // rest fills its width, a button stands at its leading edge, a
        // filling button fills.
        assert_eq!(
            cross_alignment(Some(Fill), false, false, true, false),
            Some(Start)
        );
        assert_eq!(
            cross_alignment(Some(Fill), false, false, false, false),
            Some(Fill)
        );
        assert_eq!(cross_alignment(Some(Fill), false, false, true, true), None);
        // Written directly in a window, the same.
        assert_eq!(cross_alignment(None, true, false, true, false), Some(Start));
        assert_eq!(cross_alignment(None, true, false, false, false), None);
    }

    #[test]
    fn an_alignment_places_buttons_too() {
        assert_eq!(
            cross_alignment(Some(Center), false, false, true, false),
            Some(Center)
        );
        assert_eq!(
            cross_alignment(Some(Start), false, false, false, false),
            Some(Start)
        );
        // A row aligns its items vertically; a button's width is its own.
        assert_eq!(
            cross_alignment(Some(Center), false, true, true, false),
            Some(Center)
        );
        // Any other box leaves its children as they are.
        assert_eq!(cross_alignment(None, false, false, true, false), None);
    }
}
