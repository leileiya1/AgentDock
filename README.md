# AgentFlow

AgentFlow is a local-first Rust orchestrator for a revision-based workflow: developer CLI, validation, independent CLI/API review, human approval, and merge. First-class workflow adapters currently cover Claude Code, Codex, Gemini CLI, and Qwen Code. The desktop catalog can also detect and install Grok Build, Kimi Code, and MiniMax CLI; these newer CLIs stay outside the automatic workflow fallback chain until their execution contracts complete conformance testing.

```bash
cargo run -p agentflow-cli -- env-check
cargo run -p agentflow-cli -- setup check
cargo run -p agentflow-cli -- run-task --repo /absolute/repo --title "Fix bug" --desc "..."
```

Run the Rust scheduler as a macOS LaunchAgent so tasks continue after the desktop window closes:

```bash
cargo build --release -p agentflow-daemon --bin agentflowd
target/release/agentflowd install-service
target/release/agentflowd status
```

API review is opt-in per task. Keys are read from environment variables or macOS Keychain and are never stored in SQLite:

```bash
export OPENAI_API_KEY="..."       # or ANTHROPIC_API_KEY / DEEPSEEK_API_KEY
agentflow-cli run-task --repo /absolute/repo --title "Fix bug" --desc "..." \
  --developer claude-code --reviewer deep-seek-api --allow-api-egress
```

Without this per-task approval, built-in API reviewers are rejected and API entries in an automatic fallback chain are skipped. The approval time is stored as an audit event; task content is never sent merely by configuring an API key.

For the installed LaunchAgent, store a key without putting it in shell history:

```bash
agentflow-cli api-key set open-ai
# or: agentflow-cli api-key set anthropic / deep-seek
```

OpenAI and Grok use the Responses API. Anthropic uses the Messages API. DeepSeek, MiniMax, and Kimi use OpenAI-compatible Chat Completions. Built-in defaults are Grok 4.5 at `https://api.x.ai/v1`, MiniMax M2.7 at `https://api.minimax.io/v1`, and Kimi for Coding at `https://api.kimi.com/coding/v1`. Base URL, model, key environment-variable name, retry budget, token limit, and fallback order remain configurable in project settings.

Provider fallback order is configurable independently for developer and reviewer roles. Before a developer fallback, AgentFlow resets its dedicated worktree to the pre-run commit so a failed provider cannot contaminate the next provider's attempt. Reviewer fallback always excludes the provider that actually produced the revision.

Global run settings are enforced by the daemon without a restart: concurrency is limited to 1–16 tasks, while developer, reviewer, and idle timeouts are copied into each run record and passed to the selected Provider. Review issues also have a cross-revision lifecycle: an issue that disappears in a later review is marked resolved with the resolving revision, while a recurring issue stays open.

Review output is contract-validated, but CLI Providers may still add prose or Markdown before the final JSON object. AgentFlow scans complete top-level objects and accepts only the last schema-valid result; a clean process exit with invalid contract output is recorded as a failed run. Run logs retain redacted raw events for diagnostics while the desktop defaults to short system, Agent, tool, and result summaries.

Cross-revision memory is Provider-neutral: SQLite events and immutable revision commits are authoritative, while each revision receives a deterministic `history.json`, an 8 KiB human-readable digest, and the same digest inlined into the next development prompt. This keeps Codex, Claude Code, and API reviewers interchangeable without trusting any Provider's private chat history.

Before a revision commit, the Git engine blocks generated dependency/build directories, credential file paths, likely secret contents, more than 200 files, totals above 20 MiB, or individual files above 5 MiB. A blocked commit keeps the files in the isolated worktree and clears only the index so the user can inspect and repair the result.

The desktop Provider page uses progressive disclosure: common CLIs/APIs show only a brand mark, connection dot, and primary action. Missing supported CLIs can be installed from an official-package allowlist after confirmation; API credentials are configured directly into macOS Keychain. Less common and external Providers stay under “More Provider”.

AgentFlow also supports versioned external Provider sidecars. Packages under `<app_data>/providers/<package>/` are discovered from `provider.json`, negotiate protocol v1 over NDJSON JSON-RPC stdio, and appear in the desktop UI according to their declared capabilities. An external package can override a built-in Provider ID as a compatibility shim. See `04-Provider协议-v1.0.md` and the conformance fixture in `crates/provider-protocol`.

Runtime data is stored outside managed repositories in the platform application-data directory (override with `--data-dir` or `AGENTFLOW_DATA_DIR`). Raw run logs expire after 14 days for merged/cancelled tasks, cache is capped at 2 GiB, and deleted tasks spend 7 days in a recoverable trash before purge. Project source files and committed deliverables are never part of automatic cleanup.

```bash
agentflow-cli storage status
agentflow-cli storage cleanup
agentflow-cli storage task --task-id <id> --scope logs
agentflow-cli storage task --task-id <id> --scope everything
agentflow-cli storage trash-list
agentflow-cli storage restore --task-id <id>
agentflow-cli storage empty-trash
```

Headless users can inspect, approve, merge, or cancel a task without launching the desktop UI:

```bash
agentflow-cli task status <task-id>
agentflow-cli task resume <task-id> --guidance "address the review race"
agentflow-cli task approve <task-id>
agentflow-cli task merge <task-id>
agentflow-cli task cancel <task-id>
```

The daemon runs automatic cleanup every six hours and emits macOS notifications when a task needs attention, completes, or changes provider. Shared schemas and TypeScript bindings are generated with `cargo run -p xtask`.
