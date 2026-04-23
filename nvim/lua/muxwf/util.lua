local M = {}

function M.notify(message, level)
  vim.notify(message, level or vim.log.levels.INFO, { title = "muxwf" })
end

function M.trim(value)
  return vim.trim(value or "")
end

function M.parse_lines(text)
  if not text or text == "" then
    return {}
  end
  return vim.split(text, "\n", { trimempty = true })
end

function M.decode_json_output(output)
  local ok, decoded = pcall(vim.json.decode, output)
  if not ok or type(decoded) ~= "table" then
    return nil
  end
  return decoded
end

function M.shell_escape(value)
  return vim.fn.shellescape(value)
end

return M
