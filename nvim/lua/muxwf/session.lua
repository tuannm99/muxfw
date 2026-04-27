local backend = require("muxwf.backend")
local util = require("muxwf.util")

local M = {}

local function current_editor_command()
  local prog = vim.v.progpath and vim.v.progpath ~= "" and vim.v.progpath or "nvim"
  return util.shell_escape(prog)
end

local function close_current_editor()
  vim.schedule(function()
    vim.cmd("qa")
  end)
end

function M.parse_work_session(name)
  local works = backend.work_list_json()
  if not works then
    return nil
  end

  for _, item in ipairs(works) do
    if item.name == name then
      return item.session
    end
  end
  return nil
end

function M.session_has_editor(session)
  local output, error_message = backend.tmux_stdout({
    "list-panes",
    "-a",
    "-F",
    "#{session_name}\t#{pane_current_command}",
  })
  if not output then
    util.notify(error_message or ("failed to inspect tmux session " .. session), vim.log.levels.WARN)
    return false
  end

  for _, line in ipairs(util.parse_lines(output)) do
    local cols = vim.split(line, "\t", { plain = true })
    if cols[1] == session and cols[2] and (cols[2] == "nvim" or cols[2] == "vim" or cols[2] == "vi") then
      return true
    end
  end
  return false
end

function M.session_active_pane_target(session)
  local output, error_message = backend.tmux_stdout({
    "list-panes",
    "-a",
    "-F",
    "#{session_name}\t#{window_active}\t#{pane_active}\t#{window_index}\t#{pane_index}",
  })
  if not output then
    util.notify(error_message or ("failed to inspect active pane for " .. session), vim.log.levels.WARN)
    return nil
  end

  for _, line in ipairs(util.parse_lines(output)) do
    local cols = vim.split(line, "\t", { plain = true })
    if cols[1] == session and cols[2] == "1" and cols[3] == "1" then
      return string.format("%s:%s.%s", session, cols[4], cols[5])
    end
  end
  return nil
end

function M.ensure_work_has_editor(name)
  if not vim.env.TMUX or vim.env.TMUX == "" then
    return
  end

  local session = M.parse_work_session(name)
  if not session or session == "" or M.session_has_editor(session) then
    return
  end

  local target = M.session_active_pane_target(session)
  if not target then
    util.notify("could not determine the active pane for " .. session, vim.log.levels.WARN)
    return
  end

  local code, _, stderr_text = backend.tmux_system({
    "send-keys",
    "-t",
    target,
    "-l",
    current_editor_command(),
  })
  if code ~= 0 then
    util.notify(stderr_text ~= "" and stderr_text or ("failed to start Neovim in " .. target), vim.log.levels.WARN)
    return
  end

  backend.tmux_system({ "send-keys", "-t", target, "Enter" })
end

function M.switch_work(name, opts)
  opts = opts or {}
  local close_editor = opts.close_editor == true

  local output, error_message = backend.run({ "work", "open", name }, { notify = true })
  if not output and error_message then
    return false, error_message
  end

  M.ensure_work_has_editor(name)
  if close_editor then
    close_current_editor()
  end
  return true, nil
end

function M.switch_workspace(name, opts)
  opts = opts or {}
  local close_editor = opts.close_editor == true

  local output, error_message = backend.run({ "workspace", "open", name }, { notify = true })
  if not output and error_message then
    return false, error_message
  end

  local first_work = backend.workspace_first_work(name)
  if first_work and first_work ~= "" then
    M.ensure_work_has_editor(first_work)
  end
  if close_editor then
    close_current_editor()
  end
  return true, nil
end

return M
