use crate::proc_walk;
use crate::tmux::{self, TmuxError};

#[derive(Debug, Clone)]
pub enum Resolution {
    Ok { pane_id: String },
    NoWindow,
    NoClaudePane,
    MultipleClaudePanes { pane_ids: Vec<String> },
}

pub fn locate(session: &str, color: &str) -> Result<Resolution, TmuxError> {
    let windows = tmux::list_windows(session)?;
    if !windows.iter().any(|w| w.name == color) {
        return Ok(Resolution::NoWindow);
    }
    let target = format!("{session}:{color}");
    let panes = tmux::list_panes(&target)?;
    let claude_panes: Vec<String> = panes
        .into_iter()
        .filter(|p| proc_walk::is_claude_process_tree(p.pid))
        .map(|p| p.id)
        .collect();
    Ok(match claude_panes.len() {
        0 => Resolution::NoClaudePane,
        1 => Resolution::Ok {
            pane_id: claude_panes.into_iter().next().unwrap(),
        },
        _ => Resolution::MultipleClaudePanes {
            pane_ids: claude_panes,
        },
    })
}
