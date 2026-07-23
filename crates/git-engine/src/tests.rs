use super::*;

#[test]
fn flags_control_plane() {
    assert!(is_flagged(".agentflow/project.toml"));
    assert!(is_flagged(".github/workflows/ci.yml"));
    assert!(!is_flagged("src/main.rs"));
}

#[test]
fn commit_guard_recognizes_generated_and_secret_paths() {
    assert!(unsafe_path("node_modules/pkg/index.js"));
    assert!(unsafe_path("frontend/node_modules/pkg/index.js"));
    assert!(unsafe_path(".env.local"));
    assert!(unsafe_path("certificates/client.pem"));
    assert!(!unsafe_path(".env.example"));
    assert!(!unsafe_path("src/main.rs"));
    assert_eq!(
        detected_secret("OPENAI_API_KEY=sk-thisIsARealLookingToken987654"),
        Some("API key")
    );
    assert_eq!(
        detected_secret("OPENAI_API_KEY=sk-test-placeholder-key"),
        None
    );
}

#[tokio::test]
async fn commit_ignores_dependencies_and_blocks_credentials()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let repo = temp.path();
    let git = Git::default();
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "AgentFlow Test"],
    ] {
        git.output(repo, &args).await?;
    }
    tokio::fs::write(repo.join("README.md"), "initial\n").await?;
    git.output(repo, &["add", "README.md"]).await?;
    git.output(repo, &["commit", "-m", "initial"]).await?;
    git.ensure_agentflow_excluded(repo).await?;

    tokio::fs::create_dir_all(repo.join("node_modules/pkg")).await?;
    tokio::fs::create_dir_all(repo.join("src")).await?;
    tokio::fs::write(repo.join("node_modules/pkg/index.js"), "generated\n").await?;
    tokio::fs::write(repo.join("src/main.ts"), "export {};\n").await?;
    git.commit_revision(repo, 1, 1, "safe", "codex").await?;
    let tree = text(
        git.output(repo, &["ls-tree", "-r", "--name-only", "HEAD"])
            .await?,
    )?;
    assert!(tree.contains("src/main.ts"));
    assert!(!tree.contains("node_modules"));

    tokio::fs::write(repo.join(".env"), "TOKEN=secret\n").await?;
    let error = match git.commit_revision(repo, 1, 2, "unsafe", "codex").await {
        Err(error) => error,
        Ok(_) => return Err("credential file was unexpectedly committed".into()),
    };
    assert!(matches!(error, GitError::UnsafeCommit(_)));
    assert!(repo.join(".env").exists());
    assert!(
        git.output(repo, &["diff", "--cached", "--quiet"])
            .await
            .is_ok()
    );

    tokio::fs::remove_file(repo.join(".env")).await?;
    tokio::fs::write(
        repo.join("src/config.ts"),
        "export const key = 'sk-thisIsARealLookingToken987654';\n",
    )
    .await?;
    let error = match git
        .commit_revision(repo, 1, 2, "leaked token", "codex")
        .await
    {
        Err(error) => error,
        Ok(_) => return Err("credential content was unexpectedly committed".into()),
    };
    assert!(matches!(error, GitError::UnsafeCommit(detail) if detail.contains("API key")));
    Ok(())
}

#[tokio::test]
async fn deleted_file_count_counts_only_real_deletions() -> Result<(), Box<dyn std::error::Error>> {
    // §45: the count must reflect files actually removed — not files that merely had lines deleted,
    // which would over-report and cry wolf on ordinary edits.
    let temp = tempfile::tempdir()?;
    let repo = temp.path();
    let git = Git::default();
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "t@t.local"],
        vec!["config", "user.name", "t"],
    ] {
        git.output(repo, &args).await?;
    }
    for name in ["a.rs", "b.rs", "c.rs", "keep.rs"] {
        tokio::fs::write(repo.join(name), "line1\nline2\nline3\n").await?;
    }
    git.output(repo, &["add", "."]).await?;
    git.output(repo, &["commit", "-m", "base"]).await?;
    let base = git.resolve(repo, "HEAD").await?;

    // Delete two files; only remove lines from a third (must NOT count as a deletion).
    tokio::fs::remove_file(repo.join("a.rs")).await?;
    tokio::fs::remove_file(repo.join("b.rs")).await?;
    tokio::fs::write(repo.join("keep.rs"), "line1\n").await?; // lines removed, file kept
    git.output(repo, &["add", "-A"]).await?;
    git.output(repo, &["commit", "-m", "prune"]).await?;
    let head = git.resolve(repo, "HEAD").await?;

    assert_eq!(git.deleted_file_count(repo, &base, &head).await?, 2);
    Ok(())
}

#[tokio::test]
async fn approval_guard_rejects_unsafe_commits_created_before_current_protection()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let repo = temp.path();
    let git = Git::default();
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "AgentFlow Test"],
    ] {
        git.output(repo, &args).await?;
    }
    tokio::fs::write(repo.join("README.md"), "initial\n").await?;
    git.output(repo, &["add", "README.md"]).await?;
    git.output(repo, &["commit", "-m", "initial"]).await?;
    let base = git.resolve(repo, "HEAD").await?;

    // Simulate a revision produced by an older AgentFlow version which staged
    // dependencies directly and therefore bypassed commit_revision's guard.
    tokio::fs::create_dir_all(repo.join("node_modules/pkg")).await?;
    tokio::fs::write(repo.join("node_modules/pkg/index.js"), "generated\n").await?;
    git.output(repo, &["add", "-A"]).await?;
    git.output(repo, &["commit", "-m", "legacy unsafe revision"])
        .await?;
    let sha = git.resolve(repo, "HEAD").await?;

    let error = match git.validate_commit_range(repo, &base, &sha).await {
        Err(error) => error,
        Ok(()) => return Err("approval accepted a legacy unsafe commit".into()),
    };
    assert!(matches!(error, GitError::UnsafeCommit(detail) if detail.contains("node_modules")));
    Ok(())
}
