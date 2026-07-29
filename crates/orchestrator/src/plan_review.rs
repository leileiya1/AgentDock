impl Orchestrator {
    pub async fn task_plan_review_context(
        &self,
        task_id: &str,
    ) -> Result<PlanReviewContext, OrchestratorError> {
        self.task(task_id).await?;
        let rows = sqlx::query(
            "SELECT id,version,status,summary,steps_json,risks_json,allowed_paths_json,plan_sha256,rejection_reason,created_at,approved_at \
             FROM task_plans WHERE task_id=? ORDER BY version ASC LIMIT 50",
        )
        .bind(task_id)
        .fetch_all(self.store.pool())
        .await?;
        let mut plans = Vec::with_capacity(rows.len());
        for row in rows {
            plans.push(CodingPlanHistoryEntry {
                plan: CodingPlan {
                    id: row.get("id"),
                    version: row.get("version"),
                    status: parse(row.get("status"))?,
                    summary: row.get("summary"),
                    steps: serde_json::from_str(&row.get::<String, _>("steps_json"))
                        .map_err(|error| OrchestratorError::Config(error.to_string()))?,
                    risks: serde_json::from_str(&row.get::<String, _>("risks_json"))
                        .map_err(|error| OrchestratorError::Config(error.to_string()))?,
                    allowed_paths: serde_json::from_str(
                        &row.get::<String, _>("allowed_paths_json"),
                    )
                    .map_err(|error| OrchestratorError::Config(error.to_string()))?,
                    plan_sha256: row.get("plan_sha256"),
                    created_at: row.get("created_at"),
                    approved_at: row.get("approved_at"),
                },
                rejection_reason: row.get("rejection_reason"),
            });
        }
        let latest_diff = plans
            .get(plans.len().saturating_sub(2))
            .zip(plans.last())
            .and_then(|(previous, latest)| {
                (previous.plan.version != latest.plan.version)
                    .then(|| compare_plans(&previous.plan, &latest.plan))
            });
        let deviation_row = sqlx::query(
            "SELECT payload_json,created_at FROM events WHERE task_id=? AND event_type='plan:deviation' ORDER BY id DESC LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(self.store.pool())
        .await?;
        let (detected_deviations, deviation_plan_id, deviation_detected_at) = deviation_row
            .map(|row| {
                let payload = serde_json::from_str::<Value>(
                    &row.get::<String, _>("payload_json"),
                )
                .unwrap_or(Value::Null);
                (
                    payload
                        .get("deviations")
                        .and_then(Value::as_array)
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default(),
                    payload
                        .get("plan_id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    Some(row.get("created_at")),
                )
            })
            .unwrap_or_default();
        Ok(PlanReviewContext {
            task_id: task_id.to_string(),
            plans,
            latest_diff,
            detected_deviations,
            deviation_plan_id,
            deviation_detected_at,
        })
    }
}

fn compare_plans(previous: &CodingPlan, latest: &CodingPlan) -> PlanVersionDiff {
    PlanVersionDiff {
        from_version: previous.version,
        to_version: latest.version,
        summary_changed: previous.summary != latest.summary,
        added_steps: added_values(
            previous.steps.iter().map(|step| step.title.as_str()),
            latest.steps.iter().map(|step| step.title.as_str()),
        ),
        removed_steps: added_values(
            latest.steps.iter().map(|step| step.title.as_str()),
            previous.steps.iter().map(|step| step.title.as_str()),
        ),
        added_allowed_paths: added_values(
            previous.allowed_paths.iter().map(String::as_str),
            latest.allowed_paths.iter().map(String::as_str),
        ),
        removed_allowed_paths: added_values(
            latest.allowed_paths.iter().map(String::as_str),
            previous.allowed_paths.iter().map(String::as_str),
        ),
        added_risks: added_values(
            previous.risks.iter().map(String::as_str),
            latest.risks.iter().map(String::as_str),
        ),
        removed_risks: added_values(
            latest.risks.iter().map(String::as_str),
            previous.risks.iter().map(String::as_str),
        ),
    }
}

fn added_values<'a>(
    previous: impl Iterator<Item = &'a str>,
    latest: impl Iterator<Item = &'a str>,
) -> Vec<String> {
    let previous = previous.collect::<std::collections::BTreeSet<_>>();
    latest
        .filter(|value| !previous.contains(value))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod plan_review_unit_tests {
    use super::*;

    fn plan(version: i64, steps: &[&str], paths: &[&str], risks: &[&str]) -> CodingPlan {
        CodingPlan {
            id: format!("plan-{version}"),
            version,
            status: PlanStatus::Pending,
            summary: format!("summary {version}"),
            steps: steps
                .iter()
                .map(|title| PlanStep {
                    title: (*title).into(),
                    detail: String::new(),
                    validation: None,
                })
                .collect(),
            risks: risks.iter().map(|value| (*value).into()).collect(),
            allowed_paths: paths.iter().map(|value| (*value).into()).collect(),
            plan_sha256: None,
            created_at: "now".into(),
            approved_at: None,
        }
    }

    #[test]
    fn plan_diff_keeps_scope_and_risk_changes_explicit() {
        let previous = plan(1, &["implement"], &["src/**"], &["migration"]);
        let latest = plan(
            2,
            &["implement", "test"],
            &["src/**", "tests/**"],
            &["compatibility"],
        );
        let diff = compare_plans(&previous, &latest);
        assert_eq!(diff.added_steps, vec!["test"]);
        assert_eq!(diff.added_allowed_paths, vec!["tests/**"]);
        assert_eq!(diff.added_risks, vec!["compatibility"]);
        assert_eq!(diff.removed_risks, vec!["migration"]);
        assert!(diff.summary_changed);
    }
}
