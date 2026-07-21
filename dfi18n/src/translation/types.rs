use std::ffi;

use sha2::{Digest, Sha256};

use crate::{df, types};

// An enum representing different translation input types along with their associated data
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum TranslationInput {
  addst {
    content: String,
  },
  addst_flag {
    content: String,
    flag: u32,
  },
  addcoloredst {
    markup: String,
  },
  visual_addcoloredst {
    markup: String,
    coordinate: types::Coordinate,
  },
  top_addst {
    top_content: String,
  },
  markup_text_box {
    address: usize,
    markup: String,
  },
  visual_text_block {
    content: String,
    coordinate: types::Coordinate,
    color_pair: types::ColorPair,
  },
  dfhack {
    content: String,
    coordinate: types::Coordinate,
    color_pair: types::ColorPair,
    flag: u32,
  },
}

impl TranslationInput {
  // Get the function name associated with the translation input type
  pub fn function(&self) -> &'static str {
    match &self {
      TranslationInput::addst { .. } => "addst",
      TranslationInput::addst_flag { .. } => "addst_flag",
      TranslationInput::addcoloredst { .. } => "addcoloredst",
      TranslationInput::visual_addcoloredst { .. } => "addcoloredst",
      TranslationInput::top_addst { .. } => "top_addst",
      TranslationInput::markup_text_box { .. } => "markup_text_box",
      TranslationInput::visual_text_block { .. } => "visual_text_block",
      TranslationInput::dfhack { .. } => "dfhack",
    }
  }

  // Get the original text content from the translation input
  pub fn original(&self) -> &str {
    match &self {
      TranslationInput::addst { content }
      | TranslationInput::addst_flag { content, .. }
      | TranslationInput::dfhack { content, .. } => content,
      TranslationInput::addcoloredst { markup }
      | TranslationInput::visual_addcoloredst { markup, .. }
      | TranslationInput::markup_text_box { markup, .. } => markup,
      TranslationInput::visual_text_block { content, .. } => content,
      TranslationInput::top_addst { top_content } => top_content,
    }
  }

  // Get the current view screen
  fn view_screen(&self) -> String {
    df::view_screen::get_view_screen()
  }

  // Get the color pair associated with the translation input, if any
  pub fn color_pair(&self) -> Option<types::ColorPair> {
    match &self {
      TranslationInput::addst { .. } | TranslationInput::addst_flag { .. } => Some(df::gps::get_color_pair(false)),
      TranslationInput::top_addst { .. } => Some(df::gps::get_color_pair(true)),
      TranslationInput::addcoloredst { .. }
      | TranslationInput::visual_addcoloredst { .. }
      | TranslationInput::markup_text_box { .. } => None,
      TranslationInput::visual_text_block { color_pair, .. } => Some(color_pair.to_owned()),
      TranslationInput::dfhack { color_pair, .. } => Some(color_pair.to_owned()),
    }
  }

  // Get the flag associated with the translation input, if any
  fn flag(&self) -> Option<u32> {
    match &self {
      TranslationInput::addst_flag { flag, .. } | TranslationInput::dfhack { flag, .. } => Some(*flag),
      _ => None,
    }
  }

  // Generate a unique key for the translation input
  pub fn key(&self) -> String {
    let function = self.function();
    let original = self.original();
    let viewscreen = self.view_screen();
    let color_pair = self.color_pair();
    let flag = self.flag();
    let hash_input = format!("{original:?}/{viewscreen:?}/{color_pair:?}/{flag:?}");

    let sha256 = Sha256::digest(hash_input.as_bytes());
    let key = format!("{function}:{original:?}:{:064x}", base16ct::HexDisplay(&sha256));
    key
  }
}

// An enum representing different translation contexts with associated data
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
#[allow(non_camel_case_types)]
pub enum TranslationContext {
  addst {
    content: String,
    viewscreen: String,
    coordinate: types::Coordinate,
    color_pair: types::ColorPair,
  },
  addst_flag {
    content: String,
    viewscreen: String,
    coordinate: types::Coordinate,
    color_pair: types::ColorPair,
    flag: u32,
  },
  addcoloredst {
    markup: String,
    viewscreen: String,
    coordinate: types::Coordinate,
  },
  top_addst {
    top_content: String,
    viewscreen: String,
    coordinate: types::Coordinate,
    color_pair: types::ColorPair,
  },
  markup_text_box {
    address: usize,
    markup: String,
    viewscreen: String,
    coordinate: types::Coordinate,
  },
  visual_text_block {
    content: String,
    viewscreen: String,
    coordinate: types::Coordinate,
    color_pair: types::ColorPair,
  },
  dfhack {
    content: String,
    viewscreen: String,
    coordinate: types::Coordinate,
    color_pair: types::ColorPair,
    flag: u32,
  },
}

