use std::sync::{Mutex, MutexGuard, OnceLock, RwLock};
use std::{ffi, ptr};

use anyhow::Result;

use macros::hook;

use lua53_sys as lua;
use sdl2_sys as sdl;

use crate::types::{ColorPair, DFHackPen};
use crate::{control, df, lang, logging, logo, markup, memory, screen, text, translation, translator, types};
use translation::{TranslationInput, TranslationRequest};

fn addst(gps_ptr: *const ffi::c_void, string_ptr: *const ffi::c_void, just: u8, space: i32) {
  

  let string = cp437_string::cxx_string_to_string(string_ptr);
  let request = TranslationRequest::new(TranslationInput::addst {
    content: string.clone(),
  });

  logging::log_text(&request, string_ptr);
  let text_block = text::TextBlock::get(&request);
  let columns = text_block.columns();
  let id = text_block.add_to_screen(screen::Layer::Lower, request.coordinate());

  if !control::is_enabled() {
    return call_addst(gps_ptr, string_ptr, just, space);
  }

  let coord = df::gps::get_coordinate().to_owned();
  let cpp_string = cpp::CppString::fill(b' ', columns);
  call_addst(gps_ptr, cpp_string.raw(), just, space);
  screen::mark_occupied(screen::Layer::Lower, coord, &text_block, Some(id));
}

fn addst_flag(gps_ptr: *const ffi::c_void, string_ptr: *const ffi::c_void, just: u8, space: i32, sflag: u32) {
  

  let string = cp437_string::cxx_string_to_string(string_ptr);

  let request = TranslationRequest::new(TranslationInput::addst_flag {
    content: string.clone(),
    flag: sflag,
  });

  logging::log_text(&request, string_ptr);
  let text_block = text::TextBlock::get(&request);
  let columns = text_block.columns();
  let id = text_block.add_to_screen(screen::Layer::Lower, request.coordinate());

  // do not render bottom half of the text when enabled
  if control::is_enabled() {
    const BOTTOM_OF_TEXT: u32 = 0b00010000;
    if sflag & BOTTOM_OF_TEXT != 0 {
      let coord = df::gps::get_coordinate().to_owned();
      let cpp_string = cpp::CppString::fill(b' ', columns);
      call_addst_flag(gps_ptr, cpp_string.raw(), just, space, sflag);
      screen::mark_bottom_occupied(screen::Layer::Lower, coord, columns as i32);
      return;
    }
  }

  if !control::is_enabled() {
    return call_addst_flag(gps_ptr, string_ptr, just, space, sflag);
  }

  let coord = df::gps::get_coordinate().to_owned();
  let cpp_string = cpp::CppString::fill(b' ', columns);
  call_addst_flag(gps_ptr, cpp_string.raw(), just, space, sflag);
  screen::mark_occupied(screen::Layer::Lower, coord, &text_block, Some(id));
}

fn addcoloredst(gps_ptr: *const ffi::c_void, string_ptr: *const ffi::c_void, color_string_ptr: *const ffi::c_void) {
  

  // zip color string and reconstruct mtb string for translation
  let mut prev_color = None;
  let string_bytes = unsafe { ffi::CStr::from_ptr(string_ptr as *const ffi::c_char) }.to_bytes();
  let color_bytes = unsafe { ffi::CStr::from_ptr(color_string_ptr as *const ffi::c_char) }.to_bytes();
  let mut mtb_string: Vec<u8> = Vec::new();
  string_bytes.iter().zip(color_bytes.iter()).for_each(|(s, c)| {
    // add change color markup if different
    let curr_color = (c & 7, (c & 56) >> 3, (c & 64) >> 6);
    if Some(curr_color) != prev_color {
      prev_color = Some(curr_color);
      mtb_string.extend_from_slice(format!("[C:{}:{}:{}]", curr_color.0, curr_color.1, curr_color.2).as_bytes());
    }

    // add character
    mtb_string.push(*s);
  });
  mtb_string.push(0); // null-terminate

  let string = cp437_string::c_string_to_string(mtb_string.as_ptr() as *const ffi::c_char);
  let request = TranslationRequest::new(TranslationInput::addcoloredst { markup: string.clone() });

  logging::log_text(&request, string_ptr);

  // always set width for the markup text box before rendering
  let mut markup = string.clone();
  if control::is_enabled() {
    if let Some(response) = translator::translate(&request) {
      markup = response.translated;
    }
  }
  let mut markup = markup::get(&markup);
  markup.set_width(string_bytes.len() as i32);

  let mut text_block = markup.text_block();
  // record the original markup so the block can be re-translated in place
  text_block.set_original(&string);
  let columns = text_block.columns();
  let id = text_block.add_to_screen(screen::Layer::Lower, request.coordinate());

  if !control::is_enabled() {
    return call_addcoloredst(gps_ptr, string_ptr, color_string_ptr);
  }

  let coord = df::gps::get_coordinate().to_owned();
  let cpp_string = cpp::CppString::fill(b' ', columns);
  call_addcoloredst(gps_ptr, cpp_string.raw(), color_string_ptr);
  screen::mark_occupied(screen::Layer::Lower, coord, &text_block, Some(id));
}

