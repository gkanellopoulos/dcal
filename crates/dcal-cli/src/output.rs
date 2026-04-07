use chrono::{DateTime, Utc};
use colored::Colorize;

use dcal_core::project::{ProjectPhase, ProjectStatus, RegistryEntry};

/// Format a relative time string from a timestamp (e.g. "3 days ago").
pub fn relative_time(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    let minutes = duration.num_minutes();
    let hours = duration.num_hours();
    let days = duration.num_days();

    if minutes < 1 {
        "just now".to_string()
    } else if minutes < 60 {
        format!("{minutes} min ago")
    } else if hours < 24 {
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{hours} hours ago")
        }
    } else if days == 1 {
        "1 day ago".to_string()
    } else {
        format!("{days} days ago")
    }
}

/// Colorize a project status string.
#[allow(dead_code)]
pub fn colorize_status(status: ProjectStatus) -> String {
    match status {
        ProjectStatus::Active => "active".green().to_string(),
        ProjectStatus::Paused => "paused".yellow().to_string(),
        ProjectStatus::Blocked => "blocked".red().to_string(),
        ProjectStatus::Completed => "completed".cyan().to_string(),
        ProjectStatus::Archived => "archived".dimmed().to_string(),
        ProjectStatus::Ideation => "ideation".blue().to_string(),
    }
}

/// Format a phase for display, showing "—" for Unknown.
pub fn format_phase(phase: ProjectPhase) -> String {
    match phase {
        ProjectPhase::Unknown => "—".to_string(),
        other => other.to_string(),
    }
}

/// Render the project list table.
///
/// Entries should already be sorted and filtered before calling this.
#[allow(dead_code)]
pub fn render_table(entries: &[RegistryEntry], phases: &[ProjectPhase]) {
    if entries.is_empty() {
        println!("  No projects found.");
        return;
    }

    let header_name = "NAME";
    let header_status = "STATUS";
    let header_active = "LAST ACTIVE";
    let header_phase = "PHASE";

    let name_width = entries
        .iter()
        .map(|e| e.name.len())
        .max()
        .unwrap_or(0)
        .max(header_name.len());

    let status_width = 12;
    let active_width = 16;

    println!(
        "  {:<name_width$}  {:<status_width$}  {:<active_width$}  {}",
        header_name.bold(),
        header_status.bold(),
        header_active.bold(),
        header_phase.bold(),
    );

    for (entry, phase) in entries.iter().zip(phases.iter()) {
        let status_str = colorize_status(entry.status);
        let active_str = relative_time(entry.last_active_at);
        let phase_str = format_phase(*phase);

        // Colored strings have invisible ANSI codes that affect padding,
        // so we pad the raw text and then replace with the colored version.
        let raw_status = entry.status.to_string();
        let status_padding = status_width.saturating_sub(raw_status.len());

        println!(
            "  {:<name_width$}  {}{}  {:<active_width$}  {}",
            entry.name,
            status_str,
            " ".repeat(status_padding),
            active_str,
            phase_str,
        );
    }
}

/// Parse a stale duration string like "30d" into a number of days.
#[allow(dead_code)]
pub fn parse_stale_days(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix('d') {
        num.parse().ok()
    } else {
        s.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn relative_time_just_now() {
        let now = Utc::now();
        assert_eq!(relative_time(now), "just now");
    }

    #[test]
    fn relative_time_minutes() {
        let t = Utc::now() - Duration::minutes(5);
        assert_eq!(relative_time(t), "5 min ago");
    }

    #[test]
    fn relative_time_one_hour() {
        let t = Utc::now() - Duration::hours(1);
        assert_eq!(relative_time(t), "1 hour ago");
    }

    #[test]
    fn relative_time_hours() {
        let t = Utc::now() - Duration::hours(3);
        assert_eq!(relative_time(t), "3 hours ago");
    }

    #[test]
    fn relative_time_one_day() {
        let t = Utc::now() - Duration::days(1);
        assert_eq!(relative_time(t), "1 day ago");
    }

    #[test]
    fn relative_time_days() {
        let t = Utc::now() - Duration::days(87);
        assert_eq!(relative_time(t), "87 days ago");
    }

    #[test]
    fn format_phase_unknown_is_dash() {
        assert_eq!(format_phase(ProjectPhase::Unknown), "—");
    }

    #[test]
    fn format_phase_normal() {
        assert_eq!(format_phase(ProjectPhase::Implementation), "implementation");
    }

    #[test]
    fn parse_stale_days_with_suffix() {
        assert_eq!(parse_stale_days("30d"), Some(30));
    }

    #[test]
    fn parse_stale_days_bare_number() {
        assert_eq!(parse_stale_days("14"), Some(14));
    }

    #[test]
    fn parse_stale_days_invalid() {
        assert_eq!(parse_stale_days("abc"), None);
    }

    #[test]
    fn parse_stale_days_with_whitespace() {
        assert_eq!(parse_stale_days("  7d  "), Some(7));
    }
}
