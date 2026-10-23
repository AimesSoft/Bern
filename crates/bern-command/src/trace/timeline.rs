//! The timeline: Chrome trace events in the JSON array format, which
//! ui.perfetto.dev and chrome://tracing read. The closing `]` is optional
//! in that format, so a file cut short by a crash still opens. On a web
//! page the timeline stays in memory until the session stops.

use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The most a timeline kept in memory grows to; later events are left out.
const MEMORY_LIMIT: usize = 64 << 20;

enum Output {
    File(io::BufWriter<std::fs::File>),
    Memory(Vec<u8>),
}

pub(super) struct Timeline {
    output: Output,
    path: Option<PathBuf>,
    pid: u32,
    first: bool,
    line: String,
}

/// An argument of an event.
pub(super) enum Arg<'a> {
    Int(i64),
    Text(&'a str),
}

pub(super) fn escape(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
}

/// Microseconds with nanosecond precision, as trace events count time.
fn micros(ns: u64) -> String {
    format!("{}.{:03}", ns / 1000, ns % 1000)
}

impl Timeline {
    pub fn create(path: &Path) -> io::Result<Timeline> {
        let output = if cfg!(target_arch = "wasm32") {
            Output::Memory(Vec::new())
        } else {
            if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir)?;
            }
            Output::File(io::BufWriter::new(std::fs::File::create(path)?))
        };
        #[cfg(not(target_arch = "wasm32"))]
        let pid = std::process::id();
        #[cfg(target_arch = "wasm32")]
        let pid = 1;
        let mut timeline = Timeline {
            path: matches!(output, Output::File(_)).then(|| path.to_path_buf()),
            output,
            pid,
            first: true,
            line: String::new(),
        };
        timeline.write_raw("[\n");
        Ok(timeline)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn write_raw(&mut self, text: &str) {
        match &mut self.output {
            Output::File(file) => {
                let _ = file.write_all(text.as_bytes());
            }
            Output::Memory(bytes) => {
                if bytes.len() + text.len() <= MEMORY_LIMIT {
                    bytes.extend_from_slice(text.as_bytes());
                }
            }
        }
    }

    fn event(&mut self, body: impl FnOnce(&mut String)) {
        let mut line = std::mem::take(&mut self.line);
        line.clear();
        if !self.first {
            line.push_str(",\n");
        }
        self.first = false;
        line.push('{');
        body(&mut line);
        line.push('}');
        self.write_raw(&line);
        self.line = line;
    }

    fn head(&self, out: &mut String, phase: &str, tid: u32, ts: u64) {
        let _ = write!(
            out,
            "\"ph\":\"{phase}\",\"pid\":{},\"tid\":{tid},\"ts\":{}",
            self.pid,
            micros(ts)
        );
    }

    fn name_and_args(out: &mut String, name: &str, category: &str, args: &[(&str, Arg<'_>)]) {
        out.push_str(",\"name\":\"");
        escape(name, out);
        out.push_str("\",\"cat\":\"");
        out.push_str(category);
        out.push('"');
        if !args.is_empty() {
            out.push_str(",\"args\":{");
            for (index, (key, value)) in args.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                let _ = write!(out, "\"{key}\":");
                match value {
                    Arg::Int(number) => {
                        let _ = write!(out, "{number}");
                    }
                    Arg::Text(text) => {
                        out.push('"');
                        escape(text, out);
                        out.push('"');
                    }
                }
            }
            out.push('}');
        }
    }

    /// Name a track.
    pub fn thread(&mut self, tid: u32, name: &str, order: i64) {
        let pid = self.pid;
        self.event(|out| {
            let _ = write!(
                out,
                "\"ph\":\"M\",\"pid\":{pid},\"tid\":{tid},\"name\":\"thread_name\",\"args\":{{\"name\":\""
            );
            escape(name, out);
            out.push_str("\"}");
        });
        self.event(|out| {
            let _ = write!(
                out,
                "\"ph\":\"M\",\"pid\":{pid},\"tid\":{tid},\"name\":\"thread_sort_index\",\"args\":{{\"sort_index\":{order}}}"
            );
        });
    }

    /// A span of time on a track; `ts` and `dur` in ns since the session
    /// started.
    pub fn slice(
        &mut self,
        tid: u32,
        ts: u64,
        dur: u64,
        name: &str,
        category: &str,
        args: &[(&str, Arg<'_>)],
    ) {
        let mut head = String::new();
        self.head(&mut head, "X", tid, ts);
        self.event(|out| {
            out.push_str(&head);
            let _ = write!(out, ",\"dur\":{}", micros(dur));
            Self::name_and_args(out, name, category, args);
        });
    }

    /// A moment on a track.
    pub fn instant(
        &mut self,
        tid: u32,
        ts: u64,
        name: &str,
        category: &str,
        args: &[(&str, Arg<'_>)],
    ) {
        let mut head = String::new();
        self.head(&mut head, "i", tid, ts);
        self.event(|out| {
            out.push_str(&head);
            out.push_str(",\"s\":\"t\"");
            Self::name_and_args(out, name, category, args);
        });
    }

    /// One step of the arrow `id` that links the slices of one request:
    /// `s` starts it, `t` passes through, `f` ends it in the slice around
    /// `ts`.
    pub fn flow(&mut self, phase: char, id: u64, tid: u32, ts: u64) {
        let mut head = String::new();
        self.head(&mut head, &phase.to_string(), tid, ts);
        self.event(|out| {
            out.push_str(&head);
            let _ = write!(out, ",\"id\":{id},\"name\":\"request\",\"cat\":\"flow\"");
            if phase != 's' {
                out.push_str(",\"bp\":\"e\"");
            }
        });
    }

    /// Finish the array. Returns the file, and the whole timeline when
    /// `contents` asks for it or it was kept in memory.
    pub fn close(mut self, contents: bool) -> (Option<PathBuf>, Option<String>) {
        self.write_raw("\n]\n");
        match self.output {
            Output::File(mut file) => {
                let _ = file.flush();
                drop(file);
                let text = contents
                    .then(|| {
                        self.path
                            .as_ref()
                            .and_then(|path| std::fs::read_to_string(path).ok())
                    })
                    .flatten();
                (self.path, text)
            }
            Output::Memory(bytes) => (None, Some(String::from_utf8_lossy(&bytes).into_owned())),
        }
    }
}
