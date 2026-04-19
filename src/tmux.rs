use crate::paths::find_binary;
use crate::snapshot::{PaneSnapshot, Snapshot, WindowSnapshot};
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::process::{Command, Stdio};

const SEP: char = '\t';

pub fn ensure_tmux_installed() -> Result<()> {
    if find_binary("tmux").is_none() {
        bail!("tmux is not installed or not found in PATH");
    }
    Ok(())
}

pub fn session_exists(session: &str) -> Result<bool> {
    ensure_tmux_installed()?;
    let status = Command::new("tmux")
        .args(["has-session", "-t", session])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run tmux has-session")?;
    Ok(status.success())
}

pub fn list_sessions() -> Result<BTreeSet<String>> {
    ensure_tmux_installed()?;
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .context("failed to run tmux list-sessions")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no server running") {
            return Ok(BTreeSet::new());
        }
        bail!("tmux failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let session = line.trim();
            (!session.is_empty()).then(|| session.to_string())
        })
        .collect())
}

pub fn current_session_name() -> Result<String> {
    ensure_tmux_installed()?;
    if std::env::var_os("TMUX").is_none() {
        bail!("not inside a tmux client");
    }

    let session = tmux_stdout([
        OsString::from("display-message"),
        OsString::from("-p"),
        OsString::from("#{session_name}"),
    ])?
    .trim()
    .to_string();
    if session.is_empty() {
        bail!("could not detect current tmux session");
    }
    Ok(session)
}

