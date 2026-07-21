use std::sync::{Mutex, OnceLock};

use crate::types::{ColorPair, Coordinate};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Rect {
  pub x: i32,
  pub y: i32,
  pub width: i32,
  pub height: i32,
}

#[derive(Debug, Clone)]
pub struct Fragment {
  pub coordinate: Coordinate,
  pub text: String,
  pub color_pair: ColorPair,
}

impl Fragment {
  fn columns(&self) -> i32 {
    self.text.chars().count() as i32
  }

  fn rect(&self) -> Rect {
    Rect {
      x: self.coordinate.column,
      y: self.coordinate.row,
      width: self.columns(),
      height: 1,
    }
  }
}

#[derive(Debug, Clone)]
pub struct VisualTextBlock {
  pub original: String,
  pub source_rect: Rect,
  pub fragments: Vec<Fragment>,
}

impl VisualTextBlock {
  fn new(fragment: Fragment) -> Self {
    Self {
      original: fragment.text.clone(),
      source_rect: fragment.rect(),
      fragments: vec![fragment],
    }
  }

  fn last(&self) -> &Fragment {
    self.fragments.last().unwrap()
  }

  fn append(&mut self, fragment: Fragment, insert_space: bool) {
    if insert_space && !self.original.ends_with(char::is_whitespace) && !fragment.text.starts_with(char::is_whitespace)
    {
      self.original.push(' ');
    }
    self.original.push_str(&fragment.text);
    self.extend_rect(fragment.rect());
    self.fragments.push(fragment);
  }

  fn extend_rect(&mut self, rect: Rect) {
    let right = (self.source_rect.x + self.source_rect.width).max(rect.x + rect.width);
    let bottom = (self.source_rect.y + self.source_rect.height).max(rect.y + rect.height);
    self.source_rect.x = self.source_rect.x.min(rect.x);
    self.source_rect.y = self.source_rect.y.min(rect.y);
    self.source_rect.width = right - self.source_rect.x;
    self.source_rect.height = bottom - self.source_rect.y;
  }

  pub fn origin(&self) -> Coordinate {
    Coordinate {
      column: self.source_rect.x,
      row: self.source_rect.y,
    }
  }

  pub fn columns(&self) -> usize {
    self.source_rect.width.max(1) as usize
  }

  pub fn color_pair(&self) -> ColorPair {
    self.fragments.first().map(|fragment| fragment.color_pair).unwrap_or_default()
  }

  pub fn clear_rects(&self) -> Vec<Rect> {
    let mut rects: Vec<Rect> = Vec::new();
    for fragment in &self.fragments {
      let rect = fragment.rect();
      if let Some(last) = rects.last_mut()
        && last.y == rect.y
        && rect.x <= last.x + last.width + 1
      {
        last.width = (last.x + last.width).max(rect.x + rect.width) - last.x;
      } else {
        rects.push(rect);
      }
    }
    rects
  }
}

fn ends_complete_sentence_or_label(text: &str) -> bool {
  text.trim_end().ends_with(['.', '!', '?', ':'])
}

fn begins_with_lowercase_text(text: &str) -> bool {
  text.chars().find(|character| character.is_alphabetic()).is_some_and(char::is_lowercase)
}

#[derive(Debug, Default)]
struct Collector {
  fragments: Vec<Fragment>,
}

impl Collector {
  fn push(&mut self, fragment: Fragment) {
    self.fragments.push(fragment);
  }

  fn finish(self) -> Vec<VisualTextBlock> {
    let mut blocks: Vec<VisualTextBlock> = Vec::new();
    for fragment in self.fragments {
      let Some(block) = blocks.last_mut() else {
        blocks.push(VisualTextBlock::new(fragment));
        continue;
      };
      let last = block.last();
      let same_color = fragment.color_pair == last.color_pair;
      let horizontal_gap = fragment.coordinate.column - (last.coordinate.column + last.columns());
      let horizontal =
        same_color && fragment.coordinate.row == last.coordinate.row && (horizontal_gap == 0 || horizontal_gap == 1);
      if horizontal {
        block.append(fragment, horizontal_gap == 1);
        continue;
      }

      let wrapped = same_color
        && fragment.coordinate.row == last.coordinate.row + 1
        && (fragment.coordinate.column < last.coordinate.column
          || (fragment.coordinate.column == last.coordinate.column
            && (block.original.ends_with(char::is_whitespace)
              || (!ends_complete_sentence_or_label(&last.text) && begins_with_lowercase_text(&fragment.text)))));
      if wrapped {
        block.append(fragment, true);
        continue;
      }

      blocks.push(VisualTextBlock::new(fragment));
    }
    blocks
  }
}

static ACTIVE_COLLECTOR: OnceLock<Mutex<Option<Collector>>> = OnceLock::new();

fn active_collector() -> &'static Mutex<Option<Collector>> {
  ACTIVE_COLLECTOR.get_or_init(|| Mutex::new(None))
}

pub fn begin_frame() {
  *active_collector().lock().unwrap() = Some(Collector::default());
}

pub fn is_collecting() -> bool {
  active_collector().lock().unwrap().is_some()
}

pub fn push(fragment: Fragment) -> bool {
  let mut active = active_collector().lock().unwrap();
  let Some(collector) = active.as_mut() else {
    return false;
  };
  collector.push(fragment);
  true
}

pub fn finish_frame() -> Vec<VisualTextBlock> {
  active_collector().lock().unwrap().take().map(Collector::finish).unwrap_or_default()
}

pub fn cancel_frame() {
  active_collector().lock().unwrap().take();
}

pub fn reset() {
  cancel_frame();
}

#[cfg(test)]
mod tests {
  use super::*;

  fn fragment(x: i32, y: i32, text: &str) -> Fragment {
    Fragment {
      coordinate: Coordinate { column: x, row: y },
      text: text.to_owned(),
      color_pair: ColorPair::default(),
    }
  }

  #[test]
  fn uses_the_complete_bounding_rectangle_for_layout() {
    let mut collector = Collector::default();
    collector.push(fragment(20, 10, "First line"));
    collector.push(fragment(10, 11, "second line"));

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].origin(), Coordinate { column: 10, row: 10 });
    assert_eq!(blocks[0].columns(), 20);
    assert_eq!(
      blocks[0].source_rect,
      Rect {
        x: 10,
        y: 10,
        width: 20,
        height: 2,
      }
    );
  }

  #[test]
  fn keeps_precise_per_row_clear_rectangles() {
    let mut collector = Collector::default();
    collector.push(fragment(20, 10, "First line"));
    collector.push(fragment(10, 11, "second line"));

    let blocks = collector.finish();
    assert_eq!(
      blocks[0].clear_rects(),
      vec![
        Rect {
          x: 20,
          y: 10,
          width: 10,
          height: 1,
        },
        Rect {
          x: 10,
          y: 11,
          width: 11,
          height: 1,
        },
      ]
    );
  }
}
