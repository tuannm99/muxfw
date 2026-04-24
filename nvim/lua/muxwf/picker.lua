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

local function default_actions(label, on_choice)
  return {
    {
      key = "<CR>",
      label = "keep nvim -> go work",
      handler = function(name)
        on_choice(name, { close_editor = false })
      end,
    },
  }
end

local function picker_actions(label, on_choice, actions)
  if type(actions) == "table" and #actions > 0 then
    return actions
  end
  return default_actions(label, on_choice)
end

function M.work_items()
  local decoded = backend.work_list_json()
  if not decoded then
    return backend.complete_work()
  end

  local current_name = backend.current_work_name()
  local items = {}
  for _, item in ipairs(decoded) do
    if current_name and item.name == current_name then
      goto continue
    end
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
    ::continue::
  end
  return items
end

function M.jump_items()
  local decoded = backend.jump_list_json()
  if not decoded then
    return M.work_items()
  end

  local current_name = backend.current_work_name()
  local items = {}
  for _, item in ipairs(decoded) do
    if current_name and item.name == current_name then
      goto continue
    end
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
        "",
        "actions:",
        "  <CR>   keep nvim -> go work",
        "  <C-x>  close nvim -> go work",
      },
    }
    ::continue::
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

function M.select_from_items(items, label, on_choice, opts)
  if #items == 0 then
    util.notify("no " .. label .. " found", vim.log.levels.WARN)
    return
  end

  local actions = picker_actions(label, on_choice, opts and opts.actions)

  vim.ui.select(items, { prompt = "muxwf " .. label .. ":" }, function(choice)
    local name = choice and item_name(choice) or nil
    if not (name and name ~= "") then
      return
    end
    if #actions == 1 then
      actions[1].handler(name)
      return
    end

    vim.ui.select(actions, {
      prompt = "muxwf " .. label .. " action:",
      format_item = function(action)
        return action.label
      end,
    }, function(action)
      if action and action.handler then
        action.handler(name)
      end
    end)
  end)
end

function M.select_with_telescope(items, label, on_choice, opts)
  local ok_pickers, pickers = pcall(require, "telescope.pickers")
  local ok_finders, finders = pcall(require, "telescope.finders")
  local ok_config, config = pcall(require, "telescope.config")
  local ok_actions, actions = pcall(require, "telescope.actions")
  local ok_action_state, action_state = pcall(require, "telescope.actions.state")
  local ok_previewers, previewers = pcall(require, "telescope.previewers")
  if not (ok_pickers and ok_finders and ok_config and ok_actions and ok_action_state and ok_previewers) then
    return false
  end

  local action_items = picker_actions(label, on_choice, opts and opts.actions)

  local function run_action(prompt_bufnr, action)
    local selection = action_state.get_selected_entry()
    actions.close(prompt_bufnr)
    if selection and selection.value and action and action.handler then
      action.handler(item_name(selection.value))
    end
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
    attach_mappings = function(prompt_bufnr, map)
      local default_action = action_items[1]
      actions.select_default:replace(function()
        run_action(prompt_bufnr, default_action)
      end)
      for idx, action in ipairs(action_items) do
        if idx > 1 and action.key and action.key ~= "" then
          map("i", action.key, function()
            run_action(prompt_bufnr, action)
          end)
          map("n", action.key, function()
            run_action(prompt_bufnr, action)
          end)
        end
      end
      return true
    end,
  }):find()
  return true
end

function M.choose(items, label, on_choice, opts)
  if #items == 0 then
    util.notify("no " .. label .. " found", vim.log.levels.WARN)
    return
  end
  if M.select_with_telescope(items, label, on_choice, opts) then
    return
  end
  M.select_from_items(items, label, on_choice, opts)
end

return M
