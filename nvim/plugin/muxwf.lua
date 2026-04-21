if vim.g.loaded_muxwf_nvim == 1 then
  return
end
vim.g.loaded_muxwf_nvim = 1

local M = {}

local function mw_bin()
  return vim.g.muxwf_bin or "mw"
end

local function shell_join(parts)
  local escaped = {}
  for _, part in ipairs(parts) do
    escaped[#escaped + 1] = vim.fn.shellescape(part)
  end
  return table.concat(escaped, " ")
end

local function run(args)
  vim.cmd("silent !" .. shell_join(vim.list_extend({ mw_bin() }, args)))
  vim.cmd("redraw!")
end

local function prompt(label)
  local value = vim.fn.input(label)
  if value == "" then
    return nil
  end
  return value
end

function M.open(name)
  name = name or prompt("work: ")
  if not name then
    return
  end
  run({ "open", name })
end

function M.jump()
  run({ "jump" })
end

function M.workspace_open(name)
  name = name or prompt("workspace: ")
  if not name then
    return
  end
  run({ "ws", "open", name })
end

function M.workspace_list()
  local output = vim.fn.systemlist({ mw_bin(), "ws", "list" })
  vim.cmd("botright new")
  vim.bo.buftype = "nofile"
  vim.bo.bufhidden = "wipe"
  vim.bo.swapfile = false
  vim.api.nvim_buf_set_name(0, "muxwf-workspaces")
  vim.api.nvim_buf_set_lines(0, 0, -1, false, output)
end

vim.api.nvim_create_user_command("MwOpen", function(opts)
  M.open(opts.args ~= "" and opts.args or nil)
end, { nargs = "?" })

vim.api.nvim_create_user_command("MwJump", function()
  M.jump()
end, { nargs = 0 })

vim.api.nvim_create_user_command("MwWorkspaceOpen", function(opts)
  M.workspace_open(opts.args ~= "" and opts.args or nil)
end, { nargs = "?" })

vim.api.nvim_create_user_command("MwWorkspaceList", function()
  M.workspace_list()
end, { nargs = 0 })

if vim.g.muxwf_default_mappings ~= 0 then
  vim.keymap.set("n", "<leader>mo", M.open, { silent = true, desc = "mw open work" })
  vim.keymap.set("n", "<leader>mj", M.jump, { silent = true, desc = "mw jump" })
  vim.keymap.set("n", "<leader>mw", M.workspace_open, { silent = true, desc = "mw open workspace" })
  vim.keymap.set("n", "<leader>ml", M.workspace_list, { silent = true, desc = "mw list workspaces" })
end

_G.muxwf_nvim = M

