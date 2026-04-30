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
struct RankedWorkRow {
    #[serde(flatten)]
    work: Work,
    live: bool,
    jump_rank: u8,
}

pub fn open_command(paths: &AppPaths, args: OpenArgs) -> Result<i32> {
    tmux::ensure_tmux_installed()?;
    if let Some(name) = args.name {
        open_work_by_name(paths, &name)?;
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
    let works = ranked_works(paths)?;
    if works.is_empty() {
        bail!(
            "no works found in {}",
            paths.display_path(&paths.works_dir())
        );
    }

    if args.json {
        let rows = works
            .iter()
            .map(|(work, live)| RankedWorkRow {
                work: work.clone(),
                live: *live,
                jump_rank: jump_rank(work, *live),
            })
            .collect::<Vec<_>>();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    if args.names_only {
        for (work, _) in &works {
            println!("{}", work.name);
        }
        return Ok(());
    }

    let selected = if find_binary("fzf").is_some() {
        select_with_fzf(&works)?
    } else {
        eprintln!("fzf not found; using prompt fallback");
        select_with_prompt(&works)?
    };
    if let Some(selected) = selected {
        open_work_by_name(paths, &selected)?;
    }
    Ok(())
}

fn ranked_works(paths: &AppPaths) -> Result<Vec<(Work, bool)>> {
    let live_sessions = tmux::list_sessions()?;
    let mut works = work::list_works(paths)?
        .into_iter()
        .map(|work| {
            let live = live_sessions.contains(&work.session);
            (work, live)
        })
        .collect::<Vec<_>>();

    works.sort_by(|(a, a_live), (b, b_live)| {
        jump_rank(a, *a_live)
            .cmp(&jump_rank(b, *b_live))
            .then_with(|| b.last_opened_at.cmp(&a.last_opened_at))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(works)
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

fn select_with_fzf(works: &[(Work, bool)]) -> Result<Option<String>> {
    let mut child = Command::new("fzf")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to start fzf")?;

    {
        let mut stdin = child.stdin.take().context("failed to open fzf stdin")?;
        for (work, live) in works {
            writeln!(stdin, "{}", output::format_jump_row(work, *live))
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

fn select_with_prompt(works: &[(Work, bool)]) -> Result<Option<String>> {
    for (idx, (work, live)) in works.iter().enumerate() {
        println!("{:>3}\t{}", idx + 1, output::format_jump_row(work, *live));
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
        return works
            .get(number - 1)
            .map(|(work, _)| Some(work.name.clone()))
            .with_context(|| format!("selection {} is out of range", number));
    }
    if works.iter().any(|(work, _)| work.name == input) {
        return Ok(Some(input.to_string()));
    }
    bail!("unknown work '{}'", input)
}
