use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{OnceLock, RwLock};

use sdl2_sys as sdl;

use crate::types::{SCREENTEXPOS_BOTTOM_OF_TEXT_SFLAG, SCREENTEXPOS_TOP_OF_TEXT_SFLAG};
use crate::{df, glyph, markup, screen, translation, translator, types};

// TODO: make this configurable
// Font padding pixels in original size for aligning non-curses glyphs to baseline
pub const FONT_PADDING: i32 = 3;

// Reset the texts cache
pub fn reset() {
  get_texts_mut().clear();
}

// Texts cache maps TranslationRequest key to TextBlock
static TEXTS: OnceLock<RwLock<HashMap<String, TextBlock>>> = OnceLock::new();

// Getting the write lock for the texts cache
fn get_texts_mut() -> std::sync::RwLockWriteGuard<'static, HashMap<String, TextBlock>> {
  TEXTS.get_or_init(|| RwLock::new(HashMap::new())).write().unwrap()
}

// A fragment of text with associated color information
#[derive(Debug, Default, Clone)]
pub struct TextFragment {
  // The text content of the fragment
  content: String,
  // The color pair (foreground and background) for the fragment
  color_pair: types::ColorPair,
}

impl TextFragment {
  // Creates a new TextFragment with the given content and color pair
  pub fn new(content: String, color_pair: types::ColorPair) -> Self {
    TextFragment { content, color_pair }
  }

  // Creates a new TextFragment consisting of spaces with the specified count
  pub fn spaces(count: usize) -> Self {
    Self::new(" ".repeat(count), types::ColorPair::default())
  }

  // Calculates the original width of the text fragment by summing glyph widths
  pub fn orig_width(&self) -> i32 {
    self.content.chars().map(|ch| glyph::get_glyph_orig_size(ch).0).sum()
  }

  // Gets the content of the text fragment
  pub fn content(&self) -> &str {
    &self.content
  }
}

// A row of text consisting of multiple fragments
#[derive(Debug, Default, Clone)]
pub struct TextRow {
  // The default color pair for the row
  default_color_pair: types::ColorPair,
  // The list of text fragments in the row
  fragments: Vec<TextFragment>,
}

impl Deref for TextRow {
  type Target = Vec<TextFragment>;

  // Deref implementation to access the fragments of the TextRow
  fn deref(&self) -> &Self::Target {
    &self.fragments
  }
}

impl DerefMut for TextRow {
  // DerefMut implementation to access the fragments of the TextRow mutably
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.fragments
  }
}

impl TextRow {
  // Creates a new TextRow with the specified default color pair
  pub fn new(default_color_pair: types::ColorPair) -> Self {
    TextRow {
      default_color_pair,
      fragments: Vec::new(),
    }
  }

  // Adds a text fragment with the default color pair to the row
  pub fn push_text(&mut self, content: String) {
    self.fragments.push(TextFragment::new(content, self.default_color_pair));
  }

  // Calculates the original width of the entire text row by summing fragment widths
  pub fn orig_width(&self) -> i32 {
    self.fragments.iter().map(|f| f.orig_width()).sum()
  }

  // Gets the number of columns the text row would occupy on the game screen
  pub fn columns(&self) -> usize {
    let orig_width = df::renderer::get_renderer_info().orig_size().width;
    (self.orig_width() as f32 / orig_width as f32).ceil() as usize
  }
}

// Layout information for a block of text
#[derive(Debug, Default, Clone)]
pub struct TextLayout {
  // Number of game screen columns (with the size of a single character cell for built-in curses font) in the text block
  columns: usize,
  // Text alignment within the block
  alignment: translation::TextAlignment,
  // Whether to use double line height for the text block (originally rendered with two text fragments of top and bottom parts)
  double_line_height: bool,
}

impl TextLayout {
  // Creates a new TextLayout with the specified number of columns
  fn new(columns: usize) -> Self {
    TextLayout {
      columns,
      ..Default::default()
    }
  }

