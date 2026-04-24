use crate::paths::{AppPaths, is_yaml_file};
use crate::work;
use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    #[serde(default)]
    pub works: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub policy: WorkspaceOpenPolicy,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceOpenPolicy {
    #[default]
    Smart,
    ReuseOnly,
    RestoreOnly,
    Fresh,
}

impl Workspace {
    // Validate the workspace name, work list, and duplicates to keep declaration order stable.
    pub fn validate(&self) -> Result<()> {
        work::validate_name(&self.name)?;
        if self.works.is_empty() {
            bail!("workspace '{}' has no works", self.name);
        }
        if self
            .profile
            .as_deref()
            .is_some_and(|profile| profile.trim().is_empty())
        {
            bail!("workspace '{}' has an empty profile", self.name);
        }
        let mut seen = BTreeSet::new();
        for work_name in &self.works {
            work::validate_name(work_name).with_context(|| {
                format!("invalid work '{}' in workspace '{}'", work_name, self.name)
            })?;
            if !seen.insert(work_name) {
                bail!(
                    "workspace '{}' contains duplicate work '{}'",
                    self.name,
                    work_name
                );
            }
        }
        Ok(())
    }
}

// Rewrite the whole workspace file after revalidating the input data.
pub fn write_workspace(paths: &AppPaths, workspace: &Workspace) -> Result<()> {
    let workspace = workspace.clone();
    workspace.validate()?;
    let path = paths.workspace_file(&workspace.name);
    let yaml = serde_yaml::to_string(&workspace)
        .with_context(|| format!("failed to serialize workspace '{}'", workspace.name))?;
    fs::write(&path, yaml).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

// Delete only the workspace file without touching related works or snapshots.
pub fn delete_workspace(paths: &AppPaths, name: &str) -> Result<()> {
    work::validate_name(name)?;
    let path = paths.workspace_file(name);
    if !path.exists() {
        bail!("workspace '{}' does not exist at {}", name, path.display());
    }
    fs::remove_file(&path).with_context(|| format!("failed to delete {}", path.display()))?;
    Ok(())
}

// Load a workspace by canonical filename and verify the declared name matches.
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

// Parse a standalone workspace YAML file and run full validation.
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

// Scan the whole workspace directory, parse each file, and sort by name for stable output.
pub fn list_workspaces(paths: &AppPaths) -> Result<Vec<Workspace>> {
    let mut workspaces = Vec::new();
    for file in workspace_files(paths)? {
        workspaces.push(load_workspace_file(&file)?);
    }
    workspaces.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(workspaces)
}

// Collect only valid YAML files from the workspace directory.
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
