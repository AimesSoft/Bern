//! `Text(markdown)`: inline Markdown — `**strong**`, `*emphasis*` or
//! `_emphasis_`, `` `code` ``, `~~struck~~` and `[links](address)` — as the
//! Pango markup a label shows, links opening as GTK opens them. A backslash
//! takes the next mark literally; a mark left open closes at the end.

pub(crate) fn markup(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::new();
    let mut open: Vec<&'static str> = Vec::new();
    let mut at = 0;
    let starts = |at: usize, mark: &str| {
        mark.chars()
            .enumerate()
            .all(|(i, c)| chars.get(at + i) == Some(&c))
    };
    let find = |from: usize, mark: &str| (from..chars.len()).find(|&i| starts(i, mark));
    while at < chars.len() {
        let c = chars[at];
        let mark = if starts(at, "**") {
            Some(("**", "b"))
        } else if starts(at, "~~") {
            Some(("~~", "s"))
        } else if c == '*' {
            Some(("*", "i"))
        } else if c == '_' {
            Some(("_", "i"))
        } else {
            None
        };
        if c == '\\' && at + 1 < chars.len() {
            out.push_str(&escape(&chars[at + 1].to_string()));
            at += 2;
        } else if let Some((mark, tag)) = mark {
            if open.last() == Some(&mark) {
                open.pop();
                out.push_str(&format!("</{tag}>"));
            } else {
                open.push(mark);
                out.push_str(&format!("<{tag}>"));
            }
            at += mark.chars().count();
        } else if c == '`' && find(at + 1, "`").is_some() {
            let end = find(at + 1, "`").unwrap();
            let code: String = chars[at + 1..end].iter().collect();
            out.push_str(&format!("<tt>{}</tt>", escape(&code)));
            at = end + 1;
        } else if c == '['
            && find(at, "](")
                .and_then(|middle| find(middle + 2, ")"))
                .is_some()
        {
            let middle = find(at, "](").unwrap();
            let end = find(middle + 2, ")").unwrap();
            let words: String = chars[at + 1..middle].iter().collect();
            let address: String = chars[middle + 2..end].iter().collect();
            out.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                escape(&address),
                markup(&words)
            ));
            at = end + 1;
        } else {
            out.push_str(&escape(&c.to_string()));
            at += 1;
        }
    }
    while let Some(mark) = open.pop() {
        out.push_str(match mark {
            "**" => "</b>",
            "~~" => "</s>",
            _ => "</i>",
        });
    }
    out
}

fn escape(text: &str) -> String {
    gtk::glib::markup_escape_text(text).to_string()
}

#[cfg(test)]
mod tests {
    use super::markup;

    #[test]
    fn inline_markdown_becomes_markup() {
        assert_eq!(
            markup("**a** *b* `c<d` ~~e~~"),
            "<b>a</b> <i>b</i> <tt>c&lt;d</tt> <s>e</s>"
        );
        assert_eq!(
            markup("[x](https://a.b?c=1&d=2)"),
            "<a href=\"https://a.b?c=1&amp;d=2\">x</a>"
        );
        assert_eq!(markup(r"2\*3 **open"), "2*3 <b>open</b>");
    }
}
