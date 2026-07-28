struct ExecutionNodeProbe {
    status: NodeStatus,
    platform: Option<String>,
    git_version: Option<String>,
    problem: Option<String>,
    diagnostics: Vec<ExecutionNodeDiagnostic>,
}

async fn probe_execution_node(node: &ExecutionNode) -> ExecutionNodeProbe {
    let mut diagnostics = Vec::new();
    let addresses = probe_dns(node, &mut diagnostics).await;
    if addresses.is_empty() {
        append_skipped_remote_steps(&mut diagnostics, "DNS 失败，未继续连接", node.deny_network);
        return finish_node_probe(diagnostics, None, None);
    }
    if !probe_tcp(&addresses, &mut diagnostics).await {
        append_skipped_remote_steps(&mut diagnostics, "TCP 端口不可达，未继续 SSH", node.deny_network);
        return finish_node_probe(diagnostics, None, None);
    }

    let authentication = remote_diagnostic(
        node,
        NodeDiagnosticStep::SshAuthentication,
        true,
        "SSH 认证成功",
        "printf AGENTFLOW_AUTH_OK",
        10,
    )
    .await;
    let authenticated = authentication.0.status == NodeDiagnosticStatus::Passed;
    diagnostics.push(authentication.0);
    if !authenticated {
        for (step, blocking) in [
            (NodeDiagnosticStep::WorkRoot, true),
            (NodeDiagnosticStep::NetworkIsolation, node.deny_network),
            (NodeDiagnosticStep::Platform, false),
            (NodeDiagnosticStep::Git, false),
            (NodeDiagnosticStep::ArchiveTool, true),
            (NodeDiagnosticStep::Toolchain, false),
        ] {
            diagnostics.push(skipped_diagnostic(step, blocking, "SSH 认证未通过"));
        }
        return finish_node_probe(diagnostics, None, None);
    }

    let probe_dir = format!(
        "{}/.agentflow-health-{}",
        node.work_root.trim_end_matches('/'),
        Uuid::now_v7()
    );
    let root_command = format!(
        "set -eu; {}; mkdir -p {}; test -d {}; test -w {}; mkdir {}; printf ok > {}/write; rm -rf -- {}",
        remote_environment_prelude(),
        shell_quote(&node.work_root),
        shell_quote(&node.work_root),
        shell_quote(&node.work_root),
        shell_quote(&probe_dir),
        shell_quote(&probe_dir),
        shell_quote(&probe_dir),
    );
    let work_root = remote_diagnostic(
        node,
        NodeDiagnosticStep::WorkRoot,
        true,
        "工作目录可创建、写入并清理",
        &root_command,
        12,
    )
    .await;
    diagnostics.push(work_root.0);

    if node.deny_network {
        let isolation = remote_diagnostic(
            node,
            NodeDiagnosticStep::NetworkIsolation,
            true,
            "验证命令将以断网、不可提权身份运行",
            "sudo -n /usr/local/sbin/agentflow-offline -- /bin/sh -eu -c 'command -v curl >/dev/null; test -z \"$(/usr/sbin/ip -4 route show default)\"; test -z \"$(/usr/sbin/ip -6 route show default)\"; ! curl -4 -fsS --connect-timeout 3 https://example.com >/dev/null 2>&1; ! curl -6 -fsS --connect-timeout 3 https://example.com >/dev/null 2>&1; ! sudo -n true >/dev/null 2>&1; printf AGENTFLOW_FAIL_CLOSED_OK'",
            12,
        )
        .await;
        diagnostics.push(isolation.0);
    } else {
        diagnostics.push(skipped_diagnostic(
            NodeDiagnosticStep::NetworkIsolation,
            false,
            "节点未启用断网验证",
        ));
    }

    let platform = remote_diagnostic(
        node,
        NodeDiagnosticStep::Platform,
        false,
        "已读取远端平台",
        "uname -srm",
        8,
    )
    .await;
    let platform_value = platform.1;
    diagnostics.push(platform.0);

    let git = remote_diagnostic(
        node,
        NodeDiagnosticStep::Git,
        false,
        "Git 可用",
        "git --version",
        8,
    )
    .await;
    let git_version = git.1;
    diagnostics.push(git.0);

    let archive = remote_diagnostic(
        node,
        NodeDiagnosticStep::ArchiveTool,
        true,
        "tar 归档工具可用",
        "tar --version | head -n 1",
        8,
    )
    .await;
    diagnostics.push(archive.0);

    let toolchain = remote_diagnostic(
        node,
        NodeDiagnosticStep::Toolchain,
        false,
        "已盘点常用验证工具",
        "for tool in bun node cargo rustc python3; do if command -v \"$tool\" >/dev/null 2>&1; then \"$tool\" --version 2>&1 | head -n 1 | sed \"s/^/$tool: /\"; fi; done",
        12,
    )
    .await;
    diagnostics.push(toolchain.0);
    finish_node_probe(diagnostics, platform_value, git_version)
}

