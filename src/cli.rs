use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "muxwf",
    version,
    about = "Personal tmux workflow manager",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Save the configured tmux session into ~/.muxwf/snapshots/<work>.json.
    Save(SaveArgs),
    /// Restore the configured tmux session from its snapshot.
    Restore(WorkTarget),
    /// Switch/attach to the session, restoring or creating it if needed.
    Open(WorkTarget),
    /// Kill the configured tmux session while keeping its snapshot.
    Close(WorkTarget),
    /// Print the work mapped to the current tmux session.
    Current,
    /// List works.
    List(ListArgs),
    /// List recently opened works.
    Recent,
    /// Print the saved snapshot JSON for a work.
    Show(WorkTarget),
    /// Validate environment and config files.
    Doctor,
    /// Select a work with fzf and open it.
    Jump,
    /// Create a work from the current directory.
    Init(InitArgs),
    /// Manage works.
    Work {
        #[command(subcommand)]
        command: WorkCommands,
    },
    /// Manage workspace bundles.
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommands,
    },
    /// Mark a work as favorite.
    Pin(WorkTarget),
    /// Remove a work from favorites.
    Unpin(WorkTarget),
    /// Short alias for `work create`.
    Add(CreateWorkArgs),
    /// Short alias for `work edit`.
    Edit(WorkTarget),
    /// Short alias for `work delete`.
    Rm(WorkTarget),
    /// Plugin/alias invocation: muxwf <plugin> <alias> [args...]
    #[command(external_subcommand)]
    Plugin(Vec<String>),
}

#[derive(Debug, Args)]
pub struct WorkTarget {
    pub name: String,
}

#[derive(Debug, Args)]
pub struct SaveArgs {
    /// Work name. Defaults to the work mapped to the current tmux session.
    pub name: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct ListArgs {
    /// Print only work names, one per line.
    #[arg(long, conflicts_with = "json")]
    pub names_only: bool,

    /// Print all works as JSON.
    #[arg(long)]
    pub json: bool,

    /// Only include works with this tag. Can be passed multiple times.
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Only include works in this group.
    #[arg(long)]
    pub group: Option<String>,

    /// Only include favorite works.
    #[arg(long)]
    pub favorite: bool,

    /// Only include works with last_opened_at, sorted newest first.
    #[arg(long)]
    pub recent: bool,

    /// Only include works with active tmux sessions.
    #[arg(long)]
    pub live: bool,
}

#[derive(Debug, Subcommand)]
pub enum WorkCommands {
    /// Create a work YAML file.
    Create(CreateWorkArgs),
    /// Open a work YAML file in $EDITOR.
    Edit(WorkTarget),
    /// Update common work fields.
    Update(UpdateWorkArgs),
    /// Delete a work YAML file and its matching snapshot, if present.
    Delete(WorkTarget),
    /// List works.
    List(ListArgs),
}

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommands {
    /// List workspace bundles.
    List,
    /// Open all works in a workspace bundle.
    Open(WorkTarget),
}

#[derive(Debug, Args)]
pub struct CreateWorkArgs {
    pub name: String,

    /// tmux session name. Defaults to the work name.
    #[arg(long)]
    pub session: Option<String>,

    /// Work root. Defaults to the current directory.
    #[arg(long)]
    pub root: Option<String>,

    /// Command run in restored panes unless a per-cwd rule is used.
    #[arg(long)]
    pub on_restore: Option<String>,

    /// Human-readable description.
    #[arg(long)]
    pub description: Option<String>,

    /// Group name.
    #[arg(long)]
    pub group: Option<String>,

    /// Tag. Can be passed multiple times.
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Create as a favorite work.
    #[arg(long)]
    pub favorite: bool,

    /// Open the created YAML file in $EDITOR.
    #[arg(long)]
    pub edit: bool,
}

#[derive(Debug, Args)]
pub struct UpdateWorkArgs {
    pub name: String,

    #[arg(long)]
    pub session: Option<String>,

    #[arg(long)]
    pub root: Option<String>,

    #[arg(long)]
    pub on_restore: Option<String>,

    #[arg(long)]
    pub description: Option<String>,

    #[arg(long)]
    pub group: Option<String>,

    #[arg(long)]
    pub clear_group: bool,

    #[arg(long = "tag")]
    pub tags: Vec<String>,

    #[arg(long)]
    pub clear_tags: bool,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Work name. Defaults to the current directory name.
    pub name: Option<String>,

    /// tmux session name. Defaults to the work name.
    #[arg(long)]
    pub session: Option<String>,

    /// Human-readable description.
    #[arg(long)]
    pub description: Option<String>,

    /// Group name.
    #[arg(long)]
    pub group: Option<String>,

    /// Tag. Can be passed multiple times.
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Create as a favorite work.
    #[arg(long)]
    pub favorite: bool,

    /// Command run in restored panes unless a per-cwd rule is used.
    #[arg(long)]
    pub on_restore: Option<String>,

    /// Open the created YAML file in $EDITOR.
    #[arg(long)]
    pub edit: bool,
}
