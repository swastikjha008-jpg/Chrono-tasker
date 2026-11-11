//! # chrono-tasker
//!
//! The library behind the `chrono-tasker` command-line deadline tracker.
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`task`] | `Task`, `Priority`, `Recurrence`, `Urgency` and the `Summarize` trait |
//! | [`dateparse`] | parse `tomorrow`, `+2w`, `fri`, `2026-12-25` into dates |
//! | [`store`] | the task list, filters and JSON persistence |
//! | [`report`] | statistics and the weekly agenda |
//! | [`countdown`] | time remaining until a date, generic over time zones |
//! | [`error`] | the crate-wide error type |
//!
//! ```
//! use chrono::{NaiveDate, Utc};
//! use chrono_tasker::dateparse::parse_due;
//! use chrono_tasker::store::{Store, TaskFilter};
//! use chrono_tasker::task::{NewTask, Priority};
//!
//! let today = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
//! let mut store = Store::default();
//!
//! let due = parse_due("fri", today).unwrap();
//! store
//!     .add(NewTask::new("Ship v1", due).priority(Priority::High), Utc::now())
//!     .unwrap();
//!
//! assert_eq!(store.select(&TaskFilter::default()).len(), 1);
//! ```

pub mod countdown;
pub mod dateparse;
pub mod error;
pub mod report;
pub mod store;
pub mod task;

pub use error::{Result, TaskError};
