use agentflow_contracts::{DiffPayload, DiffStat, FileDiff};
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::{
    path::{Component, Path, PathBuf},
    process::Stdio,
};
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("git I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("git {operation} failed: {stderr}")]
    Failed { operation: String, stderr: String },
    #[error("invalid git output: {0}")]
    InvalidOutput(String),
    #[error("invalid exclude glob {0}")]
    InvalidGlob(String),
    #[error("commit protection blocked the revision: {0}")]
    UnsafeCommit(String),
}

const MAX_COMMIT_FILES: usize = 200;
const MAX_COMMIT_BYTES: u64 = 20 * 1024 * 1024;
const MAX_SINGLE_FILE_BYTES: u64 = 5 * 1024 * 1024;
const SAFETY_EXCLUDES: &[&str] = &[
    "node_modules/",
    "target/",
    ".venv/",
    "venv/",
    "__pycache__/",
    ".pytest_cache/",
    ".mypy_cache/",
    ".bun-cache/",
    ".bun-tmp/",
    ".DS_Store",
];

#[derive(Clone, Debug)]
pub struct Git {
    executable: PathBuf,
}
impl Default for Git {
    fn default() -> Self {
        Self {
            executable: PathBuf::from("git"),
        }
    }
}

impl Git {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }
    async fn output(&self, cwd: &Path, args: &[&str]) -> Result<Vec<u8>, GitError> {
        let out = Command::new(&self.executable)
            .args(args)
            .current_dir(cwd)
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .output()
            .await?;
        if !out.status.success() {
            return Err(GitError::Failed {
                operation: args.join(" "),
                stderr: String::from_utf8_lossy(&out.stderr).trim().into(),
            });
        }
        Ok(out.stdout)
    }
    pub async fn is_repo(&self, path: &Path) -> bool {
        self.output(path, &["rev-parse", "--git-dir"]).await.is_ok()
    }
    pub async fn default_branch(&self, repo: &Path) -> Result<String, GitError> {
        if let Ok(symbolic) = self
            .output(repo, &["symbolic-ref", "--short", "HEAD"])
            .await
        {
            return text(symbolic);
        }
        if let Ok(symbolic) = self
            .output(
                repo,
                &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
            )
            .await
        {
            return text(symbolic).map(|value| value.trim_start_matches("origin/").to_string());
        }
        for candidate in ["main", "master", "trunk"] {
            if self.resolve(repo, candidate).await.is_ok() {
                return Ok(candidate.into());
            }
        }
        Err(GitError::InvalidOutput(
            "cannot determine a default branch from detached HEAD".into(),
        ))
    }
    pub async fn resolve(&self, repo: &Path, reference: &str) -> Result<String, GitError> {
        text(self.output(repo, &["rev-parse", reference]).await?)
    }
    /// Returns whether `ancestor` is reachable from `descendant`.
    ///
    /// Delivery uses this instead of trusting a caller-provided "merged" flag: a Git
    /// commit is only considered delivered after the target branch proves ancestry.
    pub async fn is_ancestor(
        &self,
        repo: &Path,
        ancestor: &str,
        descendant: &str,
    ) -> Result<bool, GitError> {
        let status = Command::new(&self.executable)
            .args(["merge-base", "--is-ancestor", ancestor, descendant])
            .current_dir(repo)
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .await?;
        match status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(GitError::Failed {
                operation: "merge-base --is-ancestor".into(),
                stderr: format!("git exited with {status}"),
            }),
        }
    }
    pub async fn worktree_add(
        &self,
        repo: &Path,
        path: &Path,
        branch: &str,
        base: &str,
    ) -> Result<(), GitError> {
        let path = path.to_string_lossy();
        self.output(repo, &["worktree", "add", &path, "-b", branch, base])
            .await
            .map(|_| ())
    }
    pub async fn worktree_add_existing(
        &self,
        repo: &Path,
        path: &Path,
        branch: &str,
    ) -> Result<(), GitError> {
        let path = path.to_string_lossy();
        self.output(repo, &["worktree", "prune"]).await?;
        self.output(repo, &["worktree", "add", &path, branch])
            .await
            .map(|_| ())
    }
    pub async fn worktree_remove(&self, repo: &Path, path: &Path) -> Result<(), GitError> {
        let path = path.to_string_lossy();
        self.output(repo, &["worktree", "remove", "--force", &path])
            .await
            .map(|_| ())
    }
    pub async fn ensure_agentflow_excluded(&self, repo: &Path) -> Result<(), GitError> {
        let git_dir = text(
            self.output(repo, &["rev-parse", "--git-common-dir"])
                .await?,
        )?;
        let base = if Path::new(&git_dir).is_absolute() {
            PathBuf::from(git_dir)
        } else {
            repo.join(git_dir)
        };
        let info = base.join("info");
        tokio::fs::create_dir_all(&info).await?;
        let target = info.join("exclude");
        let existing = tokio::fs::read_to_string(&target).await.unwrap_or_default();
        let mut next = existing.clone();
        for line in ["/.agentflow-in/", "/.agentflow-out/"]
            .into_iter()
            .chain(SAFETY_EXCLUDES.iter().copied())
        {
            if !existing.lines().any(|v| v.trim() == line) {
                if !next.is_empty() && !next.ends_with('\n') {
                    next.push('\n');
                }
                next.push_str(line);
                next.push('\n');
            }
        }
        if next != existing {
            tokio::fs::write(target, next).await?;
        }
        Ok(())
    }
    pub async fn has_changes(&self, worktree: &Path) -> Result<bool, GitError> {
        Ok(!self
            .output(worktree, &["status", "--porcelain", "-z"])
            .await?
            .is_empty())
    }
    /// Lists tracked and untracked files changed from `HEAD`, excluding ignored runtime files.
    pub async fn changed_paths(&self, worktree: &Path) -> Result<Vec<String>, GitError> {
        let tracked = self
            .output(worktree, &["diff", "HEAD", "--name-only", "-z"])
            .await?;
        let untracked = self
            .output(
                worktree,
                &["ls-files", "--others", "--exclude-standard", "-z"],
            )
            .await?;
        let mut paths = tracked
            .split(|byte| *byte == 0)
            .chain(untracked.split(|byte| *byte == 0))
            .filter(|path| !path.is_empty())
            .map(|path| {
                String::from_utf8(path.to_vec())
                    .map_err(|error| GitError::InvalidOutput(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
    pub async fn working_patch(&self, worktree: &Path) -> Result<Vec<u8>, GitError> {
        self.output(worktree, &["diff", "--binary", "HEAD"]).await
    }
    pub async fn untracked_files(&self, worktree: &Path) -> Result<Vec<PathBuf>, GitError> {
        let raw = self
            .output(
                worktree,
                &["ls-files", "--others", "--exclude-standard", "-z"],
            )
            .await?;
        raw.split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| {
                String::from_utf8(path.to_vec())
                    .map(PathBuf::from)
                    .map_err(|error| GitError::InvalidOutput(error.to_string()))
            })
            .collect()
    }
    pub async fn is_clean(&self, repo: &Path) -> Result<bool, GitError> {
        self.has_changes(repo).await.map(|v| !v)
    }
    pub async fn reset_owned_worktree(
        &self,
        worktree: &Path,
        commit: &str,
    ) -> Result<(), GitError> {
        self.output(worktree, &["reset", "--hard", commit]).await?;
        // AgentFlow owns this dedicated worktree, so ignored build output must also be removed;
        // otherwise a failed provider can contaminate the next provider's attempt.
        self.output(worktree, &["clean", "-fdx"]).await?;
        Ok(())
    }
    pub async fn commit_revision(
        &self,
        wt: &Path,
        seq: i64,
        revision: i64,
        title: &str,
        agent: &str,
    ) -> Result<String, GitError> {
        self.output(wt, &["add", "-A"]).await?;
        if let Err(error) = self.validate_staged_commit(wt).await {
            // Keep the developer's files intact while returning the index to a reviewable state.
            let _ = self.output(wt, &["reset"]).await;
            return Err(error);
        }
        let message = format!("[agentflow] TASK-{seq} r{revision}: {title}");
        let author = format!("AgentFlow ({agent}) <agent@agentflow.local>");
        self.output(wt, &["commit", "--author", &author, "-m", &message])
            .await?;
        self.resolve(wt, "HEAD").await
    }

    async fn validate_staged_commit(&self, wt: &Path) -> Result<(), GitError> {
        let raw = self
            .output(wt, &["diff", "--cached", "--name-only", "-z"])
            .await?;
        let paths = raw
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8(path.to_vec()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| GitError::InvalidOutput(error.to_string()))?;

        let mut violations = Vec::new();
        if paths.len() > MAX_COMMIT_FILES {
            violations.push(format!(
                "{} files exceed the {} file limit",
                paths.len(),
                MAX_COMMIT_FILES
            ));
        }
        let mut total_bytes = 0_u64;
        for path in &paths {
            if unsafe_path(path) {
                violations.push(format!("unsafe generated or credential path: {path}"));
            }
            let absolute = wt.join(path);
            let Ok(metadata) = std::fs::symlink_metadata(&absolute) else {
                continue;
            };
            if metadata.is_file() {
                total_bytes = total_bytes.saturating_add(metadata.len());
                if metadata.len() > MAX_SINGLE_FILE_BYTES {
                    violations.push(format!(
                        "{path} is {} bytes (single-file limit is {})",
                        metadata.len(),
                        MAX_SINGLE_FILE_BYTES
                    ));
                }
                if metadata.len() <= 1024 * 1024 {
                    let staged = self.output(wt, &["show", &format!(":{path}")]).await?;
                    if let Some(kind) = detected_secret(&String::from_utf8_lossy(&staged)) {
                        // Report only the credential class, never the matched value.
                        violations.push(format!("possible {kind} credential in {path}"));
                    }
                }
            }
            if violations.len() >= 8 {
                break;
            }
        }
        if total_bytes > MAX_COMMIT_BYTES {
            violations.push(format!(
                "staged files total {total_bytes} bytes (limit is {MAX_COMMIT_BYTES})"
            ));
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(GitError::UnsafeCommit(violations.join("; ")))
        }
    }
    pub async fn full_patch(
        &self,
        repo: &Path,
        base: &str,
        sha: &str,
    ) -> Result<Vec<u8>, GitError> {
        self.output(
            repo,
            &[
                "diff",
                "--find-renames",
                "--no-color",
                &format!("{base}..{sha}"),
            ],
        )
        .await
    }
    pub async fn diff(
        &self,
        repo: &Path,
        base: &str,
        sha: &str,
        exclude: &[String],
        max_bytes: usize,
    ) -> Result<DiffPayload, GitError> {
        let full = self.full_patch(repo, base, sha).await?;
        let hash = format!("{:x}", Sha256::digest(&full));
        let set = glob_set(exclude)?;
        let numstat = text(
            self.output(
                repo,
                &[
                    "diff",
                    "--numstat",
                    "--find-renames",
                    &format!("{base}..{sha}"),
                ],
            )
            .await?,
        )?;
        let mut files = Vec::new();
        for line in numstat.lines() {
            let mut p = line.splitn(3, '\t');
            let ins = p.next().unwrap_or("-");
            let del = p.next().unwrap_or("-");
            let path = p.next().unwrap_or("").to_string();
            if path.is_empty() || set.is_match(&path) {
                continue;
            }
            let binary = ins == "-" || del == "-";
            let flagged = is_flagged(&path);
            let patch = if binary || full.len() > max_bytes {
                None
            } else {
                Some(file_patch(&String::from_utf8_lossy(&full), &path))
            };
            files.push(FileDiff {
                path,
                old_path: None,
                binary,
                flagged,
                insertions: ins.parse().unwrap_or(0),
                deletions: del.parse().unwrap_or(0),
                patch,
            });
        }
        Ok(DiffPayload {
            base_commit: base.into(),
            commit_sha: sha.into(),
            diff_sha256: hash,
            truncated: full.len() > max_bytes,
            files,
        })
    }
    pub async fn merge(&self, repo: &Path, branch: &str, message: &str) -> Result<(), GitError> {
        self.output(repo, &["merge", "--no-ff", branch, "-m", message])
            .await
            .map(|_| ())
    }
    pub async fn abort_merge(&self, repo: &Path) -> Result<(), GitError> {
        self.output(repo, &["merge", "--abort"]).await.map(|_| ())
    }
}

include!("operations.rs");
include!("compatibility.rs");

fn text(bytes: Vec<u8>) -> Result<String, GitError> {
    String::from_utf8(bytes)
        .map(|v| v.trim().into())
        .map_err(|e| GitError::InvalidOutput(e.to_string()))
}
fn glob_set(patterns: &[String]) -> Result<GlobSet, GitError> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        b.add(Glob::new(p).map_err(|_| GitError::InvalidGlob(p.clone()))?);
    }
    b.build()
        .map_err(|_| GitError::InvalidGlob("glob set".into()))
}
fn is_flagged(path: &str) -> bool {
    path == "CLAUDE.md"
        || path == "AGENTS.md"
        || path.starts_with(".agentflow/")
        || path.starts_with(".claude/")
        || path.starts_with(".github/workflows/")
        || path == ".gitlab-ci.yml"
}

fn unsafe_path(path: &str) -> bool {
    let candidate = Path::new(path);
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return true;
    }
    let components = candidate
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.iter().any(|component| {
        matches!(
            *component,
            "node_modules"
                | "target"
                | ".venv"
                | "venv"
                | "__pycache__"
                | ".bun-cache"
                | ".bun-tmp"
        )
    }) {
        return true;
    }
    let Some(name) = components.last().copied() else {
        return true;
    };
    let lower = name.to_ascii_lowercase();
    let safe_env_template = matches!(
        lower.as_str(),
        ".env.example" | ".env.sample" | ".env.template"
    );
    (!safe_env_template && (lower == ".env" || lower.starts_with(".env.")))
        || matches!(
            lower.as_str(),
            "id_rsa"
                | "id_ed25519"
                | "credentials"
                | "credentials.json"
                | "secrets.json"
                | ".npmrc"
                | ".pypirc"
                | ".netrc"
        )
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
}

