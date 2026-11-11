//! The task collection and its JSON persistence.
//!
//! Rust topics shown here: ownership vs. borrowing, `Result` and the `?`
//! operator, `map_err`, struct update syntax, closures, iterator chains,
//! lifetime elision on methods that return references, and a *filter struct*
//! with an explicit lifetime parameter.

use std::cmp::Reverse;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Result, TaskError};
use crate::task::{NewTask, Priority, Task};

/// Which tasks to show. Borrows its text fields instead of owning them,
/// which is what the `'a` lifetime expresses.
///
/// ```
/// use chrono_tasker::store::TaskFilter;
/// use chrono_tasker::task::Priority;
///
/// let filter = TaskFilter {
///     tag: Some("work"),
///     min_priority: Some(Priority::Medium),
///     ..TaskFilter::default()
/// };
/// assert!(!filter.include_done);
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct TaskFilter<'a> {
    pub tag: Option<&'a str>,
    pub min_priority: Option<Priority>,
    pub search: Option<&'a str>,
    pub include_done: bool,
}

impl TaskFilter<'_> {
    pub fn matches(&self, task: &Task) -> bool {
        let wanted_status = self.include_done || !task.is_done();
        let wanted_priority = self.min_priority.is_none_or(|min| task.priority >= min);
        let wanted_tag = self.tag.is_none_or(|tag| {
            task.tags
                .iter()
                .any(|own| own.eq_ignore_ascii_case(tag.trim_start_matches('#')))
        });
        let wanted_text = self
            .search
            .is_none_or(|text| task.title.to_lowercase().contains(&text.to_lowercase()));

        wanted_status && wanted_priority && wanted_tag && wanted_text
    }
}

/// What happened when a task was completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub completed: Task,
    /// The follow-up task, if the completed one was recurring.
    pub next: Option<Task>,
}

/// All tasks plus the counter used to hand out ids.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    next_id: u32,
    tasks: Vec<Task>,
}

