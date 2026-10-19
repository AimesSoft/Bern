use super::*;

/// The notes page of the shell's main page slot.
#[component]
pub fn notes_page(notes: notes::Client) -> impl View {
    ui! {
        notes_list(notes: notes)
    }
}

/// One line about the notes, for the shell's overview, with a link to
/// the notes page wherever the shell shows it.
#[component]
pub fn notes_summary(notes: notes::Client) -> impl View {
    ui! {
        Row(gap: 8, align: center) {
            Text(content: tr! { "zh-Hans" => "{count} 条笔记", "en" => "{count} notes"; count: notes.count })
            Button(id: all_notes, label: tr! { "zh-Hans" => "全部笔记 ›", "en" => "All notes ›" }, prominence: plain) => pages.notes.show()
        }
    }
}
