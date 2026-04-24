mod autocomplete;
mod cli;
mod discover;
mod paths;
mod plugin;
mod restore;
mod rules;
mod snapshot;
mod tmux;
mod work;
mod workspace;

use crate::cli::{
    AddArgs, Cli, Commands, CreateWorkArgs, CreateWorkspaceArgs, InitArgs, JumpArgs, ListArgs,
    SaveArgs, StaleArgs, UpdateWorkArgs, UpdateWorkspaceArgs, WorkCommands, WorkspaceCommands,
    WorkspaceListArgs, WorkspaceMembersArgs,
};
use crate::paths::{AppPaths, find_binary};
use crate::work::{Work, WorkStatus};
use crate::workspace::WorkspaceOpenPolicy;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use clap::Parser;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("error: {error:#}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    let paths = AppPaths::new()?;

    match cli.command {
        Commands::Save(args) => {
            tmux::ensure_tmux_installed()?;
            paths.ensure_state_dirs()?;
            let mut work = work_for_save(&paths, args)?;
            let snapshot = tmux::capture_session(&work.session)?;
            snapshot::write_snapshot(&paths, &work.name, &snapshot)?;
            work.mark_saved_now();
            work::write_work(&paths, &work)?;
            println!(
                "saved '{}' to {}",
                work.name,
                paths.display_path(&paths.snapshot_file(&work.name))
            );
            Ok(0)
        }
        Commands::Restore(target) => {
            tmux::ensure_tmux_installed()?;
            let mut work = work::load_work(&paths, &target.name)?;
            restore::restore_work(&paths, &work, false)?;
            work.mark_restored_now();
            work.mark_opened_now();
            work::write_work(&paths, &work)?;
            restore::open_work(&paths, &work)?;
            Ok(0)
        }
        Commands::Open(target) => {
            tmux::ensure_tmux_installed()?;
            run_open(&paths, target)?;
            Ok(0)
        }
        Commands::Close(target) => {
            tmux::ensure_tmux_installed()?;
            close_work(&paths, &target.name)?;
            Ok(0)
        }
        Commands::Current => {
            tmux::ensure_tmux_installed()?;
            print_current_work(&paths)?;
            Ok(0)
        }
        Commands::List(args) => {
            print_work_list(&paths, args)?;
            Ok(0)
        }
        Commands::Recent => {
            print_recent_works(&paths)?;
            Ok(0)
        }
        Commands::Stale(args) => {
            print_stale_works(&paths, args)?;
            Ok(0)
        }
        Commands::Show(target) => {
            let raw = snapshot::raw_snapshot(&paths, &target.name)?;
            let parsed: snapshot::Snapshot =
                serde_json::from_str(&raw).context("snapshot is not valid JSON")?;
            println!("{}", serde_json::to_string_pretty(&parsed)?);
            Ok(0)
        }
        Commands::Doctor => run_doctor(&paths),
        Commands::Version => {
            println!("muxwf {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        Commands::Jump(args) => {
            tmux::ensure_tmux_installed()?;
            run_jump(&paths, args)?;
            Ok(0)
        }
        Commands::Completion(args) => {
            autocomplete::print_completion(args.shell, Some(args.name));
            Ok(0)
        }
        Commands::Init(args) => {
            tmux::ensure_tmux_installed()?;
            paths.ensure_state_dirs()?;
            init_from_running_sessions(&paths, args)?;
            Ok(0)
        }
        Commands::Work { command } => run_work_command(&paths, command),
        Commands::Workspace { command } => run_workspace_command(&paths, command),
        Commands::Pin(target) => {
            set_favorite(&paths, &target.name, true)?;
            Ok(0)
        }
        Commands::Unpin(target) => {
            set_favorite(&paths, &target.name, false)?;
            Ok(0)
        }
        Commands::Archive(target) => {
            set_work_status(&paths, &target.name, WorkStatus::Archived)?;
            Ok(0)
        }
        Commands::Add(args) => {
            paths.ensure_state_dirs()?;
            run_add_command(&paths, args)?;
            Ok(0)
        }
        Commands::Edit(target) => {
            edit_work(&paths, &target.name)?;
            Ok(0)
        }
        Commands::Rm(target) => {
            delete_work_and_snapshot(&paths, &target.name)?;
            Ok(0)
        }
        Commands::Plugin(argv) => {
            paths.ensure_state_dirs()?;
            plugin::execute_external(&paths, &argv)
        }
    }
}

fn run_work_command(paths: &AppPaths, command: WorkCommands) -> Result<i32> {
    match command {
        WorkCommands::Save(args) => {
            tmux::ensure_tmux_installed()?;
            paths.ensure_state_dirs()?;
            let mut work = work_for_save(paths, args)?;
            let snapshot = tmux::capture_session(&work.session)?;
            snapshot::write_snapshot(paths, &work.name, &snapshot)?;
            work.mark_saved_now();
            work::write_work(paths, &work)?;
            println!(
                "saved '{}' to {}",
                work.name,
                paths.display_path(&paths.snapshot_file(&work.name))
            );
            Ok(0)
        }
        WorkCommands::Open(target) => {
            tmux::ensure_tmux_installed()?;
            run_open(paths, target)?;
            Ok(0)
        }
        WorkCommands::Create(args) => {
            paths.ensure_state_dirs()?;
            let edit = args.edit;
            let work = build_created_work(args)?;
            create_work(paths, work, edit)?;
            Ok(0)
        }
        WorkCommands::Edit(target) => {
            edit_work(paths, &target.name)?;
            Ok(0)
        }
        WorkCommands::Update(args) => {
            update_work(paths, args)?;
            Ok(0)
        }
        WorkCommands::Delete(target) => {
            delete_work_and_snapshot(paths, &target.name)?;
            Ok(0)
        }
        WorkCommands::List(args) => {
            print_work_list(paths, args)?;
            Ok(0)
        }
    }
}

fn work_for_save(paths: &AppPaths, args: SaveArgs) -> Result<Work> {
    match args.name {
        Some(name) => work::load_work(paths, &name),
        None => current_work(paths).context(
            "save without a work name requires running inside a tmux session mapped to a work",
        ),
    }
}

fn run_workspace_command(paths: &AppPaths, command: WorkspaceCommands) -> Result<i32> {
    match command {
        WorkspaceCommands::List(args) => {
            print_workspace_list(paths, args)?;
            Ok(0)
        }
        WorkspaceCommands::Open(target) => {
            tmux::ensure_tmux_installed()?;
            open_workspace(paths, &target.name)?;
            Ok(0)
        }
        WorkspaceCommands::Create(args) => {
            paths.ensure_state_dirs()?;
            let edit = args.edit;
            let workspace = build_created_workspace(args)?;
            create_workspace(paths, workspace, edit)?;
            Ok(0)
        }
        WorkspaceCommands::Edit(target) => {
            edit_workspace(paths, &target.name)?;
            Ok(0)
        }
        WorkspaceCommands::Update(args) => {
            update_workspace(paths, args)?;
            Ok(0)
        }
        WorkspaceCommands::Add(args) => {
            add_workspace_works(paths, args)?;
            Ok(0)
        }
        WorkspaceCommands::Remove(args) => {
            remove_workspace_works(paths, args)?;
            Ok(0)
        }
        WorkspaceCommands::Delete(target) => {
            delete_workspace(paths, &target.name)?;
            Ok(0)
        }
    }
}

// Print workspace listings as text or JSON, shared by CLI and completion.
fn print_workspace_list(paths: &AppPaths, args: WorkspaceListArgs) -> Result<()> {
    let workspaces = workspace::list_workspaces(paths)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&workspaces)?);
        return Ok(());
    }

    for workspace in workspaces {
        print_workspace_row(&workspace, args.names_only);
    }
    Ok(())
}

