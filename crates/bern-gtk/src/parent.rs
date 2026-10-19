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
    /// The widgets of the container, without those leaving it.
    fn children(&self) -> Vec<gtk::Widget> {
        let container: gtk::Widget = match self {
            Sink::Box(container) => container.clone().upcast(),
            Sink::List(list) => list.clone().upcast(),
            Sink::Flow(flow, _) => flow.clone().upcast(),
            Sink::Layers(layers, _, _) => layers.clone().upcast(),
            Sink::Rail(rail) => rail.clone().upcast(),
            Sink::Reflow(reflow) => reflow.clone().upcast(),
            Sink::Children(container) => container.clone(),
            Sink::Capture(..) => return Vec::new(),
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
}

impl Parent {
    pub(crate) fn of_box(container: &gtk::Box) -> Self {
        Self {
            horizontal: container.orientation() == gtk::Orientation::Horizontal,
            sink: Sink::Box(container.clone()),
            region: Region::new(None),
            _above: None,
            cross: None,
        }
    }

    /// A box whose children all align the same way across its axis.
    pub(crate) fn of_aligned_box(container: &gtk::Box, cross: gtk::Align) -> Self {
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
        }
    }

    /// A column or row whose children slide, all aligned the same way
    /// across its axis.
    pub(crate) fn of_reflow(reflow: &ReflowBox, cross: gtk::Align) -> Self {
        Self {
            horizontal: reflow.horizontal(),
            sink: Sink::Reflow(reflow.clone()),
            region: Region::new(None),
            _above: None,
            cross: Some(cross),
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
        }
    }

    pub(crate) fn of_rail(rail: &Rail) -> Self {
        Self {
            horizontal: !rail.vertical(),
            sink: Sink::Rail(rail.clone()),
            region: Region::new(None),
            _above: None,
            cross: None,
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
                exit(Box::new(move || sink.remove(&gone)));
            }
            None => self.sink.remove(widget),
        }
    }

    /// Add a widget at the end of this region.
    pub fn add(&self, widget: &impl IsA<gtk::Widget>) {
        let widget = widget.clone().upcast::<gtk::Widget>();
        // Content that fills, such as a video, a drawing without a size or a
        // scroll view, fills across the box whatever its alignment.
        if let Some(cross) = self.cross {
            if self.horizontal {
                if !widget.compute_expand(gtk::Orientation::Vertical) {
                    widget.set_valign(cross);
                }
            } else if !widget.compute_expand(gtk::Orientation::Horizontal) {
                widget.set_halign(cross);
            }
        }
        let index = self.region.offset() + self.region.count();
        self.region
            .items
            .borrow_mut()
            .push(Item::Widget(widget.clone()));
        self.insert_at(&widget, index);
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
        for widget in &widgets {
            self.remove_widget(widget);
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
