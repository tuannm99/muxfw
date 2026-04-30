use crate::cli::{Cli, Commands, WorkCommands, WorkspaceCommands};
use crate::commands::{doctor, list, open, work, workspace};
use crate::paths::AppPaths;
use anyhow::{Context, Result};
use clap::Parser;

pub fn run() -> Result<i32> {
    let cli = Cli::parse();
    let paths = AppPaths::new()?;

    match cli.command {
        Commands::Save(args) => work::save(&paths, args),
        Commands::Restore(target) => work::restore(&paths, &target.name),
        Commands::Open(args) => open::open_command(&paths, args),
        Commands::Close(target) => work::close(&paths, &target.name),
        Commands::Current => list::current(&paths),
        Commands::List(args) => list::list(&paths, args),
        Commands::Recent => list::recent(&paths),
        Commands::Stale(args) => list::stale(&paths, args),
        Commands::Show(target) => {
            let raw = crate::snapshot::raw_snapshot(&paths, &target.name)?;
            let parsed: crate::snapshot::Snapshot =
                serde_json::from_str(&raw).context("snapshot is not valid JSON")?;
            println!("{}", serde_json::to_string_pretty(&parsed)?);
            Ok(0)
        }
        Commands::Doctor => doctor::run(&paths),
        Commands::Version => {
            println!("muxwf {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        Commands::Jump(args) => open::jump_command(&paths, args),
        Commands::Completion(args) => {
            crate::autocomplete::print_completion(args.shell, Some(args.name));
            Ok(0)
        }
        Commands::Init(args) => work::init(&paths, args),
        Commands::Work { command } => run_work_command(&paths, command),
        Commands::Workspace { command } => run_workspace_command(&paths, command),
        Commands::Pin(target) => list::set_favorite(&paths, &target.name, true),
        Commands::Unpin(target) => list::set_favorite(&paths, &target.name, false),
        Commands::Archive(target) => {
            list::set_work_status(&paths, &target.name, crate::work::WorkStatus::Archived)
        }
        Commands::Add(args) => work::add(&paths, args),
        Commands::Edit(target) => work::edit(&paths, &target.name),
        Commands::Rm(target) => work::delete(&paths, &target.name),
        Commands::Plugin(argv) => {
            paths.ensure_state_dirs()?;
            crate::plugin::execute_external(&paths, &argv)
        }
    }
}

fn run_work_command(paths: &AppPaths, command: WorkCommands) -> Result<i32> {
    match command {
        WorkCommands::Save(args) => work::save(paths, args),
        WorkCommands::Open(args) => open::open_command(paths, args),
        WorkCommands::Create(args) => work::create(paths, args),
        WorkCommands::Edit(target) => work::edit(paths, &target.name),
        WorkCommands::Update(args) => work::update(paths, args),
        WorkCommands::Delete(target) => work::delete(paths, &target.name),
        WorkCommands::List(args) => list::list(paths, args),
    }
}

fn run_workspace_command(paths: &AppPaths, command: WorkspaceCommands) -> Result<i32> {
    match command {
        WorkspaceCommands::List(args) => workspace::list(paths, args),
        WorkspaceCommands::Open(target) => workspace::open(paths, &target.name),
        WorkspaceCommands::Create(args) => workspace::create(paths, args),
        WorkspaceCommands::Edit(target) => workspace::edit(paths, &target.name),
        WorkspaceCommands::Update(args) => workspace::update(paths, args),
        WorkspaceCommands::Add(args) => workspace::add_members(paths, args),
        WorkspaceCommands::Remove(args) => workspace::remove_members(paths, args),
        WorkspaceCommands::Delete(target) => workspace::delete(paths, &target.name),
    }
}
