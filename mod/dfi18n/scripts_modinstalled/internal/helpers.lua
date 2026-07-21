--@module = true

local scriptmanager = require("script-manager")
local utils = require('utils')

-- the MOD ID
MOD_ID = "dfi18n"

-- MOD data directory
DATA_DIR = string.format("%s-data/", MOD_ID)

-- the MOD source path
MOD_SOURCE_PATH = scriptmanager.getModSourcePath(MOD_ID)
MOD_INFO = scriptmanager.get_mod_info_metadata(MOD_SOURCE_PATH, {'ID', 'NUMERIC_VERSION', 'DISPLAYED_VERSION'})

-- evaluate an expression in a safe environment
local eval_env = utils.df_shortcut_env()
function eval(s)
  local f, err = load('return ' .. s, 'expression', 't', eval_env)
  if err then
    qerror(err)
  end
  return f()
end

-- get all MOD data directory paths from different MODs
function data_paths()
  local paths = {}
  local seen = {}

  local function append_data_path(path)
    if not path then
      return
    end
    path = path:gsub('\\', '/')
    if seen[path] then
      return
    end
    local data_config_file = path .. '/' .. MOD_ID .. '.txt'
    if dfhack.filesystem.isfile(data_config_file) then
      seen[path] = true
      table.insert(paths, path)
    end
  end

  -- Load subscribed companion data MODs first.
  for _, v in ipairs(scriptmanager.get_mod_paths(MOD_ID .. '-data')) do
    append_data_path(v.path)
  end

  -- Load project-owned overrides last so they can extend or override workshop
  -- dictionaries without modifying the subscribed data MOD.
  for _, v in ipairs(scriptmanager.get_mod_paths(MOD_ID)) do
    append_data_path(v.path .. '/local-data')
  end
  if MOD_SOURCE_PATH then
    append_data_path(MOD_SOURCE_PATH .. '/local-data')
  end

  return paths
end

-- parse a MOD data file into a table of key-value pairs
function parse_data_file(data_path)
  local data_file = data_path .. '/dfi18n.txt'
  local data = {}
  local ok, lines = pcall(io.lines, data_file)
  if ok then
    for line in lines do
      local _, _, key, value = line:find('^%[([^:]+):(.*)%]$')
      if key and value then
        table.insert(data, {
          key = key,
          value = value,
          dir = data_path
        })
      end
    end
  end
  return data
end
