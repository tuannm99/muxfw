use crate::paths::AppPaths;
use crate::rules::Ruleset;
use crate::snapshot::{self, PaneSnapshot, Snapshot, WindowSnapshot};
use crate::tmux;
use crate::work::{Work, WorkWindow};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;

pub fn restore_work(paths: &AppPaths, work: &Work, attach: bool) -> Result<()> {
    let snapshot = snapshot::read_snapshot(paths, &work.name)?;
    if tmux::session_exists(&work.session)? {
        bail!(
            "tmux session '{}' already exists; use 'muxwf open {}' or kill the session before restoring",
            work.session,
            work.name
        );
    }

    let rules = Ruleset::load(paths)?;
    if let Err(error) = restore_snapshot(paths, work, &snapshot, &rules) {
        let _ = tmux::kill_session(&work.session);
        return Err(error).context("restore failed; partial tmux session was cleaned up");
    }

    if attach {
        tmux::switch_or_attach(&work.session)?;
    }
    Ok(())
}

pub fn open_work(paths: &AppPaths, work: &Work) -> Result<()> {
    ensure_work_session(paths, work)?;
    tmux::switch_or_attach(&work.session)
}

pub fn ensure_work_session(paths: &AppPaths, work: &Work) -> Result<bool> {
    if tmux::session_exists(&work.session)? {
        return Ok(false);
    }

    if snapshot::snapshot_exists(paths, &work.name) {
        restore_work(paths, work, false)?;
        return Ok(true);
    }

    create_session_from_work(paths, work, false)?;
    Ok(false)
}

pub fn create_session_from_work(paths: &AppPaths, work: &Work, attach: bool) -> Result<()> {
    if tmux::session_exists(&work.session)? {
        bail!("tmux session '{}' already exists", work.session);
    }

    let rules = Ruleset::load(paths)?;
    if let Err(error) = create_session_from_config(paths, work, &rules) {
        let _ = tmux::kill_session(&work.session);
        return Err(error).context("session creation failed; partial tmux session was cleaned up");
    }

    if attach {
        tmux::switch_or_attach(&work.session)?;
    }
    Ok(())
}

fn restore_snapshot(
    paths: &AppPaths,
    work: &Work,
    snapshot: &Snapshot,
    rules: &Ruleset,
) -> Result<()> {
    let mut windows = snapshot.windows.clone();
    windows.sort_by_key(|window| window.index);
    let first = windows
        .first()
        .context("cannot restore an empty snapshot")?
        .clone();
    let first_cwd = window_cwd(paths, work, &first);

    tmux::create_detached_session(&work.session, &first.name, &first_cwd)?;
    let current_window = tmux::current_window_index(&work.session)?;
    if current_window != first.index {
        tmux::move_window(&work.session, current_window, first.index)?;
    }
    tmux::rename_window(&work.session, first.index, &first.name)?;

    for window in windows.iter().skip(1) {
        let cwd = window_cwd(paths, work, window);
        tmux::new_window(&work.session, window.index, &window.name, &cwd)?;
    }

    for window in &windows {
        restore_window(paths, work, rules, window)?;
    }

    tmux::select_window(&work.session, snapshot.active_window_index)?;
    if let Some(active_window) = windows
        .iter()
        .find(|window| window.index == snapshot.active_window_index)
    {
        tmux::select_pane(
            &work.session,
            snapshot.active_window_index,
            active_window.active_pane_index,
        )?;
    }

    Ok(())
}

fn restore_window(
    paths: &AppPaths,
    work: &Work,
    rules: &Ruleset,
    window: &WindowSnapshot,
) -> Result<()> {
    let mut panes = window.panes.clone();
    panes.sort_by_key(|pane| pane.index);

    for pane in panes.iter().skip(1) {
        let cwd = pane_cwd(paths, work, pane);
        tmux::split_window(&work.session, window.index, &cwd)?;
    }

    if let Some(layout) = &window.layout
        && let Err(error) = tmux::select_layout(&work.session, window.index, layout)
    {
        eprintln!(
            "warning: could not apply tmux layout for window '{}': {error:#}",
            window.name
        );
    }

    for pane in &panes {
        let cwd = pane_cwd(paths, work, pane);
        let hook = hook_for(work, rules, &cwd);
        let command = restore_command(&cwd, hook);
        tmux::send_shell_command(&work.session, window.index, pane.index, &command)?;
    }

    tmux::select_pane(&work.session, window.index, window.active_pane_index)?;
    Ok(())
}

