//! The trace rules: what `BERN_TRACE`, `bern run --trace` and
//! `trace.start` say to record and how to report it.
//!
//! ```text
//! rules     = item { "," item }
//! category  = engine | ui | frame | span | motion | motion.values | style
//!           | view | all | -category | 1 | on
//! filter    = command:<glob> | query:<glob> | control:<glob>
//!           | field:<glob> | feature:<Contract.Feature>
//! threshold = slow:<duration> | long:<duration>
//! output    = file:<path> | file:none | quiet | buffer:<records>
//! duration  = number [us | ms | s]          (a bare number is milliseconds)
//! ```
//!
//! Filters of one kind add up; filters of different kinds narrow each
//! other. Globs know `*` (any run of characters, dots too) and `?` (one
//! character).

use super::{
    ALL, DEFAULT, ENGINE, FRAME, MOTION, MOTION_VALUES, SPAN, STYLE, UI, VIEW,
};
use std::fmt;

/// Records each thread keeps before the collector reads them.
pub const DEFAULT_BUFFER: usize = 32_768;

/// Where the timeline goes.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum TimelineFile {
    /// The temporary directory (or the one the host configured).
    #[default]
    Default,
    Path(String),
    None,
}

/// Parsed trace rules.
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    /// Categories to record, as the `trace` category bits.
    pub mask: u32,
    pub commands: Vec<String>,
    pub queries: Vec<String>,
    pub controls: Vec<String>,
    pub fields: Vec<String>,
    pub features: Vec<String>,
    /// Interactions and commands faster than this are not printed.
    pub slow_ns: u64,
    /// Frames spaced further apart are long; `None` takes 1.5 refresh
    /// intervals.
    pub long_ns: Option<u64>,
    pub file: TimelineFile,
    pub quiet: bool,
    pub buffer: usize,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            mask: DEFAULT,
            commands: Vec::new(),
            queries: Vec::new(),
            controls: Vec::new(),
            fields: Vec::new(),
            features: Vec::new(),
            slow_ns: 0,
            long_ns: None,
            file: TimelineFile::Default,
            quiet: false,
            buffer: DEFAULT_BUFFER,
        }
    }
}

const CATEGORIES: [(&str, u32); 8] = [
    ("engine", ENGINE),
    ("ui", UI),
    ("frame", FRAME),
    ("span", SPAN),
    ("motion", MOTION),
    ("motion.values", MOTION_VALUES),
    ("style", STYLE),
    ("view", VIEW),
];

fn category(name: &str) -> Option<u32> {
    match name {
        "all" => Some(ALL),
        "1" | "on" | "true" => Some(DEFAULT),
        _ => CATEGORIES
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, bit)| *bit),
    }
}

/// `16ms`, `250us`, `1.5s`, or a bare number of milliseconds.
pub fn duration(text: &str) -> Option<u64> {
    let text = text.trim();
    let (number, scale) = if let Some(number) = text.strip_suffix("us") {
        (number, 1e3)
    } else if let Some(number) = text.strip_suffix("ms") {
        (number, 1e6)
    } else if let Some(number) = text.strip_suffix('s') {
        (number, 1e9)
    } else {
        (text, 1e6)
    };
    let value: f64 = number.trim().parse().ok()?;
    (value.is_finite() && value >= 0.0).then(|| (value * scale).round() as u64)
}

fn duration_text(ns: u64) -> String {
    if ns % 1_000_000_000 == 0 && ns > 0 {
        format!("{}s", ns / 1_000_000_000)
    } else if ns % 1_000_000 == 0 {
        format!("{}ms", ns / 1_000_000)
    } else if ns % 1_000 == 0 {
        format!("{}us", ns / 1_000)
    } else {
        format!("{}ms", ns as f64 / 1e6)
    }
}

impl Rules {
    /// Parse rules; the error names the item that is not understood.
    pub fn parse(text: &str) -> Result<Rules, String> {
        let mut rules = Rules {
            mask: 0,
            ..Rules::default()
        };
        let mut categories = false;
        let mut excluded = 0;
        for item in text.split(',').map(str::trim).filter(|item| !item.is_empty()) {
            if let Some(name) = item.strip_prefix('-') {
                excluded |= category(name).ok_or_else(|| unknown(item))?;
                continue;
            }
            if let Some(bits) = category(item) {
                rules.mask |= bits;
                categories = true;
                continue;
            }
            match item.split_once(':') {
                Some(("command", glob)) => rules.commands.push(pattern(item, glob)?),
                Some(("query", glob)) => rules.queries.push(pattern(item, glob)?),
                Some(("control", glob)) => rules.controls.push(pattern(item, glob)?),
                Some(("field", glob)) => rules.fields.push(pattern(item, glob)?),
                Some(("feature", name)) => rules.features.push(pattern(item, name)?),
                Some(("slow", value)) => {
                    rules.slow_ns = duration(value).ok_or_else(|| bad_duration(item))?;
                }
                Some(("long", value)) => {
                    rules.long_ns = Some(
                        duration(value)
                            .filter(|ns| *ns > 0)
                            .ok_or_else(|| bad_duration(item))?,
                    );
                }
                Some(("file", "none")) => rules.file = TimelineFile::None,
                Some(("file", path)) if !path.is_empty() => {
                    rules.file = TimelineFile::Path(path.to_string());
                }
                Some(("buffer", count)) => {
                    rules.buffer = count
                        .parse()
                        .ok()
                        .filter(|count| (16..=1 << 24).contains(count))
                        .ok_or_else(|| {
                            format!("`{item}`: the buffer holds 16 to 16777216 records")
                        })?;
                }
                None if item == "quiet" => rules.quiet = true,
                _ => return Err(unknown(item)),
            }
        }
        if !categories {
            rules.mask = DEFAULT;
        }
        rules.mask &= !excluded;
        if rules.mask == 0 {
            return Err("the rules leave no category to record".into());
        }
        Ok(rules)
    }

