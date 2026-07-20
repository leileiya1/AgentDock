use agentflow_contracts::{development_result_schema, plan_result_schema, review_result_schema};
use std::{fs, path::PathBuf, process::Command};

fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let schemas = root.join("packages/schemas/generated");
    fs::create_dir_all(&schemas)?;
    fs::write(
        schemas.join("result.schema.json"),
        serde_json::to_vec_pretty(&development_result_schema())?,
    )?;
    fs::write(
        schemas.join("review.schema.json"),
        serde_json::to_vec_pretty(&review_result_schema())?,
    )?;
    fs::write(
        schemas.join("plan.schema.json"),
        serde_json::to_vec_pretty(&plan_result_schema())?,
    )?;
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args([
            "run",
            "-q",
            "--manifest-path",
            "apps/desktop/src-tauri/Cargo.toml",
            "--",
            "--export-bindings",
        ])
        .current_dir(&root)
        .status()?;
    anyhow::ensure!(status.success(), "tauri-specta binding export failed");
    normalize_typescript(&root.join("apps/desktop/src/generated/bindings.ts"))?;
    Ok(())
}

/// specta preserves doc-comment padding and may append extra blank lines.
/// Normalizing generated output keeps `xtask` byte-stable across platforms so
/// CI can use `git diff --exit-code` as a real contract drift guard.
fn normalize_typescript(path: &std::path::Path) -> anyhow::Result<()> {
    let source = fs::read_to_string(path)?;
    let normalized = source
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, normalized.trim_end().to_owned() + "\n")?;
    Ok(())
}
