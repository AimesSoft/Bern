//! Inline Markdown of `Text(markdown:)`: `**strong**`, `*emphasis*` (or
//! `_emphasis_`), `` `code` ``, `~~struck~~` and `[links](address)`, a
//! backslash taking the next mark literally. Everything else is text.

use std::ops::Range;

/// How a run of the text shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Strong,
    Emphasis,
    Code,
    Struck,
    Link,
}

/// The text without its marks, the marked byte ranges in it, and each
/// link's range and address.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Inline {
    pub(crate) text: String,
    pub(crate) marks: Vec<(Range<usize>, Mark)>,
    pub(crate) links: Vec<(Range<usize>, String)>,
}

pub(crate) fn parse(source: &str) -> Inline {
    let mut inline = Inline::default();
    let chars: Vec<char> = source.chars().collect();
    let mut open: Vec<(Mark, usize, &'static str)> = Vec::new();
    let mut index = 0;
    let at = |index: usize, token: &str| {
        token
            .chars()
            .enumerate()
            .all(|(offset, c)| chars.get(index + offset) == Some(&c))
    };
    while index < chars.len() {
        let c = chars[index];
        if c == '\\' && index + 1 < chars.len() {
            inline.text.push(chars[index + 1]);
            index += 2;
            continue;
        }
        // Code takes its content as it is.
        if c == '`'
            && let Some(end) = chars[index + 1..].iter().position(|&next| next == '`')
        {
            let start = inline.text.len();
            inline.text.extend(&chars[index + 1..index + 1 + end]);
            inline.marks.push((start..inline.text.len(), Mark::Code));
            index += end + 2;
            continue;
        }
        if c == '['
            && let Some(close) = chars[index + 1..].iter().position(|&next| next == ']')
            && chars.get(index + close + 2) == Some(&'(')
            && let Some(paren) = chars[index + close + 3..]
                .iter()
                .position(|&next| next == ')')
        {
            let label: String = chars[index + 1..index + 1 + close].iter().collect();
            let address: String = chars[index + close + 3..index + close + 3 + paren]
                .iter()
                .collect();
            let start = inline.text.len();
            inline.text.push_str(&label);
            let range = start..inline.text.len();
            inline.marks.push((range.clone(), Mark::Link));
            inline.links.push((range, address));
            index += close + paren + 4;
            continue;
        }
        let token = if at(index, "**") {
            Some(("**", Mark::Strong))
        } else if at(index, "~~") {
            Some(("~~", Mark::Struck))
        } else if c == '*' {
            Some(("*", Mark::Emphasis))
        } else if c == '_' {
            Some(("_", Mark::Emphasis))
        } else {
            None
        };
        if let Some((token, mark)) = token {
            if let Some(position) = open.iter().rposition(|(_, _, open)| *open == token) {
                let (mark, start, _) = open.remove(position);
                inline.marks.push((start..inline.text.len(), mark));
                index += token.len();
                continue;
            }
            // Opens only when it closes later.
            let rest: String = chars[index + token.len()..].iter().collect();
            if rest.contains(token) {
                open.push((mark, inline.text.len(), token));
                index += token.len();
                continue;
            }
        }
        inline.text.push(c);
        index += 1;
    }
    inline.marks.sort_by_key(|(range, _)| range.start);
    inline
}

#[cfg(test)]
mod tests {
    use super::{Mark, parse};

    #[test]
    fn marks_become_ranges_of_the_plain_text() {
        let inline = parse("a **b** *c* `d*e` ~~f~~ [g](https://x.org) \\*h");
        assert_eq!(inline.text, "a b c d*e f g *h");
        let marked: Vec<(&str, Mark)> = inline
            .marks
            .iter()
            .map(|(range, mark)| (&inline.text[range.clone()], *mark))
            .collect();
        assert_eq!(
            marked,
            [
                ("b", Mark::Strong),
                ("c", Mark::Emphasis),
                ("d*e", Mark::Code),
                ("f", Mark::Struck),
                ("g", Mark::Link)
            ]
        );
        assert_eq!(inline.links[0].1, "https://x.org");
    }

    #[test]
    fn a_mark_without_its_end_is_text() {
        assert_eq!(parse("2 * 3 = 6").text, "2 * 3 = 6");
        assert!(parse("2 * 3 = 6").marks.is_empty());
    }
}
