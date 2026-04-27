use crate::work::WorkStatus;
use crate::workspace::WorkspaceOpenPolicy;
use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;

// Top-level CLI entrypoint that only carries the root subcommand.
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

// Main command surface exposed by the binary.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Save the configured tmux session into ~/.muxwf/snapshots/<work>.json.
    Save(SaveArgs),
    /// Restore the configured tmux session from its snapshot.
    Restore(WorkTarget),
    /// Switch or attach to the session, restoring or creating it if needed.
    Open(OpenArgs),
    /// Kill the configured tmux session while keeping its snapshot.
    Close(WorkTarget),
    /// Print the work mapped to the current tmux session.
    Current,
    /// List works.
    List(ListArgs),
    /// List recently opened works.
    Recent,
    /// List stale works.
    Stale(StaleArgs),
    /// Print the saved snapshot JSON for a work.
    Show(WorkTarget),
    /// Validate the environment and config files.
    Doctor,
    /// Print the muxwf version.
    Version,
    /// Compatibility alias for `open` without an explicit work name.
    Jump(JumpArgs),
    /// Generate shell completion scripts.
    Completion(CompletionArgs),
    /// Generate work configs and snapshots from all running tmux sessions.
    Init(InitArgs),
    /// Manage works.
    Work {
        #[command(subcommand)]
        command: WorkCommands,
    },
    /// Manage workspace bundles.
    #[command(alias = "ws")]
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommands,
    },
    /// Mark a work as favorite.
    Pin(WorkTarget),
    /// Remove a work from favorites.
    Unpin(WorkTarget),
    /// Mark a work as archived.
    Archive(WorkTarget),
    /// Create a work, or use `add current` to add the current tmux session.
    Add(AddArgs),
    /// Short alias for `work edit`.
    Edit(WorkTarget),
    /// Short alias for `work delete`.
    Rm(WorkTarget),
    /// Plugin or alias invocation: muxwf <plugin> <alias> [args...]
    #[command(external_subcommand)]
    Plugin(Vec<String>),
}

// Shared args shape for commands that only need a work name.
#[derive(Debug, Args)]
pub struct WorkTarget {
    pub name: String,
}

#[derive(Debug, Args, Clone)]
pub struct OpenArgs {
    /// Work name. Omit to open the interactive ranked picker.
    pub name: Option<String>,
}

// Args for `save`, allowing the name to be omitted and inferred from the current session.
#[derive(Debug, Args)]
pub struct SaveArgs {
    /// Work name; defaults to the work mapped to the current tmux session.
    pub name: Option<String>,
}

// Args for generating completion scripts.
#[derive(Debug, Args)]
pub struct CompletionArgs {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    pub shell: Shell,

    /// Command name used inside the generated completion script.
    #[arg(long, default_value = "mw")]
    pub name: String,
}

#[derive(Debug, Args, Clone)]
pub struct JumpArgs {
    /// Print only work names in jump order.
    #[arg(long, conflicts_with = "json")]
    pub names_only: bool,

    /// Print ranked works as JSON instead of launching the selector.
    #[arg(long)]
    pub json: bool,
}

// Filters and output modes for `list`.
#[derive(Debug, Args, Clone)]
pub struct ListArgs {
    /// Print only work names, one per line.
    #[arg(long, conflicts_with = "json")]
    pub names_only: bool,

    /// Print all works as JSON.
    #[arg(long)]
    pub json: bool,

    /// Only include works with this tag; can be passed multiple times.
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Only include works in this group.
    #[arg(long)]
    pub group: Option<String>,

    /// Only include favorite works.
    #[arg(long)]
    pub favorite: bool,

    /// Only include works in this lifecycle status.
    #[arg(long, value_enum)]
    pub status: Option<WorkStatus>,

    /// Only include works with last_opened_at, sorted newest first.
    #[arg(long)]
    pub recent: bool,

    /// Only include works with active tmux sessions.
    #[arg(long)]
    pub live: bool,

    /// Only include works with no recent activity for at least this many days.
    #[arg(long)]
    pub stale_days: Option<i64>,
}

#[derive(Debug, Args, Clone)]
pub struct StaleArgs {
    /// Minimum inactivity age in days.
    #[arg(long, default_value_t = 30)]
    pub days: i64,

    /// Print only work names, one per line.
    #[arg(long, conflicts_with = "json")]
    pub names_only: bool,

    /// Print all works as JSON.
    #[arg(long)]
    pub json: bool,
}

// Output modes for `workspace list`.
#[derive(Debug, Args, Clone)]
pub struct WorkspaceListArgs {
    /// Print only workspace names, one per line.
    #[arg(long, conflicts_with = "json")]
    pub names_only: bool,

    /// Print all workspaces as JSON.
    #[arg(long)]
    pub json: bool,
}