impl TranslationContext {
  // Get the original text content from the translation context
  pub fn original(&self) -> &str {
    match &self {
      TranslationContext::addst { content, .. }
      | TranslationContext::addst_flag { content, .. }
      | TranslationContext::dfhack { content, .. } => content,
      TranslationContext::addcoloredst { markup, .. } | TranslationContext::markup_text_box { markup, .. } => markup,
      TranslationContext::visual_text_block { content, .. } => content,
      TranslationContext::top_addst { top_content, .. } => top_content,
    }
  }

  // Get the flag associated with the translation context, if any
  pub fn flag(&self) -> Option<u32> {
    match &self {
      TranslationContext::addst_flag { flag, .. } | TranslationContext::dfhack { flag, .. } => Some(*flag),
      _ => None,
    }
  }

  // Check if the translation context has the TOP_OF_TEXT flag set
  pub fn has_double_line_height(&self) -> bool {
    if let Some(flag) = self.flag() {
      const TOP_OF_TEXT: u32 = 0b00001000;
      if flag & TOP_OF_TEXT != 0 {
        return true;
      }
    }

    false
  }
}

// A struct representing a translation request
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct TranslationRequest {
  // A unique key identifying the translation request
  // two requests with the same key refer to the same content
  // thus should be translated to the same target text
  key: String,
  // The translation context
  context: TranslationContext,
}

impl TranslationRequest {
  // Create a new TranslationRequest from a TranslationInput
  pub fn new(input: TranslationInput) -> Self {
    let key = input.key();

    // Construct the appropriate TranslationContext based on the input
    let context = match input {
      TranslationInput::addst { content } => TranslationContext::addst {
        content,
        viewscreen: Self::current_view_screen(),
        coordinate: Self::current_coordinate(),
        color_pair: Self::current_color_pair(false),
      },
      TranslationInput::addst_flag { content, flag } => TranslationContext::addst_flag {
        content,
        viewscreen: Self::current_view_screen(),
        coordinate: Self::current_coordinate(),
        color_pair: Self::current_color_pair(false),
        flag,
      },
      TranslationInput::addcoloredst { markup } => TranslationContext::addcoloredst {
        markup,
        viewscreen: Self::current_view_screen(),
        coordinate: Self::current_coordinate(),
      },
      TranslationInput::visual_addcoloredst { markup, coordinate } => TranslationContext::addcoloredst {
        markup,
        viewscreen: Self::current_view_screen(),
        coordinate,
      },
      TranslationInput::top_addst { top_content } => TranslationContext::top_addst {
        top_content,
        viewscreen: Self::current_view_screen(),
        coordinate: Self::current_coordinate(),
        color_pair: Self::current_color_pair(true),
      },
      TranslationInput::markup_text_box { address, markup } => TranslationContext::markup_text_box {
        address,
        markup,
        viewscreen: Self::current_view_screen(),
        coordinate: Self::current_coordinate(),
      },
      TranslationInput::visual_text_block {
        content,
        coordinate,
        color_pair,
      } => TranslationContext::visual_text_block {
        content,
        viewscreen: Self::current_view_screen(),
        coordinate,
        color_pair,
      },
      TranslationInput::dfhack {
        content,
        coordinate,
        color_pair,
        flag,
      } => TranslationContext::dfhack {
        content,
        viewscreen: Self::current_view_screen(),
        coordinate,
        color_pair,
        flag,
      },
    };

    Self { key, context }
  }

  // Get the unique key of the translation request
  pub fn key(&self) -> &str {
    &self.key
  }

  // Get the translation context
  pub fn context(&self) -> &TranslationContext {
    &self.context
  }

