use std::sync::OnceLock;

use sdl2_sys as sdl;

use crate::{df, screen};

// Debugger window instance
static WINDOW: OnceLock<sdl::Window<'static>> = OnceLock::new();

// Get debugger window configurations
fn window_configs() -> (i32, i32, i32, i32) {
  let game_window = df::renderer::get_sdl_info().window();
  let (width, height) = game_window.get_size();
  let (x, y) = game_window.get_position();
  (x + width + 10, y, width, height)
}

// Get the debugger window instance
fn window() -> &'static sdl::Window<'static> {
  WINDOW.get_or_init(|| {
    let (x, y, width, height) = window_configs();
    let window = sdl::Window::new("Debugger", width, height);
    window.set_position(x, y);
    window
  })
}

// Debugger window renderer instance
static RENDERER: OnceLock<sdl::Renderer<'static>> = OnceLock::new();

// Get the debugger window renderer instance
pub fn renderer() -> &'static sdl::Renderer<'static> {
  RENDERER.get_or_init(|| window().create_renderer())
}

// Resize the debugger window and adjust the renderer logical size
pub fn resize() {
  let debug_window = window();
  let (debug_width, debug_height) = debug_window.get_size();
  let (debug_x, debug_y) = debug_window.get_position();

  let (x, y, width, height) = window_configs();

  // No need to resize
  if width == debug_width && height == debug_height {
    return;
  }

  // Resize the debugger window and adjust the renderer logical size
  debug_window.set_size(width, height);
  let renderer = renderer();
  renderer.set_logical_size(width, height);

  // Move the debugger window to the right of the game window after resized
  if x != debug_x || y != debug_y {
    debug_window.set_position(x, y);
  }
}

// Update the debugger window content
pub fn update() {
  // Clear and render the debugger window content
  let renderer = renderer();
  renderer.clear_color(16, 16, 16, 255);

  // Render all text blocks in the lower screen layer
  for (id, coordinate, sflag, text_block) in screen::get_text_blocks(screen::Layer::Lower) {
    text_block.render(&renderer, &coordinate, sflag, screen::Layer::Lower, id);
  }

  // Render all text blocks in the upper screen layer
  for (id, coordinate, sflag, text_block) in screen::get_text_blocks(screen::Layer::Upper) {
    text_block.render(&renderer, &coordinate, sflag, screen::Layer::Upper, id);
  }

  renderer.present();
}
