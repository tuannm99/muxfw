if vim.g.loaded_muxwf_nvim == 1 then
  return
end
vim.g.loaded_muxwf_nvim = 1

local M = {}

local function mw_bin()
  return vim.g.muxwf_bin or "mw"
end

local function notify(message, level)
  vim.notify(message, level or vim.log.levels.INFO, { title = "muxwf" })
end

local function trim(value)
  return vim.trim(value or "")
end

local function parse_lines(text)
  if not text or text == "" then
    return {}
  end
  return vim.split(text, "\n", { trimempty = true })
end

local function system(args)
  -- Prefer vim.system when available so stdout/stderr stay separate; fall back for older Neovim versions.
  local argv = vim.list_extend({ mw_bin() }, args)
  if vim.system then
    local result = vim.system(argv, { text = true }):wait()
    return result.code, trim(result.stdout), trim(result.stderr)
  end

  local output = vim.fn.systemlist(argv)
  local code = vim.v.shell_error
  return code, table.concat(output, "\n"), ""
end

local function run(args, opts)
  -- Centralize command execution so every action reports errors the same way inside Neovim.
  opts = opts or {}
  local code, stdout_text, stderr_text = system(args)
  if code ~= 0 then
    local error_message = stderr_text ~= "" and stderr_text or stdout_text
    notify(error_message ~= "" and error_message or ("command failed: " .. table.concat(args, " ")), vim.log.levels.ERROR)
    return nil, error_message
  end

  if opts.notify and stdout_text ~= "" then
    notify(stdout_text)
  end
  return stdout_text, nil
end

local function command_complete(args)
  -- Pull completion candidates directly from the CLI instead of keeping hard-coded lists in the plugin.
  local output = run(args)
  if not output then
    return {}
  end
  return parse_lines(output)
end

local function complete_work()
  return command_complete({ "list", "--names-only" })
end

local function complete_workspace()
  return command_complete({ "workspace", "list", "--names-only" })
end

local function complete_works(_, _, _)
  return complete_work()
end

local function complete_workspaces(_, _, _)
  return complete_workspace()
end

local function select_from_items(items, label, on_choice)
  -- When no args are passed, switch to an interactive picker flow via vim.ui.select.
  if #items == 0 then
    notify("no " .. label .. " found", vim.log.levels.WARN)
    return
  end

  vim.ui.select(items, { prompt = "muxwf " .. label .. ":" }, function(choice)
    if choice and choice ~= "" then
      on_choice(choice)
    end
  end)
end

local function parse_name_from_row(line)
  return line and vim.split(line, "\t", { plain = true })[1] or nil
end

local function set_scratch_options(bufnr, name)
  vim.bo[bufnr].buftype = "nofile"
  vim.bo[bufnr].bufhidden = "wipe"
  vim.bo[bufnr].swapfile = false
  vim.bo[bufnr].modifiable = true
  vim.bo[bufnr].filetype = "muxwf"
  pcall(vim.api.nvim_buf_set_name, bufnr, name)
end

local function render_list_buffer(name, title, lines, on_enter, on_refresh)
  -- This scratch buffer is a quick action view: Enter opens, r refreshes, q closes.
  vim.cmd("botright new")
  local bufnr = vim.api.nvim_get_current_buf()
  set_scratch_options(bufnr, name)
  vim.bo[bufnr].modifiable = true
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  vim.bo[bufnr].modifiable = false

  vim.keymap.set("n", "q", "<cmd>bd!<cr>", { buffer = bufnr, silent = true, desc = "close" })
  vim.keymap.set("n", "r", function()
    on_refresh(bufnr)
  end, { buffer = bufnr, silent = true, desc = "refresh" })
  vim.keymap.set("n", "<CR>", function()
    local line = vim.api.nvim_get_current_line()
    local item = parse_name_from_row(line)
    if item and item ~= "" then
      on_enter(item)
    end
  end, { buffer = bufnr, silent = true, desc = title })
end

local function open_work_from_choice(name, subcommand)
  run({ subcommand, name }, { notify = true })
end

local function choose_work(subcommand)
  select_from_items(complete_work(), "work", function(choice)
    open_work_from_choice(choice, subcommand)
  end)
