use std::collections::HashMap;
use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use rule_based_translator::Translator;

use crate::translation;

// Reset the rulesets translators
pub fn reset() {
  get_translators_mut().clear();
}

// Translate text based on the provided language tag and context
pub fn translate(
  lang_tag: &str,
  context: &translation::TranslationContext,
) -> Option<translation::TranslationResponse> {
  let translators = get_translators();
  let translator = translators.get(lang_tag)?;
  let text = context.original();

  // Prefer the exact text, including any color markup.
  if let Some(translated) = translator.translate(text) {
    return Some(translation::TranslationResponse {
      translated,
      alignment: translation::TextAlignment::default(),
    });
  }

  // addcoloredst strings carry a runtime color prefix ([C:r:g:b]) that the
  // ruleset rules (written against plain composed text) do not contain, so
  // ruleset coverage silently missed them and they went to the realtime API.
  // Reuse the plain body and restore the color prefix around the translation.
  if let Some((prefix_end, body)) = single_color_prefix(text) {
    if let Some(translated) = translator.translate(body) {
      let mut restored = String::with_capacity(prefix_end + translated.len());
      restored.push_str(&text[..prefix_end]);
      restored.push_str(&translated);
      return Some(translation::TranslationResponse {
        translated: restored,
        alignment: translation::TextAlignment::default(),
      });
    }
  }

  None
}

// Returns the byte offset just past a single leading [C:r:g:b] color tag and
// the remaining plain body, if the whole string has exactly one such prefix.
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

// A global registry of translators categorized by type and language tag
static TRANSLATORS: OnceLock<RwLock<HashMap<String, Translator>>> = OnceLock::new();

// Getting access to the translators registry
fn get_translators() -> RwLockReadGuard<'static, HashMap<String, Translator>> {
  TRANSLATORS.get_or_init(|| RwLock::new(HashMap::new())).read().unwrap()
}

// Getting mutable access to the translators registry
fn get_translators_mut() -> RwLockWriteGuard<'static, HashMap<String, Translator>> {
  TRANSLATORS.get_or_init(|| RwLock::new(HashMap::new())).write().unwrap()
}

// Load rulesets from a directory for a specific language
#[unsafe(no_mangle)]
extern "C" fn load_translation_rulesets(lua_state: *mut std::ffi::c_void) {
  let lang_tag = lua53_sys::check_string(lua_state, 1);
  let path = lua53_sys::check_string(lua_state, 2);

  let mut translators = get_translators_mut();
  let translator = translators.entry(lang_tag.to_owned()).or_insert_with(Translator::default);

  let _ = translator.load_from_dir(&path);

  // TODO: return load result to Lua
  log::info!("Loaded Rulesets translator for language {lang_tag:?} from {path:?}");
}
