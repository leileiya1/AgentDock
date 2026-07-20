use super::*;

async fn git(repo: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .await?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}

async fn init_repo(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    tokio::fs::create_dir_all(path).await?;
    git(path, &["init", "-q", "-b", "main"]).await?;
    git(path, &["config", "user.email", "test@agentflow.local"]).await?;
    git(path, &["config", "user.name", "AgentFlow Test"]).await?;
    tokio::fs::write(path.join("tracked.txt"), "base\n").await?;
    git(path, &["add", "."]).await?;
    git(path, &["commit", "-q", "-m", "base"]).await?;
    Ok(())
}

async fn draft_without_plan(
    orchestrator: &Orchestrator,
    project: &Project,
    title: &str,
) -> Result<TaskSummary, OrchestratorError> {
    orchestrator
        .task_create_governed(
            &project.id,
            title,
            "git compatibility",
            AgentKind::ClaudeCode,
            AgentKind::Codex,
            None,
            None,
            false,
            TaskPolicy {
                require_plan_approval: false,
                ..TaskPolicy::default()
            },
        )
        .await
}

#[tokio::test]
async fn detached_and_moved_repository_reimports_as_the_same_project()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let original = root.path().join("original");
    init_repo(&original).await?;
    git(&original, &["checkout", "--detach", "HEAD"]).await?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let first = orchestrator.project_import(&original).await?;
    assert_eq!(first.default_branch, "main");

    let moved = root.path().join("moved");
    tokio::fs::rename(&original, &moved).await?;
    let second = orchestrator.project_import(&moved).await?;
    assert_eq!(
        second.id, first.id,
        "repository identity was lost after move"
    );
    assert_eq!(
        second.repo_path,
        tokio::fs::canonicalize(&moved).await?.to_string_lossy()
    );
    Ok(())
}

#[tokio::test]
async fn uuid_branch_and_worktree_paths_ignore_legacy_branch_collisions()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let repo = root.path().join("repo");
    init_repo(&repo).await?;
    git(&repo, &["branch", "agentflow/TASK-1"]).await?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator.project_import(&repo).await?;
    let task = draft_without_plan(&orchestrator, &project, "collision").await?;
    let started = orchestrator.task_start(&task.id).await?;
    assert_eq!(started.status, TaskStatus::ReadyForDevelopment);
    let row = orchestrator.task(&task.id).await?;
    let suffix = task.id.replace('-', "").chars().take(8).collect::<String>();
    assert_eq!(
        row.branch.as_deref(),
        Some(format!("agentflow/TASK-1-{suffix}").as_str())
    );
    assert!(
        required_path(&row.worktree_path)?
            .to_string_lossy()
            .contains(&format!("t1-{suffix}"))
    );
    Ok(())
}

#[tokio::test]
async fn sparse_checkout_patterns_are_reapplied_to_the_task_worktree()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let repo = root.path().join("sparse");
    init_repo(&repo).await?;
    tokio::fs::create_dir_all(repo.join("dir-a")).await?;
    tokio::fs::create_dir_all(repo.join("dir-b")).await?;
    tokio::fs::write(repo.join("dir-a/a.txt"), "a\n").await?;
    tokio::fs::write(repo.join("dir-b/b.txt"), "b\n").await?;
    git(&repo, &["add", "."]).await?;
    git(&repo, &["commit", "-q", "-m", "directories"]).await?;
    git(&repo, &["sparse-checkout", "init", "--cone"]).await?;
    git(&repo, &["sparse-checkout", "set", "dir-a"]).await?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator.project_import(&repo).await?;
    let report = orchestrator.project_git_compatibility(&project.id).await?;
    assert!(report.sparse_checkout);
    assert_eq!(report.sparse_patterns, vec!["dir-a"]);
    let task = draft_without_plan(&orchestrator, &project, "sparse").await?;
    orchestrator.task_start(&task.id).await?;
    let worktree = required_path(&orchestrator.task(&task.id).await?.worktree_path)?;
    assert!(worktree.join("dir-a/a.txt").exists());
    assert!(!worktree.join("dir-b/b.txt").exists());
    Ok(())
}

#[tokio::test]
async fn submodules_are_initialized_in_the_isolated_task_worktree()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let child = root.path().join("child");
    init_repo(&child).await?;
    let repo = root.path().join("parent");
    init_repo(&repo).await?;
    git(&repo, &["config", "protocol.file.allow", "always"]).await?;
    git(
        &repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child.to_string_lossy().as_ref(),
            "vendor/child",
        ],
    )
    .await?;
    git(&repo, &["commit", "-q", "-am", "add submodule"]).await?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator.project_import(&repo).await?;
    let report = orchestrator.project_git_compatibility(&project.id).await?;
    assert_eq!(report.submodules, vec!["vendor/child"]);
    let task = draft_without_plan(&orchestrator, &project, "submodule").await?;
    orchestrator.task_start(&task.id).await?;
    let worktree = required_path(&orchestrator.task(&task.id).await?.worktree_path)?;
    assert!(worktree.join("vendor/child/tracked.txt").exists());
    Ok(())
}

#[tokio::test]
async fn shallow_clone_and_lfs_or_ssh_requirements_are_reported_before_execution()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    init_repo(&source).await?;
    tokio::fs::write(
        source.join(".gitattributes"),
        "*.bin filter=lfs diff=lfs merge=lfs -text\n",
    )
    .await?;
    git(&source, &["add", ".gitattributes"]).await?;
    git(&source, &["commit", "-q", "-m", "lfs attributes"]).await?;
    let bare = root.path().join("origin.git");
    let clone = Command::new("git")
        .args(["clone", "-q", "--bare"])
        .arg(&source)
        .arg(&bare)
        .output()
        .await?;
    assert!(clone.status.success());
    let shallow = root.path().join("shallow");
    let clone = Command::new("git")
        .args(["clone", "-q", "--depth", "1"])
        .arg(format!("file://{}", bare.display()))
        .arg(&shallow)
        .output()
        .await?;
    assert!(clone.status.success());
    git(
        &shallow,
        &[
            "remote",
            "set-url",
            "origin",
            "git@example.com:team/repo.git",
        ],
    )
    .await?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator.project_import(&shallow).await?;
    let report = orchestrator.project_git_compatibility(&project.id).await?;
    assert!(report.shallow);
    assert!(report.lfs_tracked);
    assert!(report.ssh_remote);
    assert_eq!(
        report.blockers.is_empty(),
        report.lfs_available,
        "missing git-lfs must be a preflight blocker"
    );
    Ok(())
}
