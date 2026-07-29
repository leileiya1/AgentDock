fn main() {
    // sqlx::migrate! embeds every migration into the daemon binary. Cargo does not otherwise
    // know that adding or editing a SQL file must invalidate agentflow-persistence and every
    // release sidecar that links it, which can ship a binary older than the database it opens.
    println!("cargo:rerun-if-changed=migrations");
}
