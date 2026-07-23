//! macOS distribution gate: turn `codesign`/`spctl`/`hdiutil` output into an actionable pass/fail
//! verdict so an improperly-signed app never ships.
//!
//! The reported failure — `codesign --verify --deep --strict` reporting *"code has no resources but
//! signature indicates they must be present"* and `spctl --assess` rejecting the app — only appears
//! on machines other than the one that built it. This crate owns no signing key; it just decides,
//! from tool output, whether a produced bundle is safe to distribute. The classifiers are pure and
//! unit-tested; `verify_bundle` shells out to the real tools and applies them.

use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

/// The outcome of one release check. `Fail` always carries a human, actionable reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    Pass,
    Fail(String),
}

impl GateVerdict {
    pub fn is_pass(&self) -> bool {
        matches!(self, GateVerdict::Pass)
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            GateVerdict::Fail(reason) => Some(reason),
            GateVerdict::Pass => None,
        }
    }
}

/// The full set of checks run against a bundle. Every check must pass for the gate to pass.
#[derive(Debug, Clone)]
pub struct GateReport {
    pub checks: Vec<(String, GateVerdict)>,
}

impl GateReport {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|(_, verdict)| verdict.is_pass())
    }
}

fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("未知错误")
}

/// Classify `codesign --verify --deep --strict` against an `.app` bundle.
pub fn classify_codesign_verify(exit_success: bool, stderr: &str) -> GateVerdict {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("code has no resources but signature indicates they must be present") {
        return GateVerdict::Fail(
            "签名与资源清单不一致（code has no resources but signature indicates they must be \
             present）：请用完整的 Developer ID Application 证书对 .app 重新签名，不要复制或改动已\
             签名的包。"
                .into(),
        );
    }
    if lower.contains("not signed at all") {
        return GateVerdict::Fail("应用尚未签名：发布前必须用 Developer ID 证书签名。".into());
    }
    if exit_success {
        GateVerdict::Pass
    } else {
        GateVerdict::Fail(format!(
            "codesign --verify --deep --strict 失败：{}",
            first_line(stderr)
        ))
    }
}

/// Classify `codesign -d --verbose=4` display output. Ad-hoc signatures pass on the build machine
/// but are rejected by Gatekeeper everywhere else, so they must fail the gate before release.
pub fn classify_codesign_authority(display: &str) -> GateVerdict {
    let lower = display.to_ascii_lowercase();
    if lower.contains("signature=adhoc") || lower.contains("signature=ad-hoc") {
        return GateVerdict::Fail(
            "当前是 ad-hoc 临时签名，其他 Mac 无法通过 Gatekeeper：请改用 Developer ID 签名。"
                .into(),
        );
    }
    if lower.contains("developer id application") {
        return GateVerdict::Pass;
    }
    if let Some(authority) = display
        .lines()
        .find_map(|line| line.trim().strip_prefix("Authority="))
    {
        return GateVerdict::Fail(format!(
            "签名主体是「{authority}」而非 Developer ID Application，对外分发前需改用发布证书。"
        ));
    }
    GateVerdict::Fail("未找到签名主体（Authority=），应用可能未正确签名。".into())
}

/// Classify `spctl --assess --type execute`. Gatekeeper only accepts a notarized Developer ID app.
pub fn classify_gatekeeper_assess(exit_success: bool, output: &str) -> GateVerdict {
    let lower = output.to_ascii_lowercase();
    if exit_success && lower.contains("accepted") {
        return GateVerdict::Pass;
    }
    if lower.contains("rejected") && !lower.contains("notariz") {
        return GateVerdict::Fail(
            "Gatekeeper 拒绝（rejected）：应用未经过 notarization，请完成 Developer ID 签名并公证。"
                .into(),
        );
    }
    GateVerdict::Fail(format!(
        "spctl --assess 未接受该应用：{}",
        first_line(output)
    ))
}

/// Compare an artifact digest against the expected value. Case-insensitive hex comparison.
pub fn verify_sha256(expected: &str, actual: &str) -> GateVerdict {
    if expected.eq_ignore_ascii_case(actual) {
        GateVerdict::Pass
    } else {
        GateVerdict::Fail(format!(
            "DMG SHA-256 不匹配：期望 {expected}，实际 {actual}"
        ))
    }
}

fn run(program: &str, args: &[&str], target: &Path) -> std::io::Result<(bool, String)> {
    let output = Command::new(program).args(args).arg(target).output()?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok((output.status.success(), combined))
}

