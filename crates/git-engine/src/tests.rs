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
