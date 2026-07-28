/// Per-start session secret for the local IPC socket.
///
/// The socket alone is not an authorization boundary: every process running as the same user can
/// connect to it, and the daemon exposes commands that carry human authority — approving a
/// permission request, force-approving a task, merging, restoring a database backup. Requiring a
/// secret that lives in a 0600 file inside the (0700) data directory means an unrelated local
/// process cannot forge those decisions just by finding the socket path.
///
/// Residual risk, deliberately documented rather than hidden: a Provider CLI started by AgentFlow
/// runs as the same user and receives HOME, so it can still read the token file. Closing that gap
/// requires per-child credential isolation (a sandbox or a broker fd) and is tracked separately.
const TOKEN_FILE: &str = "agentflowd.token";

/// Wire envelope. Every request carries the session token; the daemon rejects anything else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuthenticatedRequest {
    pub token: String,
    #[serde(flatten)]
    pub request: DaemonRequest,
}

pub(crate) fn token_path(data_dir: &Path) -> PathBuf {
    data_dir.join(TOKEN_FILE)
}

/// Generates a fresh token for this daemon start and publishes it for local clients.
/// Writing to a temporary file and renaming keeps readers from ever seeing a partial or
/// world-readable token, and rotating per start invalidates a token captured from an old run.
pub(crate) async fn publish_session_token(data_dir: &Path) -> Result<String, DaemonError> {
    let mut raw = [0_u8; 32];
    getrandom::fill(&mut raw).map_err(|error| DaemonError::Protocol(error.to_string()))?;
    let token = raw.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    let path = token_path(data_dir);
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    tokio::fs::write(&temporary, token.as_bytes()).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600)).await?;
    }
    tokio::fs::rename(&temporary, &path).await?;
    Ok(token)
}

pub(crate) async fn read_session_token(data_dir: &Path) -> Result<String, DaemonError> {
    let token = tokio::fs::read_to_string(token_path(data_dir))
        .await
        .map_err(|error| {
            DaemonError::Protocol(format!(
                "cannot read the agentflowd session token ({error}); is the background service running?"
            ))
        })?;
    Ok(token.trim().to_string())
}

/// Length-independent comparison so a caller cannot learn the token byte by byte from timing.
pub(crate) fn token_matches(expected: &str, presented: &str) -> bool {
    if expected.len() != presented.len() {
        return false;
    }
    expected
        .bytes()
        .zip(presented.bytes())
        .fold(0_u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

/// The uid the daemon runs as. `getuid` has no safe std wrapper, but a file the daemon just
/// created is owned by its effective user, so the freshly written token file answers the same
/// question without unsafe code.
#[cfg(unix)]
pub(crate) async fn own_uid(data_dir: &Path) -> Result<u32, DaemonError> {
    use std::os::unix::fs::MetadataExt;
    Ok(tokio::fs::metadata(token_path(data_dir)).await?.uid())
}

/// Restricts the data directory so the socket and token are unreachable for other users even
/// during the window between `bind` and the socket's own `chmod`.
pub(crate) async fn restrict_data_dir(data_dir: &Path) -> Result<(), DaemonError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(data_dir, std::fs::Permissions::from_mode(0o700)).await?;
    }
    #[cfg(not(unix))]
    let _ = data_dir;
    Ok(())
}

#[cfg(test)]
mod ipc_auth_tests {
    use super::*;

    #[tokio::test]
    async fn a_published_token_round_trips_and_only_matches_itself()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let token = publish_session_token(temp.path()).await?;
        assert_eq!(token.len(), 64);
        assert_eq!(read_session_token(temp.path()).await?, token);
        assert!(token_matches(&token, &token));
        assert!(!token_matches(&token, "short"));
        assert!(!token_matches(&token, &"0".repeat(64)));

        // Each daemon start rotates the secret, so a token captured earlier stops working.
        let rotated = publish_session_token(temp.path()).await?;
        assert_ne!(rotated, token);
        assert!(!token_matches(&rotated, &token));
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_token_file_is_never_readable_by_other_users()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir()?;
        publish_session_token(temp.path()).await?;
        let mode = tokio::fs::metadata(token_path(temp.path()))
            .await?
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "token file mode {mode:o} exposes the secret");
        restrict_data_dir(temp.path()).await?;
        let dir_mode = tokio::fs::metadata(temp.path()).await?.permissions().mode();
        assert_eq!(dir_mode & 0o077, 0, "data dir mode {dir_mode:o} is too open");
        Ok(())
    }
}
