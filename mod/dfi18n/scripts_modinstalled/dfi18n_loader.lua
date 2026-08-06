--@module = true

local mod = reqscript('internal/mod')

local scriptmanager = require("script-manager")

-- setup the MOD
mod.p("is loading v" .. mod.DISPLAYED_VERSION .. "...")
mod.setup()

-- load translation data
mod.load()

-- setup hooks
mod.p("is initializing...")
mod.init()

-- enable hooks
mod.p("is enabling...")
mod.enable()

-- setup hooks on DFHack
mod.setup_hooks()

-- enable auto dictionary reload by default (the built-in realtime translator
-- writes to the dictionary while the game runs; this hot-reloads it)
mod.enable_auto_reload()

-- done
mod.p("has been enabled.")
