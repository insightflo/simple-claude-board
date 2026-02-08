//! TASKS.md parser
//!
//! Parses TASKS.md format into structured Phase/Task data.
//! Supports statuses: [x], [ ], [InProgress], [Failed], [Blocked]

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, space0},
    combinator::map,
    sequence::delimited,
    IResult,
};

/// Task status parsed from TASKS.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Blocked,
}

/// A single task parsed from TASKS.md
#[derive(Debug, Clone)]
pub struct ParsedTask {
    pub id: String,
    pub name: String,
    pub status: TaskStatus,
    pub agent: Option<String>,
    pub blocked_by: Vec<String>,
}

/// A phase containing multiple tasks
#[derive(Debug, Clone)]
pub struct ParsedPhase {
    pub id: String,
    pub name: String,
    pub tasks: Vec<ParsedTask>,
}

impl ParsedPhase {
    /// Calculate progress as completed / total
    pub fn progress(&self) -> f32 {
        if self.tasks.is_empty() {
            return 0.0;
        }
        let completed = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count();
        completed as f32 / self.tasks.len() as f32
    }
}

/// Parse a task status tag like [x], [ ], [InProgress], etc.
fn parse_status(input: &str) -> IResult<&str, TaskStatus> {
    delimited(
        char('['),
        alt((
            map(tag("x"), |_| TaskStatus::Completed),
            map(tag("InProgress"), |_| TaskStatus::InProgress),
            map(tag("Failed"), |_| TaskStatus::Failed),
            map(tag("Blocked"), |_| TaskStatus::Blocked),
            map(tag("/"), |_| TaskStatus::InProgress),
            map(space0, |_| TaskStatus::Pending),
        )),
        char(']'),
    )(input)
}

/// Extract @agent-name from task body text
fn extract_agent(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(pos) = trimmed.find('@') {
            let agent_start = pos + 1;
            let agent_end = trimmed[agent_start..]
                .find(|c: char| c.is_whitespace() || c == ',' || c == '\n')
                .map(|i| agent_start + i)
                .unwrap_or(trimmed.len());
            let agent = &trimmed[agent_start..agent_end];
            if !agent.is_empty() {
                return Some(agent.to_string());
            }
        }
    }
    None
}

/// Extract blocked_by task IDs from task body text
/// Supports both `blocked_by:` and `**blocked_by**:` (markdown bold) formats
fn extract_blocked_by(body: &str) -> Vec<String> {
    let mut blocked = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        let stripped = trimmed.replace("**", "");
        if let Some(pos) = stripped.find("blocked_by:") {
            let rest = stripped[pos + "blocked_by:".len()..].trim();
            for part in rest.split(',') {
                let dep = part.trim().to_string();
                if !dep.is_empty() {
                    blocked.push(dep);
                }
            }
        }
    }
    blocked
}

/// Parse the entire TASKS.md content into phases
pub fn parse_tasks_md(input: &str) -> Result<Vec<ParsedPhase>, String> {
    let mut phases = Vec::new();
    let mut current_phase: Option<ParsedPhase> = None;
    let mut current_task_body = String::new();
    let mut pending_task: Option<(String, String, TaskStatus)> = None;

    for line in input.lines() {
        let trimmed = line.trim();

        // Phase heading: "# Phase N: Name" (H1) or "## Phase N: Name" (H2)
        let phase_header = if trimmed.starts_with("# ") && !trimmed.starts_with("## ") {
            Some(&trimmed[2..])
        } else if trimmed.starts_with("## ") && !trimmed.starts_with("### ") {
            Some(&trimmed[3..])
        } else {
            None
        };

        if let Some(header) = phase_header {
            if let Some(phase) = parse_phase_header(header) {
                flush_task(
                    &mut pending_task,
                    &mut current_task_body,
                    &mut current_phase,
                );
                if let Some(prev) = current_phase.take() {
                    phases.push(prev);
                }
                current_phase = Some(phase);
                continue;
            }
        }

        // H3 heading with status: ### [status] Task-ID: Name
        if let Some(rest) = trimmed.strip_prefix("### ") {
            flush_task(
                &mut pending_task,
                &mut current_task_body,
                &mut current_phase,
            );

            if let Ok((remaining, status)) = parse_status(rest) {
                let remaining = remaining.trim();
                let (id, name) = if let Some(colon_pos) = remaining.find(':') {
                    let id = remaining[..colon_pos].trim().to_string();
                    let name = remaining[colon_pos + 1..].trim().to_string();
                    (id, name)
                } else {
                    (remaining.to_string(), remaining.to_string())
                };
                pending_task = Some((id, name, status));
            }
            continue;
        }

        // Accumulate body lines for current task
        if pending_task.is_some() {
            current_task_body.push_str(line);
            current_task_body.push('\n');
        }
    }

    // Flush remaining
    flush_task(
        &mut pending_task,
        &mut current_task_body,
        &mut current_phase,
    );
    if let Some(phase) = current_phase.take() {
        phases.push(phase);
    }

    Ok(phases)
}

