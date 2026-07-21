use std::sync::atomic::AtomicU16;
use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{df, text, translation, types};

// Screen layers
#[derive(Debug, Clone)]
pub enum Layer {
  Upper,
  Lower,
}

// Atomic counters for unique IDs of text blocks in each screen layer
static LOWER_ID: OnceLock<AtomicU16> = OnceLock::new();

// Getting access to the atomic counter for lower screen layer
fn get_lower_id() -> &'static AtomicU16 {
  LOWER_ID.get_or_init(|| AtomicU16::new(0))
}

// Generate the next unique ID for lower screen layer
fn next_lower_id() -> u16 {
  get_lower_id().fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

// Atomic counters for unique IDs of text blocks in each screen layer
static UPPER_ID: OnceLock<AtomicU16> = OnceLock::new();

// Getting access to the atomic counter for upper screen layer
fn get_upper_id() -> &'static AtomicU16 {
  UPPER_ID.get_or_init(|| AtomicU16::new(0))
}

// Generate the next unique ID for upper screen layer
fn next_upper_id() -> u16 {
  get_upper_id().fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

// List of text blocks on a screen layer, indexed by their IDs and coordinates
type Screen = Vec<(u16, types::Coordinate, text::TextBlock)>;
// Tuple of screens representing the upper and lower screens
type Screens = (Screen, Screen);

// Lower and upper screens text blocks
static SCREENS: OnceLock<RwLock<Screens>> = OnceLock::new();

// Getting access to the screens text blocks
fn get_screens() -> RwLockReadGuard<'static, Screens> {
  SCREENS.get_or_init(|| RwLock::new(Screens::default())).read().unwrap()
}

// Getting mutable access to the screens text blocks
fn get_screens_mut() -> RwLockWriteGuard<'static, Screens> {
  SCREENS.get_or_init(|| RwLock::new(Screens::default())).write().unwrap()
}

// Clears all text blocks from the screens, then clears DFHack occupied tile coordinates
pub fn clear_screens() {
  get_lower_id().store(0, std::sync::atomic::Ordering::SeqCst);
  get_upper_id().store(0, std::sync::atomic::Ordering::SeqCst);
  let mut screens = get_screens_mut();
  screens.0.clear();
  screens.1.clear();
}

// Adds a text block to the specified screen layer at the given coordinate, returns its assigned ID
pub fn add_text_block_to_screens(layer: Layer, coordinate: types::Coordinate, text_block: text::TextBlock) -> u16 {
  let mut screens = get_screens_mut();
  let (screen, id) = match layer {
    Layer::Lower => (&mut screens.0, next_lower_id()),
    Layer::Upper => (&mut screens.1, next_upper_id()),
  };
  screen.push((id, coordinate, text_block));
  id
}

// Retrieves all text blocks from the specified screen layer
pub fn get_text_blocks(layer: Layer) -> Vec<(u16, types::Coordinate, text::TextBlock)> {
  let screens = get_screens();
  let screen = match layer {
    Layer::Lower => &screens.0,
    Layer::Upper => &screens.1,
  };
  screen.iter().map(|(id, coord, tb)| (*id, coord.to_owned(), tb.to_owned())).collect()
}

// Checks if any DFHack occupied tile exists within the specified rectangle
pub fn is_occupied_tile(x1: i32, x2: i32, y1: i32, y2: i32, layer: &Layer, id: u16) -> bool {
  let occupied = get_occupied_mut();
  let occupied = match layer {
    Layer::Lower => &occupied.0,
    Layer::Upper => &occupied.1,
  };
  let screen_tokens = id_to_screen_tokens(Some(id));

  // let tiles_pos = get_dfhack_tiles_pos();
  let ctl = get_tile_coordinate(x1, y1).0;
  let cbr = get_tile_coordinate(x2, y2).1;

  // check each tile coordinate in the rectangle
  for row in ctl.row..cbr.row {
    for column in ctl.column..cbr.column {
      let coord = types::Coordinate { column, row };
      let offset = coord.offset() / 8;
      // fix Windows OOB panic due to DF window resizing
      if offset >= occupied.len() {
        continue;
      }
      let occupied_tokens = occupied[offset];
      // use 'z' for markup texts, see markup module
      if !(occupied_tokens.0 == b'z' || occupied_tokens == screen_tokens) {
        return true;
      }
    }
  }

  false
}

// Converts pixel coordinates to tile coordinates (top-left and bottom-right)
fn get_tile_coordinate(x: i32, y: i32) -> (types::Coordinate, types::Coordinate) {
  let zoom_size = df::renderer::get_renderer_info().zoom_size();
  let origin_offset = df::renderer::get_renderer_info().origin_offset();

  // calculate tile coordinates
  let column = (x as f32 - origin_offset.column as f32) / zoom_size.width as f32;
  let row = (y as f32 - origin_offset.row as f32) / zoom_size.height as f32;

  // calculate top-left tile coordinate
  let ctl = types::Coordinate {
    column: column.floor() as i32,
    row: row.floor() as i32,
  };
  // calculate bottom-right tile coordinate
  let cbr = types::Coordinate {
    column: column.ceil() as i32,
    row: row.ceil() as i32,
  };

  (ctl, cbr)
}