    /// Whether only some commands are recorded.
    pub fn filters_commands(&self) -> bool {
        !(self.commands.is_empty() && self.queries.is_empty() && self.features.is_empty())
    }
}

fn pattern(item: &str, glob: &str) -> Result<String, String> {
    if glob.is_empty() {
        Err(format!("`{item}` needs a name or a pattern"))
    } else {
        Ok(glob.to_string())
    }
}

fn unknown(item: &str) -> String {
    format!(
        "unknown trace rule `{item}`; expected a category (engine, ui, frame, span, motion, motion.values, style, all), command:, query:, control:, field:, feature:, slow:, long:, file:, quiet or buffer:"
    )
}

fn bad_duration(item: &str) -> String {
    format!("`{item}`: expected a duration such as 16ms, 250us or 1s")
}

/// The rules in their normal spelling, which parse to the same rules.
impl fmt::Display for Rules {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut items: Vec<String> = Vec::new();
        if self.mask == ALL {
            items.push("all".into());
        } else if self.mask & ALL == ALL {
            items.push("all".into());
            if self.mask & MOTION_VALUES != 0 {
                items.push("motion.values".into());
            }
        } else {
            items.extend(
                CATEGORIES
                    .iter()
                    .filter(|(_, bit)| self.mask & bit != 0)
                    .map(|(name, _)| name.to_string()),
            );
        }
        let filters = [
            ("command", &self.commands),
            ("query", &self.queries),
            ("control", &self.controls),
            ("field", &self.fields),
            ("feature", &self.features),
        ];
        for (kind, globs) in filters {
            items.extend(globs.iter().map(|glob| format!("{kind}:{glob}")));
        }
        if self.slow_ns > 0 {
            items.push(format!("slow:{}", duration_text(self.slow_ns)));
        }
        if let Some(long) = self.long_ns {
            items.push(format!("long:{}", duration_text(long)));
        }
        match &self.file {
            TimelineFile::Default => {}
            TimelineFile::None => items.push("file:none".into()),
            TimelineFile::Path(path) => items.push(format!("file:{path}")),
        }
        if self.quiet {
            items.push("quiet".into());
        }
        if self.buffer != DEFAULT_BUFFER {
            items.push(format!("buffer:{}", self.buffer));
        }
        f.write_str(&items.join(","))
    }
}

/// Whether `text` matches `glob`: `*` stands for any run of characters,
/// `?` for one.
pub fn glob_matches(glob: &str, text: &str) -> bool {
    let glob: Vec<char> = glob.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut g, mut t) = (0, 0);
    // The last `*` seen, and where in `text` it started matching.
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if g < glob.len() && (glob[g] == '?' || glob[g] == text[t]) {
            g += 1;
            t += 1;
        } else if g < glob.len() && glob[g] == '*' {
            star = Some((g, t));
            g += 1;
        } else if let Some((star_g, star_t)) = star {
            g = star_g + 1;
            t = star_t + 1;
            star = Some((star_g, star_t + 1));
        } else {
            return false;
        }
    }
    glob[g..].iter().all(|c| *c == '*')
}

/// The rules the environment asks for: `BERN_TRACE`, and the variables it
/// replaced — `BERN_FRAME_LOG` (non-empty) records springs, frames and
/// style parsing, `BERN_MOTION_LOG` (set) prints every transform.
pub fn from_env(read: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(rules) = read("BERN_TRACE").filter(|rules| !rules.trim().is_empty()) {
        parts.push(rules);
    }
    if read("BERN_FRAME_LOG").is_some_and(|value| !value.is_empty()) {
        parts.push("motion,frame,style".into());
    }
    if read("BERN_MOTION_LOG").is_some() {
        parts.push("motion.values".into());
    }
    (!parts.is_empty()).then(|| parts.join(","))
}
