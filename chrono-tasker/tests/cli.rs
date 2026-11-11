//! End-to-end tests: run the real binary against a temporary task file.
//!
//! "Today" is pinned to Wednesday 2026-11-11 with `--today`, so results do
//! not depend on the real clock.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

struct Sandbox {
    _dir: TempDir,
    file: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("tasks.json");
        Sandbox { _dir: dir, file }
    }

    fn run(&self, args: &[&str]) -> Output {
        run_with_file(&self.file, args)
    }

    /// Run and assert success, returning stdout.
    fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "`{}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
}

fn run_with_file(file: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_chrono-tasker"))
        .args(["--file", file.to_str().expect("utf-8 path")])
        .args(["--today", "2026-11-11"])
        .args(args)
        .env("NO_COLOR", "1")
        .env_remove("CHRONO_TASKER_FILE")
        .env_remove("CHRONO_TASKER_TODAY")
        .output()
        .expect("failed to run binary")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn shows_help_and_version() {
    let sandbox = Sandbox::new();
    let help = sandbox.ok(&["--help"]);
    assert!(help.contains("Add a task"));
    assert!(help.contains("countdown") || help.contains("Countdown"));
    assert!(sandbox.ok(&["--version"]).starts_with("chrono-tasker "));
}

#[test]
fn add_then_list() {
    let sandbox = Sandbox::new();
    let added = sandbox.ok(&[
        "add",
        "Write report",
        "--due",
        "fri",
        "-p",
        "high",
        "-t",
        "work",
    ]);
    assert!(added.contains("#1 Write report [high] due Fri 13 Nov 2026"));

    let list = sandbox.ok(&["list"]);
    assert!(list.contains("Write report"));
    assert!(list.contains("in 2d"));
    assert!(list.contains("#work"));
    assert!(list.contains("1 task(s)"));
}

#[test]
fn empty_list_is_friendly() {
    let sandbox = Sandbox::new();
    assert!(sandbox.ok(&["list"]).contains("all caught up"));
    assert!(sandbox.ok(&["agenda"]).contains("agenda is empty"));
}

#[test]
fn overdue_tasks_are_flagged() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["add", "Pay rent", "--due", "2026-11-08"]);
    assert!(sandbox.ok(&["list"]).contains("3d overdue"));
    assert!(sandbox.ok(&["agenda"]).contains("Overdue"));
}

#[test]
fn done_hides_task_and_all_shows_it() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["add", "Buy milk", "-d", "tomorrow"]);
    assert!(sandbox.ok(&["done", "1"]).contains("Done"));

    assert!(sandbox.ok(&["list"]).contains("all caught up"));
    let all = sandbox.ok(&["list", "--all"]);
    assert!(all.contains("Buy milk"));
    assert!(all.contains("done"));
}

#[test]
fn recurring_task_schedules_next_round() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["add", "Water plants", "-d", "today", "-r", "weekly"]);

    let done = sandbox.ok(&["done", "1"]);
    assert!(done.contains("Next round scheduled"));
    assert!(done.contains("#2 Water plants"));
    assert!(done.contains("Wed 18 Nov 2026"));

    let list = sandbox.ok(&["list", "--plain"]);
    assert_eq!(
        list.trim(),
        "• #2 Water plants [medium] due Wed 18 Nov 2026"
    );
}

#[test]
fn move_and_remove() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["add", "Dentist", "-d", "+1w"]);

    let moved = sandbox.ok(&["move", "1", "2026-12-24"]);
    assert!(moved.contains("Thu 24 Dec 2026"));

    assert!(sandbox.ok(&["remove", "1"]).contains("Removed"));
    assert!(sandbox.ok(&["list"]).contains("all caught up"));
}

#[test]
fn filters_work_from_the_command_line() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["add", "Write report", "-p", "high", "-t", "work"]);
    sandbox.ok(&["add", "Buy milk", "-p", "low", "-t", "home"]);
    sandbox.ok(&["add", "Call mom", "-p", "medium"]);

    let work = sandbox.ok(&["list", "--plain", "--tag", "work"]);
    assert!(work.contains("Write report") && !work.contains("Buy milk"));

    let important = sandbox.ok(&["list", "--plain", "-p", "medium"]);
    assert!(important.contains("Write report") && important.contains("Call mom"));
    assert!(!important.contains("Buy milk"));

    let search = sandbox.ok(&["list", "--plain", "--search", "MILK"]);
    assert_eq!(search.lines().count(), 1);
}

#[test]
fn stats_and_agenda() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["add", "Old thing", "-d", "2026-11-01", "-t", "work"]);
    sandbox.ok(&["add", "Today thing", "-d", "today", "-p", "high"]);
    sandbox.ok(&["add", "Next week thing", "-d", "+7d"]);
    sandbox.ok(&["add", "Finished thing"]);
    sandbox.ok(&["done", "4"]);

    let stats = sandbox.ok(&["stats"]);
    assert!(stats.contains("4 total"));
    assert!(stats.contains("1 done"));
    assert!(stats.contains("3 pending"));
    assert!(stats.contains("1 overdue"));
    assert!(stats.contains("#work"));

    let agenda = sandbox.ok(&["agenda"]);
    assert!(agenda.contains("Overdue"));
    assert!(agenda.contains("This week"));
    assert!(agenda.contains("Next week"));
    assert!(!agenda.contains("Finished thing"));
}

#[test]
fn countdown_works_in_both_directions() {
    let sandbox = Sandbox::new();
    assert!(sandbox.ok(&["countdown", "2000-01-01"]).contains("ago"));
    assert!(sandbox.ok(&["countdown", "2999-01-01"]).contains("in "));
}

#[test]
fn errors_exit_non_zero_with_a_helpful_message() {
    let sandbox = Sandbox::new();

    let bad_date = sandbox.run(&["add", "x", "--due", "someday"]);
    assert!(!bad_date.status.success());
    assert!(stderr(&bad_date).contains("could not understand the date"));

    let missing = sandbox.run(&["done", "42"]);
    assert!(!missing.status.success());
    assert!(stderr(&missing).contains("no task with id 42"));

    let blank = sandbox.run(&["add", "   "]);
    assert!(!blank.status.success());
    assert!(stderr(&blank).contains("cannot be empty"));

    let bad_priority = sandbox.run(&["add", "x", "-p", "urgent"]);
    assert!(!bad_priority.status.success());
}

#[test]
fn corrupt_file_reports_the_path() {
    let sandbox = Sandbox::new();
    std::fs::write(&sandbox.file, "{ oops").expect("write");
    let out = sandbox.run(&["list"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("tasks.json"));
}

#[test]
fn tasks_persist_as_readable_json() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["add", "Persist me", "-t", "Demo", "-t", "#demo"]);
    let json = std::fs::read_to_string(&sandbox.file).expect("file written");
    assert!(json.contains("\"title\": \"Persist me\""));
    assert!(json.contains("\"due\": \"2026-11-11\""));
    // Tags are normalised and de-duplicated.
    assert_eq!(json.matches("\"demo\"").count(), 1);
}
