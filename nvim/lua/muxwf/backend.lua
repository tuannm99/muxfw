local util = require("muxwf.util")

local M = {}

function M.mw_bin()
  return vim.g.muxwf_bin or "mw"
end

local function system_command(argv)
  if vim.system then
    local result = vim.system(argv, { text = true }):wait()
    return result.code, util.trim(result.stdout), util.trim(result.stderr)
  end

  local output = vim.fn.systemlist(argv)
  local code = vim.v.shell_error
  return code, table.concat(output, "\n"), ""
end

function M.system(args)
  return system_command(vim.list_extend({ M.mw_bin() }, args))
end

function M.tmux_system(args)
  return system_command(vim.list_extend({ "tmux" }, args))
end

function M.run(args, opts)
  opts = opts or {}
  local code, stdout_text, stderr_text = M.system(args)
  if code ~= 0 then
    local error_message = stderr_text ~= "" and stderr_text or stdout_text
    util.notify(
      error_message ~= "" and error_message or ("command failed: " .. table.concat(args, " ")),
      vim.log.levels.ERROR
    )
    return nil, error_message
  end

  if opts.notify and stdout_text ~= "" then
    util.notify(stdout_text)
  end
  return stdout_text, nil
end

function M.tmux_stdout(args)
  local code, stdout_text, stderr_text = M.tmux_system(args)
  if code ~= 0 then
    return nil, stderr_text ~= "" and stderr_text or stdout_text
  end
  return stdout_text, nil
end

function M.command_complete(args)
  local output = M.run(args)
  if not output then
    return {}
  end
  return util.parse_lines(output)
end

function M.complete_work()
  return M.command_complete({ "list", "--names-only" })
end

function M.complete_jump_target()
  local names = M.complete_work()
  local seen = {}
  for _, name in ipairs(names) do
    seen[name] = true
  end

  for _, session in ipairs(M.tmux_list_sessions()) do
    if not seen[session] then
      names[#names + 1] = session
      seen[session] = true
    end
  end
  return names
end

function M.complete_workspace()
  return M.command_complete({ "workspace", "list", "--names-only" })
end

function M.work_list_json()
  local output = M.run({ "list", "--json" })
  return output and util.decode_json_output(output) or nil
end

function M.jump_list_json()
  local output = M.run({ "jump", "--json" })
  return output and util.decode_json_output(output) or nil
end

function M.workspace_list_json()
  local output = M.run({ "workspace", "list", "--json" })
  return output and util.decode_json_output(output) or nil
end

function M.current_work_name()
  local code, stdout_text = M.system({ "current" })
  if code ~= 0 or not stdout_text or stdout_text == "" then
    return nil
  end
  local first_line = util.parse_lines(stdout_text)[1]
  if not first_line or first_line == "" then
    return nil
  end
  local cols = vim.split(first_line, "\t", { plain = true })
  return cols[1] ~= "" and cols[1] or nil
end

function M.current_tmux_session_name()
  if not vim.env.TMUX or vim.env.TMUX == "" then
    return nil
  end

  local output = M.tmux_stdout({ "display-message", "-p", "#{session_name}" })
  if not output or output == "" then
    return nil
  end
  local first_line = util.parse_lines(output)[1]
  return first_line ~= "" and first_line or nil
end

function M.tmux_list_sessions()
  local code, stdout_text, stderr_text = M.tmux_system({ "list-sessions", "-F", "#{session_name}" })
  if code ~= 0 then
    local error_message = stderr_text ~= "" and stderr_text or stdout_text
    if error_message:find("no server running", 1, true) then
      return {}
    end
    util.notify(error_message ~= "" and error_message or "failed to list tmux sessions", vim.log.levels.WARN)
    return {}
  end
  return util.parse_lines(stdout_text)
end

function M.tmux_session_preview(session)
  local output, error_message = M.tmux_stdout({
    "list-panes",
    "-t",
    session,
    "-F",
    "#{window_index}\t#{window_name}\t#{window_active}\t#{pane_index}\t#{pane_active}\t#{pane_current_command}\t#{pane_current_path}",
  })
  if not output then
    return {
      "tmux session:",
      "  failed to load: " .. (error_message or session),
    }
  end

  local lines = {
    "tmux session:",
    "  name: " .. session,
  }
  local current_window = nil
  for _, line in ipairs(util.parse_lines(output)) do
    local cols = vim.split(line, "\t", { plain = true })
    local window_index = cols[1] or "?"
    local window_name = cols[2] or ""
    local window_active = cols[3] == "1"
    local pane_index = cols[4] or "?"
    local pane_active = cols[5] == "1"
    local pane_cmd = (cols[6] and cols[6] ~= "") and cols[6] or "?"
    local pane_cwd = (cols[7] and cols[7] ~= "") and cols[7] or "?"
    local window_key = window_index .. "\t" .. window_name

    if current_window ~= window_key then
      current_window = window_key
      lines[#lines + 1] =
        string.format("  window %s: %s%s", window_index, window_name, window_active and "  [active]" or "")
    end

    lines[#lines + 1] = string.format(
      "    pane %s%s  cmd=%s  cwd=%s",
      pane_index,
      pane_active and " [active]" or "",
      pane_cmd,
      pane_cwd
    )
  end

  if #lines == 2 then
    lines[#lines + 1] = "  empty"
  end
  return lines
end

function M.workspace_first_work(name)
  local decoded = M.workspace_list_json()
  if not decoded then
    return nil
  end

  for _, item in ipairs(decoded) do
    if item.name == name then
      return item.works and item.works[1] or nil
    end
  end
  return nil
end

return M
