impl Orchestrator {
    /// Exports only events belonging to the requested project/task. Payload values
    /// are redacted recursively and the file is atomically published after a full write.
    pub async fn events_export(
        &self,
        project_id: &str,
        task_id: Option<&str>,
    ) -> Result<AuditExportResult, OrchestratorError> {
        self.project(project_id).await?;
        if let Some(task_id) = task_id {
            let owner: Option<String> =
                sqlx::query_scalar("SELECT project_id FROM tasks WHERE id=?")
                    .bind(task_id)
                    .fetch_optional(self.store.pool())
                    .await?;
            if owner.as_deref() != Some(project_id) {
                return Err(OrchestratorError::InvalidState(
                    "AUDIT_EXPORT_TASK_OUTSIDE_PROJECT".into(),
                ));
            }
        }

        let rows = if let Some(task_id) = task_id {
            sqlx::query(
                "SELECT id,task_id,run_id,revision,actor,event_type,payload_json,created_at \
                 FROM events WHERE task_id=? ORDER BY id",
            )
            .bind(task_id)
            .fetch_all(self.store.pool())
            .await?
        } else {
            sqlx::query(
                "SELECT e.id,e.task_id,e.run_id,e.revision,e.actor,e.event_type,e.payload_json,e.created_at \
                 FROM events e LEFT JOIN tasks t ON e.task_id=t.id \
                 WHERE t.project_id=? OR e.task_id IS NULL ORDER BY e.id",
            )
            .bind(project_id)
            .fetch_all(self.store.pool())
            .await?
        };

        let mut task_ids = HashSet::new();
        let mut events = Vec::new();
        let home = local_home_dir();
        for row in rows {
            let event_task_id = row.get::<Option<String>, _>("task_id");
            let mut payload = serde_json::from_str::<Value>(
                &row.get::<String, _>("payload_json"),
            )
            .unwrap_or(Value::Null);
            // Task-less events are global records. Include one only when its payload
            // explicitly binds it to this project, otherwise an export could leak data
            // from settings or another project.
            if event_task_id.is_none() && !payload_belongs_to_project(&payload, project_id) {
                continue;
            }
            redact_audit_value(&mut payload, home.as_deref());
            if let Some(id) = &event_task_id {
                task_ids.insert(id.clone());
            }
            events.push(json!({
                "record_type": "event",
                "id": row.get::<i64, _>("id"),
                "task_id": event_task_id,
                "run_id": row.get::<Option<String>, _>("run_id"),
                "revision": row.get::<Option<i64>, _>("revision"),
                "actor": row.get::<String, _>("actor"),
                "event_type": row.get::<String, _>("event_type"),
                "payload": payload,
                "created_at": row.get::<String, _>("created_at"),
            }));
        }

        let created_at = Utc::now();
        let scope = if task_id.is_some() {
            AuditExportScope::Task
        } else {
            AuditExportScope::Project
        };
        let event_count = u32::try_from(events.len()).unwrap_or(u32::MAX);
        let task_count = u32::try_from(task_ids.len()).unwrap_or(u32::MAX);
        let metadata = json!({
            "record_type": "agentflow.audit_export",
            "schema_version": 1,
            "scope": scope,
            "project_id": project_id,
            "task_id": task_id,
            "event_count": event_count,
            "task_count": task_count,
            "redacted": true,
            "created_at": created_at.to_rfc3339(),
        });

        let dir = self.app_data.join("exports");
        tokio::fs::create_dir_all(&dir).await?;
        let suffix = task_id.unwrap_or(project_id);
        let file_name = format!(
            "audit-{}-{}-{}.jsonl",
            safe_file_segment(suffix),
            created_at.format("%Y%m%dT%H%M%SZ"),
            &Uuid::now_v7().to_string()[..8]
        );
        let path = dir.join(file_name);
        let temporary = path.with_extension(format!("jsonl.tmp-{}", Uuid::now_v7()));
        let mut file = tokio::fs::File::create(&temporary).await?;
        write_audit_line(&mut file, &metadata).await?;
        for event in &events {
            write_audit_line(&mut file, event).await?;
        }
        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temporary, &path).await?;
        let bytes = tokio::fs::metadata(&path).await?.len();

        sqlx::query(
            "INSERT INTO events(task_id,actor,event_type,payload_json,created_at) \
             VALUES(?,'human','audit:export',?,?)",
        )
        .bind(task_id)
        .bind(
            json!({
                "project_id": project_id,
                "scope": scope,
                "event_count": event_count,
                "task_count": task_count,
                "redacted": true,
            })
            .to_string(),
        )
        .bind(created_at.to_rfc3339())
        .execute(self.store.pool())
        .await?;

        Ok(AuditExportResult {
            path: path.to_string_lossy().into_owned(),
            scope,
            project_id: project_id.into(),
            task_id: task_id.map(str::to_owned),
            event_count,
            task_count,
            bytes,
            redacted: true,
            created_at: created_at.to_rfc3339(),
        })
    }
}

async fn write_audit_line(
    file: &mut tokio::fs::File,
    value: &Value,
) -> Result<(), OrchestratorError> {
    let line = serde_json::to_vec(value)
        .map_err(|error| OrchestratorError::InvalidState(error.to_string()))?;
    file.write_all(&line).await?;
    file.write_all(b"\n").await?;
    Ok(())
}

fn payload_belongs_to_project(payload: &Value, project_id: &str) -> bool {
    payload
        .get("project_id")
        .or_else(|| payload.get("projectId"))
        .and_then(Value::as_str)
        == Some(project_id)
}

fn redact_audit_value(value: &mut Value, home: Option<&str>) {
    match value {
        Value::Object(values) => {
            for (key, value) in values {
                if audit_key_is_sensitive(key) {
                    *value = Value::String("[REDACTED]".into());
                } else {
                    redact_audit_value(value, home);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(|v| redact_audit_value(v, home)),
        Value::String(text) => {
            let scrubbed = agentflow_process_supervisor::redact(std::mem::take(text));
            // §58: an export is meant to be shared, so strip the local home directory (which carries
            // the OS username) from any absolute path, keeping the useful project-relative part.
            *text = match home {
                Some(home) if scrubbed.contains(home) => scrubbed.replace(home, "~"),
                _ => scrubbed,
            };
        }
        _ => {}
    }
}

/// The current user's home directory, used to strip local privacy paths from exports. Guarded
/// against a too-short value (e.g. "/") that would corrupt unrelated paths.
fn local_home_dir() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| value.len() > 5)
}

fn audit_key_is_sensitive(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "apikey"
            | "password"
            | "secret"
            | "authorization"
            | "credential"
            | "accesstoken"
            | "refreshtoken"
            | "bearertoken"
            | "privatekey"
            | "accesskey"
    )
}

fn safe_file_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(80)
        .collect()
}
