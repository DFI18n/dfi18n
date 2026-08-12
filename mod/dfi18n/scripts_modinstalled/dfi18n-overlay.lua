--@module = true

local overlay = require('plugins.overlay')
local widgets = require('gui.widgets')
local mod = reqscript('internal/mod')

CloudToggleOverlay = defclass(CloudToggleOverlay, overlay.OverlayWidget)
CloudToggleOverlay.ATTRS{
  desc='Show DFI18N status and toggle cloud translation on the title screen.',
  default_pos={x=-2, y=-2},
  version=1,
  default_enabled=true,
  viewscreens='title/Default',
  frame={w=28, h=2},
  autoarrange_subviews=1,
}

function CloudToggleOverlay:init()
  local enabled = mod.get_cloud_status()
  self:addviews{
    widgets.Label{
      text='DFI18N v2 0.23',
      text_pen=COLOR_LIGHTCYAN,
    },
    widgets.ToggleHotkeyLabel{
      view_id='cloud',
      label='Cloud translation',
      initial_option=enabled,
      options={
        {label='ON', value=true, pen=COLOR_LIGHTGREEN},
        {label='OFF', value=false, pen=COLOR_GREY},
      },
      on_change=function(value)
        local active = mod.set_cloud_enabled(value)
        self.subviews.cloud:setOption(active)
      end,
    },
  }
end

function CloudToggleOverlay:render(dc)
  local enabled = mod.get_cloud_status()
  self.subviews.cloud:setOption(enabled)
  CloudToggleOverlay.super.render(self, dc)
end

OVERLAY_WIDGETS = {
  cloud_toggle=CloudToggleOverlay,
}
