//! Time formatting helpers for user-visible binary surfaces (Asia/Shanghai).

use chrono::{DateTime, FixedOffset};

/// Asia/Shanghai is a fixed UTC+8 offset with no daylight-saving time, so a
/// `FixedOffset` reproduces it exactly without the `chrono-tz` dependency
/// (dropped from the root package when upstream removed it).
///
/// The offset is a compile-time constant well inside `FixedOffset`'s valid
/// range, so `east_opt` cannot return `None` here.
fn beijing_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("UTC+8 is within FixedOffset range")
}

pub fn fmt_beijing_rfc3339(ts: DateTime<chrono::Utc>) -> String {
    ts.with_timezone(&beijing_offset()).to_rfc3339()
}
