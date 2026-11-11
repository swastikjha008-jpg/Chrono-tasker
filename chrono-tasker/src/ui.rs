//! Terminal output: tables, colours and bars. Kept out of the library so the
//! core logic stays free of presentation details.

use chrono::NaiveDate;
use chrono_tasker::report::{Section, Stats};
use chrono_tasker::task::{Priority, Task, Urgency};
use colored::{ColoredString, Colorize};

const TITLE_WIDTH: usize = 30;
const BAR_WIDTH: usize = 20;

pub fn success(label: &str, message: &str) {
    println!("{} {message}", label.green().bold());
}

pub fn info(message: &str) {
    println!("{}", message.dimmed());
}

pub fn task_table(tasks: &[&Task], today: NaiveDate) {
    if tasks.is_empty() {
        println!("{}", "Nothing to show. You're all caught up!".green());
        return;
    }

    let header = format!(
        "{:>3}  {:<6}  {:<15}  {:<11}  {:<7}  {:<TITLE_WIDTH$}  {}",
        "ID", "PRI", "DUE", "STATUS", "REPEAT", "TITLE", "TAGS"
    );
    println!("{}", header.bold().underline());

    for task in tasks {
        println!("{}", task_row(task, today));
    }
    println!("{}", format!("\n{} task(s)", tasks.len()).dimmed());
}

fn task_row(task: &Task, today: NaiveDate) -> String {
    let urgency = task.urgency(today);
    let due = task.due.format("%a %d %b %Y").to_string();
    let repeat = task
        .recurrence
        .map_or_else(|| "-".to_string(), |rule| rule.to_string());

    format!(
        "{:>3}  {}  {}  {}  {:<7}  {}  {}",
        task.id,
        paint_priority(&format!("{:<6}", task.priority), task.priority),
        paint_urgency(&format!("{due:<15}"), urgency),
        paint_urgency(&format!("{:<11}", urgency.to_string()), urgency),
        repeat,
        paint_title(
            &format!("{:<TITLE_WIDTH$}", truncate(&task.title, TITLE_WIDTH)),
            urgency
        ),
        tag_list(&task.tags).cyan(),
    )
}

pub fn agenda(sections: &[Section<'_>], today: NaiveDate) {
    if sections.is_empty() {
        println!("{}", "Your agenda is empty.".green());
        return;
    }

    for section in sections {
        let heading = if section.title == "Overdue" {
            section.title.red().bold()
        } else {
            section.title.bold()
        };
        println!("{heading}");

        for task in &section.tasks {
            let urgency = task.urgency(today);
            println!(
                "  {} {}  {}  {}",
                paint_urgency("•", urgency),
                paint_urgency(&format!("{:<10}", task.due.format("%a %d %b")), urgency),
                paint_title(&truncate(&task.title, 40), urgency),
                format!("(#{}, {})", task.id, task.priority).dimmed(),
            );
        }
        println!();
    }
}

pub fn stats(stats: &Stats) {
    println!("{}", "Overview".bold().underline());
    println!(
        "  {} total   {} done   {} pending",
        stats.total.to_string().bold(),
        stats.done.to_string().green().bold(),
        stats.pending.to_string().yellow().bold(),
    );
    println!(
        "  {}  {:.0}%",
        progress_bar(stats.completion_rate()),
        stats.completion_rate()
    );

    println!("\n{}", "Deadlines".bold().underline());
    println!(
        "  {} overdue   {} due today   {} in the next 7 days",
        stats.overdue.to_string().red().bold(),
        stats.due_today.to_string().yellow().bold(),
        stats.due_next_7_days.to_string().bold(),
    );

    println!("\n{}", "Pending by priority".bold().underline());
    // Iterate the constant so priorities with zero tasks still show up.
    for priority in Priority::ALL.iter().rev() {
        let count = stats.by_priority.get(priority).copied().unwrap_or(0);
        println!(
            "  {} {} {count}",
            paint_priority(&format!("{priority:<6}"), *priority),
            "▪".repeat(count).dimmed(),
        );
    }

    if !stats.by_tag.is_empty() {
        println!("\n{}", "Pending by tag".bold().underline());
        for (tag, count) in &stats.by_tag {
            println!("  {} {count}", format!("#{tag:<12}").cyan());
        }
    }
}

fn progress_bar(percent: f64) -> String {
    let filled = ((percent / 100.0) * BAR_WIDTH as f64).round() as usize;
    let filled = filled.min(BAR_WIDTH);
    format!(
        "{}{}",
        "█".repeat(filled).green(),
        "░".repeat(BAR_WIDTH - filled).dimmed()
    )
}

/// Slice patterns: match on the *shape* of the tag list.
fn tag_list(tags: &[String]) -> String {
    match tags {
        [] => String::new(),
        [only] => format!("#{only}"),
        [first, rest @ ..] => format!("#{first} +{}", rest.len()),
    }
}

/// Shorten `text` to `max` characters (not bytes), adding `…` if cut.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let kept: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{kept}…")
    }
}

fn paint_urgency(text: &str, urgency: Urgency) -> ColoredString {
    match urgency {
        Urgency::Done => text.dimmed(),
        Urgency::Overdue(_) => text.red().bold(),
        Urgency::Today => text.yellow().bold(),
        Urgency::Soon(_) => text.yellow(),
        Urgency::Later(_) => text.normal(),
    }
}

fn paint_title(text: &str, urgency: Urgency) -> ColoredString {
    match urgency {
        Urgency::Done => text.strikethrough().dimmed(),
        _ => text.normal(),
    }
}

fn paint_priority(text: &str, priority: Priority) -> ColoredString {
    match priority {
        Priority::High => text.red().bold(),
        Priority::Medium => text.yellow(),
        Priority::Low => text.blue(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_list_uses_slice_patterns() {
        assert_eq!(tag_list(&[]), "");
        assert_eq!(tag_list(&["work".to_string()]), "#work");
        let many = ["a", "b", "c"].map(String::from);
        assert_eq!(tag_list(&many), "#a +2");
    }

    #[test]
    fn truncate_counts_characters() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("ééééé", 3), "éé…");
    }

    #[test]
    fn progress_bar_never_overflows() {
        colored::control::set_override(false);
        assert_eq!(progress_bar(0.0).chars().count(), BAR_WIDTH);
        assert_eq!(progress_bar(100.0), "█".repeat(BAR_WIDTH));
        assert_eq!(progress_bar(250.0).chars().count(), BAR_WIDTH);
    }
}