fn top_addst(gps_ptr: *const ffi::c_void, string_ptr: *const ffi::c_void, just: u8, space: i32) {
  if handle_help_mtb(string_ptr) {
    return call_top_addst(gps_ptr, string_ptr, just, space);
  }

  let string = cp437_string::cxx_string_to_string(string_ptr);
  let request = TranslationRequest::new(TranslationInput::top_addst {
    top_content: string.clone(),
  });

  logging::log_text(&request, string_ptr);
  let text_block = text::TextBlock::get(&request);
  let columns = text_block.columns();
  let id = text_block.add_to_screen(screen::Layer::Upper, request.coordinate());

  if !control::is_enabled() {
    return call_top_addst(gps_ptr, string_ptr, just, space);
  }

  let coord = df::gps::get_coordinate().to_owned();
  let cpp_string = cpp::CppString::fill(b' ', columns);
  call_top_addst(gps_ptr, cpp_string.raw(), just, space);
  screen::mark_occupied(screen::Layer::Upper, coord, &text_block, Some(id));
}

fn update_tile(renderer_ptr: *const ffi::c_void, x: i32, y: i32) {
  // render the MOD logo on the main menu screen
  if x == 0 && y == 0 {
    // display the title logo if available
    if let Some(display_title) = get_display_title_mut().as_ref() {
      let origin_offset = df::renderer::get_renderer_info().origin_offset();
      let canvas_size = df::renderer::get_renderer_info().canvas_size();
      let (w, h) = display_title.size();
      let rect = sdl::SDL_Rect {
        x: origin_offset.column + canvas_size.width / 2 - w / 2,
        y: if canvas_size.height >= 800 {
          origin_offset.row + (canvas_size.height - 800) / 3
        } else {
          origin_offset.row
        },
        w,
        h,
      };

      let sdl_renderer = df::renderer::get_sdl_info().renderer();
      sdl_renderer.copy(display_title, None, Some(&rect));
    }

    let texture_blits = df::gps::get_texture_blits();
    for i in 0..texture_blits.size() {
      // read the texture blit information
      let texture_blit = texture_blits.get(i);
      let &df::gps::TextureBlits { x, y, tex } = texture_blit;
      let tex_id = tex as usize;

      // render the MOD logo on the right-side of the Dwarf Fortress developer logo using the same height
      // note 7..9 are three different sizes of the DF developer logo
      if (7..=9).contains(&tex_id) {
        let (logo_w, logo_h) = df::gps::get_texture_size(tex_id);
        let zoom_size = df::renderer::get_renderer_info().zoom_size();
        let origin_offset = df::renderer::get_renderer_info().origin_offset();
        let x = x * zoom_size.width + origin_offset.column + logo_w + zoom_size.width; // right-side with one tile space
        let y = y * zoom_size.height + origin_offset.row;
        let rect = sdl::SDL_Rect {
          x,
          y,
          w: logo_h,
          h: logo_h,
        };

        let sdl_renderer = df::renderer::get_sdl_info().renderer();
        let logo_texture = logo::get_logo_texture();
        sdl_renderer.copy(logo_texture, None, Some(&rect));
      }
    }
  }

  call_update_tile(renderer_ptr, x, y);

  if !control::is_enabled() {
    return;
  }

  let dimensions = df::gps::get_dimensions();
  if x != dimensions.width - 1 || y != dimensions.height - 1 {
    return;
  }

  let sdl_renderer = df::renderer::get_sdl_info().renderer();
  for (id, coordinate, text_block) in screen::get_text_blocks(screen::Layer::Lower) {
    text_block.render(&sdl_renderer, &coordinate, screen::Layer::Lower, id);
  }
}

