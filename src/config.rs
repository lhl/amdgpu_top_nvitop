//! Persisted UI state (section collapse flags). Stored as JSON under
//! ~/.config/amdgpu-top-nvitop/state.json.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct CollapseState {
    pub cpu: bool,
    pub gpu: bool,
    pub npu: bool,
    pub processes: bool,
}

fn state_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config/amdgpu-top-nvitop/state.json"))
}

impl CollapseState {
    pub fn load() -> Self {
        let Some(p) = state_path() else {
            return Self::default();
        };
        fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(p) = state_path() else { return };
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = fs::write(p, s);
        }
    }
}
