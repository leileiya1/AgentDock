//! Per-project overview rows for the desktop "town" view.
//!
//! Pure aggregation over task rows so the grouping rules are unit-tested and match the
//! desktop home tabs: attention (waiting for you), active (running), done.

use std::collections::{HashMap, HashSet};

use agentflow_contracts::{AgentKind, AgentsSource, Project, ProjectSummary, TaskStatus};

/// The task fields the overview needs.
#[derive(Debug, Clone)]
pub struct SummaryTask {
    pub id: String,
    pub project_id: String,
    pub status: TaskStatus,
    pub developer: AgentKind,
    pub reviewer: AgentKind,
    /// RFC 3339, as stored in `tasks.updated_at`.
    pub updated_at: String,
}

/// Same set as the desktop "需要你" group.
fn waits_for_human(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::WaitingForHumanApproval
            | TaskStatus::WaitingForPlanApproval
            | TaskStatus::Blocked
            | TaskStatus::MergeConflict
            | TaskStatus::Approved
    )
}

fn is_done(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Merged | TaskStatus::RolledBack | TaskStatus::Cancelled
    )
}

/// Who is actually executing for this status, if anyone (validation and merging run locally).
fn executing_agent(task: &SummaryTask) -> Option<&AgentKind> {
    match task.status {
        TaskStatus::Planning | TaskStatus::Developing | TaskStatus::Revising => {
            Some(&task.developer)
        }
        TaskStatus::Reviewing => Some(&task.reviewer),
        _ => None,
    }
}

fn push_unique(list: &mut Vec<AgentKind>, agent: &AgentKind) {
    if !list.contains(agent) {
        list.push(agent.clone());
    }
}

/// One row per project, in the order of `projects`. Projects without tasks get zero rows.
/// `failing_tasks` holds task ids whose latest recent validation failed; `recent_cutoff` is an
/// RFC 3339 timestamp — only tasks updated at or after it contribute to `agents`.
pub fn summarize_projects(
    projects: &[Project],
    tasks: &[SummaryTask],
    failing_tasks: &HashSet<String>,
    recent_cutoff: &str,
) -> Vec<ProjectSummary> {
    projects
        .iter()
        .map(|project| {
            let mut own: Vec<&SummaryTask> = tasks
                .iter()
                .filter(|t| t.project_id == project.id)
                .collect();
            own.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            let mut summary = ProjectSummary {
                project_id: project.id.clone(),
                task_count: own.len() as i64,
                running: 0,
                waiting_for_you: 0,
                done: 0,
                failing: 0,
                agents: Vec::new(),
                active_agents: Vec::new(),
                last_activity_at: own.first().map(|t| t.updated_at.clone()),
                assigned_agents: Vec::new(),
                agents_source: AgentsSource::Derived,
            };
            for task in &own {
                if waits_for_human(task.status) {
                    summary.waiting_for_you += 1;
                } else if is_done(task.status) {
                    summary.done += 1;
                } else {
                    summary.running += 1;
                }
                if failing_tasks.contains(&task.id) && !is_done(task.status) {
                    summary.failing += 1;
                }
                if task.updated_at.as_str() >= recent_cutoff {
                    push_unique(&mut summary.agents, &task.developer);
                    push_unique(&mut summary.agents, &task.reviewer);
                }
                if let Some(agent) = executing_agent(task) {
                    push_unique(&mut summary.active_agents, agent);
                }
            }
            summary
        })
        .collect()
}

