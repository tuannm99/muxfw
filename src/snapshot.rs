use crate::paths::AppPaths;
use crate::work;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_name: Option<String>,
    pub session_name: String,
    pub active_window_index: usize,
    #[serde(default)]
    pub windows: Vec<WindowSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSnapshot {
    pub index: usize,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    pub active_pane_index: usize,
    pub pane_count: usize,
    pub panes: Vec<PaneSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneSnapshot {
    pub index: usize,
    pub cwd: String,
}

impl Snapshot {
    pub fn validate(&self) -> Result<()> {
        if self.version == 0 {
            bail!("snapshot has invalid version 0");
        }
        if self.session_name.trim().is_empty() {
            bail!("snapshot has an empty session_name");
        }
        if self.windows.is_empty() {
            bail!("snapshot has no windows");
        }
        let mut window_indices = BTreeSet::new();
        for window in &self.windows {
            if !window_indices.insert(window.index) {
                bail!("snapshot has duplicate window index {}", window.index);
            }
            if window.name.trim().is_empty() {
                bail!("snapshot window {} has an empty name", window.index);
            }
            if window.pane_count == 0 {
                bail!("snapshot window '{}' has zero panes", window.name);
            }
            if window.pane_count != window.panes.len() {
                bail!(
                    "snapshot window '{}' pane_count is {}, but panes has {} entries",
                    window.name,
                    window.pane_count,
                    window.panes.len()
                );
            }
            let mut pane_indices = BTreeSet::new();
            for pane in &window.panes {
                if !pane_indices.insert(pane.index) {
                    bail!(
                        "snapshot window '{}' has duplicate pane index {}",
                        window.name,
                        pane.index
                    );
                }
                if pane.cwd.trim().is_empty() {
                    bail!(
                        "snapshot window '{}' pane {} has an empty cwd",
                        window.name,
                        pane.index
                    );
                }
            }
            if !window
                .panes
                .iter()
                .any(|pane| pane.index == window.active_pane_index)
            {
                bail!(
                    "snapshot window '{}' active pane {} is missing",
                    window.name,
                    window.active_pane_index
                );
            }
        }
        if !window_indices.contains(&self.active_window_index) {
            bail!(
                "snapshot active window {} is missing",
                self.active_window_index
            );
        }
        Ok(())
    }
}

pub fn read_snapshot(paths: &AppPaths, work_name: &str) -> Result<Snapshot> {
    work::validate_name(work_name)?;
    let snapshot = read_snapshot_file(&paths.snapshot_file(work_name))?;
    if let Some(snapshot_work_name) = &snapshot.work_name
        && snapshot_work_name != work_name
    {
        bail!(
            "snapshot is for work '{}', expected '{}'",
            snapshot_work_name,
            work_name
        );
    }
    Ok(snapshot)
}

pub fn read_snapshot_file(path: &Path) -> Result<Snapshot> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read snapshot {}", path.display()))?;
    let snapshot: Snapshot = serde_json::from_str(&raw)
        .with_context(|| format!("invalid JSON in snapshot {}", path.display()))?;
    snapshot
        .validate()
        .with_context(|| format!("invalid snapshot {}", path.display()))?;
    Ok(snapshot)
}

pub fn write_snapshot(paths: &AppPaths, work_name: &str, snapshot: &Snapshot) -> Result<()> {
    work::validate_name(work_name)?;
    let mut snapshot = snapshot.clone();
    snapshot.work_name = Some(work_name.to_string());
    snapshot.validate()?;
    let path = paths.snapshot_file(work_name);
    let json = serde_json::to_string_pretty(&snapshot)
        .with_context(|| format!("failed to serialize snapshot for '{}'", work_name))?;
    fs::write(&path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn raw_snapshot(paths: &AppPaths, work_name: &str) -> Result<String> {
    work::validate_name(work_name)?;
    let path = paths.snapshot_file(work_name);
    fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))
}

pub fn snapshot_exists(paths: &AppPaths, work_name: &str) -> bool {
    paths.snapshot_file(work_name).is_file()
}

pub fn snapshot_files(paths: &AppPaths) -> Result<Vec<PathBuf>> {
    let dir = paths.snapshots_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let path = entry
            .with_context(|| format!("failed to read entry in {}", dir.display()))?
            .path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_snapshot() -> Snapshot {
        Snapshot {
            version: 1,
            work_name: Some("api".to_string()),
            session_name: "api".to_string(),
            active_window_index: 0,
            windows: vec![WindowSnapshot {
                index: 0,
                name: "main".to_string(),
                layout: None,
                active_pane_index: 0,
                pane_count: 1,
                panes: vec![PaneSnapshot {
                    index: 0,
                    cwd: "/tmp".to_string(),
                }],
            }],
        }
    }

    #[test]
    fn validate_rejects_missing_active_window() {
        let mut snapshot = valid_snapshot();
        snapshot.active_window_index = 1;

        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn validate_rejects_duplicate_pane_indices() {
        let mut snapshot = valid_snapshot();
        snapshot.windows[0].pane_count = 2;
        snapshot.windows[0].panes.push(PaneSnapshot {
            index: 0,
            cwd: "/tmp".to_string(),
        });

        assert!(snapshot.validate().is_err());
    }
}
