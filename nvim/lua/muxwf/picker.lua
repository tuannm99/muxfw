local backend = require("muxwf.backend")
local util = require("muxwf.util")

local M = {}

local function item_name(item)
  if type(item) == "table" and type(item.name) == "string" then
    return item.name
  end
  return type(item) == "table" and item.name or item
end

local function item_display(item)
  return type(item) == "table" and (item.display or item.name) or item
end

local function item_preview(item)
  if type(item) == "table" then
    if type(item.preview) == "function" then
      return item.preview(item)
    end
    if type(item.preview) == "table" then
      return item.preview
    end
  end
  return { item_name(item) }
end

local function default_actions(label, on_choice)
  return {
    {
      key = "<CR>",
      label = "keep nvim -> go work",
      handler = function(item)
        on_choice(item, { close_editor = false })
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
  local current_name = backend.current_work_name()
  local current_session = backend.current_tmux_session_name()
  local live_sessions = {}
  for _, session_name in ipairs(backend.tmux_list_sessions()) do
    live_sessions[session_name] = true
  end

  if not decoded then
    decoded = backend.work_list_json() or {}
  end

  local items = {}
  local known_sessions = {}
  for _, item in ipairs(decoded) do
    if item.kind == "live_session" then
      local session = item.session or item.name
      if session and session ~= "" then
        known_sessions[session] = true
        if session ~= current_session then
          items[#items + 1] = {
            kind = "live_session",
            tracked = false,
            name = item.name or session,
            session = session,
            display = string.format("%s  [live session]  untracked", session),
            preview = function()
              local lines = backend.tmux_session_preview(session)
              vim.list_extend(lines, {
                "",
                "actions:",
                "  <CR>   keep nvim -> go session",
                "  <C-x>  close nvim -> go session",
                "  <C-a>  track session -> create work + snapshot",
              })
              return lines
            end,
          }
        end
      end
    else
      local work = item
      if not (current_name and work.name == current_name) then
        known_sessions[work.session] = true
        items[#items + 1] = {
          kind = "work",
          name = work.name,
          session = work.session,
          display = string.format(
            "%s  [%s]  %s%s%s",
            work.name,
            work.session,
            work.favorite and "favorite " or "",
            live_sessions[work.session] and "live " or "",
            (work.group and work.group ~= "") and ("group:" .. work.group) or ""
          ),
          preview = function()
            local lines = {
              "config:",
              "  name: " .. work.name,
              "  session: " .. work.session,
              "  root: " .. work.root,
              "  status: " .. ((work.status and work.status ~= "") and work.status or "-"),
              "  group: " .. ((work.group and work.group ~= "") and work.group or "-"),
              "  favorite: " .. ((work.favorite and "yes") or "no"),
              "  live: " .. ((live_sessions[work.session] and "yes") or "no"),
              "  jump_rank: " .. tostring(work.jump_rank or "-"),
              "  tags: " .. ((work.tags and #work.tags > 0) and table.concat(work.tags, ", ") or "-"),
              "  description: " .. ((work.description and work.description ~= "") and work.description or "-"),
              "  last_opened_at: " .. ((work.last_opened_at and work.last_opened_at ~= "") and work.last_opened_at or "-"),
            }
            if live_sessions[work.session] then
              vim.list_extend(lines, { "" })
              vim.list_extend(lines, backend.tmux_session_preview(work.session))
            end
            vim.list_extend(lines, {
              "",
              "actions:",
              "  <CR>   keep nvim -> go work",
              "  <C-x>  close nvim -> go work",
            })
            return lines
          end,
        }
      end
    end
  end

  for _, session_name in ipairs(backend.tmux_list_sessions()) do
    local session = session_name
    if not known_sessions[session_name] and session_name ~= current_session then
      items[#items + 1] = {
        kind = "live_session",
        name = session,
        session = session,
        display = string.format("%s  [live session]", session),
        preview = function()
          local lines = backend.tmux_session_preview(session)
          vim.list_extend(lines, {
            "",
            "actions:",
            "  <CR>   keep nvim -> go session",
            "  <C-x>  close nvim -> go session",
          })
          return lines
        end,
      }
    end
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
    if not choice then
      return
    end
    if #actions == 1 then
      actions[1].handler(choice)
      return
    end

    vim.ui.select(actions, {
      prompt = "muxwf " .. label .. " action:",
      format_item = function(action)
        return action.label
      end,
    }, function(action)
      if action and action.handler then
        action.handler(choice)
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
      action.handler(selection.value)
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
