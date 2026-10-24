//! plugged.host: times and durations as rux reads and writes them: everything in the settings'
//! time zone unless a string carries its own offset.

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use std::sync::LazyLock;

static DURATION: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(?:\d+(?:w|d|hrs|hr|h|mins|min|m|secs|sec|s))+$").unwrap()
});
static DURATION_PART: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(\d+)(w|d|hrs|hr|h|mins|min|m|secs|sec|s)").unwrap());
static LOCAL: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(\d{4})-(\d{2})-(\d{2})[ T](\d{1,2}):(\d{2})(?::(\d{2}))?$").unwrap()
});
static OFFSET: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})$")
        .unwrap()
});

/// Seconds in "90s", "10m", "2h", "1d12h", "1w"; a bare number is minutes. `None` when it is
/// not a duration.
pub fn duration_seconds(input: &str) -> Option<i64> {
    let value: String = input
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase();

    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        return value.parse::<i64>().ok()?.checked_mul(60);
    }
    if value.is_empty() || !DURATION.is_match(&value) {
        return None;
    }

    let mut seconds: i64 = 0;
    for part in DURATION_PART.captures_iter(&value) {
        let amount: i64 = part[1].parse().ok()?;
        let unit = match &part[2] {
            "w" => 604_800,
            "d" => 86_400,
            "h" | "hr" | "hrs" => 3_600,
            "m" | "min" | "mins" => 60,
            _ => 1,
        };
        seconds = seconds.checked_add(amount.checked_mul(unit)?)?;
    }

    Some(seconds)
}

/// [`duration_seconds`], or why not, in words the model can act on.
pub fn duration(input: &str) -> Result<i64, String> {
    duration_seconds(input).ok_or_else(|| {
        format!(
            "Cannot read the duration \"{}\". Use something like 10m, 2h or 7d.",
            input.trim()
        )
    })
}

/// "2026-10-04 17:00", "2026-10-04T17:00", an ISO-8601 time with an offset, or a duration from
/// now ("2h", "1d12h", "in 30m"), in `zone`.
pub fn parse(input: &str, now: DateTime<Tz>) -> Result<DateTime<Tz>, String> {
    let zone = now.timezone();
    let value = input.trim();
    if value.is_empty() {
        return Err("No time given.".into());
    }

    let relative = value
        .strip_prefix("in ")
        .or_else(|| value.strip_prefix("In "))
        .unwrap_or(value);
    if let Some(seconds) = duration_seconds(relative) {
        return chrono::TimeDelta::try_seconds(seconds)
            .and_then(|span| now.checked_add_signed(span))
            .ok_or_else(|| format!("\"{value}\" is too far away."));
    }

    if let Some(parts) = LOCAL.captures(value) {
        let number = |index: usize| {
            parts
                .get(index)
                .map_or(0, |m| m.as_str().parse().unwrap_or(u32::MAX))
        };
        let date = NaiveDate::from_ymd_opt(parts[1].parse().unwrap_or(0), number(2), number(3));
        let time = NaiveTime::from_hms_opt(number(4), number(5), number(6));
        let local = match (date, time) {
            (Some(date), Some(time)) => NaiveDateTime::new(date, time),
            _ => {
                return Err(format!(
                    "\"{value}\" is not a real date and time. Use YYYY-MM-DD HH:MM."
                ));
            }
        };

        return match zone.from_local_datetime(&local) {
            chrono::LocalResult::Single(time) => Ok(time),
            // the hour the clocks go back: the first of the two
            chrono::LocalResult::Ambiguous(first, _) => Ok(first),
            chrono::LocalResult::None => Err(format!(
                "\"{value}\" does not exist in {} (the clocks skip it). Use another time.",
                zone.name()
            )),
        };
    }

    if OFFSET.is_match(value) {
        let normalized = normalize_offset(value);
        if let Ok(time) = DateTime::parse_from_rfc3339(&normalized) {
            return Ok(time.with_timezone(&zone));
        }
    }

    Err(format!(
        "Cannot read the time \"{value}\". Use YYYY-MM-DD HH:MM ({} time) or a duration like 2h.",
        zone.name()
    ))
}

/// An ISO time as RFC 3339 wants it: seconds and a colon in the offset.
fn normalize_offset(value: &str) -> String {
    let (main, offset) = match value.strip_suffix('Z') {
        Some(main) => (main.to_string(), "Z".to_string()),
        None => {
            let split = value.len()
                - if value[value.len() - 6..].contains(':') {
                    6
                } else {
                    5
                };
            let (main, offset) = value.split_at(split);
            let offset = if offset.contains(':') {
                offset.to_string()
            } else {
                format!("{}:{}", &offset[..3], &offset[3..])
            };
            (main.to_string(), offset)
        }
    };
    // `YYYY-MM-DDTHH:MM` has no seconds yet
    let main = if main.len() == 16 {
        format!("{main}:00")
    } else {
        main
    };

    format!("{main}{offset}")
}

