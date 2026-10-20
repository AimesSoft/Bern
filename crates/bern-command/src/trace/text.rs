//! Number formatting for summary lines.

/// Milliseconds with the precision their size deserves.
pub(super) fn ms(ns: u64) -> String {
    let value = ns as f64 / 1e6;
    if value < 10.0 {
        format!("{value:.2}")
    } else if value < 100.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    }
}

pub(super) fn bytes(count: u64) -> String {
    const KIB: f64 = 1024.0;
    let value = count as f64;
    if count < 1024 {
        format!("{count} B")
    } else if value < KIB * KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{:.2} MiB", value / (KIB * KIB))
    }
}

/// Seconds since the session started, for "at" times.
pub(super) fn at(ns: u64) -> String {
    format!("{:.2} s", ns as f64 / 1e9)
}

/// `name time` pairs joined by commas, leaving out the empty ones.
pub(super) fn parts(parts: &[(&str, Option<u64>)]) -> String {
    parts
        .iter()
        .filter_map(|(name, ns)| ns.map(|ns| format!("{name} {}", ms(ns))))
        .collect::<Vec<_>>()
        .join(", ")
}
