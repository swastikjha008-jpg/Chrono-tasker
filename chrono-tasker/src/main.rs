mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::{Local, NaiveDate, Utc};
use chrono_tasker::countdown::{describe, time_until};
use chrono_tasker::dateparse::parse_due;
use chrono_tasker::error::{Result, TaskError};
use chrono_tasker::report::{build_agenda, bullet_list, Stats};
use chrono_tasker::store::{Completion, Store, TaskFilter};
use chrono_tasker::task::{NewTask, Priority, Recurrence, Summarize};
use clap::{Parser, Subcommand};
use colored::Colorize;

/// A colorful command-line deadline tracker.
#[derive(Debug, Parser)]
#[command(name = "chrono-tasker", version, about, arg_required_else_help = true)]
struct Cli {
    /// JSON file that stores your tasks
    #[arg(
        short,
        long,
        global = true,
        env = "CHRONO_TASKER_FILE",
        default_value = "tasks.json"
    )]
    file: PathBuf,

    /// Pretend today is this date (YYYY-MM-DD); handy for testing
    #[arg(long, global = true, env = "CHRONO_TASKER_TODAY", value_name = "DATE")]
    today: Option<NaiveDate>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Add a task
    Add {
        /// What needs doing
        title: String,

        /// Due date: today, tomorrow, +3d, +2w, +1m, fri, or 2026-12-25
        #[arg(short, long, default_value = "today")]
        due: String,

        /// low, medium or high
        #[arg(short, long, default_value = "medium")]
        priority: Priority,

        /// Repeat after completion: daily, weekly or monthly
        #[arg(short, long)]
        repeat: Option<Recurrence>,

        /// Tag (repeat the flag for several)
        #[arg(short, long = "tag")]
        tags: Vec<String>,
    },

    /// List pending tasks, soonest first
    List {
        /// Include completed tasks
        #[arg(short, long)]
        all: bool,

        /// Only tasks with this tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Only tasks at or above this priority
        #[arg(short = 'p', long)]
        min_priority: Option<Priority>,

        /// Only tasks whose title contains this text
        #[arg(short, long)]
        search: Option<String>,

        /// Plain bullet list instead of the table
        #[arg(long)]
        plain: bool,
    },

    /// Mark a task as done (recurring tasks schedule their next round)
    Done { id: u32 },

    /// Delete a task
    Remove { id: u32 },

    /// Change a task's due date
    Move {
        id: u32,
        /// New due date (same formats as `add --due`)
        due: String,
    },

    /// Show pending tasks grouped by week
    Agenda,

    /// Show totals and deadline statistics
    Stats,

    /// Count down to a date
    Countdown {
        /// Target date (same formats as `add --due`)
        date: String,
    },
}

fn main() -> ExitCode {
    #[cfg(windows)]
    let _ = colored::control::set_virtual_terminal(true);

    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{} {error}", "error:".red().bold());
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let now = Utc::now();
    let today = cli.today.unwrap_or_else(|| Local::now().date_naive());
    let file = cli.file.as_path();

    match cli.command {
        Command::Add {
            title,
            due,
            priority,
            repeat,
            tags,
        } => {
            let due = parse_due(&due, today)?;
            let new = NewTask::new(title, due)
                .priority(priority)
                .recurrence(repeat)
                .tags(tags);

            let mut store = Store::load(file)?;
            let task = store.add(new, now)?;
            store.save(file)?;
            ui::success("Added", &task.summary());
        }

        Command::List {
            all,
            tag,
            min_priority,
            search,
            plain,
        } => {
            let store = Store::load(file)?;
            let filter = TaskFilter {
                tag: tag.as_deref(),
                min_priority,
                search: search.as_deref(),
                include_done: all,
            };
            let selected = store.select(&filter);

            if plain {
                println!("{}", bullet_list(selected));
            } else {
                ui::task_table(&selected, today);
            }
        }

        Command::Done { id } => {
            let mut store = Store::load(file)?;
            let Completion { completed, next } = store.complete(id, now)?;
            store.save(file)?;

            ui::success("Done", &completed.summary());
            if let Some(next) = next {
                ui::info(&format!("Next round scheduled: {}", next.summary()));
            }
        }

        Command::Remove { id } => {
            let mut store = Store::load(file)?;
            let removed = store.remove(id)?;
            store.save(file)?;
            ui::success("Removed", &removed.summary());
        }

        Command::Move { id, due } => {
            let due = parse_due(&due, today)?;
            let mut store = Store::load(file)?;
            let summary = store.reschedule(id, due)?.summary();
            store.save(file)?;
            ui::success("Moved", &summary);
        }

        Command::Agenda => {
            let store = Store::load(file)?;
            let selected = store.select(&TaskFilter::default());
            ui::agenda(&build_agenda(&selected, today), today);
        }

        Command::Stats => {
            let store = Store::load(file)?;
            ui::stats(&Stats::compute(store.tasks(), today));
        }

        Command::Countdown { date } => {
            let target = parse_due(&date, today)?;
            let remaining = time_until(target, &Local::now())
                .ok_or_else(|| TaskError::InvalidDate(date.clone()))?;
            println!(
                "{} {} ({})",
                "Countdown".cyan().bold(),
                describe(remaining).bold(),
                target.format("%a %d %b %Y")
            );
        }
    }

    Ok(())
}