// Prepare all works in declaration order, then attach or switch to the first session.
fn open_workspace(paths: &AppPaths, name: &str) -> Result<()> {
    let workspace = workspace::load_workspace(paths, name)?;
    let mut opened = Vec::new();
    for work_name in &workspace.works {
        let mut work = work::load_work(paths, work_name).with_context(|| {
            format!("workspace '{}' references '{}'", workspace.name, work_name)
        })?;
        prepare_work_session(paths, &mut work, workspace.policy)
            .with_context(|| format!("failed to open work '{}'", work.name))?;
        println!("ready\t{}\t{}", work.name, work.session);
        opened.push(work);
    }

    let first = opened
        .first()
        .with_context(|| format!("workspace '{}' has no works", workspace.name))?;
    tmux::switch_or_attach(&first.session)
}

// Convert CLI args into a fully validated workspace struct before writing.
fn build_created_workspace(args: CreateWorkspaceArgs) -> Result<workspace::Workspace> {
    let workspace = workspace::Workspace {
        name: args.name,
        works: args.works,
        profile: args.profile,
        policy: args.policy,
    };
    workspace.validate()?;
    Ok(workspace)
}

// Create a new workspace and optionally open the editor right after writing the file.
fn create_workspace(paths: &AppPaths, workspace: workspace::Workspace, edit: bool) -> Result<()> {
    let path = paths.workspace_file(&workspace.name);
    if path.exists() {
        bail!(
            "workspace '{}' already exists at {}",
            workspace.name,
            path.display()
        );
    }
    workspace::write_workspace(paths, &workspace)?;
    println!(
        "created workspace '{}' at {}",
        workspace.name,
        paths.display_path(&path)
    );
    if edit {
        edit_path(&path)?;
    }
    Ok(())
}

