use crate::cli::SaveArgs;
use crate::paths::AppPaths;
use crate::work::{self, Work};
use anyhow::{Context, Result};

pub fn work_for_save(paths: &AppPaths, args: SaveArgs) -> Result<Work> {
    match args.name {
        Some(name) => work::load_work(paths, &name),
        None => current_work(paths).context(
            "save without a work name requires running inside a tmux session mapped to a work",
        ),
    }
}

pub fn current_work(paths: &AppPaths) -> Result<Work> {
    let session = crate::tmux::current_session_name()?;
    let works = work::list_works(paths)?;
    works
        .into_iter()
        .find(|work| work.session == session)
        .with_context(|| {
            format!(
                "current tmux session '{}' is not managed by muxwf; create one with `muxwf init {}` or pass an existing work name",
                session,
                work::sanitize_name(&session)
            )
        })
}

pub fn current_dir_string() -> Result<String> {
    std::env::current_dir()
        .context("failed to read current directory")
        .map(|path| path.display().to_string())
}