async fn probe_dns(
    node: &ExecutionNode,
    diagnostics: &mut Vec<ExecutionNodeDiagnostic>,
) -> Vec<std::net::SocketAddr> {
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(4),
        tokio::net::lookup_host((node.host.as_str(), node.port)),
    )
    .await;
    let checked_at = Utc::now().to_rfc3339();
    match result {
        Ok(Ok(addresses)) => {
            let mut addresses = addresses.collect::<Vec<_>>();
            addresses.sort();
            addresses.dedup();
            let detail = addresses
                .iter()
                .take(4)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            diagnostics.push(ExecutionNodeDiagnostic {
                step: NodeDiagnosticStep::Dns,
                status: if addresses.is_empty() {
                    NodeDiagnosticStatus::Failed
                } else {
                    NodeDiagnosticStatus::Passed
                },
                blocking: true,
                summary: if addresses.is_empty() {
                    "主机名没有可用地址".into()
                } else {
                    format!("解析到 {} 个地址", addresses.len())
                },
                detail: (!detail.is_empty()).then_some(detail),
                duration_ms: duration_ms(started.elapsed()),
                checked_at,
            });
            addresses
        }
        Ok(Err(error)) => {
            diagnostics.push(failed_diagnostic(
                NodeDiagnosticStep::Dns,
                true,
                "DNS 解析失败",
                error.to_string(),
                started.elapsed(),
                checked_at,
            ));
            Vec::new()
        }
        Err(_) => {
            diagnostics.push(failed_diagnostic(
                NodeDiagnosticStep::Dns,
                true,
                "DNS 解析超时",
                "4 秒内没有完成解析".into(),
                started.elapsed(),
                checked_at,
            ));
            Vec::new()
        }
    }
}

async fn probe_tcp(
    addresses: &[std::net::SocketAddr],
    diagnostics: &mut Vec<ExecutionNodeDiagnostic>,
) -> bool {
    let started = Instant::now();
    let mut last_error = None;
    for address in addresses {
        match tokio::time::timeout(Duration::from_secs(4), tokio::net::TcpStream::connect(address))
            .await
        {
            Ok(Ok(_)) => {
                diagnostics.push(ExecutionNodeDiagnostic {
                    step: NodeDiagnosticStep::Tcp,
                    status: NodeDiagnosticStatus::Passed,
                    blocking: true,
                    summary: "SSH TCP 端口可达".into(),
                    detail: Some(address.to_string()),
                    duration_ms: duration_ms(started.elapsed()),
                    checked_at: Utc::now().to_rfc3339(),
                });
                return true;
            }
            Ok(Err(error)) => last_error = Some(error.to_string()),
            Err(_) => last_error = Some("连接超时".into()),
        }
    }
    diagnostics.push(failed_diagnostic(
        NodeDiagnosticStep::Tcp,
        true,
        "SSH TCP 端口不可达",
        last_error.unwrap_or_else(|| "没有可尝试的地址".into()),
        started.elapsed(),
        Utc::now().to_rfc3339(),
    ));
    false
}

async fn remote_diagnostic(
    node: &ExecutionNode,
    step: NodeDiagnosticStep,
    blocking: bool,
    success_summary: &str,
    command: &str,
    timeout_secs: u64,
) -> (ExecutionNodeDiagnostic, Option<String>) {
    let started = Instant::now();
    let destination = format!("{}@{}", node.username, node.host);
    let result = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        Command::new("ssh")
            .args(ssh_base_args(node))
            .arg(destination)
            .arg(command)
            .output(),
    )
    .await;
    let checked_at = Utc::now().to_rfc3339();
    match result {
        Ok(Ok(output)) if output.status.success() => {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (
                ExecutionNodeDiagnostic {
                    step,
                    status: NodeDiagnosticStatus::Passed,
                    blocking,
                    summary: success_summary.into(),
                    detail: (!value.is_empty()).then_some(value.clone()),
                    duration_ms: duration_ms(started.elapsed()),
                    checked_at,
                },
                (!value.is_empty()).then_some(value),
            )
        }
        Ok(Ok(output)) => (
            failed_diagnostic(
                step,
                blocking,
                "远端检查未通过",
                String::from_utf8_lossy(&output.stderr).chars().take(1000).collect(),
                started.elapsed(),
                checked_at,
            ),
            None,
        ),
        Ok(Err(error)) => (
            failed_diagnostic(
                step,
                blocking,
                "无法启动 SSH 检查",
                error.to_string(),
                started.elapsed(),
                checked_at,
            ),
            None,
        ),
        Err(_) => (
            failed_diagnostic(
                step,
                blocking,
                "远端检查超时",
                format!("{timeout_secs} 秒内没有完成"),
                started.elapsed(),
                checked_at,
            ),
            None,
        ),
    }
}

