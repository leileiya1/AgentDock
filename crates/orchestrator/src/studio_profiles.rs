//! Town customisation stored in SQLite: how each project's house looks, which agents the user
//! assigned to it, and per-agent profiles (nickname, colour, accessory, catchphrase).
//!
//! Validation is pure and unit-tested; the `Orchestrator` methods only read and write rows.
//! Writes are daemon-owned (see `DaemonRequest::ProjectAppearanceUpdate` and friends).

use std::collections::{HashMap, HashSet};

use agentflow_contracts::{
    AgentKind, AgentProfile, AgentProfilePatch, MemberRole, ProjectAppearance,
    ProjectAppearancePatch, ProjectMember, ProjectMemberInput,
};
use chrono::Utc;
use sqlx::Row;

use crate::{Orchestrator, OrchestratorError};

pub const SIGN_STYLES: [&str; 3] = ["wood", "neon", "stone"];
pub const YARD_ITEMS: [&str; 6] = ["fountain", "flag", "statue", "garden", "bench", "pet"];
pub const ACCESSORIES: [&str; 7] = [
    "none",
    "cap",
    "headphones",
    "glasses",
    "bowtie",
    "antenna",
    "scarf",
];
pub const MAX_YARD: usize = 6;
pub const MAX_MEMBERS: usize = 12;
const MAX_KIND_LEN: usize = 32;
const MAX_NICKNAME: usize = 16;
const MAX_CATCHPHRASE: usize = 40;

fn invalid(message: impl Into<String>) -> OrchestratorError {
    OrchestratorError::InvalidInput(message.into())
}

/// `#rrggbb`, stored lower-case.
fn hex_color(value: &str, what: &str) -> Result<String, OrchestratorError> {
    let v = value.trim();
    let ok = v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !ok {
        return Err(invalid(format!(
            "{what}要写成 #rrggbb 这样的颜色，比如 #c8553d"
        )));
    }
    Ok(v.to_ascii_lowercase())
}

fn one_of(value: &str, allowed: &[&str], what: &str) -> Result<String, OrchestratorError> {
    if allowed.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(invalid(format!(
            "{what}「{value}」不认识，可选：{}",
            allowed.join("、")
        )))
    }
}

fn building_kind(value: &str) -> Result<String, OrchestratorError> {
    let ok = !value.is_empty()
        && value.len() <= MAX_KIND_LEN
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if ok {
        Ok(value.to_string())
    } else {
        Err(invalid(
            "房子类型只能用小写字母、数字、- 和 _，最多 32 个字符",
        ))
    }
}

/// Trimmed text with a character limit (Chinese counts as one). Blank text clears the field.
fn short_text(value: &str, max: usize, what: &str) -> Result<Option<String>, OrchestratorError> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if v.chars().count() > max {
        return Err(invalid(format!("{what}最多 {max} 个字")));
    }
    Ok(Some(v.to_string()))
}

/// Apply a patch to a project's appearance. Absent fields stay, explicit nulls clear.
pub fn apply_appearance_patch(
    mut current: ProjectAppearance,
    patch: &ProjectAppearancePatch,
) -> Result<ProjectAppearance, OrchestratorError> {
    if let Some(kind) = &patch.kind {
        current.kind = kind.as_deref().map(building_kind).transpose()?;
    }
    if let Some(roof) = &patch.roof {
        current.roof = roof
            .as_deref()
            .map(|v| hex_color(v, "屋顶颜色"))
            .transpose()?;
    }
    if let Some(sign) = &patch.sign {
        current.sign = sign
            .as_deref()
            .map(|v| one_of(v, &SIGN_STYLES, "门牌样式"))
            .transpose()?;
    }
    if let Some(yard) = &patch.yard {
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        for item in yard {
            let item = one_of(item, &YARD_ITEMS, "院子摆件")?;
            if seen.insert(item.clone()) {
                items.push(item);
            }
        }
        if items.len() > MAX_YARD {
            return Err(invalid(format!("院子最多摆 {MAX_YARD} 样东西")));
        }
        current.yard = items;
    }
    Ok(current)
}

