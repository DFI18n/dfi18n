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
  rows: Vec<VisualRow>,
}

impl VisualTextBlock {
  fn new(row: VisualRow) -> Self {
    Self {
      original: row.text.clone(),
      source_rect: row.source_rect,
      fragments: row.fragments.clone(),
      rows: vec![row],
    }
  }

  fn append_row(&mut self, row: VisualRow) {
    if !self.original.ends_with(char::is_whitespace) && !row.text.starts_with(char::is_whitespace) {
      self.original.push(' ');
    }
    self.original.push_str(&row.text);
    self.extend_rect(row.source_rect);
    self.fragments.extend(row.fragments.iter().cloned());
    self.rows.push(row);
  }

  fn extend_rect(&mut self, rect: Rect) {
    let right = (self.source_rect.x + self.source_rect.width).max(rect.x + rect.width);
    let bottom = (self.source_rect.y + self.source_rect.height).max(rect.y + rect.height);
    self.source_rect.x = self.source_rect.x.min(rect.x);
    self.source_rect.y = self.source_rect.y.min(rect.y);
    self.source_rect.width = right - self.source_rect.x;
    self.source_rect.height = bottom - self.source_rect.y;
  }

  pub fn into_sentences(self) -> Vec<VisualSentence> {
    let mut sentences = Vec::new();
    let mut pending = Vec::new();
    for row in self.rows {
      let complete = ends_complete_sentence(&row.text);
      pending.push(row);
      if complete {
        sentences.push(VisualSentence::from_rows(std::mem::take(&mut pending), true));
      }
    }
    if !pending.is_empty() {
      // No confident punctuation boundary means the remaining visual region is
      // deliberately kept as one large sentence instead of being discarded.
      sentences.push(VisualSentence::from_rows(pending, false));
    }
    sentences
  }
}

#[derive(Debug, Clone)]
struct VisualRow {
  text: String,
  source_rect: Rect,
  fragments: Vec<Fragment>,
}

#[derive(Debug, Clone)]
pub struct VisualSentence {
  pub original: String,
  pub source_rect: Rect,
  pub fragments: Vec<Fragment>,
  complete: bool,
}

impl VisualSentence {
  fn from_rows(rows: Vec<VisualRow>, complete: bool) -> Self {
    let first = rows.first().expect("a visual sentence needs at least one row");
    let mut original = String::new();
    let mut source_rect = first.source_rect;
    let mut fragments = Vec::new();
    for row in rows {
      if !original.is_empty() && !original.ends_with(char::is_whitespace) && !row.text.starts_with(char::is_whitespace)
      {
        original.push(' ');
      }
      original.push_str(&row.text);
      source_rect = union_rect(source_rect, row.source_rect);
      fragments.extend(row.fragments);
    }
    Self {
      original,
      source_rect,
      fragments,
      complete,
    }
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

  pub fn is_complete(&self) -> bool {
    self.complete
  }
}

fn union_rect(left: Rect, right: Rect) -> Rect {
  let x = left.x.min(right.x);
  let y = left.y.min(right.y);
  let far_right = (left.x + left.width).max(right.x + right.width);
  let bottom = (left.y + left.height).max(right.y + right.height);
  Rect {
    x,
    y,
    width: far_right - x,
    height: bottom - y,
  }
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
  let text = text.trim_end();
  !text.ends_with("...") && text.ends_with(['.', '!', '?'])
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
        current = Some(VisualTextBlock::new(row));
        continue;
      };
      let previous = block.rows.last().unwrap();
      let vertically_adjacent = row.source_rect.y == previous.source_rect.y + previous.source_rect.height;
      let horizontally_related = row.source_rect.x <= previous.source_rect.x + previous.source_rect.width
        && previous.source_rect.x <= row.source_rect.x + row.source_rect.width;
      if vertically_adjacent && horizontally_related {
        block.append_row(row);
      } else {
        blocks.push(current.take().unwrap());
        current = Some(VisualTextBlock::new(row));
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
  fn gives_a_multiline_sentence_one_bounding_rectangle() {
    let mut collector = Collector::default();
    collector.push(fragment(20, 10, "First line"));
    collector.push(fragment(10, 11, "second line"));

    let sentence = collector.finish().pop().unwrap().into_sentences().pop().unwrap();
    assert_eq!(
      sentence.source_rect,
      Rect {
        x: 10,
        y: 10,
        width: 20,
        height: 2,
      }
    );
  }

  #[test]
  fn waits_for_the_end_of_a_complete_row_before_finishing_a_sentence() {
    let mut collector = Collector::default();
    collector.push(fragment(2, 3, "First."));
    collector.push(fragment(9, 3, "Still here!"));

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 1);
    let sentences = blocks.into_iter().next().unwrap().into_sentences();
    assert_eq!(sentences.len(), 1);
    assert_eq!(sentences[0].original, "First. Still here!");
    assert!(sentences[0].is_complete());
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
    let sentences = blocks.into_iter().next().unwrap().into_sentences();
    assert_eq!(sentences.len(), 1);
    assert_eq!(sentences[0].original, "Ordinary text. Emphasized ending!");
    assert!(sentences[0].is_complete());
  }

  #[test]
  fn separates_complete_sentences_on_adjacent_rows() {
    let mut collector = Collector::default();
    collector.push(fragment(2, 3, "First sentence."));
    collector.push(fragment(2, 4, "Second sentence."));

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].original, "First sentence. Second sentence.");
    let sentences = blocks.into_iter().next().unwrap().into_sentences();
    assert_eq!(sentences.len(), 2);
    assert_eq!(sentences[0].original, "First sentence.");
    assert_eq!(sentences[1].original, "Second sentence.");
    assert!(sentences.iter().all(VisualSentence::is_complete));
  }

  #[test]
  fn combines_wrapped_rows_into_one_sentence() {
    let mut collector = Collector::default();
    collector.push(fragment(20, 10, "This sentence wraps"));
    collector.push(fragment(10, 11, "onto another row."));

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 1);
    let sentences = blocks.into_iter().next().unwrap().into_sentences();
    assert_eq!(sentences.len(), 1);
    assert_eq!(sentences[0].original, "This sentence wraps onto another row.");
    assert!(sentences[0].is_complete());
  }

  #[test]
  fn keeps_non_sentence_labels_translatable_but_unmarked() {
    let mut collector = Collector::default();
    collector.push(fragment(2, 3, "Health"));

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 1);
    let sentences = blocks.into_iter().next().unwrap().into_sentences();
    assert_eq!(sentences.len(), 1);
    assert_eq!(sentences[0].original, "Health");
    assert!(!sentences[0].is_complete());
  }

  #[test]
  fn treats_an_ellipsis_as_a_visual_box_fallback_not_a_complete_sentence() {
    let mut collector = Collector::default();
    collector.push(fragment(2, 3, "Loading..."));

    let sentence = collector.finish().pop().unwrap().into_sentences().pop().unwrap();
    assert_eq!(sentence.original, "Loading...");
    assert!(!sentence.is_complete());
  }

  #[test]
  fn keeps_multiple_semantic_sentences_together_when_their_x_axis_is_contiguous() {
    let mut collector = Collector::default();
    collector.push(fragment(2, 3, "She is calm. She has"));
    collector.push(fragment(2, 4, "good spatial sense."));

    let blocks = collector.finish();
    assert_eq!(blocks.len(), 1);
    let sentences = blocks.into_iter().next().unwrap().into_sentences();
    assert_eq!(sentences.len(), 1);
    assert_eq!(sentences[0].original, "She is calm. She has good spatial sense.");
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