  // Sets the text alignment for the layout
  pub fn set_alignment(&mut self, alignment: translation::TextAlignment) {
    self.alignment = alignment;
  }

  // Enables double line height for the layout
  pub fn set_double_line_height(&mut self) {
    self.double_line_height = true;
  }
}

// A block of text consisting of multiple rows and layout information, can be rendered
#[derive(Debug, Default, Clone)]
pub struct TextBlock {
  // The rows of text in the block
  rows: Vec<TextRow>,
  // The layout information for the text block
  layout: TextLayout,
}

impl Deref for TextBlock {
  type Target = Vec<TextRow>;

  // Deref implementation to access the rows of the TextBlock
  fn deref(&self) -> &Self::Target {
    &self.rows
  }
}

impl DerefMut for TextBlock {
  // DerefMut implementation to access the rows of the TextBlock mutably
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.rows
  }
}

impl TextBlock {
  // Create a TextBlock with specified number of columns and default layout
  pub fn from_columns(columns: usize) -> Self {
    TextBlock {
      layout: TextLayout::new(columns),
      ..Default::default()
    }
  }

  // Create a TextBlock from a single TextRow and layout
  fn from_row(row: TextRow, layout: TextLayout) -> Self {
    TextBlock {
      rows: vec![row],
      layout,
    }
  }

  // Get a TextBlock from original text without translation
  fn from_original(original: &str, color_pair: types::ColorPair) -> Self {
    let mut row = TextRow::new(color_pair);
    row.push_text(original.to_owned());
    let layout = TextLayout::new(original.len());
    Self::from_row(row, layout)
  }

  // Get a TextBlock from a TranslationRequest, using cache if available
  pub fn get(request: &translation::TranslationRequest) -> Self {
    let color_pair = request.color_pair().unwrap_or_default();

    if request.is_markup() {
      // attempt translation to get translated markup and fallback to original markup
      let markup = if let Some(response) = translator::translate(&request) {
        response.translated
      } else {
        request.original().to_owned()
      };

      // use the markup managed text block
      return markup::get(&markup).text_block();
    } else {
      // early return for untranslatable content or translation is not available (do not cache content that is not translated)
      let original = request.original();
      if translator::should_skip_translation(original) || translator::translate(request).is_none() {
        let mut untranslated_text_block = Self::from_original(original, color_pair);

        // set double line height when needed
        if request.context().has_double_line_height() {
          untranslated_text_block.layout.set_double_line_height();
        }

        return untranslated_text_block;
      }
    }

    // check cache first
    let key = request.key();
    let mut texts = get_texts_mut();
    if let Some(cached) = texts.get(key) {
      return cached.to_owned();
    }

    // attempt to create a new TextBlock if not cached
    let text_block = Self::from_request(request);
    texts.insert(key.to_owned(), text_block.clone());
    // log::debug!("Insert new TextBlock into cache: {text_block:#?}"); // XXX

    text_block
  }

  // Create a new TextBlock from a TranslationRequest
  fn from_request(request: &translation::TranslationRequest) -> Self {
    let color_pair = request.color_pair().unwrap_or_default();

    // Create a TextBlock from the original text
    let original = request.original();
    let mut original_text_block = Self::from_original(original, color_pair);

    // set double line height when needed
    if request.context().has_double_line_height() {
      original_text_block.layout.set_double_line_height();
    }

    // if the translation is available
    if let Some(response) = translator::translate(request) {
      // prepare the translated TextRow
      let mut row = TextRow::new(color_pair);
      row.push_text(response.translated);

      // use alignment from the translation response
      let orig_width = df::renderer::get_renderer_info().orig_size().width;
      let columns = (row.orig_width() as f32 / orig_width as f32).ceil() as usize;
      let layout = if matches!(response.alignment, translation::TextAlignment::Left) {
        let mut layout = TextLayout::new(columns);
        if original_text_block.layout.double_line_height {
          layout.set_double_line_height();
        }
        layout
      } else {
        let mut layout = original_text_block.layout.clone();
        layout.set_alignment(response.alignment);
        layout
      };

      return Self::from_row(row, layout);
    }

    // fallback to original text
    original_text_block
  }