static LAST_DIMENSIONS: OnceLock<RwLock<types::Dimensions>> = OnceLock::new();
static DISPLAY_TITLE: OnceLock<Mutex<Option<sdl::Texture<'static>>>> = OnceLock::new();

// Getting access to the display title texture
fn get_display_title_mut() -> MutexGuard<'static, Option<sdl::Texture<'static>>> {
  DISPLAY_TITLE.get_or_init(|| Mutex::new(None)).lock().unwrap()
}

fn update_all(renderer_ptr: *const ffi::c_void) {
  if control::is_enabled() {
    let display_title = df::gps::get_display_title();
    if *display_title {
      if let Some(logo_texture) = logo::get_title_logo_by_lang_tag(&lang::current_lang_tag()) {
        get_display_title_mut().replace(logo_texture);
        *display_title = false;
      }
    }
  }
  let mut dimensions = LAST_DIMENSIONS.get_or_init(|| RwLock::new(types::Dimensions::default())).write().unwrap();

  let last_dimensions = dimensions.clone();
  let curr_dimensions = df::gps::get_dimensions().clone();

  if curr_dimensions != last_dimensions {
    dimensions.clone_from(&curr_dimensions);
    // log::debug!("Dimension changed: {curr_dimensions:?}");
  }

  call_update_all(renderer_ptr);

  if control::is_enabled() {
    let sdl_renderer = df::renderer::get_sdl_info().renderer();
    for (id, coordinate, text_block) in screen::get_text_blocks(screen::Layer::Upper) {
      text_block.render(&sdl_renderer, &coordinate, screen::Layer::Upper, id);
    }
  }

  control::toggle_enabled();
  control::do_reset_if_requested();
}

fn mtb_process_string_to_lines(mtb_ptr: *const ffi::c_void, markup_string_ptr: *const ffi::c_void) {
  

  let markup = cp437_string::cxx_string_to_string(markup_string_ptr);
  let request = TranslationRequest::new(TranslationInput::markup_text_box {
    address: mtb_ptr as usize,
    markup: markup.clone(),
  });

  logging::log_text(&request, ptr::null());

  markup::track_mtb_markup(mtb_ptr as usize, markup);

  call_mtb_process_string_to_lines(mtb_ptr, markup_string_ptr);
}

fn mtb_set_width(mtb_ptr: *const ffi::c_void, width: i32) {
  // use markup to set width instead of the original function if tracked
  let address = mtb_ptr as usize;
  if let Some(mut markup) = markup::fetch_mtb_markup(address) {
    // translate the content if enabled
    if control::is_enabled() {
      let request = TranslationRequest::new(TranslationInput::markup_text_box {
        address,
        markup: markup.clone(),
      });

      if let Some(response) = translator::translate(&request) {
        markup = response.translated;
      }
    }

    // set width via markup and sync with MTB
    markup::set_width_and_sync(&markup, width, address, control::is_enabled());
  }

  call_mtb_set_width(mtb_ptr, width);
}

#[unsafe(no_mangle)]
fn dfhack_addstr_flag(lua_state: *mut ffi::c_void) {
  

  // Read parameters from Lua stack
  let x = lua::check_integer(lua_state, 1);
  let y = lua::check_integer(lua_state, 2);
  let fg = lua::check_integer(lua_state, 3);
  let bg = lua::check_integer(lua_state, 4);
  let bold = lua::check_integer(lua_state, 5) != 0;
  let string = lua::check_cp437_string(lua_state, 6);
  let flag = lua::check_integer(lua_state, 7);

  // Clamp parameters to valid ranges
  let dims = df::gps::get_dimensions();
  let x = x.clamp(0, dims.width as isize - 1) as i32;
  let y = y.clamp(0, dims.height as isize - 1) as i32;
  let flag = flag.clamp(0, u32::MAX as isize) as u32;

  // Create translation request
  let coordinate = types::Coordinate { row: y, column: x };
  let color_pair = ColorPair::from_old_16_colors(fg.clamp(0, 15) as i8, bg.clamp(0, 15) as i8, bold);
  let request = TranslationRequest::new(TranslationInput::dfhack {
    content: string.clone(),
    coordinate,
    color_pair,
    flag,
  });

  // Log the translation request and update the text block
  logging::log_text(&request, ptr::null());
  let text_block = text::TextBlock::get(&request);
  let id = text_block.add_to_screen(screen::Layer::Lower, request.coordinate());

  if !control::is_enabled() {
    return;
  }

  // Mark the screen area as occupied
  let coord = request.coordinate();
  let bottom_coord = types::Coordinate {
    row: (coord.row + 1).clamp(0, dims.height - 1),
    column: coord.column,
  };
  screen::mark_occupied(screen::Layer::Lower, coord, &text_block, Some(id));
  screen::mark_occupied(screen::Layer::Lower, bottom_coord, &text_block, Some(id));
}

