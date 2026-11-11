//! Read-only views over tasks: statistics and a week-by-week agenda.
//!
//! Rust topics shown here: `BTreeMap` and the entry API, generic functions
//! with trait bounds and `where` clauses, structs holding borrowed data,
//! iterator adaptors and `ISO` week handling with chrono.

use std::collections::BTreeMap;

use chrono::{Datelike, Days, NaiveDate, Weekday};

use crate::task::{Priority, Summarize, Task, Urgency};

/// Numbers describing a list of tasks at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stats {
    pub total: usize,
    pub done: usize,
    pub pending: usize,
    pub overdue: usize,
    pub due_today: usize,
    pub due_next_7_days: usize,
    /// Pending tasks per priority. Priorities with no tasks are absent.
    pub by_priority: BTreeMap<Priority, usize>,
    /// Pending tasks per tag.
    pub by_tag: BTreeMap<String, usize>,
}

impl Stats {
    pub fn compute(tasks: &[Task], today: NaiveDate) -> Self {
        let mut stats = Stats {
            total: tasks.len(),
            ..Stats::default()
        };

        for task in tasks {
            if task.is_done() {
                stats.done += 1;
                continue;
            }
            stats.pending += 1;

            match task.urgency(today) {
                Urgency::Overdue(_) => stats.overdue += 1,
                Urgency::Today => stats.due_today += 1,
                _ => {}
            }
            if (1..=7).contains(&task.days_until(today)) {
                stats.due_next_7_days += 1;
            }

            *stats.by_priority.entry(task.priority).or_insert(0) += 1;
            for tag in &task.tags {
                *stats.by_tag.entry(tag.clone()).or_default() += 1;
            }
        }
        stats
    }

    /// Percentage of all tasks that are done (0.0 when there are none).
    pub fn completion_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.done as f64 / self.total as f64 * 100.0
        }
    }
}

impl Summarize for Stats {
    fn summary(&self) -> String {
        format!(
            "{} pending, {} done, {} overdue ({:.0}% complete)",
            self.pending,
            self.done,
            self.overdue,
            self.completion_rate()
        )
    }
}

/// A titled group of tasks in the agenda. Holds references, so it cannot
/// outlive the tasks it points at (that is the `'a`).
#[derive(Debug)]
pub struct Section<'a> {
    pub title: String,
    pub tasks: Vec<&'a Task>,
}

/// Group pending tasks into "Overdue", "This week", "Next week", and one
/// section per later ISO week. Input order is kept inside each section.
pub fn build_agenda<'a>(tasks: &[&'a Task], today: NaiveDate) -> Vec<Section<'a>> {
    let mut overdue: Vec<&'a Task> = Vec::new();
    let mut weeks: BTreeMap<(i32, u32), Vec<&'a Task>> = BTreeMap::new();

    for &task in tasks.iter().filter(|task| !task.is_done()) {
        if task.due < today {
            overdue.push(task);
        } else {
            let week = task.due.iso_week();
            weeks
                .entry((week.year(), week.week()))
                .or_default()
                .push(task);
        }
    }

    let week_key = |date: NaiveDate| (date.iso_week().year(), date.iso_week().week());
    let this_week = week_key(today);
    let next_week = today
        .checked_add_days(Days::new(7))
        .map(week_key)
        .unwrap_or(this_week);

    let mut sections = Vec::new();
    if !overdue.is_empty() {
        sections.push(Section {
            title: "Overdue".to_string(),
            tasks: overdue,
        });
    }

    for ((year, week), tasks) in weeks {
        let title = if (year, week) == this_week {
            "This week".to_string()
        } else if (year, week) == next_week {
            "Next week".to_string()
        } else {
            NaiveDate::from_isoywd_opt(year, week, Weekday::Mon).map_or_else(
                || format!("Week {week}, {year}"),
                |monday| format!("Week of {}", monday.format("%d %b %Y")),
            )
        };
        sections.push(Section { title, tasks });
    }
    sections
}