fn detected_secret(text: &str) -> Option<&'static str> {
    let patterns = [
        ("private key", r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
        ("AWS access key", r"AKIA[0-9A-Z]{16}"),
        ("GitHub token", r"gh[pousr]_[A-Za-z0-9_]{20,}"),
        ("API key", r"sk-[A-Za-z0-9_-]{16,}"),
        (
            "environment secret",
            r#"(?im)^\s*(?:OPENAI_API_KEY|ANTHROPIC_API_KEY|DEEPSEEK_API_KEY|AWS_SECRET_ACCESS_KEY|GITHUB_TOKEN|NPM_TOKEN)\s*[:=]\s*["']?([^\s"'#]{12,})"#,
        ),
    ];
    for (label, pattern) in patterns {
        let Ok(regex) = Regex::new(pattern) else {
            continue;
        };
        for matched in regex.find_iter(text) {
            if !looks_like_placeholder(matched.as_str()) {
                return Some(label);
            }
        }
    }
    None
}

fn looks_like_placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "test",
        "fake",
        "dummy",
        "example",
        "sample",
        "placeholder",
        "redacted",
        "your_",
        "your-",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}
fn file_patch(all: &str, path: &str) -> String {
    let marker = format!("diff --git a/{path} b/{path}");
    let Some(start) = all.find(&marker) else {
        return String::new();
    };
    let tail = &all[start..];
    let end = tail[marker.len()..]
        .find("\ndiff --git ")
        .map(|v| v + marker.len())
        .unwrap_or(tail.len());
    tail[..end].to_string()
}

pub fn summarize(payload: &DiffPayload) -> DiffStat {
    DiffStat {
        files: payload.files.len() as i64,
        insertions: payload.files.iter().map(|f| f.insertions).sum(),
        deletions: payload.files.iter().map(|f| f.deletions).sum(),
        flagged: payload
            .files
            .iter()
            .filter(|f| f.flagged)
            .map(|f| f.path.clone())
            .collect(),
    }
}

#[cfg(test)]
mod tests;
