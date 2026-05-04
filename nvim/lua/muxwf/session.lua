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

local function default_track_name(session_name)
  return session_name
end

local function find_work(name)
  local works = backend.work_list_json()
  if not works then
    return nil
  end

  for _, item in ipairs(works) do
    if item.name == name then
      return item
    end
  end
  return nil
end

function M.parse_work_session(name)
  local work = find_work(name)
  return work and work.session or nil
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

function M.switch_tmux_session(session_name, opts)
  opts = opts or {}
  local close_editor = opts.close_editor == true
  local tmux_args

  if vim.env.TMUX and vim.env.TMUX ~= "" then
    tmux_args = { "switch-client", "-t", session_name }
  else
    tmux_args = { "attach-session", "-t", session_name }
  end

  local code, _, stderr_text = backend.tmux_system(tmux_args)
  if code ~= 0 then
    local error_message = stderr_text ~= "" and stderr_text or ("failed to switch to tmux session " .. session_name)
    util.notify(error_message, vim.log.levels.ERROR)
    return false, error_message
  end

  if close_editor then
    close_current_editor()
  end
  return true, nil
end

function M.switch_target(name, opts)
  if find_work(name) then
    return M.switch_work(name, opts)
  end

  for _, session_name in ipairs(backend.tmux_list_sessions()) do
    if session_name == name then
      return M.switch_tmux_session(session_name, opts)
    end
  end

  local error_message = "unknown work or tmux session: " .. name
  util.notify(error_message, vim.log.levels.ERROR)
  return false, error_message
end

function M.switch_item(item, opts)
  if type(item) ~= "table" then
    return M.switch_target(item, opts)
  end
  if item.kind == "live_session" then
    return M.switch_tmux_session(item.session or item.name, opts)
  end
  return M.switch_work(item.name, opts)
end

function M.track_tmux_session(item)
  local session_name = type(item) == "table" and (item.session or item.name) or item
  if not session_name or session_name == "" then
    util.notify("no tmux session selected to track", vim.log.levels.WARN)
    return
  end

  vim.ui.input({
    prompt = "muxwf work name: ",
    default = default_track_name(session_name),
  }, function(input)
    if input == nil then
      return
    end

    local work_name = vim.trim(input)
    local args = { "work", "import-session", session_name }
    if work_name ~= "" then
      vim.list_extend(args, { "--name", work_name })
    end
    backend.run(args, { notify = true })
  end)
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
