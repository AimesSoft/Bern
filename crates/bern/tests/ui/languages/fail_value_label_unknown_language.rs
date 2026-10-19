// Checked against `[languages] supported = ["en", "ja"]`.

/// Where a list is sorted.
#[bern::value]
pub enum Order {
    #[label(tr! { "en" => "Newest first", "ja" => "新しい順" })]
    Newest,
    #[label(tr! { "en" => "Oldest first", "jp" => "古い順" })]
    Oldest,
}

fn main() {}