// Open an existing workspace YAML file in $EDITOR.
fn edit_workspace(paths: &AppPaths, name: &str) -> Result<()> {
    work::validate_name(name)?;
    let path = paths.workspace_file(name);
    if !path.exists() {
        bail!("workspace '{}' does not exist at {}", name, path.display());
    }
    edit_path(&path)
}

// Replace the workspace work list with the new CLI input as a whole.
fn update_workspace(paths: &AppPaths, args: UpdateWorkspaceArgs) -> Result<()> {
    paths.ensure_state_dirs()?;
    let mut workspace = workspace::load_workspace(paths, &args.name)?;
    let mut changed = false;

    if workspace.works != args.works {
        workspace.works = args.works;
        changed = true;
    }
    if args.clear_profile && workspace.profile.is_some() {
        workspace.profile = None;
        changed = true;
    }
    if let Some(profile) = args.profile
        && workspace.profile.as_deref() != Some(profile.as_str())
    {
        workspace.profile = Some(profile);
        changed = true;
    }
    if let Some(policy) = args.policy
        && workspace.policy != policy
    {
        workspace.policy = policy;
        changed = true;
    }

    if !changed {
        println!("no changes for workspace '{}'", workspace.name);
        return Ok(());
    }
    workspace::write_workspace(paths, &workspace)?;
    println!("updated workspace '{}'", workspace.name);
    Ok(())
}

// Append new works while preserving existing order and skipping items already present.
fn add_workspace_works(paths: &AppPaths, args: WorkspaceMembersArgs) -> Result<()> {
    paths.ensure_state_dirs()?;
    let mut workspace = workspace::load_workspace(paths, &args.name)?;
    let mut added = 0usize;
    for work_name in args.works {
        if workspace.works.contains(&work_name) {
            continue;
        }
        workspace.works.push(work_name);
        added += 1;
    }
    if added == 0 {
        println!("no changes for workspace '{}'", workspace.name);
        return Ok(());
    }
    workspace::write_workspace(paths, &workspace)?;
    println!("updated workspace '{}' (+{})", workspace.name, added);
    Ok(())
}

// Remove the requested works, but do not allow the workspace to become empty.
fn remove_workspace_works(paths: &AppPaths, args: WorkspaceMembersArgs) -> Result<()> {
    paths.ensure_state_dirs()?;
    let mut workspace = workspace::load_workspace(paths, &args.name)?;
    let original_len = workspace.works.len();
    let removals = args.works.into_iter().collect::<BTreeSet<_>>();
    workspace
        .works
        .retain(|work_name| !removals.contains(work_name));
    if workspace.works.len() == original_len {
        println!("no changes for workspace '{}'", workspace.name);
        return Ok(());
    }
    if workspace.works.is_empty() {
        bail!(
            "workspace '{}' would become empty; delete it instead",
            workspace.name
        );
    }
    let removed = original_len - workspace.works.len();
    workspace::write_workspace(paths, &workspace)?;
    println!("updated workspace '{}' (-{})", workspace.name, removed);
    Ok(())
}

