use crate::paths::{AppPaths, is_yaml_file};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    pub name: String,
    pub session: String,
    pub root: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<WorkWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_restore: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub favorite: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened_at: Option<DateTime<Utc>>,
    #[serde(default = "now_utc")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "now_utc")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkWindow {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default = "default_panes", skip_serializing_if = "is_one")]
    pub panes: usize,
}

fn default_panes() -> usize {
    1
}

fn is_one(value: &usize) -> bool {
    *value == 1
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

impl Work {
    pub fn new(name: String, session: String, root: String) -> Self {
        let now = now_utc();
        Self {
            name,
            session,
            root,
            windows: Vec::new(),
            on_restore: Some(String::new()),
            tags: Vec::new(),
            group: None,
            favorite: false,
            description: None,
            last_opened_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_name(&self.name)?;
        validate_name(&self.session)
            .with_context(|| format!("invalid tmux session name for work '{}'", self.name))?;
        if self.root.trim().is_empty() {
            bail!("work '{}' has an empty root", self.name);
        }
        if self
            .group
            .as_deref()
            .is_some_and(|group| group.trim().is_empty())
        {
            bail!("work '{}' has an empty group", self.name);
        }
        for tag in &self.tags {
            if tag.trim().is_empty() {
                bail!("work '{}' has an empty tag", self.name);
            }
        }
        for window in &self.windows {
            if window.name.trim().is_empty() {
                bail!("work '{}' has a window with an empty name", self.name);
            }
            if window.panes == 0 {
                bail!(
                    "work '{}' window '{}' has zero panes",
                    self.name,
                    window.name
                );
            }
        }
        Ok(())
    }

    pub fn root_path(&self, paths: &AppPaths) -> PathBuf {
        paths.expand_home(&self.root)
    }

    pub fn mark_opened_now(&mut self) {
        self.last_opened_at = Some(now_utc());
    }
}

pub fn validate_name(name: &str) -> Result<()> {
    let re = Regex::new(r"^[A-Za-z0-9._-]+$").expect("valid work name regex");
    if name.trim().is_empty() {
        bail!("name cannot be empty");
    }
    if !re.is_match(name) {
        bail!(
            "'{}' is invalid; use only letters, numbers, '.', '_' and '-'",
            name
        );
    }
    Ok(())
}

pub fn load_work(paths: &AppPaths, name: &str) -> Result<Work> {
    validate_name(name)?;
    let path = paths.work_file(name);
    if !path.exists() {
        bail!(
            "work '{}' does not exist at {}; create it first with `muxwf init {}` or `muxwf add {}`",
            name,
            path.display(),
            name,
            name
        );
    }
    let work = load_work_file(&path)?;
    if work.name != name {
        bail!(
            "work file {} declares name '{}', expected '{}'",
            path.display(),
            work.name,
            name
        );
    }
    Ok(work)
}

pub fn load_work_file(path: &Path) -> Result<Work> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read work file {}", path.display()))?;
    let work: Work = serde_yaml::from_str(&raw)
        .with_context(|| format!("invalid YAML in work file {}", path.display()))?;
    work.validate()
        .with_context(|| format!("invalid work file {}", path.display()))?;
    Ok(work)
}

pub fn write_work(paths: &AppPaths, work: &Work) -> Result<()> {
    let mut work = work.clone();
    work.updated_at = now_utc();
    work.validate()?;
    let path = paths.work_file(&work.name);
    let yaml = serde_yaml::to_string(&work)
        .with_context(|| format!("failed to serialize work '{}'", work.name))?;
    fs::write(&path, yaml).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn delete_work(paths: &AppPaths, name: &str) -> Result<()> {
    validate_name(name)?;
    let path = paths.work_file(name);
    if !path.exists() {
        bail!("work '{}' does not exist at {}", name, path.display());
    }
    fs::remove_file(&path).with_context(|| format!("failed to delete {}", path.display()))?;
    Ok(())
}

pub fn list_works(paths: &AppPaths) -> Result<Vec<Work>> {
    let mut works = Vec::new();
    for file in work_files(paths)? {
        works.push(load_work_file(&file)?);
    }
    works.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(works)
}

pub fn work_files(paths: &AppPaths) -> Result<Vec<PathBuf>> {
    let dir = paths.works_dir();
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

pub fn sanitize_name(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        let normalized = if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            Some(ch)
        } else {
            Some('-')
        };

        if let Some(ch) = normalized {
            if ch == '-' {
                if !last_dash {
                    out.push(ch);
                }
                last_dash = true;
            } else {
                out.push(ch);
                last_dash = false;
            }
        }
    }

    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_work_yaml_loads_with_v2_defaults() {
        let work: Work = serde_yaml::from_str(
            r#"
name: api
session: api
root: /tmp/api
on_restore: ""
"#,
        )
        .unwrap();

        work.validate().unwrap();
        assert!(work.tags.is_empty());
        assert_eq!(work.group, None);
        assert!(!work.favorite);
        assert_eq!(work.description, None);
        assert_eq!(work.last_opened_at, None);
        assert!(work.created_at <= Utc::now());
        assert!(work.updated_at <= Utc::now());
    }
}
