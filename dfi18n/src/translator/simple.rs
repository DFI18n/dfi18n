use std::collections::HashMap;
use std::fs::File;
use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use anyhow::Result;

use lua53_sys as lua;

use crate::translation;

// Reset the simple translators
pub fn reset() {
  get_dicts_mut().clear();
}

// Simple dictionary maps original text to translated text along with tags
type SimpleDictionary = HashMap<String, (String, HashMap<String, String>)>;

// A collection of simple dictionaries grouped by language tag
type SimpleDictionaries = HashMap<String, SimpleDictionary>;

// Global storage for simple dictionaries
static DICTS: OnceLock<RwLock<SimpleDictionaries>> = OnceLock::new();

// Getting access to the dictionaries
fn get_dicts() -> RwLockReadGuard<'static, SimpleDictionaries> {
  DICTS.get_or_init(|| RwLock::new(SimpleDictionaries::new())).read().unwrap()
}

// Getting mutable access to the dictionaries
fn get_dicts_mut() -> RwLockWriteGuard<'static, SimpleDictionaries> {
  DICTS.get_or_init(|| RwLock::new(SimpleDictionaries::new())).write().unwrap()
}

// Translate text based on the provided language tag and context.
pub fn translate(
  lang_tag: &str,
  context: &translation::TranslationContext,
) -> Option<translation::TranslationResponse> {
  let text = context.original();
  let dicts = get_dicts();
  let dict = dicts.get(lang_tag)?;

  // Prefer the exact key, including markup and color transitions.
  if let Some((translated, tags)) = dict.get(text) {
    return Some(translation::TranslationResponse {
      translated: translated.to_owned(),
      alignment: alignment_from_tags(tags),
    });
  }

  // Captured addcoloredst strings often have one color prefix while the data
  // dictionary contains the same sentence without that runtime-only prefix.
  // Reuse that ordinary entry and restore the color prefix. Do not attempt
  // multi-color markup here: exact markup entries above preserve those colors.
  if let Some((prefix_end, body)) = single_color_prefix(text) {
    if let Some((translated, tags)) = dict.get(body) {
      let mut restored = String::with_capacity(prefix_end + translated.len());
      restored.push_str(&text[..prefix_end]);
      restored.push_str(translated);
      return Some(translation::TranslationResponse {
        translated: restored,
        alignment: alignment_from_tags(tags),
      });
    }
  }

  None
}

fn alignment_from_tags(tags: &HashMap<String, String>) -> translation::TextAlignment {
  match tags.get("ALIGNMENT").map(|s| s.as_str()) {
    Some("LEFT") => translation::TextAlignment::Left,
    Some("RIGHT") => translation::TextAlignment::Right,
    Some("CENTER") => translation::TextAlignment::Center,
    _ => translation::TextAlignment::Left,
  }
}

fn single_color_prefix(text: &str) -> Option<(usize, &str)> {
  if !text.starts_with("[C:") {
    return None;
  }
  let end = text.find(']')? + 1;
  let body = &text[end..];
  if body.contains("[C:") {
    return None;
  }
  Some((end, body))
}

// Check whether a string is present in the simple dictionary for a lang tag
pub fn contains(lang_tag: &str, text: &str) -> bool {
  let dicts = get_dicts();
  dicts.get(lang_tag).map(|d| d.contains_key(text)).unwrap_or(false)
}

// Insert or replace a translation into the simple dictionary for a lang tag
pub fn insert_translation(lang_tag: &str, text: &str, translation: &str) {
  let mut dicts = get_dicts_mut();
  let dict = dicts.entry(lang_tag.to_string()).or_insert_with(SimpleDictionary::new);
  dict.insert(text.to_string(), (translation.to_string(), HashMap::new()));
}

// Load a simple dictionary from a CSV file into the global storage
#[unsafe(no_mangle)]
extern "C" fn load_simple_dict(lua_state: *mut std::ffi::c_void) {
  let lang_tag = lua::check_string(lua_state, 1);
  let path_str = lua::check_string(lua_state, 2);

  let mut dicts = get_dicts_mut();
  let dict = dicts.entry(lang_tag.to_string()).or_insert_with(SimpleDictionary::new);
  if let Err(err) = load_csv(
    &path_str,
    |Entry {
       text,
       translation,
       tags,
     }| {
      dict.insert(text, (translation, parse_tags(&tags)));
    },
  ) {
    log::warn!("Failed to load simple translator data from {path_str:?}: {err}");
    // TODO: consider returning error message to Lua
    return;
  };

  log::info!("Loaded Simple translator data for language {lang_tag:?} from {path_str:?}");
}

// CSV entry
#[derive(Debug, serde::Deserialize)]
struct Entry {
  // Original text
  text: String,
  // Translated text
  translation: String,
  // Tags
  tags: String,
}

// Load CSV file and process each entry with the provided function
fn load_csv<T: serde::de::DeserializeOwned, P: AsRef<std::path::Path>, F>(path: P, mut f: F) -> Result<()>
where
  F: FnMut(T),
{
  for entry in csv::Reader::from_reader(File::open(path)?).deserialize::<T>() {
    f(entry?);
  }

  Ok(())
}

static DF_TAG_REGEX: OnceLock<regex::Regex> = OnceLock::new();

// Get the regex for DF txt tags
// TODO: move to `utils` module
fn get_df_tag_regex() -> &'static regex::Regex {
  DF_TAG_REGEX.get_or_init(|| regex::Regex::new(r"\[([^\[:]+):([^:\]]+)\]").unwrap())
}

// Parse tags in DF txt format [KEY:VALUE]
fn parse_tags(tags_str: &str) -> HashMap<String, String> {
  let mut tags = HashMap::new();
  let regex = get_df_tag_regex();
  for cap in regex.captures_iter(tags_str) {
    if let (Some(key), Some(value)) = (cap.get(1), cap.get(2)) {
      tags.insert(key.as_str().to_owned(), value.as_str().to_owned());
    }
  }
  tags
}
