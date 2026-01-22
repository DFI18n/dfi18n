// TODO: refactor DF types

use std::ffi;

use sdl2_sys as sdl;

use crate::df;

#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq)]
#[repr(C)]
#[derive(PartialOrd, Ord)]
pub struct Coordinate {
  pub column: i32,
  pub row: i32,
}

// g_src: addst_flag(..., sflag)
pub type SFlag = u32;
pub const SCREENTEXPOS_TOP_OF_TEXT_SFLAG: SFlag = 8;
pub const SCREENTEXPOS_BOTTOM_OF_TEXT_SFLAG: SFlag = 16;

impl Coordinate {
  pub fn offset(&self) -> usize {
    let dims = df::gps::get_dimensions();
    (self.column as usize * dims.height as usize + self.row as usize) * 8
  }
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct Dimensions {
  pub width: i32,
  pub height: i32,
}

#[repr(C)]
pub struct CursesColorF32 {
  pub r: f32,
  pub g: f32,
  pub b: f32,
}

#[repr(C)]
pub struct ColorInfo {
  pub screenf: i8,
  pub screenb: i8,
  pub screenbright: bool,
  pub use_old_16_colors: bool,
  pub screen_color_r: u8,
  pub screen_color_g: u8,
  pub screen_color_b: u8,
  pub screen_color_br: u8,
  pub screen_color_bg: u8,
  pub screen_color_bb: u8,
  pub ccolor: [CursesColorF32; 16],
  pub uccolor: [Color; 16],
  pub color: [Color; 18],
}

#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct Color {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct ColorPair {
  pub foreground: Color,
  pub background: Color, // TODO: consider change this to Option<Color>
}

impl ColorPair {
  // Creates a new ColorPair with the specified foreground color and default background color
  pub fn new_foreground(foreground: Color) -> Self {
    ColorPair {
      foreground,
      background: Color::default(),
    }
  }

  // Creates a ColorPair from old 16-color scheme
  pub fn from_old_16_colors(fg: i8, bg: i8, bright: bool) -> Self {
    let fg = fg + if bright { 8 } else { 0 };
    let fg = fg.clamp(0, 15) as usize;
    let bg = bg.clamp(0, 15) as usize;
    let ColorInfo { uccolor, .. } = df::gps::get_color_info();
    ColorPair {
      foreground: uccolor[fg],
      background: uccolor[bg],
    }
  }
}

impl From<&mut ColorInfo> for ColorPair {
  fn from(color_info: &mut ColorInfo) -> Self {
    let &mut ColorInfo {
      use_old_16_colors,
      screenf,
      screenb,
      screenbright,
      screen_color_r,
      screen_color_g,
      screen_color_b,
      screen_color_br,
      screen_color_bg,
      screen_color_bb,
      ..
    } = color_info;

    // From g_src/graphics.h: graphicst::addchar()
    if use_old_16_colors {
      ColorPair::from_old_16_colors(screenf, screenb, screenbright)
    } else {
      ColorPair {
        foreground: Color {
          r: screen_color_r,
          g: screen_color_g,
          b: screen_color_b,
        },
        background: Color {
          r: screen_color_br,
          g: screen_color_bg,
          b: screen_color_bb,
        },
      }
    }
  }
}

impl From<&DFHackPen> for ColorPair {
  fn from(pen: &DFHackPen) -> Self {
    let &DFHackPen {
      mut fg,
      mut bg,
      bold: bright,
      ..
    } = pen;
    if fg < 0 {
      fg = 0;
    }
    if bg < 0 {
      bg = 0;
    }

    let ColorInfo { uccolor, .. } = df::gps::get_color_info();
    let fg = (fg + if bright { 8 } else { 0 }) as usize;
    ColorPair {
      foreground: uccolor[fg],
      background: uccolor[bg as usize],
    }
  }
}

#[repr(C)]
pub struct RendererInfo {
  pub dispx: i32,
  pub dispy: i32,
  pub dimx: i32,
  pub dimy: i32,
  pub dispx_z: i32,
  pub dispy_z: i32,
  pub origin_x: i32,
  pub origin_y: i32,
  pub cur_w: i32,
  pub cur_h: i32,
}

impl RendererInfo {
  pub fn orig_size(&self) -> &Dimensions {
    unsafe { (&self.dispx as *const i32 as *const Dimensions).as_ref_unchecked() }
  }

  pub fn zoom_size(&self) -> &Dimensions {
    unsafe { (&self.dispx_z as *const i32 as *const Dimensions).as_ref_unchecked() }
  }

  pub fn canvas_size(&self) -> &Dimensions {
    unsafe { (&self.cur_w as *const i32 as *const Dimensions).as_ref_unchecked() }
  }

  pub fn origin_offset(&self) -> &Coordinate {
    unsafe { (&self.origin_x as *const i32 as *const Coordinate).as_ref_unchecked() }
  }
}

#[repr(C)]
pub struct ScreenInfo {
  pub screen: usize,
  pub screentexpos: usize,
  pub screentexpos_lower: usize,
  pub screentexpos_anchored: usize,
  pub screentexpos_anchored_x: usize,
  pub screentexpos_anchored_y: usize,
  pub screentexpos_flag: usize,
  pub screen_top: usize,
  pub screentexpos_top: usize,
  pub screentexpos_top_lower: usize,
  pub screentexpos_top_anchored: usize,
  pub screentexpos_top_anchored_x: usize,
  pub screentexpos_top_anchored_y: usize,
  pub screentexpos_top_flag: usize,
}

impl ScreenInfo {
  pub fn screen(&self) -> &mut Screen {
    // log::warn!("screen@{:p} = {:#x}", &self.screen, self.screen);
    unsafe { (&self.screen as *const usize as *mut Screen).as_mut_unchecked() }
  }

  pub fn screen_top(&self) -> &mut Screen {
    // log::warn!("screen_top@{:p} = {:#x}", &self.screen_top, self.screen_top);
    unsafe { (&self.screen_top as *const usize as *mut Screen).as_mut_unchecked() }
  }
}

#[repr(C)]
pub struct Screen {
  pub screen: usize,
  pub texpos: usize,
  pub texpos_lower: usize,
  pub texpos_anchored: usize,
  pub texpos_anchored_x: usize,
  pub texpos_anchored_y: usize,
  pub texpos_flag: usize,
}

impl Screen {
  pub fn cell(&self, coord: &Coordinate) -> &mut [u8; 8] {
    unsafe { (self.screen as *mut u8).add(coord.offset()).cast::<[u8; 8]>().as_mut_unchecked() }
  }

  pub fn get_tex(&self, coord: &Coordinate) -> ffi::c_long {
    unsafe { (self.texpos as *const ffi::c_long).add(coord.offset() / 8).read() }
  }
}

#[repr(C)]
pub struct SDLInfo {
  pub window: *mut sdl::SDL_Window,
  pub renderer: *mut sdl::SDL_Renderer,
}

impl SDLInfo {
  pub fn window(&self) -> sdl::Window<'static> {
    sdl::Window::from_raw(self.window)
  }

  pub fn renderer(&self) -> sdl::Renderer<'static> {
    sdl::Renderer::from_raw(self.renderer)
  }
}

#[derive(Debug)]
#[repr(C)]
pub struct DFHackPen {
  pub ch: u8,
  pub fg: i8,
  pub bg: i8,
  pub bold: bool,
}

impl DFHackPen {
  pub fn from_ptr(ptr: *const ffi::c_void) -> &'static Self {
    unsafe { (ptr as *const DFHackPen).as_ref_unchecked() }
  }
}
