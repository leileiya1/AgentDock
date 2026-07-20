pub fn development_result_schema() -> Value {
    let mut value = serde_json::to_value(schemars::schema_for!(DevelopmentResult))
        .unwrap_or_else(|_| serde_json::json!({}));
    harden_schema(&mut value, false);
    value
}

pub fn review_result_schema() -> Value {
    let mut value = serde_json::to_value(schemars::schema_for!(ReviewResult))
        .unwrap_or_else(|_| serde_json::json!({}));
    harden_schema(&mut value, true);
    value
}

fn harden_schema(value: &mut Value, review: bool) {
    let Some(root) = value.as_object_mut() else {
        return;
    };
    root.insert("additionalProperties".into(), Value::Bool(false));
    if let Some(properties) = root.get_mut("properties").and_then(Value::as_object_mut) {
        properties.insert("schema_version".into(), serde_json::json!({"const":1}));
        if let Some(revision) = properties
            .get_mut("revision")
            .and_then(Value::as_object_mut)
        {
            revision.insert("minimum".into(), Value::from(1));
        }
        if let Some(summary) = properties.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("maxLength".into(), Value::from(4000));
            if !review {
                summary.insert("minLength".into(), Value::from(1));
            }
        }
        if review {
            if let Some(commit) = properties
                .get_mut("commit_sha")
                .and_then(Value::as_object_mut)
            {
                commit.insert("minLength".into(), Value::from(7));
            }
        } else {
            for key in ["question", "notes"] {
                set_optional_type(properties, key, "string");
            }
            set_optional_type(properties, "changed_files", "array");
        }
    }
    if review
        && let Some(issue) = root
            .get_mut("$defs")
            .and_then(Value::as_object_mut)
            .and_then(|defs| defs.get_mut("ReviewIssueResult"))
            .and_then(Value::as_object_mut)
    {
        issue.insert("additionalProperties".into(), Value::Bool(false));
        if let Some(properties) = issue.get_mut("properties").and_then(Value::as_object_mut) {
            for key in ["file", "description", "suggested_action"] {
                set_optional_type(properties, key, "string");
            }
            for key in ["line_start", "line_end"] {
                set_optional_type(properties, key, "integer");
                if let Some(field) = properties.get_mut(key).and_then(Value::as_object_mut) {
                    field.insert("minimum".into(), Value::from(1));
                }
            }
            if let Some(title) = properties.get_mut("title").and_then(Value::as_object_mut) {
                title.insert("minLength".into(), Value::from(1));
                title.insert("maxLength".into(), Value::from(200));
            }
            for (key, max) in [("description", 4000), ("suggested_action", 2000)] {
                if let Some(field) = properties.get_mut(key).and_then(Value::as_object_mut) {
                    field.insert("maxLength".into(), Value::from(max));
                }
            }
        }
    }
}

fn set_optional_type(properties: &mut serde_json::Map<String, Value>, key: &str, kind: &str) {
    if let Some(field) = properties.get_mut(key).and_then(Value::as_object_mut) {
        field.insert("type".into(), Value::String(kind.into()));
    }
}
