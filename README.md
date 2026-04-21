# mw

`mw` is the short command for `muxwf`, a small tmux workflow manager for personal daily use. Version 2 keeps the v1 save/restore/open flow and adds metadata, filtering, favorites, recent works, live-session discovery, and workspace bundles for larger project sets.

The design goal is deterministic behavior that is easy to debug. It is intentionally not a full `tmux-resurrect` replacement.

## Current Implementation Status

This checkout contains a production-usable v2 CLI with this Rust module layout:

- `src/main.rs` routes commands and handles top-level errors.
- `src/cli.rs` defines the clap command surface.
- `src/paths.rs` centralizes `~/.muxwf` paths and binary lookup.
- `src/work.rs` manages work YAML CRUD.
- `src/snapshot.rs` reads, writes, and validates snapshot JSON.
- `src/tmux.rs` wraps deterministic tmux commands.
- `src/restore.rs` recreates sessions, windows, panes, cwd, hooks, active window, and active pane.
- `src/workspace.rs` loads workspace bundles from `~/.muxwf/workspaces/`.
- `src/plugin.rs` resolves plugin aliases and runs wrapped commands.
- `src/rules.rs` loads restore hook rules from `config.yaml`.

Verified locally:

- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `cargo clippy -- -D warnings`
- `cargo build`
- CLI smoke tests for filtered `list`, `list --json`, `pin`, `unpin`, `workspace list`, `doctor`, and `list --live`

`open`, `restore`, `jump`, and `workspace open` were not smoke-tested end-to-end in this terminal because they intentionally attach or switch the active tmux client.

Test layout:

- Unit tests live next to the module logic under `src/`.
- Integration tests live under `tests/` and execute the compiled `muxwf` binary with isolated temporary `HOME` directories.

## What It Does

- Saves tmux session structure to `~/.muxwf/snapshots/<work>.json`.
- Restores sessions, windows, panes, layouts when tmux accepts them, active window, active pane, and pane cwd.
- Runs optional restore hooks from the work config or first matching cwd rule.
- Manages work configs under `~/.muxwf/works/<name>.yaml`.
- Tracks tags, group, favorite, description, created/updated timestamps, and last-opened time per work.
- Filters works by tag, group, favorite, recent, and active tmux session.
- Opens multiple works from workspace bundles.
- Runs simple plugin aliases from `~/.muxwf/plugins/<plugin>.yaml`.
- Prints list output that works well with `fzf`.

## What It Does Not Do

- It does not restore running processes.
- It does not restore scrollback.
- It does not restore shell history.
- It does not restore editor or TUI state.
- It does not dynamically load plugins or run a TUI.

## Install

Ubuntu one-command install:

```bash
sh -c "$(curl -fsSL https://raw.githubusercontent.com/tuannm99/muxfw/master/install.sh)"
```

The installer:

- installs Ubuntu packages: `ca-certificates`, `curl`, `git`, `build-essential`, `pkg-config`, `tmux`, and `fzf`
- installs Rust with `rustup` when `cargo` is missing
- clones or updates the repo at `~/.local/src/muxwf`
- installs `muxwf` into `~/.local/bin/muxwf` and symlinks `mw`
- installs shell completions for bash, zsh, and fish under your home directory
- installs a small Neovim plugin when `nvim` is available

If `~/.local/bin` is not in `PATH`, add this to your shell profile:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Installer options:

```bash
MUXWF_BRANCH=master sh -c "$(curl -fsSL https://raw.githubusercontent.com/tuannm99/muxfw/master/install.sh)"
MUXWF_INSTALL_DIR="$HOME/src/muxwf" sh -c "$(curl -fsSL https://raw.githubusercontent.com/tuannm99/muxfw/master/install.sh)"
```

Manual requirements:

- Rust stable
- tmux
- fzf, optional, only for `mw jump`

Install from this checkout:

```bash
cargo install --path . --locked --force
```

Validate the environment on first use:

```bash
mw doctor
```

All state lives under:

