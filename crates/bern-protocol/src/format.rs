//! The message format of translated texts: `tr!` and enum labels.
//!
//! A text is plain words with placeholders, a subset of ICU MessageFormat:
//!
//! - `{name}`: the argument as text: text as it is, a number as Bern
//!   writes numbers, a labelled enum by its label.
//! - `{name, plural, one {# episode} other {# episodes}}`: the branch
//!   for the number's plural category in the text's language, or an exact
//!   `=N`; `#` in a branch is the number.
//! - `{name, date, EEE, MMM d}`: a `CalendarDate` written with the
//!   pattern: `y` year, `M` month, `MM` two-digit month, `MMM` and `MMMM`
//!   the short and full month name, `d` and `dd` the day, `EEE` and `EEEE`
//!   the short and full weekday; other characters as they are.
//! - `{name, duration, m:ss}`: a number of seconds as `h`, `m` and `s`
//!   fields, each doubled for two digits; the largest field written takes
//!   what the larger ones would hold.
//!
//! `{{` and `}}` are literal braces; in a plural branch `}` always ends the
//! branch, so a branch cannot hold a literal `}`. The names of weekdays and
//! months come from [`date_names`], in every language Bern's runtimes
//! speak.

use std::fmt::Write as _;

/// One part of a parsed text.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Text(String),
    /// `{name}`.
    Value(String),
    /// `#` in a plural branch: the number.
    Number,
    Plural {
        name: String,
        /// Category (`one`, `other`, …) or exact value (`=0`), and text.
        branches: Vec<(String, Vec<Piece>)>,
    },
    Date {
        name: String,
        pattern: String,
    },
    Duration {
        name: String,
        pattern: String,
    },
}

/// How a text uses one of its arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Use {
    /// As text.
    Value,
    /// Its plural category: a number.
    Plural,
    /// As a date: a `CalendarDate`.
    Date,
    /// As seconds: a number.
    Duration,
}

impl Use {
    pub fn name(self) -> &'static str {
        match self {
            Use::Value => "text",
            Use::Plural => "plural",
            Use::Date => "date",
            Use::Duration => "duration",
        }
    }
}

const CATEGORIES: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

/// The plural categories a language's numbers fall into: Chinese, Japanese
/// and Korean use one form for every number; the languages Bern speaks
/// otherwise tell one apart from the rest.
pub fn plural_categories(language: &str) -> &'static [&'static str] {
    let base = language.split(['-', '_']).next().unwrap_or(language);
    match base {
        "zh" | "ja" | "ko" | "vi" | "th" | "id" | "ms" => &["other"],
        _ => &["one", "other"],
    }
}

/// The plural category of `number` in `language`.
pub fn plural_category(language: &str, number: f64) -> &'static str {
    let categories = plural_categories(language);
    if categories.contains(&"one") && number == 1.0 {
        "one"
    } else {
        "other"
    }
}

/// Parse a text into its pieces.
pub fn parse(text: &str) -> Result<Vec<Piece>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    let pieces = message(&chars, &mut at, false, text)?;
    if at < chars.len() {
        return Err(format!("unmatched `}}` in \"{text}\""));
    }
    Ok(pieces)
}

fn push_text(pieces: &mut Vec<Piece>, c: char) {
    if let Some(Piece::Text(text)) = pieces.last_mut() {
        text.push(c);
    } else {
        pieces.push(Piece::Text(c.to_string()));
    }
}

