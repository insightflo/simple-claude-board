//! TASKS.md write-back
//!
//! Updates task status in TASKS.md by finding and replacing status tags
//! in task header lines.

use std::path::Path;

/// Replace a task's status in TASKS.md.
///
/// Finds lines matching `### [{old_status}] {task_id}:` and replaces
/// the status tag with `new_status`.
///
/// Uses line-by-line string matching (no regex) for safety.
pub fn update_task_status(path: &Path, task_id: &str, new_status: &str) -> anyhow::Result<bool> {
    let content = std::fs::read_to_string(path)?;
    let mut found = false;
    let mut output = String::with_capacity(content.len());

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("### [") && trimmed.contains(task_id) {
            // Parse: ### [Status] TaskId: ...
            if let Some(bracket_end) = trimmed.find("] ") {
                let after_bracket = &trimmed[bracket_end + 2..];
                // Check that the task_id follows immediately after `] `
                if after_bracket.starts_with(task_id) {
                    let prefix = &line[..line.find('[').unwrap_or(0)];
                    let suffix = &after_bracket;
                    output.push_str(&format!("{prefix}[{new_status}] {suffix}"));
                    output.push('\n');
                    found = true;
                    continue;
                }
            }
        }
        output.push_str(line);
        output.push('\n');
    }

    // Preserve original trailing newline behavior
    if !content.ends_with('\n') {
        output.pop();
    }

    if found {
        std::fs::write(path, &output)?;
    }

    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn update_failed_to_in_progress() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("TASKS.md");
        fs::write(
            &path,
            "# Phase 1: Data Engine\n\n### [Failed] P1-R3-T1: File watcher module\n- **담당**: @backend-specialist\n",
        )
        .unwrap();

        let found = update_task_status(&path, "P1-R3-T1", "InProgress").unwrap();
        assert!(found);

        let result = fs::read_to_string(&path).unwrap();
        assert!(result.contains("### [InProgress] P1-R3-T1: File watcher module"));
        assert!(!result.contains("[Failed]"));
    }

    #[test]
    fn update_blocked_to_in_progress() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("TASKS.md");
        fs::write(&path, "### [Blocked] P2-S1-T1: Gantt chart widget\n").unwrap();

        let found = update_task_status(&path, "P2-S1-T1", "InProgress").unwrap();
        assert!(found);

        let result = fs::read_to_string(&path).unwrap();
        assert!(result.contains("### [InProgress] P2-S1-T1: Gantt chart widget"));
    }

    #[test]
    fn no_match_returns_false() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("TASKS.md");
        fs::write(&path, "# Phase 0\n\n### [x] P0-T1: Init\n").unwrap();

        let found = update_task_status(&path, "NONEXISTENT", "InProgress").unwrap();
        assert!(!found);

        // File should be unchanged
        let result = fs::read_to_string(&path).unwrap();
        assert!(result.contains("[x] P0-T1"));
    }

    #[test]
    fn preserves_other_lines() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("TASKS.md");
        let content = "# Phase 1\n\n### [Failed] T1: First\n- body\n\n### [x] T2: Second\n";
        fs::write(&path, content).unwrap();

        update_task_status(&path, "T1", "InProgress").unwrap();

        let result = fs::read_to_string(&path).unwrap();
        assert!(result.contains("[InProgress] T1: First"));
        assert!(result.contains("[x] T2: Second"));
        assert!(result.contains("- body"));
    }
}
