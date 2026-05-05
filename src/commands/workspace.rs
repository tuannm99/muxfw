use crate::cli::{
    CreateWorkspaceArgs, UpdateWorkspaceArgs, WorkspaceListArgs, WorkspaceMembersArgs,
};
use crate::editor;
use crate::output;
use crate::paths::AppPaths;
use crate::work::Work;
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn list(paths: &AppPaths, args: WorkspaceListArgs) -> Result<i32> {
    let workspaces = crate::workspace::list_workspaces(paths)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&workspaces)?);
        return Ok(0);
    }

    for workspace in workspaces {
        output::print_workspace_row(&workspace, args.names_only);
    }
    Ok(0)
}

pub fn open(paths: &AppPaths, name: &str) -> Result<i32> {
    crate::tmux::ensure_tmux_installed()?;
    let workspace = crate::workspace::load_workspace(paths, name)?;
    let mut opened = Vec::new();
    for work_name in &workspace.works {
        let mut work = crate::work::load_work(paths, work_name).with_context(|| {
            format!("workspace '{}' references '{}'", workspace.name, work_name)
        })?;
        crate::commands::open::prepare_work_session(paths, &mut work, workspace.policy)
            .with_context(|| format!("failed to open work '{}'", work.name))?;
        println!("ready\t{}\t{}", work.name, work.session);
        opened.push(work);
    }

    let first = opened
        .first()
        .with_context(|| format!("workspace '{}' has no works", workspace.name))?;
    crate::tmux::switch_or_attach(&first.session)?;
    Ok(0)
}

pub fn create(paths: &AppPaths, args: CreateWorkspaceArgs) -> Result<i32> {
    paths.ensure_state_dirs()?;
    let edit = args.edit;
    let workspace = build_created_workspace(paths, args)?;
    create_workspace(paths, workspace, edit)?;
    Ok(0)
}

pub fn edit(paths: &AppPaths, name: &str) -> Result<i32> {
    crate::work::validate_name(name)?;
    let path = paths.workspace_file(name);
    if !path.exists() {
        bail!("workspace '{}' does not exist at {}", name, path.display());
    }
    editor::edit_path(&path)?;
    Ok(0)
}

pub fn update(paths: &AppPaths, args: UpdateWorkspaceArgs) -> Result<i32> {
    paths.ensure_state_dirs()?;
    let mut workspace = crate::workspace::load_workspace(paths, &args.name)?;
    let resolved_works = resolve_workspace_works(paths, args.works, args.from_dir, true)?;
    let mut changed = false;

    if workspace.works != resolved_works {
        workspace.works = resolved_works;
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
        return Ok(0);
    }
    crate::workspace::write_workspace(paths, &workspace)?;
    println!("updated workspace '{}'", workspace.name);
    Ok(0)
}

pub fn add_members(paths: &AppPaths, args: WorkspaceMembersArgs) -> Result<i32> {
    paths.ensure_state_dirs()?;
    let mut workspace = crate::workspace::load_workspace(paths, &args.name)?;
    let resolved_works = resolve_workspace_works(paths, args.works, args.from_dir, true)?;
    let mut added = 0usize;
    for work_name in resolved_works {
        if workspace.works.contains(&work_name) {
            continue;
        }
        workspace.works.push(work_name);
        added += 1;
    }
    if added == 0 {
        println!("no changes for workspace '{}'", workspace.name);
        return Ok(0);
    }
    crate::workspace::write_workspace(paths, &workspace)?;
    println!("updated workspace '{}' (+{})", workspace.name, added);
    Ok(0)
}

pub fn remove_members(paths: &AppPaths, args: WorkspaceMembersArgs) -> Result<i32> {
    paths.ensure_state_dirs()?;
    let mut workspace = crate::workspace::load_workspace(paths, &args.name)?;
    let original_len = workspace.works.len();
    let removals = resolve_workspace_works(paths, args.works, args.from_dir, false)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    workspace
        .works
        .retain(|work_name| !removals.contains(work_name));
    if workspace.works.len() == original_len {
        println!("no changes for workspace '{}'", workspace.name);
        return Ok(0);
    }
    if workspace.works.is_empty() {
        bail!(
            "workspace '{}' would become empty; delete it instead",
            workspace.name
        );
    }
    let removed = original_len - workspace.works.len();
    crate::workspace::write_workspace(paths, &workspace)?;
    println!("updated workspace '{}' (-{})", workspace.name, removed);
    Ok(0)
}