/// The zone's city, as people say it ("Paris", "New York"), or its name ("UTC").
pub fn city(zone: Tz) -> String {
    let name = zone.name();

    match name.rsplit_once('/') {
        Some((_, city)) => city.replace('_', " "),
        None => name.to_string(),
    }
}

/// "Sun 4 Oct 2026, 17:00 Paris time", for rux's own replies; Discord timestamps
/// ([`discord`]) are used wherever members read a time, so each sees it in their own zone.
pub fn human(time: DateTime<Utc>, zone: Tz) -> String {
    format!(
        "{} {} time",
        time.with_timezone(&zone).format("%a %-d %b %Y, %H:%M"),
        city(zone)
    )
}

/// "2 hours 30 minutes": the two largest units of a span.
pub fn human_duration(seconds: i64) -> String {
    const UNITS: [(i64, &str); 5] = [
        (604_800, "week"),
        (86_400, "day"),
        (3_600, "hour"),
        (60, "minute"),
        (1, "second"),
    ];

    let mut left = seconds.max(0);
    let mut parts = Vec::new();
    for (size, name) in UNITS {
        if parts.len() == 2 {
            break;
        }
        let count = left / size;
        if count > 0 {
            parts.push(format!(
                "{count} {name}{}",
                if count == 1 { "" } else { "s" }
            ));
            left -= count * size;
        } else if !parts.is_empty() {
            // "1 week 3 hours" reads like a mistake: stop at the first gap
            break;
        }
    }

    if parts.is_empty() {
        "0 seconds".into()
    } else {
        parts.join(" ")
    }
}

/// `<t:…:f>`: a time every member sees in their own zone (`style` t, T, d, D, f, F or R).
pub fn discord(time: DateTime<Utc>, style: char) -> String {
    format!("<t:{}:{style}>", time.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Tz> {
        chrono_tz::Europe::Paris
            .with_ymd_and_hms(2026, 10, 4, 12, 0, 0)
            .unwrap()
    }

    #[test]
    fn durations() {
        assert_eq!(duration_seconds("90s"), Some(90));
        assert_eq!(duration_seconds("10m"), Some(600));
        assert_eq!(duration_seconds("10 mins"), Some(600));
        assert_eq!(duration_seconds("2h"), Some(7200));
        assert_eq!(duration_seconds("1d12h"), Some(129_600));
        assert_eq!(duration_seconds("1W"), Some(604_800));
        assert_eq!(duration_seconds("45"), Some(2700));
        assert_eq!(duration_seconds("2hrs30min"), Some(9000));
        assert_eq!(duration_seconds("soon"), None);
        assert_eq!(duration_seconds(""), None);
        assert_eq!(duration_seconds("2x"), None);
        assert!(duration("nope").unwrap_err().contains("10m, 2h or 7d"));
    }

    #[test]
    fn local_times_are_in_the_zone() {
        let time = parse("2026-10-05 17:00", now()).unwrap();
        assert_eq!(time.to_rfc3339(), "2026-10-05T17:00:00+02:00");
        let time = parse("2026-12-01T9:05", now()).unwrap();
        assert_eq!(time.to_rfc3339(), "2026-12-01T09:05:00+01:00");
        assert!(
            parse("2026-02-31 10:00", now())
                .unwrap_err()
                .contains("not a real date")
        );
        // the clocks skip 02:30 on the last Sunday of March
        assert!(
            parse("2026-03-29 02:30", now())
                .unwrap_err()
                .contains("does not exist")
        );
    }

    #[test]
    fn offsets_and_durations_from_now() {
        assert_eq!(
            parse("2026-10-05T15:00:00Z", now()).unwrap().to_rfc3339(),
            "2026-10-05T17:00:00+02:00"
        );
        assert_eq!(
            parse("2026-10-05T15:00+0100", now()).unwrap().to_rfc3339(),
            "2026-10-05T16:00:00+02:00"
        );
        assert_eq!(
            parse("in 2h", now()).unwrap().to_rfc3339(),
            "2026-10-04T14:00:00+02:00"
        );
        assert!(
            parse("tomorrow", now())
                .unwrap_err()
                .contains("Europe/Paris")
        );
        assert_eq!(parse("  ", now()).unwrap_err(), "No time given.");
    }

    #[test]
    fn human_forms() {
        let time = now().with_timezone(&Utc);
        assert_eq!(
            human(time, chrono_tz::Europe::Paris),
            "Sun 4 Oct 2026, 12:00 Paris time"
        );
        assert_eq!(
            human(time, chrono_tz::America::New_York),
            "Sun 4 Oct 2026, 06:00 New York time"
        );
        assert_eq!(
            human(time, chrono_tz::UTC),
            "Sun 4 Oct 2026, 10:00 UTC time"
        );
        assert_eq!(human_duration(9000), "2 hours 30 minutes");
        assert_eq!(human_duration(90_061), "1 day 1 hour");
        assert_eq!(human_duration(1), "1 second");
        assert_eq!(human_duration(0), "0 seconds");
        assert_eq!(discord(time, 'R'), format!("<t:{}:R>", time.timestamp()));
    }
}
