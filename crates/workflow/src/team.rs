//! Pure team rules. No database, model calls, clock or process state.
use agentflow_contracts::*;
use std::collections::{BTreeSet, HashMap, HashSet};

pub fn validate_config(config: &TeamConfig) -> Result<(), String> {
    if !(3..=16).contains(&config.team_capacity)
        || config.members.len() > config.team_capacity as usize
        || config.max_concurrent_attempts == 0
        || config.max_concurrent_attempts > config.team_capacity
        || config.max_attempts < 6
        || config.max_attempts > 1000
        || config.verification_reserve < 2
        || config.verification_reserve >= config.max_attempts
        || config.max_plan_revisions == 0
        || config.max_plan_revisions > 20
        || config.max_reworks > 20
        || !(10..=7200).contains(&config.attempt_timeout_secs)
        || config.write_globs.is_empty()
    {
        return Err("invalid team, concurrency, budget, scope or bounded recovery policy".into());
    }
    let mut ids = HashSet::new();
    for member in &config.members {
        if !valid_id(&member.id)
            || !ids.insert(&member.id)
            || member.roles.is_empty()
            || member.providers.is_empty()
        {
            return Err("invalid or duplicate member seat".into());
        }
        // Legacy sidecars do not negotiate this execution/identity contract.
        if member
            .providers
            .iter()
            .any(|p| !matches!(p, AgentKind::ClaudeCode | AgentKind::Codex))
        {
            return Err("team protocol v1 currently requires built-in Claude Code/Codex CLI adapters; other providers are not advertised as tool-capable".into());
        }
    }
    let planner = config
        .members
        .iter()
        .find(|m| m.id == config.planner_member_id)
        .ok_or("planner seat missing")?;
    if !planner.roles.contains(&TeamRole::Planner) {
        return Err("planner role missing".into());
    }
    for role in [TeamRole::Developer, TeamRole::Tester, TeamRole::Reviewer] {
        if !config.members.iter().any(|m| m.roles.contains(&role)) {
            return Err(format!("missing {role} member"));
        }
    }
    let developers: Vec<_> = config
        .members
        .iter()
        .filter(|m| m.roles.contains(&TeamRole::Developer))
        .collect();
    if !config
        .members
        .iter()
        .any(|m| m.roles.contains(&TeamRole::Tester) && developers.iter().any(|d| d.id != m.id))
    {
        return Err("no independent tester assignment".into());
    }
    if config.cross_vendor_review
        && !config.members.iter().any(|r| {
            r.roles.contains(&TeamRole::Reviewer)
                && developers.iter().any(|d| {
                    d.id != r.id
                        && d.providers
                            .iter()
                            .any(|p| r.providers.iter().any(|q| family(p) != family(q)))
                })
        })
    {
        return Err("no feasible cross-vendor reviewer; reserve a family for review".into());
    }
    for prefix in &config.allowed_commands {
        if prefix.is_empty() || prefix.iter().any(|v| v.is_empty() || v.contains('\0')) {
            return Err("invalid allowed command prefix".into());
        }
    }
    Ok(())
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
pub fn family(provider: &AgentKind) -> String {
    match provider {
        AgentKind::Codex | AgentKind::OpenAiApi => "openai".into(),
        AgentKind::ClaudeCode | AgentKind::AnthropicApi => "anthropic".into(),
        p => p.to_string(),
    }
}
pub fn command_allowed(check: &TeamCheck, config: &TeamConfig) -> bool {
    check.timeout_secs > 0
        && check.timeout_secs <= config.attempt_timeout_secs
        && !check.argv.iter().any(|a| a.contains('\0'))
        && config
            .allowed_commands
            .iter()
            .any(|prefix| !prefix.is_empty() && check.argv.starts_with(prefix))
}
pub fn validate_plan(
    plan: &TeamPlan,
    requirements: &[TeamRequirement],
    config: &TeamConfig,
) -> Result<(), String> {
    if plan.schema_version != 1
        || plan.items.is_empty()
        || plan.items.len() > 100
        || plan.criteria.is_empty()
        || plan.criteria.len() > 100
    {
        return Err("plan v1 needs work and explicit acceptance criteria".into());
    }
    let mut ids = HashSet::new();
    let req_ids: HashSet<_> = requirements.iter().map(|r| &r.id).collect();
    for item in &plan.items {
        if !valid_id(&item.id)
            || item.id.starts_with("__")
            || !ids.insert(&item.id)
            || item.kind != TeamWorkKind::Implement
            || item.instructions.trim().is_empty()
            || item.member_ids.is_empty()
            || item.requirement_ids.is_empty()
            || item.requirement_ids.iter().any(|r| !req_ids.contains(r))
            || item.write_globs.is_empty()
        {
            return Err(format!("invalid development spec {}", item.id));
        }
        for id in &item.member_ids {
            if !config
                .members
                .iter()
                .any(|m| &m.id == id && m.roles.contains(&TeamRole::Developer))
            {
                return Err(format!("{} has no eligible developer {id}", item.id));
            }
        }
        for check in &item.local_checks {
            if !command_allowed(check, config) {
                return Err(format!(
                    "local check {} exceeds command authorization",
                    check.id
                ));
            }
        }
    }
    let mut done = HashSet::new();
    loop {
        let before = done.len();
        for item in &plan.items {
            if item
                .depends_on
                .iter()
                .all(|d| ids.contains(d) && done.contains(d))
            {
                done.insert(&item.id);
            }
        }
        if done.len() == ids.len() {
            break;
        }
        if done.len() == before {
            return Err("cyclic or missing dependency".into());
        }
    }
    let mut criteria = HashSet::new();
    let mut check_ids = HashSet::new();
    for c in &plan.criteria {
        if !valid_id(&c.id)
            || !criteria.insert(&c.id)
            || c.requirement_ids.is_empty()
            || c.requirement_ids.iter().any(|r| !req_ids.contains(r))
            || !ids.contains(&c.owner_work_item_id)
            || c.expected.trim().is_empty()
            || (c.required && c.checks.is_empty())
        {
            return Err(format!(
                "invalid criterion {} or missing executable checks",
                c.id
            ));
        }
        for check in &c.checks {
            if !valid_id(&check.id)
                || !check_ids.insert(&check.id)
                || !command_allowed(check, config)
            {
                return Err(format!(
                    "invalid, duplicate or unauthorized check {}",
                    check.id
                ));
            }
        }
    }
    for req in requirements.iter().filter(|r| r.required) {
        if !plan
            .criteria
            .iter()
            .any(|c| c.required && c.requirement_ids.contains(&req.id))
            || !plan
                .items
                .iter()
                .any(|i| i.requirement_ids.contains(&req.id))
        {
            return Err(format!(
                "uncovered original requirement {}: {}",
                req.id, req.original_text
            ));
        }
    }
    Ok(())
}
pub fn affected_items(old: &TeamPlan, new: &TeamPlan) -> BTreeSet<String> {
    let previous: HashMap<_, _> = old.items.iter().map(|i| (&i.id, i)).collect();
    let mut affected: BTreeSet<_> = new
        .items
        .iter()
        .filter(|i| previous.get(&i.id).is_none_or(|p| **p != **i))
        .map(|i| i.id.clone())
        .collect();
    affected.extend(
        old.items
            .iter()
            .filter(|i| !new.items.iter().any(|n| n.id == i.id))
            .map(|i| i.id.clone()),
    );
    // Overlapping/unknown scope is conservative. Then propagate dependencies.
    let changed_scopes: Vec<_> = old
        .items
        .iter()
        .chain(new.items.iter())
        .filter(|i| affected.contains(&i.id))
        .flat_map(|i| i.write_globs.clone())
        .collect();
    for i in &new.items {
        if i.write_globs
            .iter()
            .any(|g| changed_scopes.iter().any(|s| scopes_overlap(g, s)))
        {
            affected.insert(i.id.clone());
        }
    }
    loop {
        let count = affected.len();
        for i in old.items.iter().chain(new.items.iter()) {
            if i.depends_on.iter().any(|d| affected.contains(d)) {
                affected.insert(i.id.clone());
            }
        }
        if count == affected.len() {
            break;
        }
    }
    affected
}
fn scopes_overlap(a: &str, b: &str) -> bool {
    let ap = a.split(['*', '?', '[']).next().unwrap_or("");
    let bp = b.split(['*', '?', '[']).next().unwrap_or("");
    ap.starts_with(bp) || bp.starts_with(ap)
}
pub fn role_for(kind: TeamWorkKind) -> Option<TeamRole> {
    match kind {
        TeamWorkKind::Plan => Some(TeamRole::Planner),
        TeamWorkKind::Coverage | TeamWorkKind::Verify => Some(TeamRole::Tester),
        TeamWorkKind::Implement => Some(TeamRole::Developer),
        TeamWorkKind::Review => Some(TeamRole::Reviewer),
        TeamWorkKind::Integrate => None,
    }
}
pub fn independent(
    member: &TeamMember,
    provider: &AgentKind,
    kind: TeamWorkKind,
    candidate: &TeamCandidate,
    cross_vendor: bool,
) -> bool {
    if matches!(kind, TeamWorkKind::Verify | TeamWorkKind::Review)
        && candidate.contributor_member_ids.contains(&member.id)
    {
        return false;
    }
    kind != TeamWorkKind::Review
        || !cross_vendor
        || candidate
            .contributor_providers
            .iter()
            .all(|p| family(p) != family(provider))
}
pub fn criterion_status(
    c: &TeamCriterion,
    criterion_hash: &str,
    candidate: Option<&TeamCandidate>,
    evidence: &[TeamEvidence],
) -> CriterionStatus {
    let Some(candidate) = candidate else {
        return CriterionStatus::NotRun;
    };
    let mut statuses = Vec::new();
    for check in &c.checks {
        let status = evidence
            .iter()
            .rev()
            .find(|e| {
                e.criterion_id == c.id
                    && e.criterion_hash == criterion_hash
                    && e.candidate_id == candidate.id
                    && e.candidate_tree == candidate.tree_sha
                    && e.check_id == check.id
            })
            .map(|e| e.status)
            .unwrap_or(CriterionStatus::NotRun);
        statuses.push(status);
    }
    for priority in [
        CriterionStatus::Failed,
        CriterionStatus::Blocked,
        CriterionStatus::Inconclusive,
        CriterionStatus::NotRun,
    ] {
        if statuses.contains(&priority) {
            return priority;
        }
    }
    if statuses.is_empty() {
        CriterionStatus::NotRun
    } else {
        CriterionStatus::Passed
    }
}
pub fn final_gate(
    view: &TeamView,
    coverage_current: bool,
    review_count: usize,
    required_reviews: usize,
    unresolved_issues: usize,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if view.plan.is_none() {
        blockers.push("no active plan".into());
    }
    if !coverage_current {
        blockers.push("original requirements need independent coverage audit".into());
    }
    if view.candidate.is_none() {
        blockers.push("no sealed integrated candidate".into());
    }
    if view
        .items
        .iter()
        .any(|i| i.spec.kind == TeamWorkKind::Implement && i.state != TeamWorkState::Accepted)
    {
        blockers.push("development outputs not accepted".into());
    }
    if view.criteria.is_empty() {
        blockers.push("zero acceptance criteria".into());
    }
    for c in view.criteria.iter().filter(|c| c.criterion.required) {
        if c.status != CriterionStatus::Passed || c.stale {
            blockers.push(format!("{}: {}", c.criterion.id, c.status));
        }
    }
    if review_count < required_reviews {
        blockers.push(format!(
            "independent reviews {review_count}/{required_reviews}"
        ));
    }
    if unresolved_issues > 0 {
        blockers.push(format!("{unresolved_issues} unresolved blocking issues"));
    }
    if matches!(view.phase, TeamPhase::Cancelled | TeamPhase::Paused) {
        blockers.push(format!("task {}", view.phase));
    }
    blockers
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> TeamConfig {
        TeamConfig {
            team_capacity: 4,
            max_concurrent_attempts: 2,
            planner_member_id: "lead".into(),
            members: vec![
                TeamMember {
                    id: "lead".into(),
                    name: "Lead".into(),
                    roles: vec![TeamRole::Planner, TeamRole::Reviewer],
                    providers: vec![AgentKind::ClaudeCode],
                },
                TeamMember {
                    id: "dev1".into(),
                    name: "Dev".into(),
                    roles: vec![TeamRole::Developer],
                    providers: vec![AgentKind::Codex],
                },
                TeamMember {
                    id: "dev2".into(),
                    name: "Dev2".into(),
                    roles: vec![TeamRole::Developer],
                    providers: vec![AgentKind::Codex],
                },
                TeamMember {
                    id: "test".into(),
                    name: "Test".into(),
                    roles: vec![TeamRole::Tester],
                    providers: vec![AgentKind::Codex],
                },
            ],
            max_attempts: 40,
            verification_reserve: 4,
            max_reworks: 3,
            max_plan_revisions: 4,
            attempt_timeout_secs: 60,
            write_globs: vec!["**".into()],
            allowed_commands: vec![vec!["node".into()]],
            require_plan_approval: false,
            cross_vendor_review: true,
        }
    }
    fn work(id: &str, path: &str) -> TeamWorkSpec {
        TeamWorkSpec {
            id: id.into(),
            title: id.into(),
            instructions: "implement".into(),
            kind: TeamWorkKind::Implement,
            depends_on: vec![],
            member_ids: vec!["dev1".into()],
            requirement_ids: vec!["r1".into()],
            write_globs: vec![path.into()],
            local_checks: vec![],
        }
    }
    fn plan() -> TeamPlan {
        TeamPlan {
            schema_version: 1,
            rationale: "test".into(),
            items: vec![work("a", "a.js"), work("b", "b.js")],
            criteria: vec![TeamCriterion {
                id: "c1".into(),
                text: "behavior".into(),
                requirement_ids: vec!["r1".into()],
                required: true,
                expected: "works".into(),
                checks: vec![TeamCheck {
                    id: "check".into(),
                    argv: vec!["node".into(), "test.js".into()],
                    expected_exit: 0,
                    stdout_contains: None,
                    timeout_secs: 30,
                }],
                owner_work_item_id: "a".into(),
            }],
        }
    }
    #[test]
    fn m0_contract_has_feasible_same_cli_seats_and_rejects_uncovered_goals() {
        let cfg = config();
        assert!(validate_config(&cfg).is_ok());
        let requirements = vec![TeamRequirement {
            id: "r1".into(),
            original_text: "login".into(),
            source: "user".into(),
            required: true,
        }];
        let p = plan();
        assert!(validate_plan(&p, &requirements, &cfg).is_ok());
        let mut omitted = requirements.clone();
        omitted.push(TeamRequirement {
            id: "r2".into(),
            original_text: "wrong password".into(),
            source: "user".into(),
            required: true,
        });
        assert!(validate_plan(&p, &omitted, &cfg).is_err());
    }
    #[test]
    fn m0_local_revision_preserves_unrelated_work_and_propagates_inputs() {
        let old = plan();
        let mut new = old.clone();
        new.items[0].instructions = "changed interface".into();
        assert_eq!(affected_items(&old, &new), BTreeSet::from(["a".into()]));
        new.items[1].depends_on.push("a".into());
        assert!(affected_items(&old, &new).contains("b"));
    }
    #[test]
    fn m0_cycles_unknown_tools_zero_tests_and_fallback_self_review_fail_closed() {
        let cfg = config();
        let requirements = vec![TeamRequirement {
            id: "r1".into(),
            original_text: "login".into(),
            source: "user".into(),
            required: true,
        }];
        let mut p = plan();
        p.items[0].depends_on.push("b".into());
        p.items[1].depends_on.push("a".into());
        assert!(validate_plan(&p, &requirements, &cfg).is_err());
        p = plan();
        p.criteria[0].checks[0].argv = vec!["sh".into(), "-c".into(), "true".into()];
        assert!(validate_plan(&p, &requirements, &cfg).is_err());
        let c = TeamCandidate {
            id: "c".into(),
            commit_sha: "sha".into(),
            tree_sha: "tree".into(),
            base_sha: "base".into(),
            artifact_ids: vec![],
            contributor_member_ids: vec!["dev1".into()],
            contributor_providers: vec![AgentKind::Codex, AgentKind::ClaudeCode],
            plan_hash: "plan".into(),
        };
        assert!(!independent(
            &cfg.members[0],
            &AgentKind::ClaudeCode,
            TeamWorkKind::Review,
            &c,
            true
        ));
        assert!(!independent(
            &cfg.members[1],
            &AgentKind::Codex,
            TeamWorkKind::Verify,
            &c,
            false
        ));
        assert_eq!(
            criterion_status(&plan().criteria[0], "hash", Some(&c), &[]),
            CriterionStatus::NotRun
        );
    }
}