/// Pieces up to the end, or up to the `}` closing a plural branch.
fn message(chars: &[char], at: &mut usize, branch: bool, text: &str) -> Result<Vec<Piece>, String> {
    let mut pieces = Vec::new();
    while *at < chars.len() {
        let c = chars[*at];
        let next = chars.get(*at + 1).copied();
        match c {
            '{' if next == Some('{') => {
                push_text(&mut pieces, '{');
                *at += 2;
            }
            // In a plural branch a `}` always ends the branch: `}}` there is
            // the branch and then the placeholder closing.
            '}' if branch => return Ok(pieces),
            '}' if next == Some('}') => {
                push_text(&mut pieces, '}');
                *at += 2;
            }
            '}' => return Err(format!("unmatched `}}` in \"{text}\"")),
            '#' if branch => {
                pieces.push(Piece::Number);
                *at += 1;
            }
            '{' => {
                *at += 1;
                pieces.push(placeholder(chars, at, text)?);
            }
            other => {
                push_text(&mut pieces, other);
                *at += 1;
            }
        }
    }
    if branch {
        return Err(format!("unclosed plural branch in \"{text}\""));
    }
    Ok(pieces)
}

fn skip_space(chars: &[char], at: &mut usize) {
    while *at < chars.len() && chars[*at].is_whitespace() {
        *at += 1;
    }
}

/// Characters up to one of `stops`, trimmed.
fn word(chars: &[char], at: &mut usize, stops: &[char]) -> String {
    let start = *at;
    while *at < chars.len() && !stops.contains(&chars[*at]) {
        *at += 1;
    }
    chars[start..*at]
        .iter()
        .collect::<String>()
        .trim()
        .to_string()
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit())
}

/// A placeholder after its `{`, through its `}`.
fn placeholder(chars: &[char], at: &mut usize, text: &str) -> Result<Piece, String> {
    let name = word(chars, at, &[',', '}', '{']);
    if !valid_name(&name) {
        return Err(format!("invalid placeholder `{{{name}}}` in \"{text}\""));
    }
    match chars.get(*at) {
        Some('}') => {
            *at += 1;
            Ok(Piece::Value(name))
        }
        Some(',') => {
            *at += 1;
            let kind = word(chars, at, &[',', '}', '{']);
            if chars.get(*at) != Some(&',') {
                return Err(format!(
                    "`{{{name}, {kind}}}` needs its form after a comma in \"{text}\""
                ));
            }
            *at += 1;
            match kind.as_str() {
                "plural" => {
                    let mut branches = Vec::new();
                    loop {
                        skip_space(chars, at);
                        match chars.get(*at) {
                            Some('}') => {
                                *at += 1;
                                break;
                            }
                            None => {
                                return Err(format!(
                                    "unclosed `{{{name}, plural, …` in \"{text}\""
                                ));
                            }
                            _ => {}
                        }
                        let selector = word(chars, at, &['{', '}']);
                        let exact = selector
                            .strip_prefix('=')
                            .is_some_and(|value| value.parse::<i64>().is_ok());
                        if !exact && !CATEGORIES.contains(&selector.as_str()) {
                            return Err(format!(
                                "`{selector}` is not a plural category (zero, one, two, few, many, other, or =N) in \"{text}\""
                            ));
                        }
                        if chars.get(*at) != Some(&'{') {
                            return Err(format!(
                                "the `{selector}` branch needs its text in braces in \"{text}\""
                            ));
                        }
                        *at += 1;
                        let body = message(chars, at, true, text)?;
                        *at += 1;
                        if branches.iter().any(|(existing, _)| *existing == selector) {
                            return Err(format!(
                                "the `{selector}` branch is written twice in \"{text}\""
                            ));
                        }
                        branches.push((selector, body));
                    }
                    if !branches.iter().any(|(selector, _)| selector == "other") {
                        return Err(format!(
                            "`{{{name}, plural, …}}` needs an `other` branch in \"{text}\""
                        ));
                    }
                    Ok(Piece::Plural { name, branches })
                }
                "date" | "duration" => {
                    let pattern = word(chars, at, &['}', '{']);
                    if chars.get(*at) != Some(&'}') {
                        return Err(format!("unclosed `{{{name}, {kind}, …` in \"{text}\""));
                    }
                    *at += 1;
                    if pattern.is_empty() {
                        return Err(format!(
                            "`{{{name}, {kind}, …}}` needs a pattern in \"{text}\""
                        ));
                    }
                    if kind == "date" {
                        check_date(&pattern)
                            .map_err(|problem| format!("{problem} in \"{text}\""))?;
                        Ok(Piece::Date { name, pattern })
                    } else {
                        check_duration(&pattern)
                            .map_err(|problem| format!("{problem} in \"{text}\""))?;
                        Ok(Piece::Duration { name, pattern })
                    }
                }
                other => Err(format!(
                    "`{other}` is not a form of placeholder (plural, date or duration) in \"{text}\""
                )),
            }
        }
        _ => Err(format!("unclosed `{{` in \"{text}\"")),
    }
}

