use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskFact {
    pub number: u32,
    pub name: String,
    pub total_size_gb: f64,
    pub partition_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Blackboard {
    pub timestamp: String,
    pub status: String,
    pub disks: Vec<DiskFact>,
    pub allocated_drive: Option<String>,
}

impl Blackboard {
    pub fn load(path: &Path) -> Self {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(bb) = serde_json::from_str(&content) {
                return bb;
            }
        }
        Self::default()
    }

    pub fn save(&mut self, path: &Path) -> anyhow::Result<()> {
        self.timestamp = chrono::Local::now().to_rfc3339();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
