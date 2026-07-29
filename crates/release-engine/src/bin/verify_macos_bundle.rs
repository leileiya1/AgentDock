//! Release gate CLI: fail closed unless a built macOS bundle is properly signed, notarized and its
//! DMG verifies. Intended to run in the signed-release pipeline (see scripts/verify-macos-bundle.sh
//! and .github/workflows/release-macos.yml).
//!
//! Usage: verify-macos-bundle <AgentFlow.app> <AgentFlow.dmg> [expected-dmg-sha256]

use agentflow_release_engine::macos_gate::verify_bundle;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 || args.len() > 3 {
        eprintln!(
            "usage: verify-macos-bundle <AgentFlow.app> <AgentFlow.dmg> [expected-dmg-sha256]"
        );
        return ExitCode::from(2);
    }
    if !cfg!(target_os = "macos") {
        eprintln!("verify-macos-bundle 只能在 macOS 上运行（需要 codesign/spctl/hdiutil）。");
        return ExitCode::from(2);
    }
    let app = Path::new(&args[0]);
    let dmg = Path::new(&args[1]);
    let expected = args.get(2).map(String::as_str);

    let report = match verify_bundle(app, dmg, expected) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("运行签名检查失败：{error}");
            return ExitCode::from(2);
        }
    };

    for (name, verdict) in &report.checks {
        match verdict.message() {
            None => println!("  ✓ {name}"),
            Some(reason) => println!("  ✗ {name}\n      {reason}"),
        }
    }
    if report.passed() {
        println!("发布门禁通过：签名、公证与 DMG 校验均成功。");
        ExitCode::SUCCESS
    } else {
        eprintln!("发布门禁未通过：请修复上面标记为 ✗ 的问题后重新构建并签名。");
        ExitCode::FAILURE
    }
}