/// Fill in the members the user assigned (`members` maps project id → agents in display order).
/// A project with members switches to `assigned`; the derived `agents` list is left untouched.
pub fn apply_members(summaries: &mut [ProjectSummary], members: &HashMap<String, Vec<AgentKind>>) {
    for summary in summaries {
        match members.get(&summary.project_id) {
            Some(list) if !list.is_empty() => {
                summary.assigned_agents = list.clone();
                summary.agents_source = AgentsSource::Assigned;
            }
            _ => {
                summary.assigned_agents.clear();
                summary.agents_source = AgentsSource::Derived;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str) -> Project {
        Project {
            id: id.into(),
            seq: 1,
            name: id.into(),
            repo_path: format!("/tmp/{id}"),
            default_branch: "main".into(),
            worktree_root: "/tmp/w".into(),
            created_at: "2026-11-01T00:00:00+00:00".into(),
        }
    }

    fn task(
        id: &str,
        project: &str,
        status: TaskStatus,
        dev: AgentKind,
        rev: AgentKind,
        at: &str,
    ) -> SummaryTask {
        SummaryTask {
            id: id.into(),
            project_id: project.into(),
            status,
            developer: dev,
            reviewer: rev,
            updated_at: at.into(),
        }
    }

    #[test]
    fn groups_match_the_desktop_home_tabs() {
        let tasks = vec![
            task(
                "a",
                "p",
                TaskStatus::Developing,
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                "2026-11-06T10:00:00+00:00",
            ),
            task(
                "b",
                "p",
                TaskStatus::ReadyForReview,
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                "2026-11-06T09:00:00+00:00",
            ),
            task(
                "c",
                "p",
                TaskStatus::WaitingForHumanApproval,
                AgentKind::QoderCli,
                AgentKind::Codex,
                "2026-11-06T08:00:00+00:00",
            ),
            task(
                "d",
                "p",
                TaskStatus::Approved,
                AgentKind::QoderCli,
                AgentKind::Codex,
                "2026-11-06T07:00:00+00:00",
            ),
            task(
                "e",
                "p",
                TaskStatus::Merged,
                AgentKind::GeminiCli,
                AgentKind::Codex,
                "2026-11-06T06:00:00+00:00",
            ),
        ];
        let rows = summarize_projects(
            &[project("p")],
            &tasks,
            &HashSet::new(),
            "2026-10-07T00:00:00+00:00",
        );
        let s = &rows[0];
        assert_eq!(
            (s.task_count, s.running, s.waiting_for_you, s.done),
            (5, 2, 2, 1)
        );
        assert_eq!(
            s.last_activity_at.as_deref(),
            Some("2026-11-06T10:00:00+00:00")
        );
        assert_eq!(s.active_agents, vec![AgentKind::ClaudeCode]);
    }

    #[test]
    fn agents_are_recent_distinct_and_most_recent_first() {
        let tasks = vec![
            task(
                "old",
                "p",
                TaskStatus::Merged,
                AgentKind::GrokCli,
                AgentKind::GrokApi,
                "2026-09-01T00:00:00+00:00",
            ),
            task(
                "new",
                "p",
                TaskStatus::Reviewing,
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                "2026-11-06T10:00:00+00:00",
            ),
            task(
                "mid",
                "p",
                TaskStatus::Developing,
                AgentKind::ClaudeCode,
                AgentKind::QwenCode,
                "2026-11-05T10:00:00+00:00",
            ),
        ];
        let s = &summarize_projects(
            &[project("p")],
            &tasks,
            &HashSet::new(),
            "2026-10-07T00:00:00+00:00",
        )[0];
        assert_eq!(
            s.agents,
            vec![AgentKind::ClaudeCode, AgentKind::Codex, AgentKind::QwenCode]
        );
        assert_eq!(
            s.active_agents,
            vec![AgentKind::Codex, AgentKind::ClaudeCode]
        );
    }

    #[test]
    fn failing_counts_only_open_tasks_with_a_recent_failed_validation() {
        let tasks = vec![
            task(
                "f",
                "p",
                TaskStatus::ReadyForRevision,
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                "2026-11-06T10:00:00+00:00",
            ),
            task(
                "m",
                "p",
                TaskStatus::Merged,
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                "2026-11-06T09:00:00+00:00",
            ),
        ];
        let failing: HashSet<String> = ["f".to_string(), "m".to_string()].into();
        assert_eq!(
            summarize_projects(
                &[project("p")],
                &tasks,
                &failing,
                "2026-10-07T00:00:00+00:00"
            )[0]
            .failing,
            1
        );
    }

    #[test]
    fn every_project_gets_a_row_even_without_tasks() {
        let rows = summarize_projects(
            &[project("a"), project("b")],
            &[],
            &HashSet::new(),
            "2026-10-07T00:00:00+00:00",
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].project_id, "b");
        assert_eq!(
            (rows[1].task_count, rows[1].last_activity_at.clone()),
            (0, None)
        );
    }

    #[test]
    fn assigned_members_switch_the_source_and_keep_derived_agents() {
        let tasks = vec![task(
            "t",
            "a",
            TaskStatus::Developing,
            AgentKind::ClaudeCode,
            AgentKind::Codex,
            "2026-11-06T10:00:00+00:00",
        )];
        let mut rows = summarize_projects(
            &[project("a"), project("b")],
            &tasks,
            &HashSet::new(),
            "2026-10-07T00:00:00+00:00",
        );
        let members: HashMap<String, Vec<AgentKind>> = [(
            "a".to_string(),
            vec![
                AgentKind::GeminiCli,
                AgentKind::External("my-sidecar".into()),
            ],
        )]
        .into();
        apply_members(&mut rows, &members);
        assert_eq!(rows[0].agents_source, AgentsSource::Assigned);
        assert_eq!(
            rows[0].assigned_agents,
            vec![
                AgentKind::GeminiCli,
                AgentKind::External("my-sidecar".into())
            ]
        );
        assert_eq!(
            rows[0].agents,
            vec![AgentKind::ClaudeCode, AgentKind::Codex]
        );
        assert_eq!(rows[1].agents_source, AgentsSource::Derived);
        assert!(rows[1].assigned_agents.is_empty());
    }

    #[tokio::test]
    async fn reads_tasks_and_recent_failed_validations_from_the_database()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let orchestrator = crate::Orchestrator::open(root.path().join("data")).await?;
        let store = &orchestrator.store;
        let a = store
            .import_project("a", "/tmp/summary-a", "main", "/tmp/w")
            .await?;
        let b = store
            .import_project("b", "/tmp/summary-b", "main", "/tmp/w")
            .await?;
        let t1 = store
            .create_task(
                &a.id,
                "t1",
                "d",
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                "main",
                3,
            )
            .await?;
        store
            .create_task(
                &a.id,
                "t2",
                "d",
                AgentKind::QoderCli,
                AgentKind::Codex,
                "main",
                3,
            )
            .await?;
        sqlx::query("UPDATE tasks SET status='READY_FOR_REVISION' WHERE id=?")
            .bind(&t1.id)
            .execute(store.pool())
            .await?;
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO events(task_id,run_id,revision,actor,event_type,payload_json,created_at) VALUES(?,NULL,1,'system','validation:failed','{}',?)")
            .bind(&t1.id)
            .bind(&now)
            .execute(store.pool())
            .await?;
        let rows = orchestrator.project_summaries().await?;
        let ra = rows
            .iter()
            .find(|r| r.project_id == a.id)
            .ok_or("missing a")?;
        let rb = rows
            .iter()
            .find(|r| r.project_id == b.id)
            .ok_or("missing b")?;
        assert_eq!((ra.task_count, ra.running, ra.failing), (2, 2, 1));
        assert!(
            ra.agents.contains(&AgentKind::ClaudeCode) && ra.agents.contains(&AgentKind::QoderCli)
        );
        assert_eq!(rb.task_count, 0);
        assert_eq!(ra.agents_source, AgentsSource::Derived);
        assert!(ra.assigned_agents.is_empty());
        Ok(())
    }
}