/// Runs of one pattern letter, and the characters between them.
fn runs(pattern: &str) -> Vec<(char, usize)> {
    let mut out: Vec<(char, usize)> = Vec::new();
    for c in pattern.chars() {
        match out.last_mut() {
            Some((last, count)) if *last == c && c.is_ascii_alphabetic() => *count += 1,
            _ => out.push((c, 1)),
        }
    }
    out
}

fn check_date(pattern: &str) -> Result<(), String> {
    for (c, count) in runs(pattern) {
        let fine = match c {
            'y' => count == 1 || count == 2 || count == 4,
            'M' => count <= 4,
            'd' => count <= 2,
            'E' => count == 3 || count == 4,
            c if c.is_ascii_alphabetic() => false,
            _ => true,
        };
        if !fine {
            return Err(format!(
                "`{}` is not a date field (y, M, MM, MMM, MMMM, d, dd, EEE, EEEE)",
                c.to_string().repeat(count)
            ));
        }
    }
    Ok(())
}

fn check_duration(pattern: &str) -> Result<(), String> {
    for (c, count) in runs(pattern) {
        let fine = match c {
            'h' | 'm' | 's' => count <= 2,
            c if c.is_ascii_alphabetic() => false,
            _ => true,
        };
        if !fine {
            return Err(format!(
                "`{}` is not a duration field (h, hh, m, mm, s, ss)",
                c.to_string().repeat(count)
            ));
        }
    }
    Ok(())
}

/// The arguments a text uses, in order of first use, with how each is
/// used. An argument used two ways is listed with each.
pub fn uses(pieces: &[Piece]) -> Vec<(String, Use)> {
    fn visit(pieces: &[Piece], out: &mut Vec<(String, Use)>) {
        for piece in pieces {
            let (name, how) = match piece {
                Piece::Value(name) => (name, Use::Value),
                Piece::Plural { name, branches } => {
                    let entry = (name.clone(), Use::Plural);
                    if !out.contains(&entry) {
                        out.push(entry);
                    }
                    for (_, body) in branches {
                        visit(body, out);
                    }
                    continue;
                }
                Piece::Date { name, .. } => (name, Use::Date),
                Piece::Duration { name, .. } => (name, Use::Duration),
                Piece::Text(_) | Piece::Number => continue,
            };
            let entry = (name.clone(), how);
            if !out.contains(&entry) {
                out.push(entry);
            }
        }
    }
    let mut out = Vec::new();
    visit(pieces, &mut out);
    out
}

/// Plural branches a text in `language` lacks: each category the
/// language uses needs one, so every number has its form.
pub fn missing_plurals(pieces: &[Piece], language: &str) -> Vec<String> {
    let mut missing = Vec::new();
    for piece in pieces {
        if let Piece::Plural { name, branches } = piece {
            for category in plural_categories(language) {
                if !branches.iter().any(|(selector, _)| selector == category) {
                    missing.push(format!("`{category}` of `{name}`"));
                }
            }
            for (_, body) in branches {
                missing.extend(missing_plurals(body, language));
            }
        }
    }
    missing
}

/// A value a text shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Text(String),
    /// A number, and the way Bern writes it.
    Number {
        value: f64,
        text: String,
    },
    /// A calendar date: year, month 1–12, day 1–31.
    Date {
        year: i64,
        month: i64,
        day: i64,
    },
}

