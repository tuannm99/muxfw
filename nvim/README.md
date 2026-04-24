# muxwf.nvim

This directory is the Neovim runtime for `muxwf`.

## Layout

- `plugin/muxwf.lua`
  Bootstrap only. Guards against double-loading, requires `muxwf`, runs `setup()`, and exposes `_G.muxwf_nvim`.

- `lua/muxwf/init.lua`
  Main orchestration layer. Registers commands, default mappings, scratch-buffer list views, and the primary `open`, `list`, and `save` UX actions.

- `lua/muxwf/backend.lua`
  Process boundary. Runs `mw` and `tmux`, handles completion queries, and loads JSON payloads from the CLI.

- `lua/muxwf/picker.lua`
  Picker/UI layer. Builds ranked work and workspace entries, Telescope previews, and fallback `vim.ui.select()` flows.

- `lua/muxwf/session.lua`
  Tmux session inspection. Decides whether a target work already has a running editor and starts Neovim in the active pane when needed.

- `lua/muxwf/util.lua`
  Small shared helpers such as notifications, string trimming, line splitting, JSON decode wrappers, and shell escaping.

## Design Notes

- `plugin/` stays thin.
- `lua/muxwf/init.lua` owns user-facing behavior.
- Lower layers do not register commands or keymaps.
- Telescope is optional; the picker layer falls back cleanly when it is unavailable.
