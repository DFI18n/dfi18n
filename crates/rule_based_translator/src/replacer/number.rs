use crate::*;

use super::Replacer;

// Replacer for numbers
#[derive(Default)]
pub struct NumberReplacer {}

impl Replacer for NumberReplacer {
  fn replace(
    &self,
    _context: &mut Context,
    identifier: &str,
    _base_namespace: &str,
    _config: &str,
    text: &str,
    _translator: &Translator,
    _level: usize,
    _hint: ReplacerMatchHint<'_>,
  ) -> Vec<ResultTree> {
    let mut results = Vec::new();
    let mut matched = String::new();
    // while the text starts with digits
    for ch in text.chars() {
      if !ch.is_ascii_digit() {
        break;
      }

      // append the digit to the matched string
      matched.push(ch);

      // construct a result tree for this numeric prefix
      let identifier = identifier.to_owned();
      let original = format!("{{{identifier}}}");
      let matched = matched.clone();
      let translated = matched.clone();
      let remaining = text[matched.len()..].to_owned();
      let children = IndexMap::new();
      results.push(ResultTree::new(
        identifier,
        original,
        matched,
        translated,
        remaining,
        children,
        Vec::new(),
        Vec::new(),
      ));
    }
    results
  }
}
