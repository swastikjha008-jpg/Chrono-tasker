//! Core domain types: [`Task`], [`Priority`], [`Recurrence`] and [`Urgency`].
//!
//! Rust topics shown here: enums (with and without data), `impl FromStr`,
//! `impl Display`, `#[derive]`, associated constants, traits with default
//! methods, the builder pattern and pattern matching with guards.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Days, Months, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::error::TaskError;

/// Anything that can describe itself in one line.
///
/// `headline` is a *default method*: implementors only need to write
/// `summary`, and get a truncated version for free.
pub trait Summarize {
    fn summary(&self) -> String;

    /// The summary, cut to at most 60 characters.
    fn headline(&self) -> String {
        const MAX: usize = 60;
        let full = self.summary();
        match full.char_indices().nth(MAX) {
            Some((byte_index, _)) => format!("{}…", &full[..byte_index]),
            None => full,
        }
    }
}

/// How important a task is. The derived `Ord` follows declaration order,
/// so `Low < Medium < High`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    #[default]
    Medium,
    High,
}

impl Priority {
    /// Every priority, lowest first. Handy for iterating in reports.
    pub const ALL: [Priority; 3] = [Priority::Low, Priority::Medium, Priority::High];
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `pad` honours width/alignment flags such as `{:<6}`.
        f.pad(match self {
            Priority::Low => "low",
            Priority::Medium => "medium",
            Priority::High => "high",
        })
    }
}

impl FromStr for Priority {
    type Err = TaskError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" | "l" => Ok(Priority::Low),
            "medium" | "med" | "m" => Ok(Priority::Medium),
            "high" | "h" => Ok(Priority::High),
            _ => Err(TaskError::InvalidPriority(s.to_string())),
        }
    }
}

/// How often a task comes back after it is completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Recurrence {
    Daily,
    Weekly,
    Monthly,
}

impl Recurrence {
    /// The due date of the next occurrence after `from`.
    ///
    /// Monthly rules clamp to the end of shorter months (Jan 31 → Feb 28).
    pub fn next_date(self, from: NaiveDate) -> NaiveDate {
        let next = match self {
            Recurrence::Daily => from.checked_add_days(Days::new(1)),
            Recurrence::Weekly => from.checked_add_days(Days::new(7)),
            Recurrence::Monthly => from.checked_add_months(Months::new(1)),
        };
        // Only `None` at the far end of the calendar; keep the date then.
        next.unwrap_or(from)
    }
}

impl fmt::Display for Recurrence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(match self {
            Recurrence::Daily => "daily",
            Recurrence::Weekly => "weekly",
            Recurrence::Monthly => "monthly",
        })
    }
}

impl FromStr for Recurrence {
    type Err = TaskError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "daily" | "day" | "d" => Ok(Recurrence::Daily),
            "weekly" | "week" | "w" => Ok(Recurrence::Weekly),
            "monthly" | "month" | "m" => Ok(Recurrence::Monthly),
            _ => Err(TaskError::InvalidRecurrence(s.to_string())),
        }
    }
}

/// Where a task stands relative to today. Variants carry the day count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Done,
    Overdue(i64),
    Today,
    Soon(i64),
    Later(i64),
}

impl fmt::Display for Urgency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Urgency::Done => write!(f, "done"),
            Urgency::Overdue(days) => write!(f, "{days}d overdue"),
            Urgency::Today => write!(f, "due today"),
            // Or-pattern: both variants bind the same `days`.
            Urgency::Soon(days) | Urgency::Later(days) => write!(f, "in {days}d"),
        }
    }
}

/// A single to-do item with a due date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: u32,
    pub title: String,
    pub due: NaiveDate,
    pub priority: Priority,
    pub recurrence: Option<Recurrence>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Task {
    pub fn is_done(&self) -> bool {
        self.completed_at.is_some()
    }

    /// Whole days until the due date (negative when overdue).
    pub fn days_until(&self, today: NaiveDate) -> i64 {
        (self.due - today).num_days()
    }

    /// Classify the task relative to `today`.
    pub fn urgency(&self, today: NaiveDate) -> Urgency {
        if self.is_done() {
            return Urgency::Done;
        }
        match self.days_until(today) {
            days if days < 0 => Urgency::Overdue(-days),
            0 => Urgency::Today,
            days @ 1..=3 => Urgency::Soon(days),
            days => Urgency::Later(days),
        }
    }
}

impl Summarize for Task {
    fn summary(&self) -> String {
        format!(
            "#{} {} [{}] due {}",
            self.id,
            self.title,
            self.priority,
            self.due.format("%a %d %b %Y")
        )
    }
}

