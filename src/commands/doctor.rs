use crate::paths::{AppPaths, find_binary};
use anyhow::Result;
use std::collections::BTreeSet;

pub fn run(paths: &AppPaths) -> Result<i32> {
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

    match crate::rules::Ruleset::load(paths) {
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
    let files = crate::work::work_files(paths)?;
    if files.is_empty() {
        println!("warn  {:<14} no work files found", "works");
        return Ok(0);
    }

    let mut failures = 0;
    for file in &files {
        match crate::work::load_work_file(file) {
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
    let files = crate::workspace::workspace_files(paths)?;
    if files.is_empty() {
        println!("warn  {:<14} no workspace files found", "workspaces");
        return Ok(0);
    }

    let mut known_works = BTreeSet::new();
    for file in crate::work::work_files(paths)? {
        if let Ok(work) = crate::work::load_work_file(&file) {
            known_works.insert(work.name);
        }
    }
    let mut failures = 0;
    for file in &files {
        match crate::workspace::load_workspace_file(file) {
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
    let files = crate::snapshot::snapshot_files(paths)?;
    if files.is_empty() {
        println!("warn  {:<14} no snapshot files found", "snapshots");
        return Ok(0);
    }

    let mut failures = 0;
    for file in &files {
        match crate::snapshot::read_snapshot_file(file) {
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
    let files = crate::plugin::plugin_files(paths)?;
    if files.is_empty() {
        println!("warn  {:<14} no plugin files found", "plugins");
        return Ok(0);
    }

    let mut failures = 0;
    for file in &files {
        match crate::plugin::load_plugin_file(file) {
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
