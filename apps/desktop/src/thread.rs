use std::fmt::Display;

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Deserializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Via {
    App,
    Mcp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Created,
    Completed,
    Reopened,
}

impl EventKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            EventKind::Created => "Created",
            EventKind::Completed => "Completed",
            EventKind::Reopened => "Reopened",
        }
    }
}

fn utc<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DateTime<Utc>, D::Error> {
    let text = String::deserialize(deserializer)?;
    DateTime::parse_from_rfc3339(&text)
        .map(|at| at.with_timezone(&Utc))
        .map_err(serde::de::Error::custom)
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub via: Via,
    #[serde(deserialize_with = "utc")]
    pub at: DateTime<Utc>,
    pub edited_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TimelineItem {
    Event {
        kind: EventKind,
        #[serde(deserialize_with = "utc")]
        at: DateTime<Utc>,
    },
    Comment(Comment),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Timeline {
    pub items: Vec<TimelineItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Logged(TimelineItem),
    Pending {
        ticket: u64,
        body: String,
        at: DateTime<Utc>,
    },
}

impl Entry {
    #[must_use]
    pub fn at(&self) -> DateTime<Utc> {
        match self {
            Entry::Logged(
                TimelineItem::Event { at, .. } | TimelineItem::Comment(Comment { at, .. }),
            )
            | Entry::Pending { at, .. } => *at,
        }
    }

    #[must_use]
    pub fn is_written(&self) -> bool {
        !matches!(self, Entry::Logged(TimelineItem::Event { .. }))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Day<'a> {
    pub label: String,
    pub entries: Vec<(&'a Entry, String)>,
}

#[must_use]
pub fn clock_time<Tz: TimeZone>(at: DateTime<Utc>, zone: &Tz) -> String
where
    Tz::Offset: Display,
{
    at.with_timezone(zone).format("%H:%M").to_string()
}

fn day_label(date: NaiveDate, today: NaiveDate) -> String {
    if date == today {
        "Today".to_owned()
    } else if today.pred_opt() == Some(date) {
        "Yesterday".to_owned()
    } else if date.year() == today.year() {
        date.format("%b %-d").to_string()
    } else {
        date.format("%b %-d, %Y").to_string()
    }
}

#[must_use]
pub fn by_day<'a, Tz: TimeZone>(entries: &'a [Entry], now: DateTime<Utc>, zone: &Tz) -> Vec<Day<'a>>
where
    Tz::Offset: Display,
{
    let today = now.with_timezone(zone).date_naive();
    let mut days: Vec<Day<'a>> = Vec::new();
    let mut current = None;
    for entry in entries {
        let at = entry.at();
        let date = at.with_timezone(zone).date_naive();
        if current != Some(date) {
            days.push(Day {
                label: day_label(date, today),
                entries: Vec::new(),
            });
            current = Some(date);
        }
        if let Some(day) = days.last_mut() {
            day.entries.push((entry, clock_time(at, zone)));
        }
    }
    days
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    fn tokyo() -> FixedOffset {
        FixedOffset::east_opt(9 * 3600).unwrap()
    }

    fn time(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn event(at: &str) -> Entry {
        Entry::Logged(TimelineItem::Event {
            kind: EventKind::Completed,
            at: time(at),
        })
    }

    #[test]
    fn reads_server_times_with_or_without_milliseconds_since_events_and_comments_are_stored_differently()
     {
        let timeline: Timeline = serde_json::from_str(
            r#"{"items":[
                {"type":"event","kind":"created","at":"2026-11-16T02:19:55Z"},
                {"type":"comment","id":3,"body":"asked\nB","via":"mcp","at":"2026-11-16T02:19:55.282Z","editedAt":null},
                {"type":"comment","id":4,"body":"done","via":"app","at":"2026-11-16T03:00:00.000Z","editedAt":"2026-11-16T03:05:00.000Z"}
            ]}"#,
        )
        .unwrap();

        assert_eq!(
            timeline.items,
            [
                TimelineItem::Event {
                    kind: EventKind::Created,
                    at: time("2026-11-16T02:19:55Z"),
                },
                TimelineItem::Comment(Comment {
                    id: 3,
                    body: "asked\nB".into(),
                    via: Via::Mcp,
                    at: time("2026-11-16T02:19:55.282Z"),
                    edited_at: None,
                }),
                TimelineItem::Comment(Comment {
                    id: 4,
                    body: "done".into(),
                    via: Via::App,
                    at: time("2026-11-16T03:00:00Z"),
                    edited_at: Some("2026-11-16T03:05:00.000Z".into()),
                }),
            ]
        );
        assert!(time("2026-11-16T02:19:55.282Z") > time("2026-11-16T02:19:55Z"));
    }

    #[test]
    fn refuses_a_timeline_with_a_time_it_cannot_read_instead_of_filing_the_entry_under_a_made_up_day()
     {
        let read = serde_json::from_str::<Timeline>(
            r#"{"items":[{"type":"event","kind":"created","at":"yesterday"}]}"#,
        );

        assert!(read.is_err());
    }

    #[test]
    fn groups_the_log_by_the_members_own_days_so_a_comment_written_after_midnight_there_is_not_filed_under_the_day_before()
     {
        let now = time("2026-11-16T03:00:00Z");
        let entries = [event("2026-11-15T14:30:00Z"), event("2026-11-15T15:30:00Z")];

        let days = by_day(&entries, now, &tokyo());

        let shown: Vec<(&str, Vec<&str>)> = days
            .iter()
            .map(|day| {
                (
                    day.label.as_str(),
                    day.entries.iter().map(|(_, time)| time.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(
            shown,
            [("Yesterday", vec!["23:30"]), ("Today", vec!["00:30"])]
        );
    }

    #[test]
    fn names_older_days_by_date_and_adds_the_year_only_when_it_is_not_this_year_so_the_labels_stay_short()
     {
        let now = time("2026-11-16T03:00:00Z");
        let entries = [
            event("2025-12-31T01:00:00Z"),
            event("2026-11-12T00:14:00.123Z"),
        ];

        let labels: Vec<String> = by_day(&entries, now, &tokyo())
            .into_iter()
            .map(|day| day.label)
            .collect();

        assert_eq!(labels, ["Dec 31, 2025", "Nov 12"]);
    }

    #[test]
    fn shows_times_on_a_24_hour_clock_the_same_width_for_every_entry() {
        let now = time("2026-11-16T03:00:00Z");
        let entries = [event("2026-11-16T05:02:00Z")];

        let days = by_day(&entries, now, &Utc);

        assert_eq!(days[0].entries[0].1, "05:02");
    }

    #[test]
    fn keeps_entries_of_one_day_together_in_the_order_given_so_comments_sit_under_their_day() {
        let now = time("2026-11-16T03:00:00Z");
        let entries = [
            event("2026-11-16T01:00:00Z"),
            Entry::Pending {
                ticket: 1,
                body: "sending".into(),
                at: time("2026-11-16T02:00:00Z"),
            },
        ];

        let days = by_day(&entries, now, &Utc);

        assert_eq!(days.len(), 1);
        assert_eq!(days[0].entries.len(), 2);
    }
}
