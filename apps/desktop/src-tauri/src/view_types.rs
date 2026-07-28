use serde::Serialize;
use specta::Type;

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunLogPage {
    pub lines: Vec<agentflow_contracts::AgentEvent>,
    /// Absolute file line number of `lines[0]`. The viewer merges pages and live batches by
    /// this number, so a re-seed or an overlapping stream can never duplicate or reorder output.
    pub from_line: u32,
    pub next_from_line: u32,
    pub eof: bool,
    pub total_lines: u32,
}
