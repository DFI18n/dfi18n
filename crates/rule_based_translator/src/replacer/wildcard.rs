use crate::*;

use super::{MAX_WILDCARD_CANDIDATES, MAX_WILDCARD_CAPTURE_BYTES, Replacer, ReplacerMatchHint};

#[derive(Default)]
pub struct WordReplacer {}

#[derive(Default)]
pub struct AnyReplacer {}

fn result(identifier: &str, matched: &str, remaining: &str) -> ResultTree {
  ResultTree::new(
    identifier.to_owned(),
    format!("{{{identifier}}}"),
    matched.to_owned(),
    matched.to_owned(),
    remaining.to_owned(),
    IndexMap::new(),
    Vec::new(),
    Vec::new(),
  )
}

fn starts_with_case_insensitive(text: &str, literal: &str) -> bool {
  text.get(..literal.len()).is_some_and(|prefix| prefix.eq_ignore_ascii_case(literal))
    || text.to_lowercase().starts_with(&literal.to_lowercase())
}

fn line_end(text: &str) -> usize {
  text.char_indices().find_map(|(index, ch)| matches!(ch, '\r' | '\n').then_some(index)).unwrap_or(text.len())
}

impl Replacer for WordReplacer {
  fn replace(
    &self,
    _context: &mut Context,
    identifier: &str,
    _base_namespace: &str,
    _config: &str,
    text: &str,
    _translator: &Translator,
    _level: usize,
    hint: ReplacerMatchHint<'_>,
  ) -> Vec<ResultTree> {
    let end = text
      .char_indices()
      .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
      .unwrap_or(text.len())
      .min(MAX_WILDCARD_CAPTURE_BYTES);
    if end == 0 || !text.is_char_boundary(end) {
      return Vec::new();
    }

    if let Some(literal) = hint.next_literal {
      let boundaries =
        text[..end].char_indices().skip(1).map(|(index, _)| index).chain(std::iter::once(end)).collect::<Vec<_>>();
      return boundaries
        .into_iter()
        .rev()
        .filter(|index| starts_with_case_insensitive(&text[*index..], literal))
        .take(MAX_WILDCARD_CANDIDATES)
        .map(|index| result(identifier, &text[..index], &text[index..]))
        .collect();
    }

    vec![result(identifier, &text[..end], &text[end..])]
  }
}

impl Replacer for AnyReplacer {
  fn replace(
    &self,
    _context: &mut Context,
    identifier: &str,
    _base_namespace: &str,
    _config: &str,
    text: &str,
    _translator: &Translator,
    _level: usize,
    hint: ReplacerMatchHint<'_>,
  ) -> Vec<ResultTree> {
    let end = line_end(text).min(MAX_WILDCARD_CAPTURE_BYTES);
    if end == 0 || !text.is_char_boundary(end) {
      return Vec::new();
    }

    if hint.terminal {
      return vec![result(identifier, &text[..end], &text[end..])];
    }

    let Some(literal) = hint.next_literal else {
      return Vec::new();
    };
    let boundaries =
      text[..end].char_indices().skip(1).map(|(index, _)| index).chain(std::iter::once(end)).collect::<Vec<_>>();
    boundaries
      .into_iter()
      .rev()
      .filter(|index| starts_with_case_insensitive(&text[*index..], literal))
      .take(MAX_WILDCARD_CANDIDATES)
      .map(|index| result(identifier, &text[..index], &text[index..]))
      .collect()
  }
}