/// Apply a patch to an agent profile. Absent fields stay, explicit nulls (or blank text) clear.
pub fn apply_profile_patch(
    mut current: AgentProfile,
    patch: &AgentProfilePatch,
) -> Result<AgentProfile, OrchestratorError> {
    if let Some(nickname) = &patch.nickname {
        current.nickname = match nickname {
            Some(v) => short_text(v, MAX_NICKNAME, "名字")?,
            None => None,
        };
    }
    if let Some(color) = &patch.color {
        current.color = color
            .as_deref()
            .map(|v| hex_color(v, "机身颜色"))
            .transpose()?;
    }
    if let Some(accessory) = &patch.accessory {
        current.accessory = accessory
            .as_deref()
            .map(|v| one_of(v, &ACCESSORIES, "配件"))
            .transpose()?;
    }
    if let Some(catchphrase) = &patch.catchphrase {
        current.catchphrase = match catchphrase {
            Some(v) => short_text(v, MAX_CATCHPHRASE, "口头禅")?,
            None => None,
        };
    }
    Ok(current)
}

/// Members are replaced as a whole: no duplicates, at most `MAX_MEMBERS`.
pub fn validate_members(members: &[ProjectMemberInput]) -> Result<(), OrchestratorError> {
    if members.len() > MAX_MEMBERS {
        return Err(invalid(format!("一个项目最多分配 {MAX_MEMBERS} 位 Agent")));
    }
    let mut seen = HashSet::new();
    for m in members {
        if !seen.insert(m.agent.as_str().to_string()) {
            return Err(invalid(format!("{} 重复分配了", m.agent.as_str())));
        }
    }
    Ok(())
}

fn parse_agent(raw: String) -> Result<AgentKind, OrchestratorError> {
    raw.parse()
        .map_err(|e: String| OrchestratorError::Config(e))
}

fn appearance_from_row(r: &sqlx::sqlite::SqliteRow) -> ProjectAppearance {
    ProjectAppearance {
        project_id: r.get("project_id"),
        kind: r.get("kind"),
        roof: r.get("roof"),
        sign: r.get("sign"),
        yard: serde_json::from_str(&r.get::<String, _>("yard_json")).unwrap_or_default(),
        updated_at: Some(r.get("updated_at")),
    }
}

impl Orchestrator {
    async fn ensure_project(&self, project_id: &str) -> Result<(), OrchestratorError> {
        let found: Option<String> = sqlx::query_scalar("SELECT id FROM projects WHERE id=?")
            .bind(project_id)
            .fetch_optional(self.store.pool())
            .await?;
        found.map(|_| ()).ok_or_else(|| invalid("找不到这个项目"))
    }

    async fn project_appearance_row(
        &self,
        project_id: &str,
    ) -> Result<ProjectAppearance, OrchestratorError> {
        let row = sqlx::query("SELECT * FROM project_appearance WHERE project_id=?")
            .bind(project_id)
            .fetch_optional(self.store.pool())
            .await?;
        Ok(row
            .as_ref()
            .map(appearance_from_row)
            .unwrap_or_else(|| ProjectAppearance {
                project_id: project_id.to_string(),
                ..Default::default()
            }))
    }

    /// One row per project, in project order; projects never customised get defaults.
    pub async fn project_appearance_list(
        &self,
    ) -> Result<Vec<ProjectAppearance>, OrchestratorError> {
        let projects = self.store.projects().await?;
        let rows = sqlx::query("SELECT * FROM project_appearance")
            .fetch_all(self.store.pool())
            .await?;
        let mut stored: HashMap<String, ProjectAppearance> = rows
            .iter()
            .map(appearance_from_row)
            .map(|a| (a.project_id.clone(), a))
            .collect();
        Ok(projects
            .into_iter()
            .map(|p| {
                stored.remove(&p.id).unwrap_or(ProjectAppearance {
                    project_id: p.id,
                    ..Default::default()
                })
            })
            .collect())
    }