/// Run every release check against a built bundle. Shells out to the real macOS tools, so this only
/// produces meaningful results on macOS; on other platforms the missing tools surface as an error.
pub fn verify_bundle(
    app: &Path,
    dmg: &Path,
    expected_dmg_sha256: Option<&str>,
) -> std::io::Result<GateReport> {
    let mut checks = Vec::new();

    let (ok, out) = run(
        "codesign",
        &["--verify", "--deep", "--strict", "--verbose=2"],
        app,
    )?;
    checks.push((
        "codesign --verify --deep --strict".into(),
        classify_codesign_verify(ok, &out),
    ));

    let (_ok, out) = run("codesign", &["-d", "--verbose=4"], app)?;
    checks.push((
        "codesign 签名主体".into(),
        classify_codesign_authority(&out),
    ));

    let (ok, out) = run(
        "spctl",
        &["--assess", "--type", "execute", "--verbose=4"],
        app,
    )?;
    checks.push((
        "spctl --assess".into(),
        classify_gatekeeper_assess(ok, &out),
    ));

    let (ok, out) = run("hdiutil", &["verify"], dmg)?;
    checks.push((
        "hdiutil verify".into(),
        if ok {
            GateVerdict::Pass
        } else {
            GateVerdict::Fail(format!("hdiutil verify 失败：{}", first_line(&out)))
        },
    ));

    let digest = format!("{:x}", Sha256::digest(std::fs::read(dmg)?));
    let sha_verdict = match expected_dmg_sha256 {
        Some(expected) => verify_sha256(expected, &digest),
        None => GateVerdict::Pass,
    };
    checks.push((format!("DMG SHA-256 = {digest}"), sha_verdict));

    Ok(GateReport { checks })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract a failure reason, failing the test loudly if the verdict was a pass. Avoids the
    /// workspace-denied `Option::unwrap`/`expect` while keeping assertions readable.
    fn fail_reason(verdict: GateVerdict) -> String {
        match verdict {
            GateVerdict::Fail(reason) => reason,
            GateVerdict::Pass => panic!("expected a Fail verdict, got Pass"),
        }
    }

    #[test]
    fn codesign_verify_flags_the_reported_no_resources_failure() {
        let stderr = "/Applications/AgentFlow.app: code has no resources but signature indicates they must be present";
        let verdict = classify_codesign_verify(false, stderr);
        assert!(!verdict.is_pass());
        assert!(fail_reason(verdict).contains("Developer ID"));
    }

    #[test]
    fn codesign_verify_passes_on_clean_success() {
        assert_eq!(classify_codesign_verify(true, ""), GateVerdict::Pass);
    }

    #[test]
    fn codesign_verify_reports_unsigned_apps() {
        let verdict = classify_codesign_verify(false, "test-app: code object is not signed at all");
        assert!(fail_reason(verdict).contains("尚未签名"));
    }

    #[test]
    fn codesign_authority_rejects_adhoc_and_accepts_developer_id() {
        assert!(!classify_codesign_authority("Signature=adhoc").is_pass());
        assert_eq!(
            classify_codesign_authority(
                "Authority=Developer ID Application: Example Inc (TEAMID)\nAuthority=Apple Root CA"
            ),
            GateVerdict::Pass
        );
    }

    #[test]
    fn codesign_authority_flags_a_non_release_certificate() {
        let verdict = classify_codesign_authority("Authority=Apple Development: dev@example.com");
        assert!(fail_reason(verdict).contains("Apple Development"));
    }

    #[test]
    fn gatekeeper_assess_requires_acceptance() {
        assert_eq!(
            classify_gatekeeper_assess(true, "/App: accepted\nsource=Notarized Developer ID"),
            GateVerdict::Pass
        );
        let rejected = classify_gatekeeper_assess(false, "/App: rejected");
        assert!(fail_reason(rejected).contains("notarization"));
    }

    #[test]
    fn sha256_comparison_is_case_insensitive() {
        assert_eq!(verify_sha256("ABCD", "abcd"), GateVerdict::Pass);
        assert!(!verify_sha256("abcd", "beef").is_pass());
    }

    #[test]
    fn report_passes_only_when_every_check_passes() {
        let all_ok = GateReport {
            checks: vec![
                ("a".into(), GateVerdict::Pass),
                ("b".into(), GateVerdict::Pass),
            ],
        };
        assert!(all_ok.passed());
        let one_bad = GateReport {
            checks: vec![
                ("a".into(), GateVerdict::Pass),
                ("b".into(), GateVerdict::Fail("x".into())),
            ],
        };
        assert!(!one_bad.passed());
    }
}