/// Input for creating a task, assembled with the builder pattern:
///
/// ```
/// use chrono::NaiveDate;
/// use chrono_tasker::task::{NewTask, Priority};
///
/// let due = NaiveDate::from_ymd_opt(2026, 12, 25).unwrap();
/// let new = NewTask::new("Buy gifts", due)
///     .priority(Priority::High)
///     .tags(["family", "shopping"]);
/// assert_eq!(new.tags.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct NewTask {
    pub title: String,
    pub due: NaiveDate,
    pub priority: Priority,
    pub recurrence: Option<Recurrence>,
    pub tags: Vec<String>,
}

impl NewTask {
    pub fn new(title: impl Into<String>, due: NaiveDate) -> Self {
        NewTask {
            title: title.into(),
            due,
            priority: Priority::default(),
            recurrence: None,
            tags: Vec::new(),
        }
    }

    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn recurrence(mut self, rule: Option<Recurrence>) -> Self {
        self.recurrence = rule;
        self
    }

    /// Accepts any iterator of string-like items (`&str`, `String`, ...).
    pub fn tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn sample(due: NaiveDate) -> Task {
        Task {
            id: 1,
            title: "Write report".into(),
            due,
            priority: Priority::High,
            recurrence: None,
            tags: vec!["work".into()],
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    #[test]
    fn priority_order_and_parse() {
        assert!(Priority::Low < Priority::Medium && Priority::Medium < Priority::High);
        assert_eq!("HIGH".parse::<Priority>().unwrap(), Priority::High);
        assert_eq!("m".parse::<Priority>().unwrap(), Priority::Medium);
        assert!("urgent".parse::<Priority>().is_err());
        assert_eq!(Priority::default(), Priority::Medium);
    }

    #[test]
    fn recurrence_parse_and_dates() {
        assert_eq!("weekly".parse::<Recurrence>().unwrap(), Recurrence::Weekly);
        assert!("yearly".parse::<Recurrence>().is_err());

        let start = date(2026, 11, 11);
        assert_eq!(Recurrence::Daily.next_date(start), date(2026, 11, 12));
        assert_eq!(Recurrence::Weekly.next_date(start), date(2026, 11, 18));
        assert_eq!(Recurrence::Monthly.next_date(start), date(2026, 12, 11));
    }

    #[test]
    fn monthly_clamps_to_end_of_month() {
        assert_eq!(
            Recurrence::Monthly.next_date(date(2026, 1, 31)),
            date(2026, 2, 28)
        );
        // 2028 is a leap year.
        assert_eq!(
            Recurrence::Monthly.next_date(date(2028, 1, 31)),
            date(2028, 2, 29)
        );
    }

    #[test]
    fn urgency_buckets() {
        let today = date(2026, 11, 11);
        assert_eq!(
            sample(date(2026, 11, 8)).urgency(today),
            Urgency::Overdue(3)
        );
        assert_eq!(sample(today).urgency(today), Urgency::Today);
        assert_eq!(sample(date(2026, 11, 14)).urgency(today), Urgency::Soon(3));
        assert_eq!(sample(date(2026, 11, 15)).urgency(today), Urgency::Later(4));

        let mut done = sample(today);
        done.completed_at = Some(Utc::now());
        assert_eq!(done.urgency(today), Urgency::Done);
    }

    #[test]
    fn urgency_display() {
        assert_eq!(Urgency::Overdue(2).to_string(), "2d overdue");
        assert_eq!(Urgency::Today.to_string(), "due today");
        assert_eq!(Urgency::Soon(1).to_string(), "in 1d");
        assert_eq!(Urgency::Later(9).to_string(), "in 9d");
        assert_eq!(Urgency::Done.to_string(), "done");
    }

    #[test]
    fn summary_and_headline() {
        let task = sample(date(2026, 11, 13));
        assert_eq!(task.summary(), "#1 Write report [high] due Fri 13 Nov 2026");
        assert_eq!(task.headline(), task.summary());

        let mut long = task;
        long.title = "é".repeat(100);
        let headline = long.headline();
        assert!(headline.ends_with('…'));
        assert_eq!(headline.chars().count(), 61);
    }

    #[test]
    fn padding_is_respected() {
        assert_eq!(format!("[{:<6}]", Priority::Low), "[low   ]");
        assert_eq!(format!("[{:>7}]", Recurrence::Daily), "[  daily]");
    }

    #[test]
    fn builder_collects_everything() {
        let new = NewTask::new("Plan trip", date(2026, 12, 1))
            .priority(Priority::Low)
            .recurrence(Some(Recurrence::Monthly))
            .tags(vec![String::from("travel")]);
        assert_eq!(new.title, "Plan trip");
        assert_eq!(new.priority, Priority::Low);
        assert_eq!(new.recurrence, Some(Recurrence::Monthly));
        assert_eq!(new.tags, vec!["travel"]);
    }
}