/// Render any collection of [`Summarize`] items as a bullet list.
///
/// Generic over the item type `T` *and* the collection type `I`.
///
/// ```
/// use chrono_tasker::report::{bullet_list, Stats};
///
/// let stats = Stats::default();
/// assert_eq!(
///     bullet_list([&stats]),
///     "• 0 pending, 0 done, 0 overdue (0% complete)"
/// );
/// ```
pub fn bullet_list<'a, T, I>(items: I) -> String
where
    T: Summarize + 'a,
    I: IntoIterator<Item = &'a T>,
{
    items
        .into_iter()
        .map(|item| format!("• {}", item.summary()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn task(id: u32, due: NaiveDate, priority: Priority, tags: &[&str], done: bool) -> Task {
        Task {
            id,
            title: format!("Task {id}"),
            due,
            priority,
            recurrence: None,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            created_at: Utc::now(),
            completed_at: done.then(Utc::now),
        }
    }

    /// Wednesday 11 Nov 2026.
    fn today() -> NaiveDate {
        date(2026, 11, 11)
    }

    fn sample() -> Vec<Task> {
        vec![
            task(1, date(2026, 11, 9), Priority::High, &["work"], false), // overdue
            task(
                2,
                date(2026, 11, 11),
                Priority::Medium,
                &["work", "home"],
                false,
            ), // today
            task(3, date(2026, 11, 13), Priority::Low, &[], false),       // this week
            task(4, date(2026, 11, 17), Priority::High, &["home"], false), // next week
            task(5, date(2026, 12, 2), Priority::Low, &[], false),        // later
            task(6, date(2026, 11, 10), Priority::High, &["work"], true), // done
        ]
    }

    #[test]
    fn stats_count_everything() {
        let stats = Stats::compute(&sample(), today());
        assert_eq!(stats.total, 6);
        assert_eq!(stats.done, 1);
        assert_eq!(stats.pending, 5);
        assert_eq!(stats.overdue, 1);
        assert_eq!(stats.due_today, 1);
        assert_eq!(stats.due_next_7_days, 2);
        assert_eq!(stats.by_priority[&Priority::High], 2);
        assert_eq!(stats.by_priority[&Priority::Medium], 1);
        assert_eq!(stats.by_tag["work"], 2);
        assert_eq!(stats.by_tag["home"], 2);
        assert!((stats.completion_rate() - 100.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn stats_of_nothing() {
        let stats = Stats::compute(&[], today());
        assert_eq!(stats, Stats::default());
        assert_eq!(stats.completion_rate(), 0.0);
    }

    #[test]
    fn agenda_groups_by_week() {
        let tasks = sample();
        let refs: Vec<&Task> = tasks.iter().collect();
        let agenda = build_agenda(&refs, today());

        let layout: Vec<(&str, Vec<u32>)> = agenda
            .iter()
            .map(|s| (s.title.as_str(), s.tasks.iter().map(|t| t.id).collect()))
            .collect();

        assert_eq!(
            layout,
            [
                ("Overdue", vec![1]),
                ("This week", vec![2, 3]),
                ("Next week", vec![4]),
                ("Week of 30 Nov 2026", vec![5]),
            ]
        );
    }

    #[test]
    fn agenda_skips_done_and_handles_empty() {
        let done = task(1, today(), Priority::Low, &[], true);
        assert!(build_agenda(&[&done], today()).is_empty());
        assert!(build_agenda(&[], today()).is_empty());
    }

    #[test]
    fn bullet_list_works_for_tasks() {
        let tasks = sample();
        let text = bullet_list(tasks.iter().take(2));
        assert_eq!(
            text,
            "• #1 Task 1 [high] due Mon 09 Nov 2026\n• #2 Task 2 [medium] due Wed 11 Nov 2026"
        );
    }

    #[test]
    fn stats_summary_line() {
        let stats = Stats::compute(&sample(), today());
        assert_eq!(
            stats.summary(),
            "5 pending, 1 done, 1 overdue (17% complete)"
        );
    }
}
