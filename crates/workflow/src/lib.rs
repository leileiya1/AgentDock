use agentflow_contracts::{BlockedReason, RunRole, TaskStatus};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowState {
    pub status: TaskStatus,
    pub revision: i64,
    pub max_revisions: i64,
    pub has_revision_commit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowEvent {
    UserStart,
    UserStartWithPlan,
    PlanProposed,
    HumanPlanApprove,
    HumanPlanReject,
    SchedulerSlot,
    DevelopmentSucceeded { has_changes: bool },
    NeedsClarification,
    RunFailed,
    ValidationPassed,
    ValidationFailed,
    ValidationInfraFailed,
    ReviewPassed,
    ReviewChangesRequested,
    ReviewBlocked,
    HumanApprove,
    HumanReject,
    HumanMerge,
    MergeSucceeded,
    MergeConflicted,
    RetryMerge,
    MarkMergedExternal,
    Rollback,
    ResumeWithGuidance,
    ForceApprove,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideEffect {
    InitializeWorktree,
    StartRun(RunRole),
    CommitRevision,
    RunValidation,
    StoreReview,
    StoreApproval,
    Merge,
    AbortMerge,
    CleanupWorktree,
    KillActiveRun,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub status: TaskStatus,
    pub revision: i64,
    pub blocked_reason: Option<BlockedReason>,
    pub effects: Vec<SideEffect>,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("event {event:?} is invalid while task is {status}")]
pub struct InvalidTransition {
    pub status: TaskStatus,
    pub event: WorkflowEvent,
}

fn ready_for_revision(state: &WorkflowState, effects: Vec<SideEffect>) -> Transition {
    if state.revision >= state.max_revisions {
        Transition {
            status: TaskStatus::Blocked,
            revision: state.revision,
            blocked_reason: Some(BlockedReason::MaxRevisions),
            effects,
        }
    } else {
        Transition {
            status: TaskStatus::ReadyForRevision,
            revision: state.revision,
            blocked_reason: None,
            effects,
        }
    }
}

pub fn apply(state: &WorkflowState, event: WorkflowEvent) -> Result<Transition, InvalidTransition> {
    use SideEffect as Fx;
    use TaskStatus as S;
    use WorkflowEvent as E;
    let plain = |status, effects| Transition {
        status,
        revision: state.revision,
        blocked_reason: None,
        effects,
    };
    let blocked = |reason| Transition {
        status: S::Blocked,
        revision: state.revision,
        blocked_reason: Some(reason),
        effects: vec![],
    };
    let out = match (&state.status, &event) {
        (S::Draft, E::UserStart) => plain(S::ReadyForDevelopment, vec![Fx::InitializeWorktree]),
        (S::Draft, E::UserStartWithPlan) => plain(
            S::Planning,
            vec![Fx::InitializeWorktree, Fx::StartRun(RunRole::Planner)],
        ),
        (S::Planning, E::PlanProposed) => plain(S::WaitingForPlanApproval, vec![]),
        (S::WaitingForPlanApproval, E::HumanPlanApprove) => plain(S::ReadyForDevelopment, vec![]),
        (S::WaitingForPlanApproval, E::HumanPlanReject) => {
            plain(S::Planning, vec![Fx::StartRun(RunRole::Planner)])
        }
        (S::ReadyForDevelopment, E::SchedulerSlot) => Transition {
            status: S::Developing,
            revision: state.revision + 1,
            blocked_reason: None,
            effects: vec![Fx::StartRun(RunRole::Developer)],
        },
        (S::ReadyForRevision, E::SchedulerSlot) => Transition {
            status: S::Revising,
            revision: state.revision + 1,
            blocked_reason: None,
            effects: vec![Fx::StartRun(RunRole::Developer)],
        },
        (S::Developing | S::Revising, E::DevelopmentSucceeded { has_changes: true }) => {
            plain(S::Validating, vec![Fx::CommitRevision, Fx::RunValidation])
        }
        (S::Developing | S::Revising, E::DevelopmentSucceeded { has_changes: false }) => {
            blocked(BlockedReason::NoChanges)
        }
        (S::Developing | S::Revising, E::NeedsClarification) => {
            blocked(BlockedReason::NeedsClarification)
        }
        (S::Developing | S::Revising, E::RunFailed) => blocked(BlockedReason::RunFailed),
        (S::Validating, E::SchedulerSlot) => {
            plain(S::Validating, vec![Fx::StartRun(RunRole::Validator)])
        }
        (S::Validating, E::ValidationPassed) => plain(S::ReadyForReview, vec![]),
        (S::Validating, E::ValidationFailed) => ready_for_revision(state, vec![]),
        (S::Validating, E::ValidationInfraFailed) => blocked(BlockedReason::ValidationInfra),
        (S::ReadyForReview, E::SchedulerSlot) => {
            plain(S::Reviewing, vec![Fx::StartRun(RunRole::Reviewer)])
        }
        (S::Reviewing, E::ReviewPassed) => plain(S::WaitingForHumanApproval, vec![Fx::StoreReview]),
        (S::Reviewing, E::ReviewChangesRequested) => {
            ready_for_revision(state, vec![Fx::StoreReview])
        }
        (S::Reviewing, E::ReviewBlocked) => Transition {
            status: S::Blocked,
            revision: state.revision,
            blocked_reason: Some(BlockedReason::ReviewBlock),
            effects: vec![Fx::StoreReview],
        },
        (S::Reviewing, E::RunFailed) => blocked(BlockedReason::ReviewFailed),
        (S::WaitingForHumanApproval, E::HumanApprove) => {
            plain(S::Approved, vec![Fx::StoreApproval])
        }
        (S::WaitingForHumanApproval, E::HumanReject) => {
            ready_for_revision(state, vec![Fx::StoreApproval])
        }
        (S::Approved, E::HumanMerge) => plain(S::Merging, vec![Fx::Merge]),
        (S::Merging, E::MergeSucceeded) => plain(S::Merged, vec![Fx::CleanupWorktree]),
        (S::Merging, E::MergeConflicted) => plain(S::MergeConflict, vec![Fx::AbortMerge]),
        (S::MergeConflict, E::RetryMerge) => plain(S::Merging, vec![Fx::Merge]),
        (S::Approved | S::MergeConflict, E::MarkMergedExternal) => {
            plain(S::Merged, vec![Fx::CleanupWorktree])
        }
        (S::Merged, E::Rollback) => plain(S::RolledBack, vec![]),
        (S::Blocked, E::ResumeWithGuidance) if state.revision == 0 => {
            plain(S::ReadyForDevelopment, vec![])
        }
        (S::Blocked, E::ResumeWithGuidance) => plain(S::ReadyForRevision, vec![]),
        (S::Blocked, E::ForceApprove) if state.has_revision_commit => {
            plain(S::WaitingForHumanApproval, vec![])
        }
        (status, E::Cancel) if !matches!(status, S::Merged | S::RolledBack | S::Cancelled) => {
            plain(S::Cancelled, vec![Fx::KillActiveRun, Fx::CleanupWorktree])
        }
        _ => {
            return Err(InvalidTransition {
                status: state.status,
                event,
            });
        }
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(status: TaskStatus, revision: i64) -> WorkflowState {
        WorkflowState {
            status,
            revision,
            max_revisions: 3,
            has_revision_commit: revision > 0,
        }
    }

    #[test]
    fn happy_path_covers_core_transition_table() {
        let cases = [
            (
                TaskStatus::Draft,
                WorkflowEvent::UserStart,
                TaskStatus::ReadyForDevelopment,
            ),
            (
                TaskStatus::ReadyForDevelopment,
                WorkflowEvent::SchedulerSlot,
                TaskStatus::Developing,
            ),
            (
                TaskStatus::Developing,
                WorkflowEvent::DevelopmentSucceeded { has_changes: true },
                TaskStatus::Validating,
            ),
            (
                TaskStatus::Validating,
                WorkflowEvent::ValidationPassed,
                TaskStatus::ReadyForReview,
            ),
            (
                TaskStatus::ReadyForReview,
                WorkflowEvent::SchedulerSlot,
                TaskStatus::Reviewing,
            ),
            (
                TaskStatus::Reviewing,
                WorkflowEvent::ReviewPassed,
                TaskStatus::WaitingForHumanApproval,
            ),
            (
                TaskStatus::WaitingForHumanApproval,
                WorkflowEvent::HumanApprove,
                TaskStatus::Approved,
            ),
            (
                TaskStatus::Approved,
                WorkflowEvent::HumanMerge,
                TaskStatus::Merging,
            ),
            (
                TaskStatus::Merging,
                WorkflowEvent::MergeSucceeded,
                TaskStatus::Merged,
            ),
        ];
        for (from, event, to) in cases {
            assert_eq!(apply(&state(from, 1), event).map(|t| t.status), Ok(to));
        }
    }

    #[test]
    fn plan_gate_prevents_development_until_human_approval() {
        let started = apply(
            &state(TaskStatus::Draft, 0),
            WorkflowEvent::UserStartWithPlan,
        )
        .map(|value| value.status);
        assert_eq!(started, Ok(TaskStatus::Planning));
        let proposed = apply(&state(TaskStatus::Planning, 0), WorkflowEvent::PlanProposed)
            .map(|value| value.status);
        assert_eq!(proposed, Ok(TaskStatus::WaitingForPlanApproval));
        assert!(
            apply(
                &state(TaskStatus::WaitingForPlanApproval, 0),
                WorkflowEvent::SchedulerSlot
            )
            .is_err()
        );
        let approved = apply(
            &state(TaskStatus::WaitingForPlanApproval, 0),
            WorkflowEvent::HumanPlanApprove,
        )
        .map(|value| value.status);
        assert_eq!(approved, Ok(TaskStatus::ReadyForDevelopment));
    }

    #[test]
    fn failure_and_revision_edges_are_complete() {
        let cases = [
            (
                TaskStatus::Developing,
                WorkflowEvent::DevelopmentSucceeded { has_changes: false },
                TaskStatus::Blocked,
            ),
            (
                TaskStatus::Revising,
                WorkflowEvent::NeedsClarification,
                TaskStatus::Blocked,
            ),
            (
                TaskStatus::Developing,
                WorkflowEvent::RunFailed,
                TaskStatus::Blocked,
            ),
            (
                TaskStatus::Validating,
                WorkflowEvent::ValidationFailed,
                TaskStatus::ReadyForRevision,
            ),
            (
                TaskStatus::Validating,
                WorkflowEvent::ValidationInfraFailed,
                TaskStatus::Blocked,
            ),
            (
                TaskStatus::Reviewing,
                WorkflowEvent::ReviewChangesRequested,
                TaskStatus::ReadyForRevision,
            ),
            (
                TaskStatus::Reviewing,
                WorkflowEvent::ReviewBlocked,
                TaskStatus::Blocked,
            ),
            (
                TaskStatus::Reviewing,
                WorkflowEvent::RunFailed,
                TaskStatus::Blocked,
            ),
            (
                TaskStatus::WaitingForHumanApproval,
                WorkflowEvent::HumanReject,
                TaskStatus::ReadyForRevision,
            ),
            (
                TaskStatus::Merging,
                WorkflowEvent::MergeConflicted,
                TaskStatus::MergeConflict,
            ),
            (
                TaskStatus::MergeConflict,
                WorkflowEvent::RetryMerge,
                TaskStatus::Merging,
            ),
        ];
        for (from, event, to) in cases {
            assert_eq!(apply(&state(from, 1), event).map(|t| t.status), Ok(to));
        }
    }

    #[test]
    fn max_revisions_blocks_loop() {
        let mut s = state(TaskStatus::Reviewing, 3);
        s.max_revisions = 3;
        let transition = apply(&s, WorkflowEvent::ReviewChangesRequested).ok();
        assert_eq!(
            transition.as_ref().map(|v| v.status),
            Some(TaskStatus::Blocked)
        );
        assert_eq!(
            transition.and_then(|v| v.blocked_reason),
            Some(BlockedReason::MaxRevisions)
        );
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        for status in [
            TaskStatus::Draft,
            TaskStatus::Developing,
            TaskStatus::Merged,
            TaskStatus::Cancelled,
        ] {
            assert!(apply(&state(status, 0), WorkflowEvent::HumanApprove).is_err());
        }
        let mut s = state(TaskStatus::Blocked, 0);
        s.has_revision_commit = false;
        assert!(apply(&s, WorkflowEvent::ForceApprove).is_err());
    }
}
