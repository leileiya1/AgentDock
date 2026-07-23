// Auto-rework convergence heuristics (§32-34).
//
// The hard `max_revisions` ceiling (§27) guarantees the loop can never run forever. These checks
// run *before* that ceiling, on every "request changes" decision, to pause early when the loop is
// clearly not making progress — so the task doesn't burn its whole rework budget thrashing.
//
// Three orthogonal signals feed one verdict (`ConvergenceHealth`):
//   - §32 stall:      blocking-issue count did not drop across two consecutive rounds.
//   - §33 churn:      the same file was rewritten in three consecutive rounds.
//   - §34 regression: this round is strictly worse — more blockers, or a fixed issue came back.
//
// A regression outranks a stall, because its recommended recovery differs (roll the round back
// vs. give new guidance). Every DB/git lookup degrades to "not a problem" on missing data, so a
// partially-recorded task never trips a false pause.
//
// This file is `include!`d into lib.rs, so it shares the crate-root scope (no `use` header) and its
// private methods are callable from the review loop, which lives in the same scope.

/// The health of the auto-rework loop (§32-34).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ConvergenceHealth {
    /// Blockers are dropping (or it's too early to tell) — keep iterating.
    Ok,
    /// No progress across two rounds, or the same file is being churned (§32/§33).
    Stalled,
    /// This round is worse than the last (§34) — recommend rolling it back.
    Regressed,
}

impl Orchestrator {
    /// Judge whether the rework loop is making progress (§32-34), from the review issue history and
    /// the per-round diff. Ordered by severity: a regression (worse than before) outranks a stall.
    async fn assess_convergence(
        &self,
        task: &TaskRow,
    ) -> Result<ConvergenceHealth, OrchestratorError> {
        let current = self.blocking_issue_count(&task.id, task.revision).await?;
        let previous = self.blocking_issue_count(&task.id, task.revision - 1).await?;
        // §34: this round is strictly worse than the last — more blocking issues than before, or an
        // issue an earlier round had resolved has come back. Recommend rolling this round back.
        if (previous > 0 && current > previous)
            || self.resolved_issue_reappeared(&task.id, task.revision).await?
        {
            return Ok(ConvergenceHealth::Regressed);
        }
        // §32: two consecutive rounds without any drop in blocking issues → not converging.
        if previous > 0 && current >= previous {
            return Ok(ConvergenceHealth::Stalled);
        }
        // §33: the same file has been rewritten in three consecutive rounds → thrashing, even if
        // the blocker count happens to be drifting down.
        if self.same_file_churned_three_rounds(task).await? {
            return Ok(ConvergenceHealth::Stalled);
        }
        Ok(ConvergenceHealth::Ok)
    }

    /// Count the blocking (critical/high) review issues recorded for one revision's review.
    async fn blocking_issue_count(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<i64, OrchestratorError> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM review_issues ri JOIN reviews r ON ri.review_id = r.id \
             WHERE r.task_id = ? AND r.revision = ? AND ri.severity IN ('critical','high')",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_one(self.store.pool())
        .await?;
        Ok(count)
    }

    /// §34「旧问题重现」: an issue present in this round and two rounds ago, but not in the round
    /// between, has come back after being fixed. Matched on (file, normalized title) — robust enough
    /// to catch a genuine reappearance while a rephrased issue simply doesn't match (a safe miss).
    async fn resolved_issue_reappeared(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<bool, OrchestratorError> {
        if revision < 3 {
            return Ok(false);
        }
        let now = self.blocking_issue_keys(task_id, revision).await?;
        let prev = self.blocking_issue_keys(task_id, revision - 1).await?;
        let older = self.blocking_issue_keys(task_id, revision - 2).await?;
        Ok(now.iter().any(|key| older.contains(key) && !prev.contains(key)))
    }

    /// (file, normalized-title) keys of the blocking issues for one revision's review.
    async fn blocking_issue_keys(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<std::collections::HashSet<String>, OrchestratorError> {
        let rows = sqlx::query(
            "SELECT COALESCE(ri.file,'') AS file, ri.title AS title \
             FROM review_issues ri JOIN reviews r ON ri.review_id = r.id \
             WHERE r.task_id = ? AND r.revision = ? AND ri.severity IN ('critical','high')",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_all(self.store.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let file: String = row.get("file");
                let title: String = row.get("title");
                // Normalize whitespace + case so trivial phrasing differences still match.
                let title = title.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
                format!("{file}|{title}")
            })
            .collect())
    }

    /// §33: true when some file was changed in each of the three most recent rework rounds.
    async fn same_file_churned_three_rounds(
        &self,
        task: &TaskRow,
    ) -> Result<bool, OrchestratorError> {
        if task.revision < 3 {
            return Ok(false);
        }
        let latest = self.round_changed_files(task, task.revision).await?;
        if latest.is_empty() {
            return Ok(false);
        }
        let prev = self.round_changed_files(task, task.revision - 1).await?;
        let older = self.round_changed_files(task, task.revision - 2).await?;
        Ok(latest.iter().any(|file| prev.contains(file) && older.contains(file)))
    }

    /// Files changed in a single rework round, i.e. between the previous revision's commit (or the
    /// task base for round 1) and this revision's commit. Missing commits yield an empty set.
    async fn round_changed_files(
        &self,
        task: &TaskRow,
        revision: i64,
    ) -> Result<std::collections::HashSet<String>, OrchestratorError> {
        let Some(head) = self.revision_commit_opt(&task.id, revision).await? else {
            return Ok(std::collections::HashSet::new());
        };
        let base = if revision <= 1 {
            task.base_commit.clone()
        } else {
            self.revision_commit_opt(&task.id, revision - 1).await?
        };
        let Some(base) = base else {
            return Ok(std::collections::HashSet::new());
        };
        let project = self.project(&task.project_id).await?;
        Ok(self
            .git
            .changed_files(&project.repo, &base, &head)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect())
    }

    async fn revision_commit_opt(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<Option<String>, OrchestratorError> {
        Ok(sqlx::query_scalar::<_, Option<String>>(
            "SELECT commit_sha FROM task_revisions WHERE task_id = ? AND revision = ?",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_optional(self.store.pool())
        .await?
        .flatten())
    }
}
