//! Time formatting helpers for user-visible binary surfaces (Asia/Shanghai).

use chrono::DateTime;
use chrono_tz::Asia::Shanghai;

pub fn fmt_beijing_rfc3339(ts: DateTime<chrono::Utc>) -> String {
    ts.with_timezone(&Shanghai).to_rfc3339()
}