fn create_session_from_config(paths: &AppPaths, work: &Work, rules: &Ruleset) -> Result<()> {
    let windows = if work.windows.is_empty() {
        vec![WorkWindow {
            name: "main".to_string(),
            cwd: Some(work.root.clone()),
            panes: 1,
        }]
    } else {
        work.windows.clone()
    };

    let first = windows.first().context("work has no windows")?;
    let first_cwd = work_window_cwd(paths, work, first);
    tmux::create_detached_session(&work.session, &first.name, &first_cwd)?;
    let first_index = tmux::current_window_index(&work.session)?;
    configure_work_window(paths, work, rules, first_index, first)?;

    let mut next_index = first_index + 1;
    for window in windows.iter().skip(1) {
        let cwd = work_window_cwd(paths, work, window);
        tmux::new_window(&work.session, next_index, &window.name, &cwd)?;
        configure_work_window(paths, work, rules, next_index, window)?;
        next_index += 1;
    }

    tmux::select_window(&work.session, first_index)?;
    Ok(())
}

fn configure_work_window(
    paths: &AppPaths,
    work: &Work,
    rules: &Ruleset,
    window_index: usize,
    window: &WorkWindow,
) -> Result<()> {
    let cwd = work_window_cwd(paths, work, window);
    for _ in 1..window.panes {
        tmux::split_window(&work.session, window_index, &cwd)?;
    }

    for pane in tmux::pane_indices(&work.session, window_index)? {
        let hook = hook_for(work, rules, &cwd);
        let command = restore_command(&cwd, hook);
        tmux::send_shell_command(&work.session, window_index, pane, &command)?;
    }
    Ok(())
}

fn window_cwd(paths: &AppPaths, work: &Work, window: &WindowSnapshot) -> String {
    window
        .panes
        .first()
        .map(|pane| pane_cwd(paths, work, pane))
        .unwrap_or_else(|| fallback_work_root(paths, work))
}

fn pane_cwd(paths: &AppPaths, work: &Work, pane: &PaneSnapshot) -> String {
    existing_dir(paths, &pane.cwd, "snapshot pane cwd")
        .or_else(|| existing_dir(paths, &work.root, "work root"))
        .unwrap_or_else(|| paths.home_dir().display().to_string())
}

fn fallback_work_root(paths: &AppPaths, work: &Work) -> String {
    existing_dir(paths, &work.root, "work root")
        .unwrap_or_else(|| paths.home_dir().display().to_string())
}

fn work_window_cwd(paths: &AppPaths, work: &Work, window: &WorkWindow) -> String {
    let configured = window.cwd.as_deref().unwrap_or(&work.root);
    existing_dir(paths, configured, "work window cwd")
        .or_else(|| existing_dir(paths, &work.root, "work root"))
        .unwrap_or_else(|| paths.home_dir().display().to_string())
}

fn existing_dir(paths: &AppPaths, value: &str, label: &str) -> Option<String> {
    let path: PathBuf = paths.expand_home(value);
    if path.is_dir() {
        Some(path.display().to_string())
    } else {
        eprintln!(
            "warning: {} '{}' does not exist; using fallback",
            label,
            path.display()
        );
        None
    }
}

fn hook_for<'a>(work: &'a Work, rules: &'a Ruleset, cwd: &str) -> Option<&'a str> {
    work.on_restore
        .as_deref()
        .filter(|hook| !hook.trim().is_empty())
        .or_else(|| rules.hook_for(cwd).filter(|hook| !hook.trim().is_empty()))
}

fn restore_command(cwd: &str, hook: Option<&str>) -> String {
    let mut command = format!("cd -- {}", shell_quote(cwd));
    if let Some(hook) = hook {
        command.push_str(" && ");
        command.push_str(hook.trim());
    }
    command
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
