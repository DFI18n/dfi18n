--@module = true

local helpers = reqscript('internal/helpers')

-- collect DF version
OS = dfhack.getOSType()
DF_PLATFORM = dfhack.getDFVersion():split(' ', true)[3]:lower()
DF_VERSION = dfhack.getCompiledDFVersion()

-- get a native library function
local lib_file = (OS == "windows") and "dfi18n.dll" or "libdfi18n.so"
local lib_path = helpers.MOD_SOURCE_PATH .. "libs/" .. lib_file
function lib_function(symbol)
  return package.loadlib(lib_path, symbol)
end

-- load native library functions
setup = lib_function("setup")
init = lib_function("init")
enable = lib_function("enable")
disable = lib_function("disable")
toggle = lib_function("toggle")
set_view_screen = lib_function("set_view_screen")
add_memory_region = lib_function("add_memory_region")
add_memory_search = lib_function("add_memory_search")
add_font = lib_function("add_font")
load_title_logo = lib_function("load_title_logo")
load_simple_dict = lib_function("load_simple_dict")
load_translation_rulesets = lib_function("load_translation_rulesets")
set_lang_tag = lib_function("set_lang_tag")
reset = lib_function("reset")
sync_translate = lib_function("sync_translate")
async_translate = lib_function("async_translate")
is_enabled = lib_function("is_enabled")
cloud_enable = lib_function("cloud_enable")
cloud_disable = lib_function("cloud_disable")
cloud_get_status = lib_function("cloud_get_status")
cloud_set_endpoint = lib_function("cloud_set_endpoint")
dfhack_addstr_flag = lib_function("dfhack_addstr_flag")

-- setup global variable or field offset
function setup_globals(stmt)
  local symbol = ("set_%s"):format(stmt:gsub('%.', '_'))
  local setter_function = lib_function(symbol)
  local field = helpers.eval(string.gsub("df.global." .. stmt, "%.([^.]*)$", ":_field('%1')"))
  local _, addr = df.sizeof(field)
  setter_function(addr)
end