// Delete the workspace from the state directory.
fn delete_workspace(paths: &AppPaths, name: &str) -> Result<()> {
    paths.ensure_state_dirs()?;
    workspace::delete_workspace(paths, name)?;
    println!("deleted workspace '{}'", name);
    Ok(())
}

fn build_created_work(args: CreateWorkArgs) -> Result<Work> {
    let root = args.root.unwrap_or(current_dir_string()?);
    let session = args.session.unwrap_or_else(|| args.name.clone());
    let mut work = Work::new(args.name, session, root);
    if let Some(on_restore) = args.on_restore {
        work.on_restore = Some(on_restore);
    }
    work.description = args.description;
    work.status = args.status;
    work.group = args.group;
    work.tags = args.tags;
    work.favorite = args.favorite;
    work.validate()?;
    Ok(work)
}

fn create_work_args_from_add(args: AddArgs) -> CreateWorkArgs {
    CreateWorkArgs {
        name: args.target,
        session: args.session,
        root: args.root,
        on_restore: args.on_restore,
        description: args.description,
        status: args.status,
        group: args.group,
        tags: args.tags,
        favorite: args.favorite,
        edit: args.edit,
    }
}

fn run_add_command(paths: &AppPaths, args: AddArgs) -> Result<()> {
    if args.target != "current" {
        if args.name.is_some() {
            bail!("--name is only valid with `muxwf add current`");
        }
        let edit = args.edit;
        let work = build_created_work(create_work_args_from_add(args))?;
        return create_work(paths, work, edit);
    }

    discover::ensure_session_option_absent(&args.session)?;
    tmux::ensure_tmux_installed()?;
    let session = tmux::current_session_name()?;
    let snapshot = tmux::capture_session(&session)?;
    let work = discover::work_from_snapshot(
        &snapshot,
        args.name.clone(),
        args.root.clone(),
        discover::apply_add_args_metadata(&args),
    )
    .with_context(|| format!("failed to generate work config from session '{}'", session))?;
    create_discovered_work(paths, work, &snapshot, args.edit, false).map(|_| ())
}

fn init_from_running_sessions(paths: &AppPaths, args: InitArgs) -> Result<()> {
    let sessions = tmux::list_sessions()?;
    if sessions.is_empty() {
        bail!("no running tmux sessions found");
    }

    let mut created = 0usize;
    let mut skipped = 0usize;
    for session in sessions {
        let snapshot = tmux::capture_session(&session)
            .with_context(|| format!("failed to capture tmux session '{}'", session))?;
        let work =
            discover::work_from_snapshot(&snapshot, None, None, discover::WorkMetadata::default())
                .with_context(|| {
                    format!("failed to generate work config from session '{}'", session)
                })?;
        if create_discovered_work(paths, work, &snapshot, false, args.overwrite)? {
            created += 1;
        } else {
            skipped += 1;
        }
    }

    println!("init complete: {created} created, {skipped} skipped");
    Ok(())
}

fn create_discovered_work(
    paths: &AppPaths,
    work: Work,
    snapshot: &snapshot::Snapshot,
    edit: bool,
    overwrite: bool,
) -> Result<bool> {
    let work_path = paths.work_file(&work.name);
    let snapshot_path = paths.snapshot_file(&work.name);

    let write_work = overwrite || !work_path.exists();
    let write_snapshot = overwrite || !snapshot_path.exists();

    if !write_work && !write_snapshot {
        println!(
            "skipped '{}' ({}) because config or snapshot already exists",
            work.name, work.session
        );
        return Ok(false);
    }

    if write_work {
        work::write_work(paths, &work)?;
        println!(
            "created '{}' from tmux session '{}' at {}",
            work.name,
            work.session,
            paths.display_path(&work_path)
        );
    } else {
        println!(
            "kept existing work config '{}'",
            paths.display_path(&work_path)
        );
    }

    if write_snapshot {
        snapshot::write_snapshot(paths, &work.name, snapshot)?;
        println!(
            "saved snapshot for '{}' to {}",
            work.name,
            paths.display_path(&snapshot_path)
        );
    } else {
        println!(
            "kept existing snapshot '{}'",
            paths.display_path(&snapshot_path)
        );
    }

    if edit && write_work {
        edit_path(&work_path)?;
    }
    Ok(true)
}