// Subcommands dedicated to managing work YAML files.
#[derive(Debug, Subcommand)]
pub enum WorkCommands {
    /// Save the configured tmux session into ~/.muxwf/snapshots/<work>.json.
    Save(SaveArgs),
    /// Switch or attach to the session, restoring or creating it if needed.
    Open(OpenArgs),
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

// Subcommands for managing workspace bundles.
#[derive(Debug, Subcommand)]
pub enum WorkspaceCommands {
    /// List workspace bundles.
    List(WorkspaceListArgs),
    /// Open all works in a workspace bundle.
    Open(WorkTarget),
    /// Create a workspace bundle YAML file.
    Create(CreateWorkspaceArgs),
    /// Open a workspace bundle YAML file in $EDITOR.
    Edit(WorkTarget),
    /// Replace the full work list for a workspace bundle.
    Update(UpdateWorkspaceArgs),
    /// Append works to a workspace bundle.
    Add(WorkspaceMembersArgs),
    /// Remove works from a workspace bundle.
    Remove(WorkspaceMembersArgs),
    /// Delete a workspace bundle YAML file.
    Delete(WorkTarget),
}

// Args for `workspace create`.
#[derive(Debug, Args)]
pub struct CreateWorkspaceArgs {
    pub name: String,

    /// Work names included in the workspace; can be passed multiple times.
    #[arg(long = "work", required = true)]
    pub works: Vec<String>,

    /// Optional workspace profile label such as daily, release, or incident.
    #[arg(long)]
    pub profile: Option<String>,

    /// How workspace open should treat existing or missing tmux sessions.
    #[arg(long, value_enum, default_value_t = WorkspaceOpenPolicy::Smart)]
    pub policy: WorkspaceOpenPolicy,

    /// Open the created YAML file in $EDITOR.
    #[arg(long)]
    pub edit: bool,
}

// Args for `workspace update`.
#[derive(Debug, Args)]
pub struct UpdateWorkspaceArgs {
    pub name: String,

    /// Full replacement work list; can be passed multiple times.
    #[arg(long = "work", required = true)]
    pub works: Vec<String>,

    /// Set or replace the workspace profile label.
    #[arg(long)]
    pub profile: Option<String>,

    /// Clear the workspace profile label.
    #[arg(long)]
    pub clear_profile: bool,

    /// Set the workspace open policy.
    #[arg(long, value_enum)]
    pub policy: Option<WorkspaceOpenPolicy>,
}

// Shared args for `workspace add/remove`.
#[derive(Debug, Args)]
pub struct WorkspaceMembersArgs {
    pub name: String,

    /// Work names to add or remove; can be passed multiple times.
    #[arg(long = "work", required = true)]
    pub works: Vec<String>,
}

// Args for `work create`.
#[derive(Debug, Args)]
pub struct CreateWorkArgs {
    pub name: String,

    /// tmux session name; defaults to the work name.
    #[arg(long)]
    pub session: Option<String>,

    /// Work root; defaults to the current directory.
    #[arg(long)]
    pub root: Option<String>,

    /// Command run in restored panes unless a per-cwd rule is used.
    #[arg(long)]
    pub on_restore: Option<String>,

    /// Human-readable description.
    #[arg(long)]
    pub description: Option<String>,

    /// Lifecycle status.
    #[arg(long, value_enum, default_value_t = WorkStatus::Active)]
    pub status: WorkStatus,

    /// Group name.
    #[arg(long)]
    pub group: Option<String>,

    /// Tag; can be passed multiple times.
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Create as a favorite work.
    #[arg(long)]
    pub favorite: bool,

    /// Open the created YAML file in $EDITOR.
    #[arg(long)]
    pub edit: bool,
}

// Args for `add`, covering both normal add and `add current`.
#[derive(Debug, Args)]
pub struct AddArgs {
    /// Work name, or `current` to add the current tmux session.
    pub target: String,

    /// Work name override when using `add current`.
    #[arg(long)]
    pub name: Option<String>,

    /// tmux session name; defaults to the work name; not valid with `add current`.
    #[arg(long)]
    pub session: Option<String>,

    /// Work root; defaults to the current directory or the discovered session cwd.
    #[arg(long)]
    pub root: Option<String>,

    /// Command run in restored panes unless a per-cwd rule is used.
    #[arg(long)]
    pub on_restore: Option<String>,

    /// Human-readable description.
    #[arg(long)]
    pub description: Option<String>,

    /// Lifecycle status.
    #[arg(long, value_enum, default_value_t = WorkStatus::Active)]
    pub status: WorkStatus,

    /// Group name.
    #[arg(long)]
    pub group: Option<String>,

    /// Tag; can be passed multiple times.
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Create as a favorite work.
    #[arg(long)]
    pub favorite: bool,

    /// Open the created YAML file in $EDITOR.
    #[arg(long)]
    pub edit: bool,
}

// Args for `work update`, touching only common scalar fields.
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

    #[arg(long, value_enum)]
    pub status: Option<WorkStatus>,

    #[arg(long)]
    pub group: Option<String>,

    #[arg(long)]
    pub clear_group: bool,

    #[arg(long = "tag")]
    pub tags: Vec<String>,

    #[arg(long)]
    pub clear_tags: bool,
}

// Args for `init`.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Replace existing generated work configs and snapshots.
    #[arg(long)]
    pub overwrite: bool,
}
