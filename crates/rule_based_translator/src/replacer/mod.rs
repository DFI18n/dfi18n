use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::*;

mod item_designation;
mod number;
mod wildcard;

pub const MAX_WILDCARD_CANDIDATES: usize = 64;
pub const MAX_WILDCARD_CAPTURE_BYTES: usize = 4096;

#[derive(Clone, Copy, Default)]
pub struct ReplacerMatchHint<'a> {
  pub next_literal: Option<&'a str>,
  pub terminal: bool,
}

// Replacer trait for programmatically match and replace
pub trait Replacer {
  fn replace(
    &self,
    context: &mut Context,
    identifier: &str,
    base_namespace: &str,
    config: &str,
    text: &str,
    translator: &Translator,
    level: usize,
    hint: ReplacerMatchHint<'_>,
  ) -> Vec<ResultTree>;
}

// All registered replacers
static REPLACERS: OnceLock<RwLock<IndexMap<String, Box<dyn Replacer + Sync + Send>>>> = OnceLock::new();

// Getting access to the registered replacers
pub fn get_replacers() -> RwLockReadGuard<'static, IndexMap<String, Box<dyn Replacer + Sync + Send>>> {
  REPLACERS.get_or_init(|| RwLock::new(IndexMap::new())).read().unwrap()
}

// Getting mutable access to the registered replacers
pub fn get_replacers_mut() -> RwLockWriteGuard<'static, IndexMap<String, Box<dyn Replacer + Sync + Send>>> {
  REPLACERS.get_or_init(|| RwLock::new(IndexMap::new())).write().unwrap()
}

// Register a new replacer
pub fn register_replacer(name: &str, replacer: Box<dyn Replacer + Sync + Send>) {
  let mut replacers = get_replacers_mut();
  replacers.insert(name.to_owned(), replacer);
}

// Register default replacers
pub fn register_default_replacers() {
  register_replacer(
    "item_designation",
    Box::new(item_designation::ItemDesignationReplacer::default()),
  );
  register_replacer("number", Box::new(number::NumberReplacer::default()));
  register_replacer("word", Box::new(wildcard::WordReplacer::default()));
  register_replacer("any", Box::new(wildcard::AnyReplacer::default()));
}