fn create_work(paths: &AppPaths, work: Work, edit: bool) -> Result<()> {
    let path = paths.work_file(&work.name);
    if path.exists() {
        bail!("work '{}' already exists at {}", work.name, path.display());
    }
    work::write_work(paths, &work)?;
    println!("created '{}' at {}", work.name, paths.display_path(&path));
    if edit {
        edit_path(&path)?;
    }
    Ok(())
}

fn update_work(paths: &AppPaths, args: UpdateWorkArgs) -> Result<()> {
    paths.ensure_state_dirs()?;
    let mut work = work::load_work(paths, &args.name)?;
    let mut changed = false;

    if let Some(session) = args.session {
        work.session = session;
        changed = true;
    }
    if let Some(root) = args.root {
        work.root = root;
        changed = true;
    }
    if let Some(on_restore) = args.on_restore {
        work.on_restore = Some(on_restore);
        changed = true;
    }
    if let Some(description) = args.description {
        work.description = Some(description);
        changed = true;
    }
    if let Some(status) = args.status {
        work.status = status;
        changed = true;
    }
    if args.clear_group {
        work.group = None;
        changed = true;
    }
    if let Some(group) = args.group {
        work.group = Some(group);
        changed = true;
    }
    if args.clear_tags {
        work.tags.clear();
        changed = true;
    }
    if !args.tags.is_empty() {
        work.tags = args.tags;
        changed = true;
    }

    if !changed {
        println!("no changes for '{}'", work.name);
        return Ok(());
    }

    work::write_work(paths, &work)?;
    println!("updated '{}'", work.name);
    Ok(())
}

fn edit_work(paths: &AppPaths, name: &str) -> Result<()> {
    work::validate_name(name)?;
    let path = paths.work_file(name);
    if !path.exists() {
        bail!("work '{}' does not exist at {}", name, path.display());
    }
    edit_path(&path)
}

fn edit_path(path: &Path) -> Result<()> {
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

fn delete_work_and_snapshot(paths: &AppPaths, name: &str) -> Result<()> {
    paths.ensure_state_dirs()?;
    let work = work::load_work(paths, name)?;
    let mut removed_session = false;
    if tmux::session_exists(&work.session)? {
        tmux::kill_session(&work.session)
            .with_context(|| format!("failed to kill tmux session '{}'", work.session))?;
        removed_session = true;
    }

    work::delete_work(paths, name)?;
    let snapshot_path = paths.snapshot_file(name);
    if snapshot_path.exists() {
        fs::remove_file(&snapshot_path)
            .with_context(|| format!("failed to delete {}", snapshot_path.display()))?;
        if removed_session {
            println!(
                "deleted '{}' (killed session '{}') and {}",
                name,
                work.session,
                paths.display_path(&snapshot_path)
            );
        } else {
            println!(
                "deleted '{}' and {}",
                name,
                paths.display_path(&snapshot_path)
            );
        }
    } else {
        if removed_session {
            println!("deleted '{}' (killed session '{}')", name, work.session);
        } else {
            println!("deleted '{}'", name);
        }
    }
    Ok(())
}

fn print_work_list(paths: &AppPaths, args: ListArgs) -> Result<()> {
    let works = filtered_works(paths, &args)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&works)?);
        return Ok(());
    }

    for work in &works {
        print_work_row(work, args.names_only);
    }
    Ok(())
}

fn print_recent_works(paths: &AppPaths) -> Result<()> {
    print_work_list(
        paths,
        ListArgs {
            names_only: false,
            json: false,
            tags: Vec::new(),
            group: None,
            favorite: false,
            status: None,
            recent: true,
            live: false,
            stale_days: None,
        },
    )
}

fn print_stale_works(paths: &AppPaths, args: StaleArgs) -> Result<()> {
    print_work_list(
        paths,
        ListArgs {
            names_only: args.names_only,
            json: args.json,
            tags: Vec::new(),
            group: None,
            favorite: false,
            status: None,
            recent: false,
            live: false,
            stale_days: Some(args.days),
        },
    )
}