// Type representing screen tokens for collision checking, low-byte comes first and is an ASCII85 character
pub type ScreenTokens = (u8, u8);

// Type representing a vector of occupied screen tokens
pub type Occupied = Vec<ScreenTokens>;

// Set of occupied screen tokens
static OCCUPIED: OnceLock<RwLock<(Occupied, Occupied)>> = OnceLock::new();

// Getting access to the occupied screen tokens
pub fn get_occupied_mut() -> RwLockWriteGuard<'static, (Occupied, Occupied)> {
  OCCUPIED.get_or_init(|| RwLock::new((Vec::new(), Vec::new()))).write().unwrap()
}

// Mark a text block on a screen layer as occupied by an ID
// TODO: remove Option from id?
pub fn mark_occupied(layer: Layer, coord: types::Coordinate, text_block: &text::TextBlock, id: Option<u16>) {
  let (lo, hi) = id_to_screen_tokens(id);
  let types::Coordinate { row, column } = coord;
  let dims = df::gps::get_dimensions();
  for i in 0..text_block.len() {
    let row = row + i as i32;
    if row < 0 || row >= dims.height {
      continue;
    }

    let text_row = &text_block[i];
    let cols_diff = text_block.columns() as f32 - text_row.columns() as f32;
    let cols_offset = match text_block.alignment() {
      translation::TextAlignment::Left => 0f32,
      translation::TextAlignment::Center => cols_diff / 2.0,
      translation::TextAlignment::Right => cols_diff,
    };
    let columns = text_row.columns() + if cols_offset == 0.0 { 0 } else { 1 };
    let cols_offset = cols_offset.floor() as i32;

    for j in 0..columns {
      let column = column + cols_offset + j as i32;
      if column < 0 || column >= dims.width {
        continue;
      }

      let coord = types::Coordinate { column, row };
      let cell = df::renderer::get_cell(&coord, matches!(layer, Layer::Upper));
      cell[0] = lo;
      cell[7] = hi;
    }
  }
}

pub fn mark_source_region(layer: Layer, coord: types::Coordinate, width: i32, height: i32, id: u16) {
  let (lo, hi) = id_to_screen_tokens(Some(id));
  let dims = df::gps::get_dimensions();
  for row in coord.row..coord.row.saturating_add(height) {
    if row < 0 || row >= dims.height {
      continue;
    }
    for column in coord.column..coord.column.saturating_add(width) {
      if column < 0 || column >= dims.width {
        continue;
      }
      let cell = df::renderer::get_cell(&types::Coordinate { column, row }, matches!(layer, Layer::Upper));
      cell[0] = lo;
      cell[7] = hi;
    }
  }
}

// Move all occupied marks on a screen layer
pub fn move_occupied() {
  let mut occupied = get_occupied_mut();
  let dims = df::gps::get_dimensions();
  occupied.0.clear();
  occupied.1.clear();
  let size = dims.width as usize * dims.height as usize;
  occupied.0.resize(size, (0, 0));
  occupied.1.resize(size, (0, 0));
  for row in 0..dims.height {
    for column in 0..dims.width {
      let coord = types::Coordinate { column, row };

      // offset boundary check
      let offset = coord.offset() / 8;
      if offset >= occupied.0.len() {
        continue;
      }

      let cell = df::renderer::get_cell(&coord, false);
      occupied.0[offset] = (cell[0], cell[7]);
      if df::renderer::get_tex_cell(&coord, false) != 0 {
        occupied.0[offset] = (0, 0);
      }
      if cell[7] != 0 {
        cell[0] = b' ';
        cell[7] = 0;
      }

      let cell = df::renderer::get_cell(&coord, true);
      occupied.1[offset] = (cell[0], cell[7]);
      if df::renderer::get_tex_cell(&coord, true) != 0 {
        occupied.1[offset] = (0, 0);
      }
      if cell[7] != 0 {
        cell[0] = b' ';
        cell[7] = 0;
      }
    }
  }
}

// Mark a horizontal line for the bottom half of text as occupied by coping ID from upper half
pub fn mark_bottom_occupied(layer: Layer, coord: types::Coordinate, length: i32) {
  let types::Coordinate { row, column } = coord;
  if row == 0 {
    return;
  }
  for i in 0..length {
    let column = column + i as i32;
    if column > df::gps::get_dimensions().width - 1 {
      continue;
    }

    let coord = types::Coordinate { column, row };
    let cell = df::renderer::get_cell(&coord, matches!(layer, Layer::Upper));

    let upper_cell = df::renderer::get_cell(
      &types::Coordinate { column, row: row - 1 },
      matches!(layer, Layer::Upper),
    );
    cell[0] = upper_cell[0];
    cell[7] = upper_cell[7];
  }
}

// ASCII85 character set for encoding IDs
const ASCII85: [u8; 85] = *b"!\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstu";

// Convert ID to a pair of screen tokens for collision checking
fn id_to_screen_tokens(id: Option<u16>) -> ScreenTokens {
  let Some(id) = id else {
    return (0, 0);
  };

  let hi = (id / 85 + 1) as u8;
  let lo = ASCII85[(id % 85) as usize];
  (lo, hi)
}
