//! UI of the notes module: the list of notes and the field adding one.

use bern::prelude::*;
use notes_protocol::notes;

mod list;
mod page;

pub use list::*;
pub use page::*;

/// Drafts of the notes module's page.
#[bern::session]
pub struct NotesDraft {
    pub text: String,
}