/// Helper to flush a pending task into its phase
fn flush_task(
    pending_task: &mut Option<(String, String, TaskStatus)>,
    body: &mut String,
    phase: &mut Option<ParsedPhase>,
) {
    if let Some((id, name, status)) = pending_task.take() {
        if let Some(ref mut p) = phase {
            let agent = extract_agent(body);
            let blocked_by = extract_blocked_by(body);
            p.tasks.push(ParsedTask {
                id,
                name,
                status,
                agent,
                blocked_by,
            });
        }
        body.clear();
    }
}

/// Parse phase header text like "Phase 0: Setup"
fn parse_phase_header(header: &str) -> Option<ParsedPhase> {
    let header = header.trim();
    if !header.starts_with("Phase") {
        return None;
    }

    let colon_pos = header.find(':')?;
    let id_part = header[..colon_pos].trim();
    let name_part = header[colon_pos + 1..].trim();

    let phase_num = id_part.strip_prefix("Phase")?.trim();
    let id = format!("P{phase_num}");

    Some(ParsedPhase {
        id,
        name: name_part.to_string(),
        tasks: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_completed() {
        let (_, s) = parse_status("[x]").unwrap();
        assert_eq!(s, TaskStatus::Completed);
    }

    #[test]
    fn status_pending() {
        let (_, s) = parse_status("[ ]").unwrap();
        assert_eq!(s, TaskStatus::Pending);
    }

    #[test]
    fn status_in_progress() {
        let (_, s) = parse_status("[InProgress]").unwrap();
        assert_eq!(s, TaskStatus::InProgress);
    }

    #[test]
    fn status_failed() {
        let (_, s) = parse_status("[Failed]").unwrap();
        assert_eq!(s, TaskStatus::Failed);
    }

    #[test]
    fn status_blocked() {
        let (_, s) = parse_status("[Blocked]").unwrap();
        assert_eq!(s, TaskStatus::Blocked);
    }

    #[test]
    fn status_slash_is_in_progress() {
        let (_, s) = parse_status("[/]").unwrap();
        assert_eq!(s, TaskStatus::InProgress);
    }

    #[test]
    fn agent_extraction() {
        let body = "- **담당**: @backend-specialist\n- **스펙**: something";
        assert_eq!(extract_agent(body), Some("backend-specialist".to_string()));
    }

    #[test]
    fn agent_extraction_none() {
        assert_eq!(extract_agent("no agent here"), None);
    }

    #[test]
    fn blocked_by_single() {
        let body = "- **blocked_by**: P0-T0.1\n";
        assert_eq!(extract_blocked_by(body), vec!["P0-T0.1"]);
    }

    #[test]
    fn blocked_by_multiple() {
        let body = "- **blocked_by**: P1-R1-T1, P1-R2-T1\n";
        assert_eq!(extract_blocked_by(body), vec!["P1-R1-T1", "P1-R2-T1"]);
    }

    #[test]
    fn blocked_by_none() {
        assert!(extract_blocked_by("no deps here").is_empty());
    }

    #[test]
    fn phase_header_basic() {
        let p = parse_phase_header("Phase 0: Setup").unwrap();
        assert_eq!(p.id, "P0");
        assert_eq!(p.name, "Setup");
    }

    #[test]
    fn phase_header_with_parens() {
        let p = parse_phase_header("Phase 1: Data Engine (리소스 모듈)").unwrap();
        assert_eq!(p.id, "P1");
        assert_eq!(p.name, "Data Engine (리소스 모듈)");
    }

    #[test]
    fn phase_header_non_phase() {
        assert!(parse_phase_header("Not a phase").is_none());
    }

    #[test]
    fn empty_input() {
        let result = parse_tasks_md("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn sample_tasks_phases() {
        let input = include_str!("../../tests/fixtures/sample_tasks.md");
        let phases = parse_tasks_md(input).unwrap();
        assert_eq!(phases.len(), 3);

        assert_eq!(phases[0].id, "P0");
        assert_eq!(phases[0].name, "Setup");
        assert_eq!(phases[0].tasks.len(), 2);
        assert_eq!(phases[0].tasks[0].status, TaskStatus::Completed);

        assert_eq!(phases[1].id, "P1");
        assert_eq!(phases[1].tasks.len(), 3);
        assert_eq!(phases[1].tasks[0].status, TaskStatus::InProgress);
        assert_eq!(phases[1].tasks[1].status, TaskStatus::Pending);
        assert_eq!(phases[1].tasks[2].status, TaskStatus::Failed);

        assert_eq!(phases[2].id, "P2");
        assert_eq!(phases[2].tasks.len(), 3);
        assert_eq!(phases[2].tasks[0].status, TaskStatus::Blocked);
    }

    #[test]
    fn sample_tasks_agents() {
        let input = include_str!("../../tests/fixtures/sample_tasks.md");
        let phases = parse_tasks_md(input).unwrap();
        assert_eq!(
            phases[0].tasks[0].agent,
            Some("backend-specialist".to_string())
        );
        assert_eq!(
            phases[2].tasks[1].agent,
            Some("test-specialist".to_string())
        );
    }

    #[test]
    fn sample_tasks_blocked_by() {
        let input = include_str!("../../tests/fixtures/sample_tasks.md");
        let phases = parse_tasks_md(input).unwrap();
        assert_eq!(phases[1].tasks[0].blocked_by, vec!["P0-T0.1"]);
        assert_eq!(phases[2].tasks[0].blocked_by, vec!["P1-R4-T1"]);
    }

    #[test]
    fn phase_progress_calculation() {
        let input = include_str!("../../tests/fixtures/sample_tasks.md");
        let phases = parse_tasks_md(input).unwrap();
        assert!((phases[0].progress() - 1.0).abs() < f32::EPSILON);
        assert!((phases[1].progress() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn h2_phase_headers() {
        let input = "## Phase 0: 프로젝트 셋업\n\n### [x] P0-T1: 설계 문서 완료\n- **담당**: @orchestrator\n\n---\n\n## Phase 1: 에이전트 정의\n\n### [x] P1-T1: 에이전트 생성\n- **담당**: @backend-specialist\n";
        let phases = parse_tasks_md(input).unwrap();
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].id, "P0");
        assert_eq!(phases[0].name, "프로젝트 셋업");
        assert_eq!(phases[0].tasks.len(), 1);
        assert_eq!(phases[1].id, "P1");
        assert_eq!(phases[1].tasks.len(), 1);
    }

    #[test]
    fn h2_non_phase_heading_ignored() {
        let input = "# Phase 0: Setup\n\n## P0-T0.1: Subheading\n\n### [x] P0-T0.1: Task\n";
        let phases = parse_tasks_md(input).unwrap();
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].tasks.len(), 1);
    }

    #[test]
    fn partial_content_still_parses() {
        let input = "# Phase 0: Setup\n\n### [x] T1: Done\n\ngarbage\n\n### [ ] T2: Pending\n";
        let phases = parse_tasks_md(input).unwrap();
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].tasks.len(), 2);
    }
}
