//! Build metadata helpers: deterministic timestamps for reproducible builds.
//!
//! `SOURCE_DATE_EPOCH` (a Unix timestamp) lets build systems pin the
//! `generated_at` field so the same input produces byte-identical manifests.
//! The conversion is hand-rolled (civil-from-days) to avoid a `chrono`
//! dependency.

/// Convert a Unix epoch (seconds since 1970-01-01T00:00:00Z) into an
/// ISO-8601 UTC timestamp like `2023-11-14T22:13:20Z`, as used by the
/// manifest's `generated_at` field.
pub fn generated_at_from_epoch(epoch_secs: i64) -> String {
  let days = epoch_secs.div_euclid(86_400);
  let rem = epoch_secs.rem_euclid(86_400);
  let (year, month, day) = civil_from_days(days);
  let hours = rem / 3_600;
  let minutes = (rem % 3_600) / 60;
  let seconds = rem % 60;
  format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

/// Howard Hinnant's `civil_from_days` algorithm: days since the Unix epoch
/// → `(year, month, day)` in the proleptic Gregorian calendar.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
  let z = days + 719_468;
  let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
  let doe = (z - era * 146_097) as u64;
  let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
  let year = yoe as i64 + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
  let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
  (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn unix_epoch_is_1970() {
    assert_eq!(generated_at_from_epoch(0), "1970-01-01T00:00:00Z");
  }

  #[test]
  fn known_timestamp() {
    assert_eq!(
      generated_at_from_epoch(1_700_000_000),
      "2023-11-14T22:13:20Z"
    );
  }

  #[test]
  fn end_of_year_rollover() {
    // 2020-12-31T23:59:59Z → next day
    assert_eq!(
      generated_at_from_epoch(1_609_459_199),
      "2020-12-31T23:59:59Z"
    );
    assert_eq!(
      generated_at_from_epoch(1_609_459_200),
      "2021-01-01T00:00:00Z"
    );
  }

  #[test]
  fn negative_epochs_are_supported() {
    assert_eq!(generated_at_from_epoch(-1), "1969-12-31T23:59:59Z");
  }

  #[test]
  fn leap_year_february() {
    // 2024-02-29T00:00:00Z (leap day)
    assert_eq!(
      generated_at_from_epoch(1_709_164_800),
      "2024-02-29T00:00:00Z"
    );
  }
}