  // Get the number of rows in the TextBlock
  pub fn columns(&self) -> usize {
    self.layout.columns
  }

  // Get the text alignment of the TextBlock
  pub fn alignment(&self) -> &translation::TextAlignment {
    &self.layout.alignment
  }

  // Adds the TextBlock to the specified screen layer at the given coordinate, returns its assigned ID
  pub fn add_to_screen(&self, layer: screen::Layer, coordinate: types::Coordinate, sflag: types::SFlag) -> u16 {
    screen::add_text_block_to_screens(layer, coordinate, sflag, self.to_owned())
  }

  // Render the TextBlock at the specified position using the given renderer, with layer and ID for collision checking
  pub fn render(
    &self,
    renderer: &sdl::Renderer<'static>,
    coordinate: &types::Coordinate,
    sflag: types::SFlag,
    layer: screen::Layer,
    id: u16,
  ) {
    // get the origin offset for rendering to center aligned the rendering area
    let origin_offset = df::renderer::get_renderer_info().origin_offset();
    // original glyph size (the curses font size)
    let orig_size = df::renderer::get_renderer_info().orig_size();
    // zoomed glyph size (the actual rendered size)
    let zoom_size = df::renderer::get_renderer_info().zoom_size();

    let px = coordinate.column * zoom_size.width + origin_offset.column;
    let py = coordinate.row * zoom_size.height + origin_offset.row;

    if self.rows.is_empty() {
      return;
    }

    // zoom factors for width and height
    let width_zoom_factor = zoom_size.width as f32 / orig_size.width as f32;
    let height_zoom_factor = zoom_size.height as f32 / orig_size.height as f32;

    // the actual line height
    let line_height = zoom_size.height as f32;

    // number of rows and columns
    let rows = self.rows.len();
    let columns = self.layout.columns;

    // maximum original width of the text block for alignment calculation
    let orig_max_width = columns as i32 * orig_size.width;

    // the minimum offset x and maximum width of the actual rendered text area
    let mut min_ox = i32::MAX;
    let mut max_w = i32::MIN;

    /* NOTE: changed top/bottom half rendering.
     *
    // add 1/2 zoomed glyph height for double line height layout
    // let mut py = py;
    // if self.layout.double_line_height {
    //   py += (zoom_size.height as f32 / 2.0).round() as i32;
    // }
    let py = py;
     *
     */

    // for each row in the text block
    for (i, row) in self.rows.iter().enumerate() {
      // offset y in zoomed size for the row
      let oy = (i as f32 * line_height).round() as i32;

      // row width after scaling
      let orig_row_width = row.orig_width();
      // offset x in original size based on alignment after scaling
      let original_ox = match self.layout.alignment {
        translation::TextAlignment::Left => 0f32,
        translation::TextAlignment::Center => (orig_max_width as f32 - orig_row_width as f32) / 2f32,
        translation::TextAlignment::Right => orig_max_width as f32 - orig_row_width as f32,
      };
      // offset x in zoomed size
      let ox = (original_ox * width_zoom_factor).round() as i32;

      // update minimum offset x
      if ox < min_ox {
        min_ox = ox;
      }

      // render each fragment in the row
      let mut cx = 0;
      for fragment in &row.fragments {
        // render each character in the fragment
        for ch in fragment.content.chars() {
          // get the glyph texture for the character, note the texture may be scaled if it's not a built-in curses glyph
          let texture = glyph::get_glyph_texture(renderer, ch);

          // calculate the scale factor
          let (texture_width, texture_height) = texture.size();
          let texture_scale_factor = texture_height as f32 / orig_size.height as f32;
          let width_scale_factor = 1f32 / texture_scale_factor * width_zoom_factor;
          let height_scale_factor = 1f32 / texture_scale_factor * height_zoom_factor;

          // add padding for non-curses glyphs to align to baseline
          let padding = if cp437_string::char_is_cp437(ch) {
            0
          } else {
            FONT_PADDING
          };
          // padding pixels in zoomed size add to the right
          let padding_x = (padding as f32 * width_scale_factor).round() as i32;
          // padding pixels in zoomed size add to the bottom
          let padding_y = (padding as f32 * height_scale_factor).round() as i32;

          // calculate the destination rectangle for rendering the glyph
          let x = px + ox + cx;
          let y = py + oy;
          let w = (texture_width as f32 * width_scale_factor).round() as i32 - padding_x;
          let h = (texture_height as f32 * height_scale_factor).round() as i32 - padding_y;
          let rect = sdl::SDL_Rect { x, y, w, h };

          /* NOTE: screen::is_occupied_tile() cause text flicker. */
          // check if the glyph fits within the text block area and does not overlap occupied tiles
          if ox + w <= columns as i32 * zoom_size.width && !screen::is_occupied_tile(x, x + w, y, y + h, &layer, id) {
            // render the background color if not black (which means transparent)
            let bg = &fragment.color_pair.background;
            if bg.r != 0 || bg.g != 0 || bg.b != 0 {
              renderer.fill_rect(&rect, bg.r, bg.g, bg.b, 255);
            }

            // render the glyph with the foreground color
            let fg = &fragment.color_pair.foreground;
            texture.set_color_mod(fg.r, fg.g, fg.b);

            /* Mimic g_src's top and bottom half glyph rendering */
            if (sflag & SCREENTEXPOS_TOP_OF_TEXT_SFLAG) != 0 {
              let src_rect = sdl::SDL_Rect {
                x: 0,
                y: 0,
                w: texture_width,
                h: texture_height / 2,
              };
              let dst_rect = sdl::SDL_Rect {
                x,
                y: y + zoom_size.height - h / 2,
                w,
                h: h / 2,
              };
              renderer.copy(&texture, Some(&src_rect), Some(&dst_rect));
            } else if (sflag & SCREENTEXPOS_BOTTOM_OF_TEXT_SFLAG) != 0 {
              let src_rect = sdl::SDL_Rect {
                x: 0,
                y: texture_height / 2,
                w: texture_width,
                h: texture_height - texture_height / 2,
              };
              let dst_rect = sdl::SDL_Rect { x, y, w, h: h - h / 2 };
              renderer.copy(&texture, Some(&src_rect), Some(&dst_rect));
            } else {
              renderer.copy(&texture, None, Some(&rect));
            }

            // render the debug blue box for the glyph only in the debugger window
            if renderer != &df::renderer::get_sdl_info().renderer() {
              renderer.draw_rect(&rect, 0, 0, 255, 255);
            }
          }

          // advance offset x by the glyph zoomed width after scaling
          cx += w + padding_x;
        }
      }

      // update maximum width to current offset x of the row if larger
      if cx > max_w {
        max_w = cx;
      }
    } // end for each row in the text block

    // render the debug boxes only in the debugger window
    if renderer != &df::renderer::get_sdl_info().renderer() {
      // red: the original text block area
      let rect = sdl::SDL_Rect {
        x: px,
        y: py,
        w: columns as i32 * zoom_size.width,
        h: rows as i32 * zoom_size.height,
      };
      renderer.draw_rect(&rect, 255, 0, 0, 255);

      // yellow: the actual rendered text area
      let rect = sdl::SDL_Rect {
        x: px + min_ox,
        y: py,
        w: max_w,
        h: rows as i32 * zoom_size.height,
      };
      renderer.draw_rect(&rect, 255, 255, 0, 255);
    }
  }
}
