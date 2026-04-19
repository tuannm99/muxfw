use anyhow::{Context, Result};
use directories::BaseDirs;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    home_dir: PathBuf,
    base_dir: PathBuf,
}

impl AppPaths {
    pub fn new() -> Result<Self> {
        let base_dirs =
            BaseDirs::new().context("could not resolve the current user's home directory")?;
        let home_dir = base_dirs.home_dir().to_path_buf();
        let base_dir = home_dir.join(".muxwf");
        Ok(Self { home_dir, base_dir })
    }

    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn works_dir(&self) -> PathBuf {
        self.base_dir.join("works")
    }

    pub fn snapshots_dir(&self) -> PathBuf {
        self.base_dir.join("snapshots")
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.base_dir.join("plugins")
    }

    pub fn workspaces_dir(&self) -> PathBuf {
        self.base_dir.join("workspaces")
    }

    pub fn config_file(&self) -> PathBuf {
        self.base_dir.join("config.yaml")
    }

    pub fn work_file(&self, name: &str) -> PathBuf {
        self.works_dir().join(format!("{name}.yaml"))
    }

    pub fn snapshot_file(&self, name: &str) -> PathBuf {
        self.snapshots_dir().join(format!("{name}.json"))
    }

    pub fn workspace_file(&self, name: &str) -> PathBuf {
        self.workspaces_dir().join(format!("{name}.yaml"))
    }

    pub fn ensure_state_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.works_dir())
            .with_context(|| format!("failed to create {}", self.works_dir().display()))?;
        fs::create_dir_all(self.snapshots_dir())
            .with_context(|| format!("failed to create {}", self.snapshots_dir().display()))?;
        fs::create_dir_all(self.plugins_dir())
            .with_context(|| format!("failed to create {}", self.plugins_dir().display()))?;
        fs::create_dir_all(self.workspaces_dir())
            .with_context(|| format!("failed to create {}", self.workspaces_dir().display()))?;
        Ok(())
    }

    pub fn expand_home(&self, value: &str) -> PathBuf {
        if value == "~" {
            return self.home_dir.clone();
        }

        if let Some(rest) = value.strip_prefix("~/") {
            return self.home_dir.join(rest);
        }

        PathBuf::from(value)
    }

    pub fn display_path(&self, path: &Path) -> String {
        if path == self.home_dir {
            return "~".to_string();
        }

        match path.strip_prefix(&self.home_dir) {
            Ok(stripped) => format!("~/{}", stripped.display()),
            Err(_) => path.display().to_string(),
        }
    }
}

pub fn find_binary(binary: &str) -> Option<PathBuf> {
    if binary.contains(std::path::MAIN_SEPARATOR) {
        let path = PathBuf::from(binary);
        return path.is_file().then_some(path);
    }

    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .map(|path| path.join(binary))
        .find(|path| path.is_file())
}

pub fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml" | "yml")
    )
}