pub fn delete(paths: &AppPaths, name: &str) -> Result<i32> {
    paths.ensure_state_dirs()?;
    crate::workspace::delete_workspace(paths, name)?;
    println!("deleted workspace '{}'", name);
    Ok(0)
}

fn build_created_workspace(
    paths: &AppPaths,
    args: CreateWorkspaceArgs,
) -> Result<crate::workspace::Workspace> {
    let workspace = crate::workspace::Workspace {
        name: args.name,
        works: resolve_workspace_works(paths, args.works, args.from_dir, true)?,
        profile: args.profile,
        policy: args.policy,
    };
    workspace.validate()?;
    Ok(workspace)
}

fn create_workspace(
    paths: &AppPaths,
    workspace: crate::workspace::Workspace,
    edit: bool,
) -> Result<()> {
    let path = paths.workspace_file(&workspace.name);
    if path.exists() {
        bail!(
            "workspace '{}' already exists at {}",
            workspace.name,
            path.display()
        );
    }
    crate::workspace::write_workspace(paths, &workspace)?;
    println!(
        "created workspace '{}' at {}",
        workspace.name,
        paths.display_path(&path)
    );
    if edit {
        editor::edit_path(&path)?;
    }
    Ok(())
}

fn resolve_workspace_works(
    paths: &AppPaths,
    explicit_works: Vec<String>,
    from_dir: Option<String>,
    create_missing: bool,
) -> Result<Vec<String>> {
    let mut works = Vec::new();
    let mut seen = BTreeSet::new();
    let mut derived_from_dir = BTreeSet::new();

    for work_name in explicit_works {
        if seen.insert(work_name.clone()) {
            works.push(work_name);
        }
    }

    if let Some(dir) = from_dir {
        for entry in direct_subdir_entries(paths, &dir)? {
            let source_name = entry
                .file_name()
                .with_context(|| {
                    format!("failed to read directory name for '{}'", entry.display())
                })?
                .to_string_lossy()
                .to_string();
            let work_name = crate::work::sanitize_name(&source_name);
            if work_name.is_empty() {
                bail!(
                    "directory '{}' cannot be converted into a work name",
                    entry.display()
                );
            }
            if !derived_from_dir.insert(work_name.clone()) {
                bail!(
                    "duplicate work name '{}' derived from '{}'",
                    work_name,
                    entry.display()
                );
            }
            if !seen.insert(work_name.clone()) {
                continue;
            }
            if create_missing {
                ensure_work_exists_for_dir(paths, &work_name, &entry)?;
            }
            works.push(work_name);
        }
    }

    if works.is_empty() {
        bail!("pass at least one `--work <name>` or `--from-dir <path>`");
    }

    Ok(works)
}

fn direct_subdir_entries(paths: &AppPaths, dir: &str) -> Result<Vec<PathBuf>> {
    let dir = resolve_scan_dir(paths, dir)?;
    if !dir.exists() {
        bail!("directory '{}' does not exist", dir.display());
    }
    if !dir.is_dir() {
        bail!("'{}' is not a directory", dir.display());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("failed to read entry in {}", dir.display()))?;
        let path = entry.path();
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?
            .is_dir()
        {
            entries.push(path);
        }
    }
    entries.sort();
    Ok(entries)
}

fn resolve_scan_dir(paths: &AppPaths, dir: &str) -> Result<PathBuf> {
    let expanded = paths.expand_home(dir);
    if expanded.is_absolute() {
        return Ok(expanded);
    }
    Ok(std::env::current_dir()
        .context("failed to read current directory")?
        .join(expanded))
}

fn ensure_work_exists_for_dir(paths: &AppPaths, work_name: &str, dir: &Path) -> Result<()> {
    let work_path = paths.work_file(work_name);
    if work_path.exists() {
        return Ok(());
    }

    let work = Work::new(
        work_name.to_string(),
        work_name.to_string(),
        dir.display().to_string(),
    );
    crate::work::write_work(paths, &work)?;
    println!(
        "created work '{}' at {}",
        work_name,
        paths.display_path(&work_path)
    );
    Ok(())
}
