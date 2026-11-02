//! The documentation in English and in Simplified Chinese.
//!
//! English is at the usual paths (`README.md`, `CONTRIBUTING.md`, `Docs/`),
//! Simplified Chinese next to it (`README.zh-CN.md`, `CONTRIBUTING.zh-CN.md`,
//! `Docs/zh-CN/`, with the same file names). A change to one language goes
//! into the other in the same commit; these tests find a page, heading,
//! table row or code block that one language has and the other does not,
//! and links or anchors that lead nowhere.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn every_page_is_in_both_languages() {
    let mut missing = Vec::new();
    for (english, chinese) in pages() {
        if !root().join(&chinese).is_file() {
            missing.push(format!("{english} has no {chinese}"));
        }
    }
    for chinese in markdown_under("Docs/zh-CN") {
        let english = format!("Docs/{}", &chinese["Docs/zh-CN/".len()..]);
        if !root().join(&english).is_file() {
            missing.push(format!("{chinese} has no {english}"));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn both_languages_have_the_same_headings_tables_and_code_blocks() {
    let mut differing = Vec::new();
    for (english, chinese) in pages() {
        let (Some(en), Some(zh)) = (shape(&english), shape(&chinese)) else {
            continue;
        };
        if let Some(difference) = en.difference(&zh) {
            differing.push(format!("{english} and {chinese}: {difference}"));
        }
    }
    assert!(differing.is_empty(), "{}", differing.join("\n"));
}

#[test]
fn links_and_anchors_lead_somewhere() {
    let mut files: Vec<String> = pages().into_iter().flat_map(|(en, zh)| [en, zh]).collect();
    files.extend(["README.ja.md".to_owned(), "README.zh-Hant.md".to_owned()]);
    let mut broken = Vec::new();
    for file in &files {
        let Ok(text) = fs::read_to_string(root().join(file)) else {
            continue;
        };
        for target in links(&text) {
            if target.contains("://") || target.starts_with("mailto:") {
                continue;
            }
            let (path, anchor) = target.split_once('#').unwrap_or((&target, ""));
            let page = if path.is_empty() {
                file.clone()
            } else {
                resolve(file, path)
            };
            if !root().join(&page).exists() {
                broken.push(format!("{file}: {target} (no {page})"));
            } else if !anchor.is_empty()
                && page.ends_with(".md")
                && !anchors(&read(&page)).contains(anchor)
            {
                broken.push(format!("{file}: {target} (no heading #{anchor} in {page})"));
            }
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

/// The repository root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(file: &str) -> String {
    fs::read_to_string(root().join(file)).unwrap_or_else(|error| panic!("{file}: {error}"))
}

/// Every page as (English, Simplified Chinese), relative to the root.
fn pages() -> Vec<(String, String)> {
    let mut pages = vec![
        ("README.md".to_owned(), "README.zh-CN.md".to_owned()),
        (
            "CONTRIBUTING.md".to_owned(),
            "CONTRIBUTING.zh-CN.md".to_owned(),
        ),
    ];
    for page in markdown_under("Docs") {
        if !page.starts_with("Docs/zh-CN/") {
            let chinese = format!("Docs/zh-CN/{}", &page["Docs/".len()..]);
            pages.push((page, chinese));
        }
    }
    pages
}

/// The Markdown files under a directory of the root, relative to the root.
fn markdown_under(dir: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut directories = vec![root().join(dir)];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                let relative = path.strip_prefix(root()).unwrap();
                found.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found
}

/// A path relative to `file`, as a path relative to the root.
fn resolve(file: &str, path: &str) -> String {
    let mut parts: Vec<&str> = file.split('/').collect();
    parts.pop();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// What the two languages of a page share: the headings' levels in order,
/// the code blocks and the table rows.
struct Shape {
    headings: Vec<(usize, String)>,
    code_blocks: usize,
    table_rows: usize,
}

impl Shape {
    fn difference(&self, other: &Shape) -> Option<String> {
        let levels = |shape: &Shape| shape.headings.iter().map(|h| h.0).collect::<Vec<_>>();
        if levels(self) != levels(other) {
            let at = self
                .headings
                .iter()
                .zip(&other.headings)
                .position(|(a, b)| a.0 != b.0)
                .unwrap_or(self.headings.len().min(other.headings.len()));
            let name = |shape: &Shape| {
                shape.headings.get(at).map_or("(none)".to_owned(), |h| {
                    format!("{} {}", "#".repeat(h.0), h.1)
                })
            };
            return Some(format!(
                "{} and {} headings, first different: {:?} and {:?}",
                self.headings.len(),
                other.headings.len(),
                name(self),
                name(other)
            ));
        }
        if self.code_blocks != other.code_blocks {
            return Some(format!(
                "{} and {} code blocks",
                self.code_blocks, other.code_blocks
            ));
        }
        if self.table_rows != other.table_rows {
            return Some(format!(
                "{} and {} table rows",
                self.table_rows, other.table_rows
            ));
        }
        None
    }
}

fn shape(file: &str) -> Option<Shape> {
    let text = fs::read_to_string(root().join(file)).ok()?;
    let mut shape = Shape {
        headings: headings(&text),
        code_blocks: 0,
        table_rows: 0,
    };
    let mut in_code = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            if in_code {
                shape.code_blocks += 1;
            }
        } else if !in_code && line.trim_start().starts_with('|') {
            shape.table_rows += 1;
        }
    }
    Some(shape)
}

/// The lines outside code blocks.
fn prose(text: &str) -> Vec<&str> {
    let mut in_code = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
        } else if !in_code {
            lines.push(line);
        }
    }
    lines
}

fn headings(text: &str) -> Vec<(usize, String)> {
    prose(text)
        .into_iter()
        .filter_map(|line| {
            let level = line.len() - line.trim_start_matches('#').len();
            let title = line[level..].strip_prefix(' ')?;
            (1..=6)
                .contains(&level)
                .then(|| (level, title.trim().to_owned()))
        })
        .collect()
}

/// The anchors GitHub gives the headings: lowercase, letters, digits, `-`
/// and `_` kept, spaces turned into `-`, the rest dropped; a repeated
/// anchor gets `-1`, `-2` and so on.
fn anchors(text: &str) -> BTreeSet<String> {
    let mut seen = HashMap::new();
    let mut anchors = BTreeSet::new();
    for (_, title) in headings(text) {
        let slug: String = without_link_targets(&title)
            .to_lowercase()
            .chars()
            .filter_map(|c| match c {
                ' ' => Some('-'),
                '-' | '_' => Some(c),
                c if c.is_alphanumeric() => Some(c),
                _ => None,
            })
            .collect();
        let count = seen.entry(slug.clone()).or_insert(0);
        anchors.insert(if *count == 0 {
            slug
        } else {
            format!("{slug}-{count}")
        });
        *count += 1;
    }
    anchors
}

/// `[text](target)` as `[text]`.
fn without_link_targets(title: &str) -> String {
    let mut out = String::new();
    let mut rest = title;
    while let Some(at) = rest.find("](") {
        out.push_str(&rest[..=at]);
        rest = rest[at..].find(')').map_or("", |end| &rest[at + end + 1..]);
    }
    out.push_str(rest);
    out
}

/// The targets of Markdown links and of `href`, `src` and `srcset`
/// attributes, outside code.
fn links(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in prose(text) {
        let line = without_code_spans(line);
        for (open, close) in [
            ("](", ')'),
            ("href=\"", '"'),
            ("src=\"", '"'),
            ("srcset=\"", '"'),
        ] {
            let mut rest = line.as_str();
            while let Some(at) = rest.find(open) {
                rest = &rest[at + open.len()..];
                let end = rest.find(close).unwrap_or(rest.len());
                found.push(rest[..end].to_owned());
                rest = &rest[end..];
            }
        }
    }
    found
}

fn without_code_spans(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        out.push_str(&rest[..start]);
        let run = rest[start..].len() - rest[start..].trim_start_matches('`').len();
        let ticks = &rest[start..start + run];
        let after = &rest[start + run..];
        match after.find(ticks) {
            Some(end) => rest = &after[end + run..],
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}