pub fn capture_session(session: &str) -> Result<Snapshot> {
    ensure_tmux_installed()?;
    if !session_exists(session)? {
        bail!("tmux session '{}' does not exist", session);
    }

    let active_window_index = tmux_stdout([
        OsString::from("display-message"),
        OsString::from("-p"),
        OsString::from("-t"),
        OsString::from(session),
        OsString::from("#{window_index}"),
    ])?
    .trim()
    .parse::<usize>()
    .with_context(|| {
        format!(
            "failed to parse active window index for session '{}'",
            session
        )
    })?;

    let window_format = format!(
        "#{{window_index}}{SEP}#{{window_name}}{SEP}#{{window_layout}}{SEP}#{{window_panes}}"
    );
    let windows_raw = tmux_stdout([
        OsString::from("list-windows"),
        OsString::from("-t"),
        OsString::from(session),
        OsString::from("-F"),
        OsString::from(window_format),
    ])?;

    let mut windows = Vec::new();
    for line in windows_raw.lines().filter(|line| !line.trim().is_empty()) {
        let cols: Vec<&str> = line.split(SEP).collect();
        if cols.len() != 4 {
            bail!("unexpected tmux list-windows output: '{}'", line);
        }

        let index = cols[0]
            .parse::<usize>()
            .with_context(|| format!("failed to parse window index from '{}'", line))?;
        let name = cols[1].to_string();
        let layout = (!cols[2].trim().is_empty()).then(|| cols[2].to_string());
        let pane_count = cols[3]
            .parse::<usize>()
            .with_context(|| format!("failed to parse pane count from '{}'", line))?;
        let panes = capture_panes(session, index)?;
        let active_pane_index = panes
            .iter()
            .find(|pane| pane.active)
            .map(|pane| pane.index)
            .or_else(|| panes.first().map(|pane| pane.index))
            .with_context(|| format!("window '{}' has no panes", name))?;

        windows.push(WindowSnapshot {
            index,
            name,
            layout,
            active_pane_index,
            pane_count,
            panes: panes
                .into_iter()
                .map(|pane| PaneSnapshot {
                    index: pane.index,
                    cwd: pane.cwd,
                })
                .collect(),
        });
    }

    windows.sort_by_key(|window| window.index);

    let snapshot = Snapshot {
        version: 1,
        work_name: None,
        session_name: session.to_string(),
        active_window_index,
        windows,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

#[derive(Debug)]
struct CapturedPane {
    index: usize,
    active: bool,
    cwd: String,
}

fn capture_panes(session: &str, window_index: usize) -> Result<Vec<CapturedPane>> {
    let pane_format = format!("#{{pane_index}}{SEP}#{{pane_active}}{SEP}#{{pane_current_path}}");
    let target = format!("{session}:{window_index}");
    let panes_raw = tmux_stdout([
        OsString::from("list-panes"),
        OsString::from("-t"),
        OsString::from(target),
        OsString::from("-F"),
        OsString::from(pane_format),
    ])?;

    let mut panes = Vec::new();
    for line in panes_raw.lines().filter(|line| !line.trim().is_empty()) {
        let cols: Vec<&str> = line.split(SEP).collect();
        if cols.len() != 3 {
            bail!("unexpected tmux list-panes output: '{}'", line);
        }
        panes.push(CapturedPane {
            index: cols[0]
                .parse::<usize>()
                .with_context(|| format!("failed to parse pane index from '{}'", line))?,
            active: cols[1] == "1",
            cwd: cols[2].to_string(),
        });
    }
    panes.sort_by_key(|pane| pane.index);
    Ok(panes)
}

pub fn create_detached_session(session: &str, window_name: &str, cwd: &str) -> Result<()> {
    tmux_status([
        OsString::from("new-session"),
        OsString::from("-d"),
        OsString::from("-s"),
        OsString::from(session),
        OsString::from("-n"),
        OsString::from(window_name),
        OsString::from("-c"),
        OsString::from(cwd),
    ])
    .with_context(|| format!("failed to create tmux session '{}'", session))
}

pub fn current_window_index(session: &str) -> Result<usize> {
    tmux_stdout([
        OsString::from("display-message"),
        OsString::from("-p"),
        OsString::from("-t"),
        OsString::from(session),
        OsString::from("#{window_index}"),
    ])?
    .trim()
    .parse::<usize>()
    .with_context(|| {
        format!(
            "failed to parse current window index for session '{}'",
            session
        )
    })
}

pub fn move_window(session: &str, from: usize, to: usize) -> Result<()> {
    tmux_status([
        OsString::from("move-window"),
        OsString::from("-s"),
        OsString::from(format!("{session}:{from}")),
        OsString::from("-t"),
        OsString::from(format!("{session}:{to}")),
    ])
    .with_context(|| format!("failed to move window {from} to {to} in session '{session}'"))
}

pub fn rename_window(session: &str, window: usize, name: &str) -> Result<()> {
    tmux_status([
        OsString::from("rename-window"),
        OsString::from("-t"),
        OsString::from(format!("{session}:{window}")),
        OsString::from(name),
    ])
    .with_context(|| format!("failed to rename window {window} in session '{session}'"))
}

pub fn new_window(session: &str, window: usize, name: &str, cwd: &str) -> Result<()> {
    tmux_status([
        OsString::from("new-window"),
        OsString::from("-d"),
        OsString::from("-t"),
        OsString::from(format!("{session}:{window}")),
        OsString::from("-n"),
        OsString::from(name),
        OsString::from("-c"),
        OsString::from(cwd),
    ])
    .with_context(|| format!("failed to create window {window} in session '{session}'"))
}

pub fn split_window(session: &str, window: usize, cwd: &str) -> Result<()> {
    tmux_status([
        OsString::from("split-window"),
        OsString::from("-d"),
        OsString::from("-t"),
        OsString::from(format!("{session}:{window}")),
        OsString::from("-c"),
        OsString::from(cwd),
    ])
    .with_context(|| format!("failed to split window {window} in session '{session}'"))
}

pub fn select_layout(session: &str, window: usize, layout: &str) -> Result<()> {
    tmux_status([
        OsString::from("select-layout"),
        OsString::from("-t"),
        OsString::from(format!("{session}:{window}")),
        OsString::from(layout),
    ])
    .with_context(|| format!("failed to apply layout to window {window} in session '{session}'"))
}

pub fn send_shell_command(session: &str, window: usize, pane: usize, command: &str) -> Result<()> {
    let target = format!("{session}:{window}.{pane}");
    tmux_status([
        OsString::from("send-keys"),
        OsString::from("-t"),
        OsString::from(&target),
        OsString::from("-l"),
        OsString::from(command),
    ])
    .with_context(|| format!("failed to send command to pane {target}"))?;
    tmux_status([
        OsString::from("send-keys"),
        OsString::from("-t"),
        OsString::from(&target),
        OsString::from("Enter"),
    ])
    .with_context(|| format!("failed to press Enter in pane {target}"))
}

pub fn select_window(session: &str, window: usize) -> Result<()> {
    tmux_status([
        OsString::from("select-window"),
        OsString::from("-t"),
        OsString::from(format!("{session}:{window}")),
    ])
    .with_context(|| format!("failed to select window {window} in session '{session}'"))
}

pub fn select_pane(session: &str, window: usize, pane: usize) -> Result<()> {
    tmux_status([
        OsString::from("select-pane"),
        OsString::from("-t"),
        OsString::from(format!("{session}:{window}.{pane}")),
    ])
    .with_context(|| format!("failed to select pane {pane} in window {window}"))
}

pub fn switch_or_attach(session: &str) -> Result<()> {
    if std::env::var_os("TMUX").is_some() {
        tmux_interactive_status([
            OsString::from("switch-client"),
            OsString::from("-t"),
            OsString::from(session),
        ])
        .with_context(|| format!("failed to switch tmux client to session '{}'", session))
    } else {
        tmux_interactive_status([
            OsString::from("attach-session"),
            OsString::from("-t"),
            OsString::from(session),
        ])
        .with_context(|| format!("failed to attach to tmux session '{}'", session))
    }
}

pub fn kill_session(session: &str) -> Result<()> {
    tmux_status([
        OsString::from("kill-session"),
        OsString::from("-t"),
        OsString::from(session),
    ])
}

pub fn pane_indices(session: &str, window: usize) -> Result<Vec<usize>> {
    let target = format!("{session}:{window}");
    let raw = tmux_stdout([
        OsString::from("list-panes"),
        OsString::from("-t"),
        OsString::from(target),
        OsString::from("-F"),
        OsString::from("#{pane_index}"),
    ])?;

    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.trim()
                .parse::<usize>()
                .with_context(|| format!("failed to parse pane index '{}'", line))
        })
        .collect()
}

fn tmux_stdout<I, S>(args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("tmux")
        .args(args)
        .output()
        .context("failed to run tmux")?;
    if !output.status.success() {
        bail!(
            "tmux failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn tmux_status<I, S>(args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("tmux")
        .args(args)
        .output()
        .context("failed to run tmux")?;
    if !output.status.success() {
        bail!(
            "tmux failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn tmux_interactive_status<I, S>(args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let status = Command::new("tmux")
        .args(args)
        .status()
        .context("failed to run tmux")?;
    if !status.success() {
        bail!("tmux exited with status {}", status);
    }
    Ok(())
}