fn render_things() {
  screen::clear_screens();
  get_display_title_mut().take();

  call_render_things();

  // move occupied tiles before rendering
  if control::is_enabled() {
    screen::move_occupied();
  }
}

fn dfhack_paint_string(pen_str: *const ffi::c_void, x: i32, y: i32, string_ptr: *const ffi::c_void, map: bool) -> bool {
  

  let string = cp437_string::cxx_string_to_string(string_ptr);
  let coordinate = types::Coordinate { row: y, column: x };
  let request = TranslationRequest::new(TranslationInput::dfhack {
    content: string.clone(),
    coordinate,
    color_pair: ColorPair::from(DFHackPen::from_ptr(pen_str)),
    flag: 0,
  });

  logging::log_text(&request, ptr::null());
  let text_block = text::TextBlock::get(&request);
  let columns = text_block.columns();
  let id = text_block.add_to_screen(screen::Layer::Lower, request.coordinate());

  if !control::is_enabled() {
    return call_dfhack_paint_string(pen_str, x, y, string_ptr, map);
  }

  let coord = request.coordinate();
  let cpp_string = cpp::CppString::fill(b' ', columns);
  let ret = call_dfhack_paint_string(pen_str, x, y, cpp_string.raw(), map);
  screen::mark_occupied(screen::Layer::Lower, coord, &text_block, Some(id));

  ret
}

// Hook the game's personality-value description composer (observed at
// 0x140664be0 on DF 53.16; located by runtime byte-pattern search, so the
// address is version-adaptive). It writes a composed description string into
// the std::string* at `out` (r8). That text is rendered by the game's newer
// text path, which dfi18n's classic addst hooks never see. Read-only: capture
// untranslated
// strings (into the log and the realtime translate queue); we do NOT rewrite
// `out` because the game's renderer does not reliably display the injected
// CJK (encoding/width issues).
fn description_composer(index: i32, variant: i32, out: *mut ffi::c_void) {
  // let the original build the composed string first
  call_description_composer(index, variant, out);

  // read the composed string
  let content = cp437_string::cxx_string_to_string(out as *const ffi::c_void);
  if translator::should_skip_translation(&content) {
    return;
  }

  let request = TranslationRequest::new(TranslationInput::addst { content: content.clone() });

  // log it (with the runtime call stack) if it is not covered by the dictionaries
  
  logging::log_text(&request, ptr::null());
}

// Hook the game's thought composer (observed at 0x140e2c460 on DF 53.16;
// located by runtime byte-pattern search, so the address is version-adaptive).
// It writes a composed thought string into the std::string* at arg2 (rdx).
// Thoughts are generated for every dwarf constantly, so this collects hundreds
// of actual composed strings per session -- much faster than waiting for
// screens.
#[allow(clippy::too_many_arguments)]
fn thought_composer(
  a1: usize,
  out: *mut ffi::c_void,
  a3: i32,
  a4: i32,
  a5: i32,
  a6: u32,
  a7: u32,
  a8: u32,
) {
  // let the original build the composed thought string first
  call_thought_composer(a1, out, a3, a4, a5, a6, a7, a8);

  // read the composed string
  let content = cp437_string::cxx_string_to_string(out as *const ffi::c_void);
  if translator::should_skip_translation(&content) {
    return;
  }

  let request = TranslationRequest::new(TranslationInput::addst { content: content.clone() });

  // log it (with the runtime call stack) if it is not covered by the dictionaries
  
  logging::log_text(&request, ptr::null());
}

