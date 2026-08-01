//! A minimal UTC RFC-3339 clock, used for `captured_at` fields in HTTP Server
//! responses (see `docs/specs/http-server.md`). Implemented by hand (rather
//! than pulling in `chrono`/`time`) since formatting the current instant as
//! `YYYY-MM-DDTHH:MM:SSZ` is the only thing needed. The day/civil-date math
//! mirrors `pane_locator::days_from_civil`'s inverse (Howard Hinnant's
//! well-known algorithm), so the two stay consistent if either is revisited.

use std::time::{SystemTime, UNIX_EPOCH};

use super::http_server::Clock;

/// The real system clock.
pub struct RealClock;

impl Clock for RealClock {
    fn now_rfc3339(&self) -> String {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        rfc3339_from_unix_secs(secs)
    }
}

/// Days since the Unix epoch to a proleptic-Gregorian civil date `(y, m, d)`.
/// Inverse of the algorithm in `pane_locator::days_from_civil`.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Format a unix timestamp (seconds since epoch) as `YYYY-MM-DDTHH:MM:SSZ`.
fn rfc3339_from_unix_secs(secs: u64) -> String {
    let secs = secs as i64;
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let hh = secs_of_day / 3600;
    let mm = (secs_of_day % 3600) / 60;
    let ss = secs_of_day % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_formats_as_1970_01_01() {
        assert_eq!(rfc3339_from_unix_secs(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_timestamp_formats_correctly() {
        // 2026-07-26T16:54:03Z, cross-checked against
        // pane_locator::tests::parse_lstart_known_timestamp's 1_785_084_843.
        assert_eq!(rfc3339_from_unix_secs(1_785_084_843), "2026-07-26T16:54:03Z");
    }

    #[test]
    fn civil_from_days_matches_pane_locator_anchors() {
        // Mirrors pane_locator::tests::days_from_civil_epoch_anchors, inverted.
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
        assert_eq!(civil_from_days(20_454), (2026, 1, 1));
    }
}