```text
~/.muxwf/
  works/
    <name>.yaml
  snapshots/
    <name>.json
  plugins/
    <plugin>.yaml
  workspaces/
    <workspace>.yaml
  config.yaml
```

## Commands

```bash
mw work create <name> [--root <path>] [--session <name>] [--group <group>] [--tag <tag>] [--favorite] [--description <text>]
mw work edit <name>
mw work update <name> [--root <path>] [--session <name>] [--group <group>] [--clear-group] [--tag <tag>] [--clear-tags] [--on-restore <cmd>]
mw work delete <name>
mw work list [--names-only] [--json] [--tag <tag>] [--group <group>] [--favorite] [--recent] [--live]
```

Short aliases:

```bash
mw add <name>
mw add current [--name <work>] [--group <group>] [--tag <tag>] [--favorite] [--description <text>]
mw edit <name>
mw rm <name>
mw list
```

Core workflow:

```bash
mw init                 # generate work configs/snapshots from all running tmux sessions
mw add current          # generate a work config/snapshot from the current tmux session
mw add <name>           # manually create a work from the current directory
mw save [work]          # capture configured/current tmux session
mw restore <work>       # restore from snapshot, then attach/switch
mw open <work>          # attach if session exists, else restore, else create
mw close <work>         # kill the tmux session, keeping the snapshot
mw current              # print the work for the current tmux session
mw pin <work>           # mark favorite
mw unpin <work>         # remove favorite
mw recent               # list works by last_opened_at descending
mw show <work>          # print snapshot JSON
mw doctor               # validate tmux, configs, snapshots, plugins
mw version              # print CLI version
mw jump                 # fzf select and open
mw completion zsh       # print a completion script for bash, zsh, fish, etc.
mw ws list
mw ws open <name>
```

`mw init` skips configs and snapshots that already exist. If a work config exists but its snapshot is missing, `mw init` keeps the config and writes the missing snapshot. Use `mw init --overwrite` when you intentionally want to regenerate both from the currently running tmux sessions.

## Shell Completion

The installer writes completions to:

```text
~/.local/share/bash-completion/completions/mw
~/.local/share/zsh/site-functions/_mw
~/.config/fish/completions/mw.fish
```

Manual generation:

```bash
mkdir -p ~/.local/share/zsh/site-functions
mw completion zsh > ~/.local/share/zsh/site-functions/_mw
```

For zsh, make sure `~/.local/share/zsh/site-functions` is in `fpath` before `compinit` runs.

Zsh completion dynamically loads work names from:

```bash
mw list --names-only
```

After reinstalling completion, reload zsh and clear stale completion cache if needed:

```bash
rm -f ~/.zcompdump ~/.zcompdump-*
exec zsh
```

## Work Config

Minimal `~/.muxwf/works/sample-app.yaml`:

```yaml
name: sample-app
session: sample-app
root: ~/dev/sample-app
on_restore: ""
tags: []
group: demo
favorite: false
description: Sample application workspace
created_at: "2026-04-19T08:00:00Z"
updated_at: "2026-04-19T08:00:00Z"
```

Optional windows are used when `mw open <work>` creates a brand-new session with no snapshot:

```yaml
name: api
session: api
root: ~/dev/api
windows:
  - name: main
    cwd: ~/dev/api
    panes: 2
  - name: logs
    cwd: ~/dev/api
on_restore: ""
tags:
  - backend
group: platform
favorite: true
description: API workspace
last_opened_at: "2026-04-19T09:00:00Z"
created_at: "2026-04-19T08:00:00Z"
updated_at: "2026-04-19T09:00:00Z"
```

If `on_restore` is non-empty, it runs in every restored pane after `cd <cwd>`. If `on_restore` is empty, `mw` checks restore rules.

Old v1 work YAML remains valid. Missing v2 fields default to empty tags, no group, not favorite, no description, no last-opened time, and current timestamps when the file is loaded and next written.

## Listing and Jump

List output is tab-separated and keeps the work name as the first field:

