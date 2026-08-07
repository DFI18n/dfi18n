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
  elseif action == "cloud" then
    local cloud_action = args[2]
    if cloud_action == "enable" then
      mod.set_cloud_enabled(true)
    elseif cloud_action == "disable" then
      mod.set_cloud_enabled(false)
    elseif cloud_action == "set" then
      local host = args[3]
      if not host then
        print("Usage: dfi18n cloud set <host-or-endpoint>")
        return
      end
      mod.set_cloud_endpoint(host)
    else
      print("Usage: dfi18n cloud [enable|disable|set <host-or-endpoint>]")
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
    print("Usage: dfi18n [enable|disable|toggle|reload|cloud|change]")
  end
end

dfi18n {...}
