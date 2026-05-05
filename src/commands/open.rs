use crate::cli::{JumpArgs, OpenArgs};
use crate::output;
use crate::paths::{AppPaths, find_binary};
use crate::restore;
use crate::snapshot;
use crate::tmux;
use crate::work::{self, Work};
use crate::workspace::WorkspaceOpenPolicy;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::io::{self, Write};
use std::process::{Command, Stdio};

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum JumpTargetRow {
    Work {
        tracked: bool,
        #[serde(flatten)]
        work: Work,
        live: bool,
        jump_rank: u8,
    },
    LiveSession {
        tracked: bool,
        name: String,
        session: String,
        live: bool,
        jump_rank: u8,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum JumpTarget {
    Work { work: Work, live: bool },
    LiveSession { session: String },
}

impl JumpTarget {
    fn selection_name(&self) -> &str {
        match self {
            Self::Work { work, .. } => &work.name,
            Self::LiveSession { session } => session,
        }
    }

    fn session_name(&self) -> &str {
        match self {
            Self::Work { work, .. } => &work.session,
            Self::LiveSession { session } => session,
        }
    }

    fn jump_rank(&self) -> u8 {
        match self {
            Self::Work { work, live } => jump_rank(work, *live),
            Self::LiveSession { .. } => 2,
        }
    }

    fn to_row(&self) -> JumpTargetRow {
        match self {
            Self::Work { work, live } => JumpTargetRow::Work {
                tracked: true,
                work: work.clone(),
                live: *live,
                jump_rank: self.jump_rank(),
            },
            Self::LiveSession { session } => JumpTargetRow::LiveSession {
                tracked: false,
                name: session.clone(),
                session: session.clone(),
                live: true,
                jump_rank: self.jump_rank(),
            },
        }
    }
}

pub fn open_command(paths: &AppPaths, args: OpenArgs) -> Result<i32> {
    tmux::ensure_tmux_installed()?;
    if let Some(name) = args.name {
        open_target_by_name(paths, &name)?;
    } else {
        run_jump(
            paths,
            JumpArgs {
                names_only: false,
                json: false,
            },
        )?;
    }
    Ok(0)
}

pub fn jump_command(paths: &AppPaths, args: JumpArgs) -> Result<i32> {
    tmux::ensure_tmux_installed()?;
    run_jump(paths, args)?;
    Ok(0)
}

pub fn open_work_by_name(paths: &AppPaths, name: &str) -> Result<()> {
    crate::commands::work::save_current_work_if_needed(paths, Some(name))?;
    let mut work = work::load_work(paths, name)?;
    prepare_work_session(paths, &mut work, WorkspaceOpenPolicy::Smart)?;
    tmux::switch_or_attach(&work.session)
}

pub fn open_target_by_name(paths: &AppPaths, name: &str) -> Result<()> {
    if paths.work_file(name).exists() {
        return open_work_by_name(paths, name);
    }

    if tmux::session_exists(name)? {
        crate::commands::work::save_current_work_if_needed(paths, None)?;
        return tmux::switch_or_attach(name);
    }

    bail!(
        "unknown work or live tmux session '{}'; create a work first or start that session",
        name
    );
}

pub fn prepare_work_session(
    paths: &AppPaths,
    work: &mut Work,
    policy: WorkspaceOpenPolicy,
) -> Result<()> {
    match policy {
        WorkspaceOpenPolicy::Smart => {
            if restore::ensure_work_session(paths, work)? {
                work.mark_restored_now();
            }
        }
        WorkspaceOpenPolicy::ReuseOnly => {
            if !tmux::session_exists(&work.session)? {
                bail!(
                    "workspace policy 'reuse-only' requires tmux session '{}' for work '{}'",
                    work.session,
                    work.name
                );
            }
        }
        WorkspaceOpenPolicy::RestoreOnly => {
            if !tmux::session_exists(&work.session)? {
                if !snapshot::snapshot_exists(paths, &work.name) {
                    bail!(
                        "workspace policy 'restore-only' requires a running session or snapshot for work '{}'",
                        work.name
                    );
                }
                restore::restore_work(paths, work, false)?;
                work.mark_restored_now();
            }
        }
        WorkspaceOpenPolicy::Fresh => {
            if tmux::session_exists(&work.session)? {
                tmux::kill_session(&work.session).with_context(|| {
                    format!(
                        "failed to kill tmux session '{}' for fresh open",
                        work.session
                    )
                })?;
            }
            if snapshot::snapshot_exists(paths, &work.name) {
                restore::restore_work(paths, work, false)?;
                work.mark_restored_now();
            } else {
                restore::create_session_from_work(paths, work, false)?;
            }
        }
    }
    work.mark_opened_now();
    work::write_work(paths, work)?;
    Ok(())
}

pub fn run_jump(paths: &AppPaths, args: JumpArgs) -> Result<()> {
    let targets = ranked_targets(paths)?;
    if targets.is_empty() {
        bail!("no works or running tmux sessions found");
    }

    if args.json {
        let rows = targets.iter().map(JumpTarget::to_row).collect::<Vec<_>>();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    if args.names_only {
        for target in &targets {
            println!("{}", target.selection_name());
        }
        return Ok(());
    }

    let selected = if find_binary("fzf").is_some() {
        select_with_fzf(&targets)?
    } else {
        eprintln!("fzf not found; using prompt fallback");
        select_with_prompt(&targets)?
    };
    if let Some(selected) = selected {
        open_target_by_name(paths, &selected)?;
    }
    Ok(())
}

fn ranked_targets(paths: &AppPaths) -> Result<Vec<JumpTarget>> {
    let live_sessions = tmux::list_sessions()?;
    let mut targets = work::list_works(paths)?
        .into_iter()
        .map(|work| JumpTarget::Work {
            live: live_sessions.contains(&work.session),
            work,
        })
        .collect::<Vec<_>>();

    for session in live_sessions {
        if targets
            .iter()
            .any(|target| target.session_name() == session)
        {
            continue;
        }
        targets.push(JumpTarget::LiveSession { session });
    }

    targets.sort_by(|a, b| {
        a.jump_rank()
            .cmp(&b.jump_rank())
            .then_with(|| match (a, b) {
                (JumpTarget::Work { work: a_work, .. }, JumpTarget::Work { work: b_work, .. }) => {
                    b_work
                        .last_opened_at
                        .cmp(&a_work.last_opened_at)
                        .then_with(|| a_work.name.cmp(&b_work.name))
                }
                _ => a.selection_name().cmp(b.selection_name()),
            })
    });
    Ok(targets)
}

fn jump_rank(work: &Work, live: bool) -> u8 {
    if work.favorite {
        0
    } else if work.last_opened_at.is_some() {
        1
    } else if live {
        2
    } else {
        3
    }
}

fn select_with_fzf(targets: &[JumpTarget]) -> Result<Option<String>> {
    let mut child = Command::new("fzf")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to start fzf")?;

    {
        let mut stdin = child.stdin.take().context("failed to open fzf stdin")?;
        for target in targets {
            writeln!(stdin, "{}", output::format_jump_row(target))
                .context("failed to write work list to fzf")?;
        }
    }

    let output = child.wait_with_output().context("failed to wait for fzf")?;
    if !output.status.success() {
        return Ok(None);
    }

    let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if selected.is_empty() {
        return Ok(None);
    }

    Ok(selected
        .split('\t')
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_string))
}

fn select_with_prompt(targets: &[JumpTarget]) -> Result<Option<String>> {
    for (idx, target) in targets.iter().enumerate() {
        println!("{:>3}\t{}", idx + 1, output::format_jump_row(target));
    }
    print!("select work: ");
    io::stdout().flush().context("failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read selection")?;
    let input = input.trim();
    if input.is_empty() {
        return Ok(None);
    }
    if let Ok(number) = input.parse::<usize>() {
        if number == 0 {
            bail!("selection 0 is out of range");
        }
        return targets
            .get(number - 1)
            .map(|target| Some(target.selection_name().to_string()))
            .with_context(|| format!("selection {} is out of range", number));
    }
    if targets
        .iter()
        .any(|target| target.selection_name() == input)
    {
        return Ok(Some(input.to_string()));
    }
    bail!("unknown work '{}'", input)
}
