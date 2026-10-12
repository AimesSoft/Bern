//! Where builders put widgets.
//!
//! A parent is a container (a box, a list of rows, or a header's tool
//! area) and a region inside it. Conditional and repeated content gets a
//! region of its own: clearing or reordering a region touches only its
//! widgets, and new widgets land at the region's place among its
//! siblings, as the markers of the Web runtime do.

use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

#[derive(Clone)]
enum Sink {
    Box(gtk::Box),
    List(gtk::ListBox),
    /// The cells of a `Grid`, each at least this wide.
    Flow(gtk::FlowBox, i32),
    /// The layers of a `Stack`: every child in the grid's one cell, placed
    /// by the alignments.
    Layers(gtk::Grid, gtk::Align, gtk::Align),
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
            horizontal: false,
            cross: None,
        }
    }

    /// Cells of a flow box, `min_width` wide or wider.
    pub(crate) fn of_flow(flow: &gtk::FlowBox, min_width: i32) -> Self {
        Self {
            sink: Sink::Flow(flow.clone(), min_width),
            region: Region::new(None),
            horizontal: false,
            cross: None,
        }
    }

    /// Layers in the one cell of `grid`, aligned in it.
    pub(crate) fn of_layers(grid: &gtk::Grid, halign: gtk::Align, valign: gtk::Align) -> Self {
        Self {
            sink: Sink::Layers(grid.clone(), halign, valign),
            region: Region::new(None),
            horizontal: false,
            cross: None,
        }
    }

    /// Whether children are rows of a boxed list, as in form sections.
    pub fn is_list(&self) -> bool {
        matches!(self.sink, Sink::List(_))
    }

    pub fn is_horizontal(&self) -> bool {
        self.horizontal
    }

    fn insert_at(&self, widget: &gtk::Widget, index: usize) {
        match &self.sink {
            Sink::List(list) => list.insert(widget, index as i32),
            Sink::Flow(flow, min_width) => {
                flow.insert(widget, index as i32);
                // The cell, not its content, is what the flow box sizes
                // and focuses; content keeps the focus.
                if let Some(cell) = widget.parent() {
                    cell.set_size_request(*min_width, -1);
                    cell.set_focusable(false);
                }
            }
            Sink::Layers(grid, halign, valign) => {
                widget.set_halign(*halign);
                widget.set_valign(*valign);
                grid.attach(widget, 0, 0, 1, 1);
                // Later children draw above earlier ones: move it to its
                // place among the layers.
                let mut previous = None;
                let mut child = grid.first_child();
                let mut position = 0;
                while let Some(current) = child {
                    if position == index {
                        break;
                    }
                    child = current.next_sibling();
                    if &current != widget {
                        position += 1;
                        previous = Some(current);
                    }
                }
                widget.insert_after(grid, previous.as_ref());
            }
            Sink::Box(container) => {
                let mut previous = None;
                let mut child = container.first_child();
                let mut position = 0;
                while let Some(current) = child {
                    if position == index {
                        break;
                    }
                    position += 1;
                    child = current.next_sibling();
                    previous = Some(current);
                }
                container.insert_child_after(widget, previous.as_ref());
            }
        }
    }

    fn remove_widget(&self, widget: &gtk::Widget) {
        match &self.sink {
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
            Sink::Layers(grid, _, _) => grid.remove(widget),
            Sink::Box(container) => container.remove(widget),
        }
    }

    /// Add a widget at the end of this region.
    pub fn add(&self, widget: &impl IsA<gtk::Widget>) {
        let widget = widget.clone().upcast::<gtk::Widget>();
        if let Some(cross) = self.cross {
            if self.horizontal {
                widget.set_valign(cross);
            } else {
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
            horizontal: self.horizontal,
            cross: self.cross,
        }
    }

    /// Remove every widget of this region.
    pub fn clear(&self) {
        let mut widgets = Vec::new();
        self.region.widgets(&mut widgets);
        for widget in &widgets {
            self.remove_widget(widget);
        }
        self.region.items.borrow_mut().clear();
    }

    /// Put child regions in a new order, keeping their widgets.
    pub(crate) fn reorder(&self, order: &[Parent]) {
        let mut widgets = Vec::new();
        self.region.widgets(&mut widgets);
        for widget in &widgets {
            self.remove_widget(widget);
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
