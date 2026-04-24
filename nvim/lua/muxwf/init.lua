local backend = require("muxwf.backend")
local picker = require("muxwf.picker")
local session = require("muxwf.session")
local util = require("muxwf.util")

local M = {}

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
  backend.run({ subcommand, name }, { notify = true })
end

local function choose_work(subcommand)
  picker.choose(picker.jump_items(), "work", function(choice)
    open_work_from_choice(choice, subcommand)
  end)
end

local function show_work_list()
  local function refresh(bufnr)
    local output = backend.run({ "list" })
    if not output then
      return
    end
    vim.bo[bufnr].modifiable = true
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, util.parse_lines(output))
    vim.bo[bufnr].modifiable = false
  end

  local output = backend.run({ "list" })
  if not output then
    return
  end

  render_list_buffer("muxwf-works", "open work", util.parse_lines(output), function(name)
    M.open(name)
  end, refresh)
end

local function show_workspace_list()
  local function refresh(bufnr)
    local output = backend.run({ "workspace", "list" })
    if not output then
      return
    end
    vim.bo[bufnr].modifiable = true
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, util.parse_lines(output))
    vim.bo[bufnr].modifiable = false
  end

  local output = backend.run({ "workspace", "list" })
  if not output then
    return
  end

  render_list_buffer("muxwf-workspaces", "open workspace", util.parse_lines(output), function(name)
    M.workspace_open(name)
  end, refresh)
end

function M.open(name)
  if name and name ~= "" then
    open_work_from_choice(name, "open")
    session.ensure_work_has_editor(name)
    return
  end

  picker.choose(picker.jump_items(), "work", function(choice)
    open_work_from_choice(choice, "open")
    session.ensure_work_has_editor(choice)
  end)
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
  backend.run(args, { notify = true })
end

function M.jump()
  M.open()
end

function M.current()
  backend.run({ "current" }, { notify = true })
end

function M.doctor()
  backend.run({ "doctor" }, { notify = true })
end

function M.work_list()
  show_work_list()
end

function M.list()
  M.work_list()
end

function M.workspace_open(name)
  if name and name ~= "" then
    backend.run({ "workspace", "open", name }, { notify = true })
    return
  end

  picker.choose(picker.workspace_items(), "workspace", function(choice)
    backend.run({ "workspace", "open", choice }, { notify = true })
  end)
end

function M.workspace_list()
  show_workspace_list()
end

local function complete_works(_, _, _)
  return backend.complete_work()
end

local function complete_workspaces(_, _, _)
  return backend.complete_workspace()
end

function M.setup()
  vim.api.nvim_create_user_command("MwOpen", function(opts)
    M.open(opts.args ~= "" and opts.args or nil)
  end, { nargs = "?", complete = complete_works })

  vim.api.nvim_create_user_command("MwSwitch", function(opts)
    M.open(opts.args ~= "" and opts.args or nil)
  end, { nargs = "?", complete = complete_works })

  vim.api.nvim_create_user_command("MwList", function()
    M.list()
  end, { nargs = 0 })

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
    vim.keymap.set("n", "<leader>mo", M.open, { silent = true, desc = "mw switch work" })
    vim.keymap.set("n", "<leader>ml", M.list, { silent = true, desc = "mw list works" })
    vim.keymap.set("n", "<leader>mw", M.workspace_open, { silent = true, desc = "mw open workspace" })
  end
end

return M