    pub async fn project_appearance_update(
        &self,
        project_id: &str,
        patch: &ProjectAppearancePatch,
    ) -> Result<ProjectAppearance, OrchestratorError> {
        self.ensure_project(project_id).await?;
        let mut next =
            apply_appearance_patch(self.project_appearance_row(project_id).await?, patch)?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO project_appearance(project_id,kind,roof,sign,yard_json,updated_at) VALUES(?,?,?,?,?,?)
             ON CONFLICT(project_id) DO UPDATE SET kind=excluded.kind,roof=excluded.roof,sign=excluded.sign,yard_json=excluded.yard_json,updated_at=excluded.updated_at",
        )
        .bind(project_id)
        .bind(&next.kind)
        .bind(&next.roof)
        .bind(&next.sign)
        .bind(serde_json::to_string(&next.yard).unwrap_or_else(|_| "[]".into()))
        .bind(&now)
        .execute(self.store.pool())
        .await?;
        next.updated_at = Some(now);
        Ok(next)
    }

    pub async fn project_members_get(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectMember>, OrchestratorError> {
        let rows = sqlx::query(
            "SELECT agent,role,position,added_at FROM project_members WHERE project_id=? ORDER BY position",
        )
        .bind(project_id)
        .fetch_all(self.store.pool())
        .await?;
        rows.into_iter()
            .map(|r| {
                let role: String = r.get("role");
                Ok(ProjectMember {
                    agent: parse_agent(r.get("agent"))?,
                    role: MemberRole::parse(&role)
                        .ok_or_else(|| OrchestratorError::Config(format!("unknown role {role}")))?,
                    position: r.get::<i64, _>("position") as i32,
                    added_at: r.get("added_at"),
                })
            })
            .collect()
    }

    /// Replace the project's members; order in `members` becomes `position`. Agents already
    /// assigned keep their original `added_at`.
    pub async fn project_members_set(
        &self,
        project_id: &str,
        members: &[ProjectMemberInput],
    ) -> Result<Vec<ProjectMember>, OrchestratorError> {
        validate_members(members)?;
        self.ensure_project(project_id).await?;
        let previous: HashMap<String, String> = self
            .project_members_get(project_id)
            .await?
            .into_iter()
            .map(|m| (m.agent.as_str().to_string(), m.added_at))
            .collect();
        let now = Utc::now().to_rfc3339();
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("DELETE FROM project_members WHERE project_id=?")
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        for (i, m) in members.iter().enumerate() {
            let agent = m.agent.as_str();
            sqlx::query(
                "INSERT INTO project_members(project_id,agent,role,position,added_at) VALUES(?,?,?,?,?)",
            )
            .bind(project_id)
            .bind(agent)
            .bind(m.role.as_str())
            .bind(i as i64)
            .bind(previous.get(agent).unwrap_or(&now))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.project_members_get(project_id).await
    }

    /// Project id → assigned agents in display order, for the summaries.
    pub(crate) async fn project_members_all(
        &self,
    ) -> Result<HashMap<String, Vec<AgentKind>>, OrchestratorError> {
        let rows = sqlx::query(
            "SELECT project_id,agent FROM project_members ORDER BY project_id,position",
        )
        .fetch_all(self.store.pool())
        .await?;
        let mut out: HashMap<String, Vec<AgentKind>> = HashMap::new();
        for r in rows {
            out.entry(r.get("project_id"))
                .or_default()
                .push(parse_agent(r.get("agent"))?);
        }
        Ok(out)
    }

    /// Only agents the user customised; the client falls back to defaults for the rest.
    pub async fn agent_profiles_list(&self) -> Result<Vec<AgentProfile>, OrchestratorError> {
        let rows = sqlx::query("SELECT * FROM agent_profiles ORDER BY agent")
            .fetch_all(self.store.pool())
            .await?;
        rows.into_iter()
            .map(|r| {
                Ok(AgentProfile {
                    agent: parse_agent(r.get("agent"))?,
                    nickname: r.get("nickname"),
                    color: r.get("color"),
                    accessory: r.get("accessory"),
                    catchphrase: r.get("catchphrase"),
                    updated_at: Some(r.get("updated_at")),
                })
            })
            .collect()
    }

    pub async fn agent_profile_update(
        &self,
        agent: &AgentKind,
        patch: &AgentProfilePatch,
    ) -> Result<AgentProfile, OrchestratorError> {
        let current = self
            .agent_profiles_list()
            .await?
            .into_iter()
            .find(|p| &p.agent == agent)
            .unwrap_or(AgentProfile {
                agent: agent.clone(),
                nickname: None,
                color: None,
                accessory: None,
                catchphrase: None,
                updated_at: None,
            });
        let mut next = apply_profile_patch(current, patch)?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO agent_profiles(agent,nickname,color,accessory,catchphrase,updated_at) VALUES(?,?,?,?,?,?)
             ON CONFLICT(agent) DO UPDATE SET nickname=excluded.nickname,color=excluded.color,accessory=excluded.accessory,catchphrase=excluded.catchphrase,updated_at=excluded.updated_at",
        )
        .bind(agent.as_str())
        .bind(&next.nickname)
        .bind(&next.color)
        .bind(&next.accessory)
        .bind(&next.catchphrase)
        .bind(&now)
        .execute(self.store.pool())
        .await?;
        next.updated_at = Some(now);
        Ok(next)
    }

    /// Back to the default look. Returns whether a profile existed.
    pub async fn agent_profile_reset(&self, agent: &AgentKind) -> Result<bool, OrchestratorError> {
        let done = sqlx::query("DELETE FROM agent_profiles WHERE agent=?")
            .bind(agent.as_str())
            .execute(self.store.pool())
            .await?;
        Ok(done.rows_affected() > 0)
    }
}
