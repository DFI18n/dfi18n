use std::{
  collections::HashSet,
  fs::{self, File, OpenOptions},
  path::{Path, PathBuf},
  sync::{Mutex, OnceLock},
  time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;

use crate::DATA_DIRECTORY;
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
  is_sentence: bool,
}

impl VisualTextBlock {
  fn new(row: VisualRow) -> Self {
    Self {
      original: row.text,
      source_rect: row.source_rect,
      fragments: row.fragments,
      is_sentence: false,
    }
  }

  fn last(&self) -> &Fragment {
    self.fragments.last().unwrap()
  }

  fn append_row(&mut self, row: VisualRow) {
    if !self.original.ends_with(char::is_whitespace) && !row.text.starts_with(char::is_whitespace) {
      self.original.push(' ');
    }
    self.original.push_str(&row.text);
    self.extend_rect(row.source_rect);
    self.fragments.extend(row.fragments);
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

  pub fn is_sentence(&self) -> bool {
    self.is_sentence
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

#[derive(Debug)]
struct VisualRow {
  text: String,
  source_rect: Rect,
  fragments: Vec<Fragment>,
}

impl VisualRow {
  fn new(fragment: Fragment) -> Self {
    Self {
      text: fragment.text.clone(),
      source_rect: fragment.rect(),
      fragments: vec![fragment],
    }
  }

  fn last(&self) -> &Fragment {
    self.fragments.last().unwrap()
  }

  fn append(&mut self, fragment: Fragment, insert_space: bool) {
    if insert_space && !self.text.ends_with(char::is_whitespace) && !fragment.text.starts_with(char::is_whitespace) {
      self.text.push(' ');
    }
    self.text.push_str(&fragment.text);
    let right = (self.source_rect.x + self.source_rect.width).max(fragment.coordinate.column + fragment.columns());
    self.source_rect.width = right - self.source_rect.x;
    self.fragments.push(fragment);
  }
}

fn ends_complete_sentence(text: &str) -> bool {
  text.trim_end().ends_with(['.', '!', '?'])
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
    let mut rows: Vec<VisualRow> = Vec::new();
    for fragment in self.fragments {
      let Some(row) = rows.last_mut() else {
        rows.push(VisualRow::new(fragment));
        continue;
      };
      let last = row.last();
      let horizontal_gap = fragment.coordinate.column - (last.coordinate.column + last.columns());
      if fragment.coordinate.row == last.coordinate.row && (horizontal_gap == 0 || horizontal_gap == 1) {
        row.append(fragment, horizontal_gap == 1);
      } else {
        rows.push(VisualRow::new(fragment));
      }
    }

    let mut blocks: Vec<VisualTextBlock> = Vec::new();
    let mut current: Option<VisualTextBlock> = None;
    for row in rows {
      let Some(block) = current.as_mut() else {
        let complete = ends_complete_sentence(&row.text);
        current = Some(VisualTextBlock::new(row));
        if complete {
          let mut block = current.take().unwrap();
          block.is_sentence = true;
          blocks.push(block);
        }
        continue;
      };
      let last = block.last();
      let first = row.fragments.first().unwrap();
      let wrapped = first.coordinate.row == last.coordinate.row + 1
        && (first.coordinate.column < last.coordinate.column
          || (first.coordinate.column == last.coordinate.column
            && (block.original.ends_with(char::is_whitespace)
              || (!ends_complete_sentence(&last.text) && begins_with_lowercase_text(&row.text)))));
      if wrapped {
        let complete = ends_complete_sentence(&row.text);
        block.append_row(row);
        if complete {
          let mut block = current.take().unwrap();
          block.is_sentence = true;
          blocks.push(block);
        }
      } else {
        blocks.push(current.take().unwrap());
        let complete = ends_complete_sentence(&row.text);
        current = Some(VisualTextBlock::new(row));
        if complete {
          let mut block = current.take().unwrap();
          block.is_sentence = true;
          blocks.push(block);
        }
      }
    }
    blocks.extend(current);
    blocks
  }
}

struct SentenceRecorder {
  seen: HashSet<String>,
  writer: csv::Writer<File>,
}

impl SentenceRecorder {
  fn open(path: &Path) -> Result<Self> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let mut seen = HashSet::new();
    if path.exists() {
      let mut reader = csv::Reader::from_path(path)?;
      for record in reader.records().flatten() {
        if let Some(text) = record.get(0) {
          seen.insert(text.to_owned());
        }
      }
    }
    let is_new = !path.exists() || path.metadata()?.len() == 0;
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut writer = csv::WriterBuilder::new().has_headers(false).from_writer(file);
    if is_new {
      writer.write_record(["text", "context", "first_seen"])?;
      writer.flush()?;
    }
    Ok(Self { seen, writer })
  }

  fn record(&mut self, text: &str, context: &str) -> Result<()> {
    if !self.seen.insert(text.to_owned()) {
      return Ok(());
    }
    let first_seen = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().to_string();
    self.writer.write_record([text, context, &first_seen])?;
    self.writer.flush()?;
    Ok(())
  }
}

static SENTENCE_RECORDER: OnceLock<Mutex<Option<SentenceRecorder>>> = OnceLock::new();

fn sentence_path() -> PathBuf {
  Path::new(DATA_DIRECTORY).join("logs").join("sentences.csv")
}

pub fn record_sentence(text: &str, context: &str) {
  let recorder = SENTENCE_RECORDER.get_or_init(|| match SentenceRecorder::open(&sentence_path()) {
    Ok(recorder) => Mutex::new(Some(recorder)),
    Err(error) => {
      log::error!("failed to open sentence log: {error:#}");
      Mutex::new(None)
    }
  });
  let mut recorder = recorder.lock().unwrap();
  if let Some(recorder) = recorder.as_mut()
    && let Err(error) = recorder.record(text, context)
  {
    log::error!("failed to record sentence: {error:#}");
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

  #[test]
  fn waits_for_the_end_of_a_complete_row_before_finishing_a_sentence() {
    let mut collector = Collector::default();
    collector.push(fragment(2, 3, "First."));
    collector.push(fragment(9, 3, "Still here!"));

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].original, "First. Still here!");
    assert!(blocks[0].is_sentence());
  }