  // Get the view screen associated with the translation context, if any
  pub fn view_screen(&self) -> String {
    match self.context() {
      TranslationContext::addst { viewscreen, .. }
      | TranslationContext::addst_flag { viewscreen, .. }
      | TranslationContext::addcoloredst { viewscreen, .. }
      | TranslationContext::top_addst { viewscreen, .. }
      | TranslationContext::markup_text_box { viewscreen, .. }
      | TranslationContext::visual_text_block { viewscreen, .. }
      | TranslationContext::dfhack { viewscreen, .. } => viewscreen.to_owned(),
    }
  }

  // Get the coordinate associated with the translation context, if any
  pub fn coordinate(&self) -> types::Coordinate {
    match self.context() {
      TranslationContext::addst { coordinate, .. }
      | TranslationContext::addst_flag { coordinate, .. }
      | TranslationContext::addcoloredst { coordinate, .. }
      | TranslationContext::top_addst { coordinate, .. }
      | TranslationContext::markup_text_box { coordinate, .. }
      | TranslationContext::visual_text_block { coordinate, .. }
      | TranslationContext::dfhack { coordinate, .. } => coordinate.to_owned(),
    }
  }

  // Get the color pair associated with the translation context, if any
  pub fn color_pair(&self) -> Option<types::ColorPair> {
    match self.context() {
      TranslationContext::addst { color_pair, .. }
      | TranslationContext::addst_flag { color_pair, .. }
      | TranslationContext::top_addst { color_pair, .. }
      | TranslationContext::visual_text_block { color_pair, .. }
      | TranslationContext::dfhack { color_pair, .. } => Some(color_pair.to_owned()),
      TranslationContext::addcoloredst { .. } | TranslationContext::markup_text_box { .. } => None,
    }
  }

  // Check if the translation context is using markup instead of plain text
  pub fn is_markup(&self) -> bool {
    matches!(
      self.context(),
      TranslationContext::addcoloredst { .. } | TranslationContext::markup_text_box { .. }
    )
  }

  // Get the original text from the translation context
  pub fn original(&self) -> &str {
    self.context().original()
  }

  // Get the current view screen
  fn current_view_screen() -> String {
    df::view_screen::get_view_screen()
  }

  // Get the current coordinate
  fn current_coordinate() -> types::Coordinate {
    df::gps::get_coordinate().to_owned()
  }

  // Get the current color pair based on the translation function
  fn current_color_pair(is_top: bool) -> types::ColorPair {
    let mut color_pair = types::ColorPair::from(df::gps::get_color_info());
    let screen_info = df::renderer::get_screen_info();
    let screen = if is_top {
      screen_info.screen_top()
    } else {
      screen_info.screen()
    };

    // From g_src/enabler.cpp: renderer::screen_to_texid()
    let coordinate = df::gps::get_coordinate();
    let dimentions = df::gps::get_dimensions();
    let tile = (coordinate.column * dimentions.height + coordinate.row) as usize;
    let texpos = unsafe { (screen.texpos as *const ffi::c_long).add(tile).read() };
    let stp_flag = unsafe { (screen.texpos_flag as *const u32).add(tile).read() };
    if texpos != 0 {
      const SCREENTEXPOS_FLAG_GRAYSCALE: u32 = 0x1;
      const SCREENTEXPOS_FLAG_ADDCOLOR: u32 = 0x2;
      if stp_flag & SCREENTEXPOS_FLAG_GRAYSCALE != 0 {
        log::warn!("Unhandled grayscale tile in translation");
      } else if stp_flag & SCREENTEXPOS_FLAG_ADDCOLOR != 0 {
        // proceed normally
      } else {
        color_pair.foreground = types::Color { r: 255, g: 255, b: 255 };
        color_pair.background = types::Color { r: 0, g: 0, b: 0 };
      }
    }

    color_pair
  }
}

// An enum representing text alignment options
#[derive(Debug, Clone)]
pub enum TextAlignment {
  Left,
  Center,
  Right,
}

impl Default for TextAlignment {
  // Default text alignment is Left
  fn default() -> Self {
    TextAlignment::Left
  }
}

// A struct representing a translation response
#[derive(Debug, Clone)]
pub struct TranslationResponse {
  // The translated content
  pub translated: String,
  // The text alignment for the translated content
  pub alignment: TextAlignment,
}