/// The day of the week of a date, 0 for Monday.
pub fn weekday(year: i64, month: i64, day: i64) -> usize {
    // Sakamoto's method, from Sunday 0.
    const OFFSETS: [i64; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let month = month.clamp(1, 12);
    let year = if month < 3 { year - 1 } else { year };
    let sunday_first = (year + year.div_euclid(4) - year.div_euclid(100)
        + year.div_euclid(400)
        + OFFSETS[(month - 1) as usize]
        + day)
        .rem_euclid(7);
    ((sunday_first + 6) % 7) as usize
}

/// Write a date with a checked pattern; `name(key)` is a framework text.
pub fn format_date(
    pattern: &str,
    (year, month, day): (i64, i64, i64),
    name: &dyn Fn(&str) -> String,
) -> String {
    let mut out = String::new();
    for (c, count) in runs(pattern) {
        match (c, count) {
            ('y', 2) => {
                let _ = write!(out, "{:02}", year.rem_euclid(100));
            }
            ('y', _) => {
                let _ = write!(out, "{year}");
            }
            ('M', 1) => {
                let _ = write!(out, "{month}");
            }
            ('M', 2) => {
                let _ = write!(out, "{month:02}");
            }
            ('M', 3) => out.push_str(&name(&format!("bern.date.month.{month}"))),
            ('M', _) => out.push_str(&name(&format!("bern.date.month_long.{month}"))),
            ('d', 1) => {
                let _ = write!(out, "{day}");
            }
            ('d', _) => {
                let _ = write!(out, "{day:02}");
            }
            ('E', 3) => out.push_str(&name(&format!(
                "bern.date.weekday.{}",
                weekday(year, month, day) + 1
            ))),
            ('E', _) => out.push_str(&name(&format!(
                "bern.date.weekday_long.{}",
                weekday(year, month, day) + 1
            ))),
            (c, count) => {
                for _ in 0..count {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Write a number of seconds with a checked pattern.
pub fn format_duration(pattern: &str, seconds: f64) -> String {
    let total = if seconds.is_finite() {
        seconds.max(0.0).floor() as i64
    } else {
        0
    };
    let has_hours = pattern.contains('h');
    let has_minutes = pattern.contains('m');
    let hours = total / 3600;
    let minutes = if has_hours {
        total % 3600 / 60
    } else {
        total / 60
    };
    let seconds = if has_minutes || has_hours {
        total % 60
    } else {
        total
    };
    let mut out = String::new();
    for (c, count) in runs(pattern) {
        let value = match c {
            'h' => Some(hours),
            'm' => Some(minutes),
            's' => Some(seconds),
            _ => None,
        };
        match value {
            Some(value) if count == 2 => {
                let _ = write!(out, "{value:02}");
            }
            Some(value) => {
                let _ = write!(out, "{value}");
            }
            None => {
                for _ in 0..count {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Write a text in `language` with its arguments. `name(key)` gives the
/// framework texts for month and weekday names.
pub fn format(
    pieces: &[Piece],
    language: &str,
    arg: &dyn Fn(&str) -> Option<Arg>,
    name: &dyn Fn(&str) -> String,
) -> String {
    fn number(arg: Option<Arg>) -> (f64, String) {
        match arg {
            Some(Arg::Number { value, text }) => (value, text),
            Some(Arg::Text(text)) => (text.parse().unwrap_or(0.0), text),
            _ => (0.0, String::new()),
        }
    }
    fn write(
        pieces: &[Piece],
        language: &str,
        arg: &dyn Fn(&str) -> Option<Arg>,
        name: &dyn Fn(&str) -> String,
        hash: Option<&str>,
        out: &mut String,
    ) {
        for piece in pieces {
            match piece {
                Piece::Text(text) => out.push_str(text),
                Piece::Number => out.push_str(hash.unwrap_or_default()),
                Piece::Value(key) => match arg(key) {
                    Some(Arg::Text(text)) => out.push_str(&text),
                    Some(Arg::Number { text, .. }) => out.push_str(&text),
                    Some(Arg::Date { year, month, day }) => {
                        let _ = write!(out, "{year:04}-{month:02}-{day:02}");
                    }
                    None => {}
                },
                Piece::Plural {
                    name: key,
                    branches,
                } => {
                    let (value, text) = number(arg(key));
                    let exact = format!("={value}");
                    let category = plural_category(language, value);
                    let chosen = branches
                        .iter()
                        .find(|(selector, _)| *selector == exact)
                        .or_else(|| branches.iter().find(|(selector, _)| selector == category))
                        .or_else(|| branches.iter().find(|(selector, _)| selector == "other"));
                    if let Some((_, body)) = chosen {
                        write(body, language, arg, name, Some(&text), out);
                    }
                }
                Piece::Date { name: key, pattern } => {
                    if let Some(Arg::Date { year, month, day }) = arg(key) {
                        out.push_str(&format_date(pattern, (year, month, day), name));
                    }
                }
                Piece::Duration { name: key, pattern } => {
                    let (value, _) = number(arg(key));
                    out.push_str(&format_duration(pattern, value));
                }
            }
        }
    }
    let mut out = String::new();
    write(pieces, language, arg, name, None, &mut out);
    out
}

/// The names of weekdays (from Monday) and months, short and full, in the
/// languages Bern's runtimes speak, as framework texts:
/// `bern.date.weekday.1` … `.7`, `bern.date.weekday_long.N`,
/// `bern.date.month.1` … `.12`, `bern.date.month_long.N`.
pub fn date_names() -> Vec<(String, Vec<(&'static str, String)>)> {
    const WEEKDAYS: [(&str, [&str; 7], [&str; 7]); 4] = [
        (
            "en",
            ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
            [
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
                "Sunday",
            ],
        ),
        (
            "zh-Hans",
            ["周一", "周二", "周三", "周四", "周五", "周六", "周日"],
            [
                "星期一",
                "星期二",
                "星期三",
                "星期四",
                "星期五",
                "星期六",
                "星期日",
            ],
        ),
        (
            "zh-Hant",
            ["週一", "週二", "週三", "週四", "週五", "週六", "週日"],
            [
                "星期一",
                "星期二",
                "星期三",
                "星期四",
                "星期五",
                "星期六",
                "星期日",
            ],
        ),
        (
            "ja",
            ["月", "火", "水", "木", "金", "土", "日"],
            [
                "月曜日",
                "火曜日",
                "水曜日",
                "木曜日",
                "金曜日",
                "土曜日",
                "日曜日",
            ],
        ),
    ];
    const MONTHS_EN: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    const MONTHS_EN_LONG: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    const MONTHS_ZH_LONG: [&str; 12] = [
        "一月",
        "二月",
        "三月",
        "四月",
        "五月",
        "六月",
        "七月",
        "八月",
        "九月",
        "十月",
        "十一月",
        "十二月",
    ];
    let mut names = Vec::new();
    for day in 0..7 {
        names.push((
            format!("bern.date.weekday.{}", day + 1),
            WEEKDAYS
                .iter()
                .map(|(language, short, _)| (*language, short[day].to_string()))
                .collect(),
        ));
        names.push((
            format!("bern.date.weekday_long.{}", day + 1),
            WEEKDAYS
                .iter()
                .map(|(language, _, long)| (*language, long[day].to_string()))
                .collect(),
        ));
    }
    for month in 0..12 {
        let numbered = format!("{}月", month + 1);
        names.push((
            format!("bern.date.month.{}", month + 1),
            vec![
                ("en", MONTHS_EN[month].to_string()),
                ("zh-Hans", numbered.clone()),
                ("zh-Hant", numbered.clone()),
                ("ja", numbered.clone()),
            ],
        ));
        names.push((
            format!("bern.date.month_long.{}", month + 1),
            vec![
                ("en", MONTHS_EN_LONG[month].to_string()),
                ("zh-Hans", MONTHS_ZH_LONG[month].to_string()),
                ("zh-Hant", MONTHS_ZH_LONG[month].to_string()),
                ("ja", numbered),
            ],
        ));
    }
    names
}

/// The name of the framework type of calendar dates.
pub const CALENDAR_DATE: &str = "CalendarDate";

/// The record of a calendar date: year, month 1–12, day 1–31.
pub fn calendar_date_def() -> crate::RecordDef {
    let field = |name: &str| crate::FieldDef {
        name: name.to_string(),
        ty: crate::Type::Int,
        doc: String::new(),
        content: false,
    };
    crate::RecordDef {
        name: CALENDAR_DATE.to_string(),
        fields: vec![field("year"), field("month"), field("day")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(key: &str) -> String {
        date_names()
            .into_iter()
            .find(|(name, _)| name == key)
            .and_then(|(_, translations)| {
                translations
                    .into_iter()
                    .find(|(language, _)| *language == "en")
                    .map(|(_, text)| text)
            })
            .unwrap_or_default()
    }

    fn show(text: &str, language: &str, args: &[(&str, Arg)]) -> String {
        let pieces = parse(text).unwrap();
        format(
            &pieces,
            language,
            &|name| {
                args.iter()
                    .find(|(candidate, _)| *candidate == name)
                    .map(|(_, arg)| arg.clone())
            },
            &names,
        )
    }

    fn n(value: f64) -> Arg {
        Arg::Number {
            value,
            text: if value.fract() == 0.0 {
                format!("{}", value as i64)
            } else {
                value.to_string()
            },
        }
    }

    #[test]
    fn plain_placeholders_and_braces() {
        assert_eq!(
            show("已播放 {count} 集 {{x}}", "zh-Hans", &[("count", n(3.0))]),
            "已播放 3 集 {x}"
        );
        assert_eq!(
            uses(&parse("{a} {b} {a}").unwrap()),
            vec![("a".into(), Use::Value), ("b".into(), Use::Value)]
        );
        assert!(parse("{Count}").is_err());
        assert!(parse("{count").is_err());
        assert!(parse("a } b").is_err());
    }

    #[test]
    fn plurals_follow_the_language() {
        let text = "{n, plural, =0 {no episodes} one {# episode} other {# episodes}}";
        assert_eq!(show(text, "en", &[("n", n(1.0))]), "1 episode");
        assert_eq!(show(text, "en", &[("n", n(12.0))]), "12 episodes");
        assert_eq!(show(text, "en", &[("n", n(0.0))]), "no episodes");
        assert_eq!(
            show("共 {n, plural, other {# 话}}", "zh-Hans", &[("n", n(1.0))]),
            "共 1 话"
        );
        let pieces = parse("{n, plural, other {# episodes}}").unwrap();
        assert_eq!(missing_plurals(&pieces, "en"), vec!["`one` of `n`"]);
        assert!(missing_plurals(&pieces, "zh-Hans").is_empty());
        assert!(parse("{n, plural, one {#}}").is_err(), "other is required");
        assert!(parse("{n, plural, single {#} other {#}}").is_err());
    }

    #[test]
    fn dates_and_durations() {
        // 2026-10-12 is a Monday.
        assert_eq!(weekday(2026, 10, 12), 0);
        assert_eq!(weekday(2000, 1, 1), 5);
        let today = Arg::Date {
            year: 2026,
            month: 10,
            day: 12,
        };
        assert_eq!(
            show("{d, date, EEE, MMM d}", "en", &[("d", today.clone())]),
            "Mon, Oct 12"
        );
        assert_eq!(
            show("{d, date, EEEE yyyy-MM-dd}", "en", &[("d", today)]),
            "Monday 2026-10-12"
        );
        assert_eq!(format_duration("m:ss", 555.0), "9:15");
        assert_eq!(format_duration("h:mm:ss", 3725.0), "1:02:05");
        assert_eq!(format_duration("hh:mm", 18.0 * 3600.0 + 1800.0), "18:30");
        assert!(parse("{d, date, YYYY}").is_err());
        assert!(parse("{t, duration, mmm}").is_err());
        assert!(parse("{t, size, m}").is_err());
    }
}