impl Store {
    /// Load from `path`. A missing or empty file is an empty store.
    pub fn load(path: &Path) -> Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Store::default()),
            Err(source) => {
                return Err(TaskError::Io {
                    path: path.to_path_buf(),
                    source,
                })
            }
        };

        if text.trim().is_empty() {
            return Ok(Store::default());
        }

        serde_json::from_str(&text).map_err(|source| TaskError::Json {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Write pretty-printed JSON to `path`, creating parent folders.
    pub fn save(&self, path: &Path) -> Result<()> {
        let io_error = |source| TaskError::Io {
            path: path.to_path_buf(),
            source,
        };

        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(io_error)?;
        }

        let json = serde_json::to_string_pretty(self).map_err(|source| TaskError::Json {
            path: path.to_path_buf(),
            source,
        })?;
        fs::write(path, json + "\n").map_err(io_error)
    }

    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    pub fn get(&self, id: u32) -> Option<&Task> {
        self.tasks.iter().find(|task| task.id == id)
    }

    fn allocate_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    fn index_of(&self, id: u32) -> Result<usize> {
        self.tasks
            .iter()
            .position(|task| task.id == id)
            .ok_or(TaskError::NotFound(id))
    }

    /// Validate `new`, store it and return the created task.
    pub fn add(&mut self, new: NewTask, now: DateTime<Utc>) -> Result<Task> {
        let title = new.title.trim().to_string();
        if title.is_empty() {
            return Err(TaskError::EmptyTitle);
        }

        // Normalise tags: trim, drop '#', lowercase, remove blanks and repeats.
        let mut tags: Vec<String> = new
            .tags
            .iter()
            .map(|tag| tag.trim().trim_start_matches('#').to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        tags.sort();
        tags.dedup();

        let task = Task {
            id: self.allocate_id(),
            title,
            due: new.due,
            priority: new.priority,
            recurrence: new.recurrence,
            tags,
            created_at: now,
            completed_at: None,
        };
        self.tasks.push(task.clone());
        Ok(task)
    }

    /// Mark a task done. Recurring tasks spawn their next occurrence.
    pub fn complete(&mut self, id: u32, now: DateTime<Utc>) -> Result<Completion> {
        let index = self.index_of(id)?;
        if self.tasks[index].is_done() {
            return Err(TaskError::AlreadyDone(id));
        }

        self.tasks[index].completed_at = Some(now);
        let completed = self.tasks[index].clone();

        let next = completed.recurrence.map(|rule| Task {
            id: self.allocate_id(),
            due: rule.next_date(completed.due),
            created_at: now,
            completed_at: None,
            // Struct update syntax: everything else is copied over.
            ..completed.clone()
        });
        if let Some(task) = &next {
            self.tasks.push(task.clone());
        }

        Ok(Completion { completed, next })
    }

    pub fn remove(&mut self, id: u32) -> Result<Task> {
        let index = self.index_of(id)?;
        Ok(self.tasks.remove(index))
    }

    /// Change a task's due date.
    pub fn reschedule(&mut self, id: u32, due: NaiveDate) -> Result<&Task> {
        let task = self
            .tasks
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or(TaskError::NotFound(id))?;
        task.due = due;
        Ok(task)
    }

    /// Tasks matching `filter`, soonest first (pending before done, then
    /// by due date, then highest priority first, then id).
    ///
    /// Lifetime elision ties the returned references to `&self` only.
    pub fn select(&self, filter: &TaskFilter<'_>) -> Vec<&Task> {
        let mut selected: Vec<&Task> = self
            .tasks
            .iter()
            .filter(|task| filter.matches(task))
            .collect();
        selected.sort_by_key(|task| (task.is_done(), task.due, Reverse(task.priority), task.id));
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Recurrence;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn filled() -> Store {
        let now = Utc::now();
        let mut store = Store::default();
        store
            .add(
                NewTask::new("Write report", date(2026, 11, 14))
                    .priority(Priority::High)
                    .tags(["work"]),
                now,
            )
            .unwrap();
        store
            .add(
                NewTask::new("Buy milk", date(2026, 11, 12)).tags(["home", "Home", "#shop"]),
                now,
            )
            .unwrap();
        store
            .add(
                NewTask::new("Call mom", date(2026, 11, 12)).priority(Priority::High),
                now,
            )
            .unwrap();
        store
    }

    #[test]
    fn ids_increase_and_tags_are_normalised() {
        let store = filled();
        let ids: Vec<u32> = store.tasks().iter().map(|t| t.id).collect();
        assert_eq!(ids, [1, 2, 3]);
        assert_eq!(store.get(2).unwrap().tags, ["home", "shop"]);
    }

    #[test]
    fn rejects_blank_titles() {
        let mut store = Store::default();
        let err = store
            .add(NewTask::new("   ", date(2026, 1, 1)), Utc::now())
            .unwrap_err();
        assert!(matches!(err, TaskError::EmptyTitle));
        assert!(store.tasks().is_empty());
    }

    #[test]
    fn select_sorts_by_due_then_priority() {
        let store = filled();
        let titles: Vec<&str> = store
            .select(&TaskFilter::default())
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(titles, ["Call mom", "Buy milk", "Write report"]);
    }

    #[test]
    fn filters_combine() {
        let store = filled();

        let work = TaskFilter {
            tag: Some("#WORK"),
            ..TaskFilter::default()
        };
        assert_eq!(store.select(&work).len(), 1);

        let important = TaskFilter {
            min_priority: Some(Priority::High),
            ..TaskFilter::default()
        };
        assert_eq!(store.select(&important).len(), 2);

        let search = TaskFilter {
            search: Some("MILK"),
            ..TaskFilter::default()
        };
        assert_eq!(store.select(&search)[0].title, "Buy milk");
    }

    #[test]
    fn complete_hides_task_unless_all() {
        let mut store = filled();
        let done = store.complete(1, Utc::now()).unwrap();
        assert!(done.completed.is_done());
        assert!(done.next.is_none());

        assert_eq!(store.select(&TaskFilter::default()).len(), 2);
        let all = TaskFilter {
            include_done: true,
            ..TaskFilter::default()
        };
        assert_eq!(store.select(&all).len(), 3);
    }

    #[test]
    fn completing_twice_fails() {
        let mut store = filled();
        store.complete(1, Utc::now()).unwrap();
        assert!(matches!(
            store.complete(1, Utc::now()),
            Err(TaskError::AlreadyDone(1))
        ));
    }

    #[test]
    fn recurring_task_spawns_next() {
        let mut store = Store::default();
        let new = NewTask::new("Water plants", date(2026, 11, 11))
            .recurrence(Some(Recurrence::Weekly))
            .tags(["home"]);
        store.add(new, Utc::now()).unwrap();

        let done = store.complete(1, Utc::now()).unwrap();
        let next = done.next.expect("recurring task should repeat");
        assert_eq!(next.id, 2);
        assert_eq!(next.due, date(2026, 11, 18));
        assert_eq!(next.title, "Water plants");
        assert_eq!(next.tags, ["home"]);
        assert!(!next.is_done());
        assert_eq!(store.tasks().len(), 2);
    }

    #[test]
    fn remove_and_reschedule() {
        let mut store = filled();
        let moved = store.reschedule(1, date(2026, 12, 1)).unwrap();
        assert_eq!(moved.due, date(2026, 12, 1));
        assert!(matches!(
            store.reschedule(99, date(2026, 12, 1)),
            Err(TaskError::NotFound(99))
        ));

        assert_eq!(store.remove(2).unwrap().title, "Buy milk");
        assert!(store.get(2).is_none());
        assert!(matches!(store.remove(2), Err(TaskError::NotFound(2))));
    }

    #[test]
    fn ids_are_never_reused() {
        let mut store = filled();
        store.remove(3).unwrap();
        let task = store
            .add(NewTask::new("New", date(2026, 11, 20)), Utc::now())
            .unwrap();
        assert_eq!(task.id, 4);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("tasks.json");

        let store = filled();
        store.save(&path).unwrap();
        let loaded = Store::load(&path).unwrap();

        assert_eq!(loaded.tasks(), store.tasks());
        // The id counter survives too.
        let mut loaded = loaded;
        let next = loaded
            .add(NewTask::new("Another", date(2026, 11, 20)), Utc::now())
            .unwrap();
        assert_eq!(next.id, 4);
    }

    #[test]
    fn missing_and_empty_files_are_empty_stores() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.json");
        assert!(Store::load(&missing).unwrap().tasks().is_empty());

        let empty = dir.path().join("empty.json");
        fs::write(&empty, "  \n").unwrap();
        assert!(Store::load(&empty).unwrap().tasks().is_empty());
    }

    #[test]
    fn corrupt_file_is_a_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, "{ not json").unwrap();
        let err = Store::load(&path).unwrap_err();
        assert!(matches!(err, TaskError::Json { .. }));
        assert!(err.to_string().contains("bad.json"));
    }
}