fn filtered_works(paths: &AppPaths, args: &ListArgs) -> Result<Vec<Work>> {
    let live_sessions = if args.live {
        Some(tmux::list_sessions()?)
    } else {
        None
    };
    let mut works = work::list_works(paths)?;

    works.retain(|work| {
        args.tags
            .iter()
            .all(|tag| work.tags.iter().any(|work_tag| work_tag == tag))
    });
    if let Some(group) = &args.group {
        works.retain(|work| work.group.as_deref() == Some(group.as_str()));
    }
    if args.favorite {
        works.retain(|work| work.favorite);
    }
    if let Some(status) = args.status {
        works.retain(|work| work.status == status);
    }
    if args.recent {
        works.retain(|work| work.last_opened_at.is_some());
    }
    if let Some(stale_days) = args.stale_days {
        works.retain(|work| work.is_stale(stale_days));
    }
    if let Some(live_sessions) = &live_sessions {
        works.retain(|work| live_sessions.contains(&work.session));
    }

    if args.recent {
        works.sort_by(|a, b| {
            b.last_opened_at
                .cmp(&a.last_opened_at)
                .then_with(|| a.name.cmp(&b.name))
        });
    }

    Ok(works)
}

fn print_work_row(work: &Work, names_only: bool) {
    if names_only {
        println!("{}", work.name);
        return;
    }

    let tags = if work.tags.is_empty() {
        "-".to_string()
    } else {
        work.tags.join(",")
    };
    let group = work.group.as_deref().unwrap_or("-");
    let favorite = if work.favorite { "yes" } else { "-" };
    let description = work.description.as_deref().unwrap_or("-");
    let last_opened_at = format_timestamp(work.last_opened_at.as_ref());
    let status = format!("{:?}", work.status).to_lowercase();
    println!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        work.name,
        work.session,
        work.root,
        status,
        tags,
        description,
        group,
        favorite,
        last_opened_at
    );
}

fn print_workspace_row(workspace: &workspace::Workspace, names_only: bool) {
    if names_only {
        println!("{}", workspace.name);
        return;
    }
    let profile = workspace.profile.as_deref().unwrap_or("-");
    println!(
        "{}\t{}\t{}\t{}",
        workspace.name,
        profile,
        format!("{:?}", workspace.policy)
            .to_lowercase()
            .replace('_', "-"),
        workspace.works.join(",")
    );
}

fn run_open(paths: &AppPaths, args: crate::cli::OpenArgs) -> Result<()> {
    if let Some(name) = args.name {
        return open_work_by_name(paths, &name);
    }
    run_jump(
        paths,
        JumpArgs {
            names_only: false,
            json: false,
        },
    )
}

fn open_work_by_name(paths: &AppPaths, name: &str) -> Result<()> {
    save_current_work_if_needed(paths, Some(name))?;
    let mut work = work::load_work(paths, name)?;
    prepare_work_session(paths, &mut work, WorkspaceOpenPolicy::Smart)?;
    restore::open_work(paths, &work)
}

fn save_current_work_if_needed(paths: &AppPaths, target_name: Option<&str>) -> Result<()> {
    let Ok(current) = current_work(paths) else {
        return Ok(());
    };
    if target_name.is_some_and(|target| target == current.name) {
        return Ok(());
    }
    if !tmux::session_exists(&current.session)? {
        return Ok(());
    }

    let snapshot = tmux::capture_session(&current.session)?;
    let mut current = current;
    snapshot::write_snapshot(paths, &current.name, &snapshot)?;
    current.mark_saved_now();
    work::write_work(paths, &current)?;
    eprintln!("saved '{}' before switch", current.name);
    Ok(())
}