```bash
mw list --names-only
mw list --tag backend --group platform
mw list --favorite
mw list --recent
mw list --live
mw list --json
```

`mw jump` ranks favorites first, then recently opened works, then live tmux sessions, then the remaining works. It uses `fzf` when available and falls back to a numbered prompt when `fzf` is missing.

## Workspace Bundles

Workspace bundles live under `~/.muxwf/workspaces/<name>.yaml`:

```yaml
name: demo-suite
works:
  - sample-app
  - sample-api
```

`mw ws open demo-suite` prepares each listed work in order using the same open path, updates `last_opened_at`, and then attaches or switches to the first work's tmux session. `mw workspace ...` remains available as the long form.

## Neovim

The installer copies a native package to:

```text
~/.config/nvim/pack/muxwf/start/muxwf.nvim/plugin/muxwf.lua
```

Commands:

```vim
:MwOpen [work]
:MwJump
:MwWorkspaceOpen [workspace]
:MwWorkspaceList
```

Default normal-mode mappings:

```text
<leader>mo  prompt/open work
<leader>mj  run mw jump
<leader>mw  prompt/open workspace
<leader>ml  list workspaces in a scratch buffer
```

Disable default mappings before the plugin loads:

```lua
vim.g.muxwf_default_mappings = 0
```

Use a custom binary path:

```lua
vim.g.muxwf_bin = vim.fn.expand("~/.local/bin/mw")
```

## Restore Rules

`~/.muxwf/config.yaml`:

```yaml
rules:
  - cwd_regex: ".*/pythonproject$"
    on_restore: "source .venv/bin/activate"

  - cwd_regex: ".*/frontend$"
    on_restore: "pnpm install"
```

Rules are evaluated in order. First match wins.

## Plugin Aliases

`~/.muxwf/plugins/kubectl.yaml`:

```yaml
name: k
binary: kubectl
aliases:
  pods: "get pods -A"
  po: "get pods -A"
  logs: "logs -f {{arg1}}"
  describe: "describe {{args}}"
```

Usage:

```bash
mw k pods
mw k logs mypod
```

Python example:

```yaml
name: py
binary: bash
aliases:
  venv: "python -m venv .venv"
  act: "source .venv/bin/activate"
  test: "pytest -q"
```

Shell binaries (`bash`, `sh`, `zsh`, `fish`) run aliases with `-lc`. Other binaries receive argv directly after simple template expansion.

Supported placeholders:

- `{{arg1}}`, `{{arg2}}`, etc.
- `{{args}}` for all provided args
- Extra args are appended when the alias does not consume them

## FZF Usage

```bash
mw list --names-only | fzf | xargs mw open
```

Or use the built-in wrapper:

```bash
mw jump
```

If `fzf` is not installed, `mw jump` prints the ranked list and accepts either a number or a work name.

## Restore Behavior

`mw save [work]` captures:

- session name
- active window index
- window name
- layout, if tmux provides one
- active pane index
- pane count
- pane current working directory

`mw restore <work>`:

- refuses to overwrite an existing tmux session
- creates missing sessions and windows
- creates panes
- sends `cd -- '<cwd>'` to each pane
- sends `cd -- '<cwd>' && <hook>` when a hook applies
- restores active window and active pane
- updates `last_opened_at` after a successful restore

If a saved cwd no longer exists, `mw` falls back to the work root, then to `$HOME`, and prints a warning.

## Examples

Minimal work file:

```yaml
name: sample-app
session: sample-app
root: ~/dev/sample-app
on_restore: ""
tags:
  - demo
group: apps
favorite: false
description: Sample app workspace
```

## Limitations

- Existing sessions are never overwritten by `restore`; kill or rename the tmux session yourself first.
- Pane split orientation is recreated by pane count first, then tmux layout is applied if possible.
- Plugin templating is intentionally minimal.
- Work `update` only edits common scalar fields; use `mw work edit <name>` for windows or complex changes.
- Workspace bundles are edited as YAML files; there are no `workspace create/add/remove` commands yet.
