local backend = require("muxwf.backend")
local util = require("muxwf.util")

local M = {}

local function item_name(item)
  return type(item) == "table" and item.name or item
end

local function item_display(item)
  return type(item) == "table" and (item.display or item.name) or item
end

local function item_preview(item)
  if type(item) == "table" and type(item.preview) == "table" then
    return item.preview
  end
  return { item_name(item) }
end

function M.work_items()
  local decoded = backend.work_list_json()
  if not decoded then
    return backend.complete_work()
  end

  local items = {}
  for _, item in ipairs(decoded) do
    items[#items + 1] = {
      name = item.name,
      display = string.format("%s  [%s]", item.name, item.session),
      preview = {
        "name: " .. item.name,
        "session: " .. item.session,
        "root: " .. item.root,
        "group: " .. ((item.group and item.group ~= "") and item.group or "-"),
        "favorite: " .. ((item.favorite and "yes") or "no"),
        "tags: " .. ((item.tags and #item.tags > 0) and table.concat(item.tags, ", ") or "-"),
        "description: " .. ((item.description and item.description ~= "") and item.description or "-"),
        "last_opened_at: " .. ((item.last_opened_at and item.last_opened_at ~= "") and item.last_opened_at or "-"),
      },
    }
  end
  return items
end

function M.jump_items()
  local decoded = backend.jump_list_json()
  if not decoded then
    return M.work_items()
  end

  local items = {}
  for _, item in ipairs(decoded) do
    items[#items + 1] = {
      name = item.name,
      display = string.format(
        "%s  [%s]  %s%s%s",
        item.name,
        item.session,
        item.favorite and "favorite " or "",
        item.live and "live " or "",
        (item.group and item.group ~= "") and ("group:" .. item.group) or ""
      ),
      preview = {
        "name: " .. item.name,
        "session: " .. item.session,
        "root: " .. item.root,
        "status: " .. ((item.status and item.status ~= "") and item.status or "-"),
        "group: " .. ((item.group and item.group ~= "") and item.group or "-"),
        "favorite: " .. ((item.favorite and "yes") or "no"),
        "live: " .. ((item.live and "yes") or "no"),
        "jump_rank: " .. tostring(item.jump_rank or "-"),
        "tags: " .. ((item.tags and #item.tags > 0) and table.concat(item.tags, ", ") or "-"),
        "description: " .. ((item.description and item.description ~= "") and item.description or "-"),
        "last_opened_at: " .. ((item.last_opened_at and item.last_opened_at ~= "") and item.last_opened_at or "-"),
      },
    }
  end
  return items
end

function M.workspace_items()
  local decoded = backend.workspace_list_json()
  if not decoded then
    return backend.complete_workspace()
  end

  local items = {}
  for _, item in ipairs(decoded) do
    items[#items + 1] = {
      name = item.name,
      display = string.format("%s  (%d works)", item.name, item.works and #item.works or 0),
      preview = vim.list_extend({ "name: " .. item.name, "works:" }, vim.tbl_map(function(work_name)
        return "  - " .. work_name
      end, item.works or {})),
    }
  end
  return items
end

function M.select_from_items(items, label, on_choice)
  if #items == 0 then
    util.notify("no " .. label .. " found", vim.log.levels.WARN)
    return
  end

  vim.ui.select(items, { prompt = "muxwf " .. label .. ":" }, function(choice)
    local name = choice and item_name(choice) or nil
    if name and name ~= "" then
      on_choice(name)
    end
  end)
end

function M.select_with_telescope(items, label, on_choice)
  local ok_pickers, pickers = pcall(require, "telescope.pickers")
  local ok_finders, finders = pcall(require, "telescope.finders")
  local ok_config, config = pcall(require, "telescope.config")
  local ok_actions, actions = pcall(require, "telescope.actions")
  local ok_state, action_state = pcall(require, "telescope.actions.state")
  local ok_previewers, previewers = pcall(require, "telescope.previewers")
  if not (ok_pickers and ok_finders and ok_config and ok_actions and ok_state and ok_previewers) then
    return false
  end

  pickers.new({}, {
    prompt_title = "muxwf " .. label,
    finder = finders.new_table({
      results = items,
      entry_maker = function(item)
        return {
          value = item,
          display = item_display(item),
          ordinal = item_display(item),
        }
      end,
    }),
    sorter = config.values.generic_sorter({}),
    previewer = previewers.new_buffer_previewer({
      define_preview = function(self, entry)
        vim.bo[self.state.bufnr].filetype = "yaml"
        vim.api.nvim_buf_set_lines(self.state.bufnr, 0, -1, false, item_preview(entry.value))
      end,
    }),
    attach_mappings = function(prompt_bufnr)
      actions.select_default:replace(function()
        local selection = action_state.get_selected_entry()
        actions.close(prompt_bufnr)
        if selection and selection.value then
          on_choice(item_name(selection.value))
        end
      end)
      return true
    end,
  }):find()
  return true
end

function M.choose(items, label, on_choice)
  if #items == 0 then
    util.notify("no " .. label .. " found", vim.log.levels.WARN)
    return
  end
  if M.select_with_telescope(items, label, on_choice) then
    return
  end
  M.select_from_items(items, label, on_choice)
end

return M
