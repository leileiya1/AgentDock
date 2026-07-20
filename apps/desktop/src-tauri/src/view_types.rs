use serde::Serialize;
use specta::Type;

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunLogPage {
    pub lines: Vec<agentflow_contracts::AgentEvent>,
    pub next_from_line: u32,
    pub eof: bool,
}

#[derive(Serialize, Type)]
pub(crate) struct ExportPath {
    pub path: String,
}