fn prepare_work_session(
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

fn close_work(paths: &AppPaths, name: &str) -> Result<()> {
    let mut work = work::load_work(paths, name)?;
    if tmux::session_exists(&work.session)? {
        tmux::kill_session(&work.session)
            .with_context(|| format!("failed to kill tmux session '{}'", work.session))?;
        work.mark_closed_now();
        work::write_work(paths, &work)?;
        println!("closed '{}' ({})", work.name, work.session);
    } else {
        println!("'{}' is not running ({})", work.name, work.session);
    }
    Ok(())
}

fn print_current_work(paths: &AppPaths) -> Result<()> {
    let work = current_work(paths)?;
    print_work_row(&work, false);
    Ok(())
}

fn current_work(paths: &AppPaths) -> Result<Work> {
    let session = tmux::current_session_name()?;
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

fn set_favorite(paths: &AppPaths, name: &str, favorite: bool) -> Result<()> {
    let mut work = work::load_work(paths, name)?;
    if work.favorite == favorite {
        println!(
            "'{}' is already {}",
            work.name,
            if favorite { "pinned" } else { "unpinned" }
        );
        return Ok(());
    }

    work.favorite = favorite;
    work::write_work(paths, &work)?;
    println!(
        "{} '{}'",
        if favorite { "pinned" } else { "unpinned" },
        work.name
    );
    Ok(())
}

fn set_work_status(paths: &AppPaths, name: &str, status: WorkStatus) -> Result<()> {
    let mut work = work::load_work(paths, name)?;
    if work.status == status {
        println!(
            "'{}' is already {}",
            work.name,
            format!("{:?}", status).to_lowercase()
        );
        return Ok(());
    }

    work.status = status;
    work::write_work(paths, &work)?;
    println!(
        "updated '{}' status to {}",
        work.name,
        format!("{:?}", status).to_lowercase()
    );
    Ok(())
}

#[derive(Debug, Serialize)]
struct RankedWorkRow {
    #[serde(flatten)]
    work: Work,
    live: bool,
    jump_rank: u8,
}

fn run_jump(paths: &AppPaths, args: JumpArgs) -> Result<()> {
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
            writeln!(stdin, "{}", format_jump_row(work, *live))
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
        println!("{:>3}\t{}", idx + 1, format_jump_row(work, *live));
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

fn format_jump_row(work: &Work, live: bool) -> String {
    let favorite = if work.favorite { "favorite" } else { "-" };
    let live = if live { "live" } else { "-" };
    let group = work.group.as_deref().unwrap_or("-");
    let last_opened_at = format_timestamp(work.last_opened_at.as_ref());
    let description = work.description.as_deref().unwrap_or("-");
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        work.name, favorite, live, group, last_opened_at, description
    )
}

fn format_timestamp(value: Option<&DateTime<Utc>>) -> String {
    value
        .map(DateTime::to_rfc3339)
        .unwrap_or_else(|| "-".to_string())
}

fn run_doctor(paths: &AppPaths) -> Result<i32> {
    let mut failures = 0;

    println!("ok    {:<14} {}", "version", env!("CARGO_PKG_VERSION"));

    report(
        "tmux",
        find_binary("tmux")
            .map(|path| format!("found {}", path.display()))
            .ok_or_else(|| "tmux not found in PATH".to_string()),
        &mut failures,
    );

    if paths.base_dir().exists() {
        println!("ok    {:<14} {}", "base", paths.base_dir().display());
    } else {
        println!(
            "warn  {:<14} {} does not exist yet",
            "base",
            paths.base_dir().display()
        );
    }
    check_state_dirs(paths);

    match rules::Ruleset::load(paths) {
        Ok(_) => {
            if paths.config_file().exists() {
                println!("ok    {:<14} {}", "config", paths.config_file().display());
            } else {
                println!(
                    "warn  {:<14} {} missing; restore rules disabled",
                    "config",
                    paths.config_file().display()
                );
            }
        }
        Err(error) => {
            failures += 1;
            println!("fail  {:<14} {error:#}", "config");
        }
    }

    failures += check_works(paths)?;
    failures += check_workspaces(paths)?;
    failures += check_snapshots(paths)?;
    failures += check_plugins(paths)?;

    if let Some(path) = find_binary("fzf") {
        println!("ok    {:<14} found {}", "fzf", path.display());
    } else {
        println!(
            "warn  {:<14} fzf not found; 'muxwf jump' will prompt",
            "fzf"
        );
    }

    Ok(if failures == 0 { 0 } else { 1 })
}

fn check_state_dirs(paths: &AppPaths) {
    for (label, dir) in [
        ("works_dir", paths.works_dir()),
        ("snapshots_dir", paths.snapshots_dir()),
        ("plugins_dir", paths.plugins_dir()),
        ("workspaces_dir", paths.workspaces_dir()),
    ] {
        if dir.exists() {
            println!("ok    {label:<14} {}", dir.display());
        } else {
            println!("warn  {label:<14} {} does not exist yet", dir.display());
        }
    }
}

fn check_works(paths: &AppPaths) -> Result<i32> {
    let files = work::work_files(paths)?;
    if files.is_empty() {
        println!("warn  {:<14} no work files found", "works");
        return Ok(0);
    }

    let mut failures = 0;
    for file in &files {
        match work::load_work_file(file) {
            Ok(work) => {
                if !work.root_path(paths).is_dir() {
                    failures += 1;
                    println!(
                        "fail  {:<14} work '{}' root does not exist: {}",
                        "works",
                        work.name,
                        work.root_path(paths).display()
                    );
                }
            }
            Err(error) => {
                failures += 1;
                println!("fail  {:<14} {}: {error:#}", "works", file.display());
            }
        }
    }
    if failures == 0 {
        println!("ok    {:<14} {} valid work file(s)", "works", files.len());
    }
    Ok(failures)
}

fn check_workspaces(paths: &AppPaths) -> Result<i32> {
    let files = workspace::workspace_files(paths)?;
    if files.is_empty() {
        println!("warn  {:<14} no workspace files found", "workspaces");
        return Ok(0);
    }

    let mut known_works = BTreeSet::new();
    for file in work::work_files(paths)? {
        if let Ok(work) = work::load_work_file(&file) {
            known_works.insert(work.name);
        }
    }
    let mut failures = 0;
    for file in &files {
        match workspace::load_workspace_file(file) {
            Ok(workspace) => {
                for work_name in &workspace.works {
                    if !known_works.contains(work_name) {
                        failures += 1;
                        println!(
                            "fail  {:<14} workspace '{}' references missing work '{}'",
                            "workspaces", workspace.name, work_name
                        );
                    }
                }
            }
            Err(error) => {
                failures += 1;
                println!("fail  {:<14} {}: {error:#}", "workspaces", file.display());
            }
        }
    }
    if failures == 0 {
        println!(
            "ok    {:<14} {} valid workspace file(s)",
            "workspaces",
            files.len()
        );
    }
    Ok(failures)
}

fn check_snapshots(paths: &AppPaths) -> Result<i32> {
    let files = snapshot::snapshot_files(paths)?;
    if files.is_empty() {
        println!("warn  {:<14} no snapshot files found", "snapshots");
        return Ok(0);
    }

    let mut failures = 0;
    for file in &files {
        match snapshot::read_snapshot_file(file) {
            Ok(snapshot) => {
                if let Some(expected) = file.file_stem().and_then(|name| name.to_str())
                    && let Some(work_name) = snapshot.work_name
                    && work_name != expected
                {
                    failures += 1;
                    println!(
                        "fail  {:<14} {} is tied to work '{}', expected '{}'",
                        "snapshots",
                        file.display(),
                        work_name,
                        expected
                    );
                }
            }
            Err(error) => {
                failures += 1;
                println!("fail  {:<14} {}: {error:#}", "snapshots", file.display());
            }
        }
    }
    if failures == 0 {
        println!(
            "ok    {:<14} {} valid snapshot file(s)",
            "snapshots",
            files.len()
        );
    }
    Ok(failures)
}

fn check_plugins(paths: &AppPaths) -> Result<i32> {
    let files = plugin::plugin_files(paths)?;
    if files.is_empty() {
        println!("warn  {:<14} no plugin files found", "plugins");
        return Ok(0);
    }

    let mut failures = 0;
    for file in &files {
        match plugin::load_plugin_file(file) {
            Ok(plugin) => {
                if find_binary(&plugin.binary).is_none() {
                    failures += 1;
                    println!(
                        "fail  {:<14} plugin '{}' binary '{}' not found",
                        "plugins", plugin.name, plugin.binary
                    );
                }
            }
            Err(error) => {
                failures += 1;
                println!("fail  {:<14} {}: {error:#}", "plugins", file.display());
            }
        }
    }
    if failures == 0 {
        println!(
            "ok    {:<14} {} valid plugin file(s)",
            "plugins",
            files.len()
        );
    }
    Ok(failures)
}

fn report(label: &str, result: std::result::Result<String, String>, failures: &mut i32) {
    match result {
        Ok(message) => println!("ok    {label:<14} {message}"),
        Err(message) => {
            *failures += 1;
            println!("fail  {label:<14} {message}");
        }
    }
}

fn current_dir_string() -> Result<String> {
    std::env::current_dir()
        .context("failed to read current directory")
        .map(|path| path.display().to_string())
}