fn finish_node_probe(
    diagnostics: Vec<ExecutionNodeDiagnostic>,
    platform: Option<String>,
    git_version: Option<String>,
) -> ExecutionNodeProbe {
    let failed = diagnostics
        .iter()
        .find(|item| item.blocking && item.status == NodeDiagnosticStatus::Failed);
    ExecutionNodeProbe {
        status: if failed.is_some() {
            NodeStatus::Offline
        } else {
            NodeStatus::Online
        },
        platform,
        git_version,
        problem: failed.map(|item| item.detail.clone().unwrap_or_else(|| item.summary.clone())),
        diagnostics,
    }
}

fn append_skipped_remote_steps(
    diagnostics: &mut Vec<ExecutionNodeDiagnostic>,
    reason: &str,
    deny_network: bool,
) {
    for (step, blocking) in [
        (NodeDiagnosticStep::SshAuthentication, true),
        (NodeDiagnosticStep::WorkRoot, true),
        (NodeDiagnosticStep::NetworkIsolation, deny_network),
        (NodeDiagnosticStep::Platform, false),
        (NodeDiagnosticStep::Git, false),
        (NodeDiagnosticStep::ArchiveTool, true),
        (NodeDiagnosticStep::Toolchain, false),
    ] {
        diagnostics.push(skipped_diagnostic(step, blocking, reason));
    }
}

fn skipped_diagnostic(
    step: NodeDiagnosticStep,
    blocking: bool,
    reason: &str,
) -> ExecutionNodeDiagnostic {
    ExecutionNodeDiagnostic {
        step,
        status: NodeDiagnosticStatus::Skipped,
        blocking,
        summary: "未执行".into(),
        detail: Some(reason.into()),
        duration_ms: 0,
        checked_at: Utc::now().to_rfc3339(),
    }
}

fn failed_diagnostic(
    step: NodeDiagnosticStep,
    blocking: bool,
    summary: &str,
    detail: String,
    elapsed: Duration,
    checked_at: String,
) -> ExecutionNodeDiagnostic {
    ExecutionNodeDiagnostic {
        step,
        status: NodeDiagnosticStatus::Failed,
        blocking,
        summary: summary.into(),
        detail: Some(agentflow_process_supervisor::redact(detail)),
        duration_ms: duration_ms(elapsed),
        checked_at,
    }
}

fn duration_ms(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod execution_node_diagnostic_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires AGENTFLOW_TEST_SSH_HOST, USER and ROOT"]
    async fn live_remote_node_diagnostics_are_stepwise_and_cleanup_the_probe()
    -> Result<(), Box<dyn std::error::Error>> {
        let node = ExecutionNode {
            id: "live-diagnostic".into(),
            name: "live diagnostic".into(),
            host: std::env::var("AGENTFLOW_TEST_SSH_HOST")?,
            port: std::env::var("AGENTFLOW_TEST_SSH_PORT")
                .unwrap_or_else(|_| "22".into())
                .parse()?,
            username: std::env::var("AGENTFLOW_TEST_SSH_USER")?,
            work_root: std::env::var("AGENTFLOW_TEST_SSH_ROOT")?,
            identity_file: std::env::var("AGENTFLOW_TEST_SSH_IDENTITY").ok(),
            deny_network: std::env::var("AGENTFLOW_TEST_SSH_DENY_NETWORK").as_deref() == Ok("1"),
            enabled: true,
            status: NodeStatus::Unknown,
            platform: None,
            git_version: None,
            problem: None,
            last_checked_at: None,
            diagnostics: Vec::new(),
        };
        let probe = probe_execution_node(&node).await;
        println!("{}", serde_json::to_string_pretty(&probe.diagnostics)?);
        assert_eq!(probe.status, NodeStatus::Online, "{:?}", probe.problem);
        assert!(
            probe
                .diagnostics
                .iter()
                .filter(|item| item.blocking)
                .all(|item| item.status == NodeDiagnosticStatus::Passed)
        );
        Ok(())
    }
}
