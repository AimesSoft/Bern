use super::*;

/// The notes page of the shell's main page slot.
#[component]
pub fn notes_page(notes: notes::Client) -> impl View {
    ui! {
        notes_list(notes: notes)
    }
}

/// One line about the notes, for the shell's overview.
#[component]
pub fn notes_summary(notes: notes::Client) -> impl View {
    ui! {
        Text(content: tr! { "zh-Hans" => "{count} 条笔记", "en" => "{count} notes"; count: notes.count })
    }
}
