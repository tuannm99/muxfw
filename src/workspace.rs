use crate::paths::{AppPaths, is_yaml_file};
use crate::work;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    #[serde(default)]
    pub works: Vec<String>,
}

impl Workspace {
    pub fn validate(&self) -> Result<()> {
        work::validate_name(&self.name)?;
        if self.works.is_empty() {
            bail!("workspace '{}' has no works", self.name);
        }
        for work_name in &self.works {
            work::validate_name(work_name).with_context(|| {
                format!("invalid work '{}' in workspace '{}'", work_name, self.name)
            })?;
        }
        Ok(())
    }
}

pub fn load_workspace(paths: &AppPaths, name: &str) -> Result<Workspace> {
    work::validate_name(name)?;
    let path = paths.workspace_file(name);
    let workspace = load_workspace_file(&path)?;
    if workspace.name != name {
        bail!(
            "workspace file {} declares name '{}', expected '{}'",
            path.display(),
            workspace.name,
            name
        );
    }
    Ok(workspace)
}

pub fn load_workspace_file(path: &Path) -> Result<Workspace> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read workspace file {}", path.display()))?;
    let workspace: Workspace = serde_yaml::from_str(&raw)
        .with_context(|| format!("invalid YAML in workspace file {}", path.display()))?;
    workspace
        .validate()
        .with_context(|| format!("invalid workspace file {}", path.display()))?;
    Ok(workspace)
}

pub fn list_workspaces(paths: &AppPaths) -> Result<Vec<Workspace>> {
    let mut workspaces = Vec::new();
    for file in workspace_files(paths)? {
        workspaces.push(load_workspace_file(&file)?);
    }
    workspaces.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(workspaces)
}

pub fn workspace_files(paths: &AppPaths) -> Result<Vec<PathBuf>> {
    let dir = paths.workspaces_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let path = entry
            .with_context(|| format!("failed to read entry in {}", dir.display()))?
            .path();
        if is_yaml_file(&path) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