hook! {
  fn addst(gps_ptr: *const ffi::c_void, string_ptr: *const ffi::c_void, just: u8, space: i32);
  fn addst_flag(gps_ptr: *const ffi::c_void, string_ptr: *const ffi::c_void, just: u8, space: i32, sflag: u32);
  fn addcoloredst(gps_ptr: *const ffi::c_void, string_ptr: *const ffi::c_void, color_string_ptr: *const ffi::c_void);
  fn top_addst(gps_ptr: *const ffi::c_void, string_ptr: *const ffi::c_void, just: u8, space: i32);
  fn update_tile(renderer_ptr: *const ffi::c_void, x: i32, y: i32);
  fn update_all(renderer_ptr: *const ffi::c_void);
  fn mtb_process_string_to_lines(mtb_ptr: *const ffi::c_void, markup_string_ptr: *const ffi::c_void);
  fn mtb_set_width(mtb_ptr: *const ffi::c_void, width: i32);
  fn render_things();
  fn dfhack_paint_string(pen_str: *const ffi::c_void, x: i32, y: i32, string_ptr: *const ffi::c_void, map: bool) -> bool;
  fn description_composer(index: i32, variant: i32, out: *mut ffi::c_void);
  fn thought_composer(a1: usize, out: *mut ffi::c_void, a3: i32, a4: i32, a5: i32, a6: u32, a7: u32, a8: u32);
}

pub fn attach_all() -> Result<()> {
  attach_addst(memory::get_raw_pointer_by_key("addst")?)?;
  attach_addst_flag(memory::get_raw_pointer_by_key("addst_flag")?)?;
  attach_addcoloredst(memory::get_raw_pointer_by_key("addcoloredst")?)?;
  attach_top_addst(memory::get_raw_pointer_by_key("top_addst")?)?;
  attach_update_tile(memory::get_raw_pointer_by_key("update_tile")?)?;
  attach_update_all(memory::get_raw_pointer_by_key("update_all")?)?;
  attach_mtb_process_string_to_lines(memory::get_raw_pointer_by_key("mtb_process_string_to_lines")?)?;
  attach_mtb_set_width(memory::get_raw_pointer_by_key("mtb_set_width")?)?;
  attach_render_things(memory::get_raw_pointer_by_key("render_things")?)?;
  attach_dfhack_paint_string(memory::get_raw_pointer_by_key("dfhack_paint_string")?)?;

  // Optional hooks: the new-render-path composers only exist in newer DF
  // builds. Skip them gracefully when the pattern does not match, so older
  // DF versions keep working without an attach error.
  if let Ok(ptr) = memory::get_raw_pointer_by_key("description_composer") {
    attach_description_composer(ptr)?;
  }
  if let Ok(ptr) = memory::get_raw_pointer_by_key("thought_composer") {
    attach_thought_composer(ptr)?;
  }

  Ok(())
}

// handle translation for help markup text boxes, return true if handled
fn handle_help_mtb(string_ptr: *const ffi::c_void) -> bool {
  // TODO: add other markup text boxes as well
  let help = df::game::main_interface::get_help_mut();
  // for each markup text box
  for mtb in help.text.iter_mut() {
    let word = cpp::CppVector::from_raw(ptr::from_mut(&mut mtb.word));
    for i in 0..word.size() {
      let mtw_ptr: &mut df::game::MarkupTextWord = word.get(i);
      let mtw = unsafe { (mtw_ptr as *const df::game::MarkupTextWord).as_ref_unchecked() };
      let mtw_str_ptr = &mtw.str as *const _ as *const ffi::c_void;
      // check if the string_ptr matches
      if string_ptr == mtw_str_ptr {
        // only render on the first markup text word
        if i == 0 {
          let address = mtb as *const df::game::MarkupTextBox as usize;
          if let Some(mut markup) = markup::fetch_mtb_markup(address) {
            let request = TranslationRequest::new(TranslationInput::markup_text_box {
              address,
              markup: markup.clone(),
            });

            logging::log_text(&request, string_ptr);

            // always sync the markup text box before rendering
            if control::is_enabled() {
              if let Some(response) = translator::translate(&request) {
                markup = response.translated;
              }
            }
            markup::sync(&markup, address, control::is_enabled());
            let text_block = markup::get(&markup).text_block();

            // no need to occupy the tiles if not enabled as the original function will do the rendering
            if control::is_enabled() {
              let id = text_block.add_to_screen(screen::Layer::Upper, request.coordinate());
              screen::mark_occupied(screen::Layer::Upper, request.coordinate(), &text_block, Some(id));
            }
          }
        }

        // override original rendering
        if control::is_enabled() {
          return true;
        }
      }
    }
  }

  // use original rendering
  false
}
