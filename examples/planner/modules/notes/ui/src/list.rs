use super::*;

/// Every note, with a field that adds another.
#[component]
pub fn notes_list(notes: notes::Client, #[session] draft: NotesDraft) -> impl View {
    ui! {
        Form {
            Section(title: tr! { "zh-Hans" => "笔记（{count}）", "en" => "Notes ({count})"; count: notes.count }) {
                ForEach(items: notes.items, key: id) |note| {
                    Text(content: note.text, appear: slide, disappear: fade)
                }
                TextInput(id: note_text, label: tr! { "zh-Hans" => "新笔记", "en" => "New note" }, value: draft.text) => notes.add(draft.text)
                Button(id: clear_notes, label: tr! { "zh-Hans" => "全部清除", "en" => "Clear All" }) => notes.clear()
            }
        }
    }
}
