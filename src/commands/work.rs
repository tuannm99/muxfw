use crate::cli::{AddArgs, CreateWorkArgs, InitArgs, SaveArgs, UpdateWorkArgs};
use crate::context;
use crate::editor;
use crate::paths::AppPaths;
use crate::snapshot;
use crate::tmux;
use crate::work::{self, Work};
use anyhow::{Context, Result, bail};
use std::fs;

pub fn save(paths: &AppPaths, args: SaveArgs) -> Result<i32> {
    tmux::ensure_tmux_installed()?;
    paths.ensure_state_dirs()?;
    let mut work = context::work_for_save(paths, args)?;
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

pub fn restore(paths: &AppPaths, name: &str) -> Result<i32> {
    tmux::ensure_tmux_installed()?;
    let mut work = work::load_work(paths, name)?;
    crate::restore::restore_work(paths, &work, false)?;
    work.mark_restored_now();
    work.mark_opened_now();
    work::write_work(paths, &work)?;
    crate::restore::open_work(paths, &work)?;
    Ok(0)
}

pub fn create(paths: &AppPaths, args: CreateWorkArgs) -> Result<i32> {
    paths.ensure_state_dirs()?;
    let edit = args.edit;
    let work = build_created_work(args)?;
    create_work(paths, work, edit)?;
    Ok(0)
}

pub fn add(paths: &AppPaths, args: AddArgs) -> Result<i32> {
    paths.ensure_state_dirs()?;
    if args.target != "current" {
        if args.name.is_some() {
            bail!("--name is only valid with `muxwf add current`");
        }
        let edit = args.edit;
        let work = build_created_work(create_work_args_from_add(args))?;
        create_work(paths, work, edit)?;
        return Ok(0);
    }

    crate::discover::ensure_session_option_absent(&args.session)?;
    tmux::ensure_tmux_installed()?;
    let session = tmux::current_session_name()?;
    let snapshot = tmux::capture_session(&session)?;
    let work = crate::discover::work_from_snapshot(
        &snapshot,
        args.name.clone(),
        args.root.clone(),
        crate::discover::apply_add_args_metadata(&args),
    )
    .with_context(|| format!("failed to generate work config from session '{}'", session))?;
    create_discovered_work(paths, work, &snapshot, args.edit, false)?;
    Ok(0)
}

pub fn init(paths: &AppPaths, args: InitArgs) -> Result<i32> {
    tmux::ensure_tmux_installed()?;
    paths.ensure_state_dirs()?;
    init_from_running_sessions(paths, args)?;
    Ok(0)
}

pub fn update(paths: &AppPaths, args: UpdateWorkArgs) -> Result<i32> {
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
        return Ok(0);
    }

    work::write_work(paths, &work)?;
    println!("updated '{}'", work.name);
    Ok(0)
}

pub fn edit(paths: &AppPaths, name: &str) -> Result<i32> {
    work::validate_name(name)?;
    let path = paths.work_file(name);
    if !path.exists() {
        bail!("work '{}' does not exist at {}", name, path.display());
    }
    editor::edit_path(&path)?;
    Ok(0)
}

pub fn close(paths: &AppPaths, name: &str) -> Result<i32> {
    tmux::ensure_tmux_installed()?;
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
    Ok(0)
}

pub fn delete(paths: &AppPaths, name: &str) -> Result<i32> {
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
    } else if removed_session {
        println!("deleted '{}' (killed session '{}')", name, work.session);
    } else {
        println!("deleted '{}'", name);
    }
    Ok(0)
}

pub fn save_current_work_if_needed(paths: &AppPaths, target_name: Option<&str>) -> Result<()> {
    let Ok(current) = context::current_work(paths) else {
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

fn build_created_work(args: CreateWorkArgs) -> Result<Work> {
    let root = args.root.unwrap_or(context::current_dir_string()?);
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
        let work = crate::discover::work_from_snapshot(
            &snapshot,
            None,
            None,
            crate::discover::WorkMetadata::default(),
        )
        .with_context(|| format!("failed to generate work config from session '{}'", session))?;
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
        editor::edit_path(&work_path)?;
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
        editor::edit_path(&path)?;
    }
    Ok(())
}
