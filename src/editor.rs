use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

pub fn edit_path(path: &Path) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = Command::new("sh")
        .arg("-lc")
        .arg("exec ${EDITOR:-vi} \"$@\"")
        .arg("muxwf-editor")
        .arg(path)
        .env("EDITOR", editor)
        .status()
        .context("failed to launch editor")?;
    if !status.success() {
        bail!("editor exited with status {}", status);
    }
    Ok(())
}