end

local function choose_workspace()
  select_from_items(complete_workspace(), "workspace", function(choice)
    run({ "workspace", "open", choice }, { notify = true })
  end)
end

local function show_work_list()
  -- The work list keeps the raw CLI format so users see the same data as in the terminal.
  local function refresh(bufnr)
    local output = run({ "list" })
    if not output then
      return
    end
    vim.bo[bufnr].modifiable = true
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, parse_lines(output))
    vim.bo[bufnr].modifiable = false
  end

  local output = run({ "list" })
  if not output then
    return
  end

  render_list_buffer("muxwf-works", "open work", parse_lines(output), function(name)
    M.open(name)
  end, refresh)
end

local function show_workspace_list()
  -- The workspace list also reuses CLI output to avoid drift between the two interfaces.
  local function refresh(bufnr)
    local output = run({ "workspace", "list" })
    if not output then
      return
    end
    vim.bo[bufnr].modifiable = true
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, parse_lines(output))
    vim.bo[bufnr].modifiable = false
  end

  local output = run({ "workspace", "list" })
  if not output then
    return
  end

  render_list_buffer("muxwf-workspaces", "open workspace", parse_lines(output), function(name)
    M.workspace_open(name)
  end, refresh)
end

function M.open(name)
  if name and name ~= "" then
    open_work_from_choice(name, "open")
    return
  end
  choose_work("open")
end

function M.restore(name)
  if name and name ~= "" then
    open_work_from_choice(name, "restore")
    return
  end
  choose_work("restore")
end

function M.save(name)
  local args = { "save" }
  if name and name ~= "" then
    table.insert(args, name)
  end
  run(args, { notify = true })
end

function M.jump()
  run({ "jump" }, { notify = true })
end

function M.current()
  run({ "current" }, { notify = true })
end

function M.doctor()
  run({ "doctor" }, { notify = true })
end

function M.work_list()
  show_work_list()
end

function M.workspace_open(name)
  if name and name ~= "" then
    run({ "workspace", "open", name }, { notify = true })
    return
  end
  choose_workspace()
end

function M.workspace_list()
  show_workspace_list()
end

vim.api.nvim_create_user_command("MwOpen", function(opts)
  M.open(opts.args ~= "" and opts.args or nil)
end, { nargs = "?", complete = complete_works })

vim.api.nvim_create_user_command("MwRestore", function(opts)
  M.restore(opts.args ~= "" and opts.args or nil)
end, { nargs = "?", complete = complete_works })

vim.api.nvim_create_user_command("MwSave", function(opts)
  M.save(opts.args ~= "" and opts.args or nil)
end, { nargs = "?", complete = complete_works })

vim.api.nvim_create_user_command("MwJump", function()
  M.jump()
end, { nargs = 0 })

vim.api.nvim_create_user_command("MwCurrent", function()
  M.current()
end, { nargs = 0 })

vim.api.nvim_create_user_command("MwDoctor", function()
  M.doctor()
end, { nargs = 0 })

vim.api.nvim_create_user_command("MwWorkList", function()
  M.work_list()
end, { nargs = 0 })

vim.api.nvim_create_user_command("MwWorkspaceOpen", function(opts)
  M.workspace_open(opts.args ~= "" and opts.args or nil)
end, { nargs = "?", complete = complete_workspaces })

vim.api.nvim_create_user_command("MwWorkspaceList", function()
  M.workspace_list()
end, { nargs = 0 })

if vim.g.muxwf_default_mappings ~= 0 then
  vim.keymap.set("n", "<leader>mo", M.open, { silent = true, desc = "mw open work" })
  vim.keymap.set("n", "<leader>mj", M.jump, { silent = true, desc = "mw jump" })
  vim.keymap.set("n", "<leader>mw", M.workspace_open, { silent = true, desc = "mw open workspace" })
  vim.keymap.set("n", "<leader>ml", M.workspace_list, { silent = true, desc = "mw list workspaces" })
  vim.keymap.set("n", "<leader>mm", M.work_list, { silent = true, desc = "mw list works" })
end

_G.muxwf_nvim = M
