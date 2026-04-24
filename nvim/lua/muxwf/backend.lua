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

return M