  #[test]
  fn includes_a_differently_colored_fragment_on_the_same_row() {
    let mut collector = Collector::default();
    collector.push(fragment(2, 3, "Ordinary text."));
    let mut emphasized = fragment(17, 3, "Emphasized ending!");
    emphasized.color_pair.foreground = crate::types::Color { r: 7, g: 8, b: 9 };
    collector.push(emphasized);

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].original, "Ordinary text. Emphasized ending!");
    assert!(blocks[0].is_sentence());
  }

  #[test]
  fn separates_complete_sentences_on_adjacent_rows() {
    let mut collector = Collector::default();
    collector.push(fragment(2, 3, "First sentence."));
    collector.push(fragment(2, 4, "Second sentence."));

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].original, "First sentence.");
    assert_eq!(blocks[1].original, "Second sentence.");
    assert!(blocks.iter().all(VisualTextBlock::is_sentence));
  }

  #[test]
  fn combines_wrapped_rows_into_one_sentence() {
    let mut collector = Collector::default();
    collector.push(fragment(20, 10, "This sentence wraps"));
    collector.push(fragment(10, 11, "onto another row."));

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].original, "This sentence wraps onto another row.");
    assert!(blocks[0].is_sentence());
  }

  #[test]
  fn keeps_non_sentence_labels_translatable_but_unmarked() {
    let mut collector = Collector::default();
    collector.push(fragment(2, 3, "Health"));

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 1);
    assert!(!blocks[0].is_sentence());
  }

  #[test]
  fn records_each_sentence_only_once() {
    let path = std::env::temp_dir().join(format!("dfi18n-sentences-{}.csv", std::process::id()));
    let _ = fs::remove_file(&path);
    let mut recorder = SentenceRecorder::open(&path).unwrap();
    recorder.record("A complete sentence.", "viewscreen_a").unwrap();
    recorder.record("A complete sentence.", "viewscreen_b").unwrap();
    drop(recorder);

    let records = csv::Reader::from_path(&path).unwrap().records().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(&records[0][0], "A complete sentence.");
    assert_eq!(&records[0][1], "viewscreen_a");
    fs::remove_file(path).unwrap();
  }
}
