local mod = reqscript('internal/mod')

local function dfi18n(args)
  if #args == 0 then
    args = {"usage"}
  end

  local action = args[1]
  if action == "enable" then
    mod.enable()
  elseif action == "disable" then
    mod.disable()
  elseif action == "toggle" then
    mod.toggle()
  elseif action == "reload" then
    mod.reload()
  elseif action == "autoreload" then
    local state = args[2]
    if state == "on" or state == "enable" then
      mod.enable_auto_reload()
    elseif state == "off" or state == "disable" then
      mod.disable_auto_reload()
    else
      print(("auto dictionary reload: %s"):format(
        mod.auto_reload_enabled_state() and "on" or "off"))
      print("Usage: dfi18n autoreload on|off")
    end
  elseif action == "change" then
    local lang_tag = args[2]
    if not lang_tag then
      print("Usage: dfi18n change <lang_tag>")
      return
    end
    mod.change_lang_tag(lang_tag)
  elseif action == "t" then
    -- hidden command for sync translate
    local original = args[2]
    local translated = mod.sync_translate(original)
    print(translated)
  elseif action == "at" then
    -- hidden command for async translate
    local original = args[2]
    local translated = mod.async_translate(original)
    print(translated)
  else
    print("Usage: dfi18n [enable|disable|toggle|reload|change]")
  end
end

dfi18n {...}
