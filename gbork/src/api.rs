use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaneStatus {
    Ok,
    NoWindow,
    NoClaudePane,
    MultipleClaudePanes,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionEntry {
    pub color: String,
    pub tmux_window: Option<String>,
    pub claude_pane: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_panes: Option<Vec<String>>,
    pub pane_status: PaneStatus,
    pub output: Option<String>,
    pub captured_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionDetail {
    pub color: String,
    pub claude_pane: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_panes: Option<Vec<String>>,
    pub pane_status: PaneStatus,
    pub captured_at: Option<String>,
    pub output: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_to_pane: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: String,
}
