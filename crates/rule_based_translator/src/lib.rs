use anyhow::{Context as _, Result, anyhow};
use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap};
use std::{fs, path, sync::OnceLock};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

mod replacer;
pub use replacer::*;
mod preference;
pub use preference::*;

const MAX_CAPTURE_DEPTH: usize = 4;

// A rule-based translator
#[derive(Debug, Default)]
pub struct Translator {
  // All rulesets loaded
  rulesets: RuleSets,
}

impl Translator {
  // Load rulesets from a directory
  pub fn load_from_dir(&mut self, path: impl AsRef<path::Path>) -> Result<()> {
    let base = path.as_ref().to_path_buf();
    let mut visited_root: Option<String> = None;
    self.parse_dir(&base, path, &mut visited_root)?;
    self.validate_references()?;

    Ok(())
  }

  // Translate text using the rulesets, returning the best match if any
  pub fn translate(&self, text: &str) -> Option<String> {
    let results = self.get_all_translations(text, false);
    results.into_iter().min_by_key(ResultTree::specificity).map(|result| result.translated)
  }

  // Get all translation results for the given text, for debugging purposes
  pub fn get_all_translations(&self, text: &str, partial_match: bool) -> Vec<ResultTree> {
    let mut context = Context::default();
    let results = self.do_translate(&mut context, text, "::", 0);
    let mut results = if partial_match {
      results
    } else {
      results.into_iter().filter(|result| result.remaining.is_empty()).collect()
    };
    for result in results.iter_mut().filter(|result| result.remaining.is_empty()) {
      let mut capture_path = vec![result.matched.clone()];
      *result = self.resolve_captures(&mut context, result.clone(), 0, &mut capture_path);
    }
    results
  }

  // Internal recursive function to translate text using a specific ruleset
  fn do_translate(&self, context: &mut Context, text: &str, identifier: &str, level: usize) -> Vec<ResultTree> {
    let indent = "  ".repeat(level);

    // handle replacer references
    if identifier.starts_with("%") {
      return self.do_replace(context, text, identifier, level, ReplacerMatchHint::default());
    }

    // track the identifier path to detect cycles (do not track replacer references)
    context.identifier_path.push(identifier.to_owned());

    let mut results = Vec::new();

    // for each rule in the ruleset identified by the identifier
    log::trace!("{indent}Translate {text:?} using ruleset {identifier:?}...");
    for (original, translated_tokens) in
      self.rulesets.get(identifier).expect(&format!("ruleset {identifier:?} not found"))
    {
      // start matching from the beginning of the text
      let first_candidate = Candidate::new(IndexMap::new(), text);
      let mut candidates = vec![first_candidate];

      // for each token in the original rule
      let has_wildcard = original.iter().any(is_wildcard_token);
      for (token_index, token) in original.iter().enumerate() {
        // break when no candidates left
        if candidates.is_empty() {
          break;
        }

        // prepare next set of candidates
        let mut next_candidates = Vec::new();

        // match each candidate against the current token
        for candidate in &candidates {
          match token {
            Token::Literal(literal) => {
              // for literal tokens, check if the candidate's remaining text starts with the literal (case-insensitive)
              if candidate.remaining.to_lowercase().starts_with(&literal.to_lowercase()) {
                // just advance the candidate's remaining text
                let remaining = &candidate.remaining[literal.len()..];
                log::trace!("{indent}Token {token:?} matched, remaining: {remaining:?}...");
                next_candidates.push(Candidate::new(candidate.results.clone(), remaining));
              }
            }
            Token::Reference(reference) => {
              let rule_node = RuleNode {
                identifier: identifier.to_owned(),
                rule: original.clone(),
              };

              if context.cyclic_rules.contains(&rule_node) {
                // log::error!("{indent}Skipping cyclic reference to {reference:?} in ruleset {identifier:?}...",);
                // log::error!("{indent}Current identifier path: {:?}", context.identifier_path);
                continue;
              }

              if context.identifier_path.contains(reference) {
                context.cyclic_rules.insert(rule_node.clone());
              }

              // recursively translate the candidate's remaining text using the referenced ruleset
              let result_trees = if reference.starts_with('%') {
                let next_literal = original.get(token_index + 1).and_then(|token| match token {
                  Token::Literal(literal) if !literal.is_empty() => Some(literal.as_str()),
                  _ => None,
                });
                let hint = ReplacerMatchHint {
                  next_literal,
                  terminal: token_index + 1 == original.len(),
                };
                self.do_replace(context, &candidate.remaining, reference, level + 1, hint)
              } else {
                self.do_translate(context, &candidate.remaining, reference, level + 1)
              };

              context.cyclic_rules.remove(&rule_node);

              // for each successful translation, create a new candidate
              for result_tree in result_trees {
                // record the result tree for the reference, and advance the remaining text
                let mut results = candidate.results.clone();
                let remaining = result_tree.remaining.to_owned();
                results.insert(result_tree.identifier.clone(), result_tree);
                next_candidates.push(Candidate::new(results, &remaining));
              }
            }
          }
        }

        // advance to next set of candidates
        if has_wildcard && next_candidates.len() > MAX_WILDCARD_CANDIDATES {
          next_candidates.truncate(MAX_WILDCARD_CANDIDATES);
        }
        candidates = next_candidates;
      }

      // all tokens processed, collect results
      for candidate in candidates {
        // duplicate the identifier
        let identifier = identifier.to_owned();
        // getting the matched and remaining text
        let matched = text[..text.len() - candidate.remaining.len()].to_owned();
        let remaining = candidate.remaining.to_owned();
        // getting children from the candidate
        let children = candidate.results;
        let match_tokens = original.clone();
        // constructing original token string
        let original = {
          let mut joined = String::new();
          for token in original {
            match token {
              Token::Literal(literal) => {
                joined.push_str(literal);
              }
              Token::Reference(reference) => {
                joined.push_str(&format!("{{{}}}", reference));
              }
            }
          }
          joined
        };
        // constructing translated string after replacing references with their translated text
        let translated = {
          let mut replaced = String::new();
          for token in translated_tokens {
            match token {
              Token::Literal(literal) => {
                replaced.push_str(literal);
              }
              Token::Reference(reference) => {
                let child = children.get(reference).expect(&format!(
                  "translated reference {reference:?} should exist in children for ruleset {identifier:?}"
                ));
                replaced.push_str(&child.translated);
              }
            }
          }
          replaced
        };

        // append the result tree
        results.push(ResultTree::new(
          identifier,
          original,
          matched,
          translated,
          remaining,
          children,
          match_tokens,
          translated_tokens.clone(),
        ));
      }
    }

    // log the results if in trace mode
    if log::log_enabled!(log::Level::Trace) {
      let results_display: Vec<String> = results
        .iter()
        .map(
          |ResultTree {
             identifier,
             original,
             matched,
             translated,
             ..
           }| format!("{identifier}@{original}: {matched} -> {translated}"),
        )
        .collect();
      log::trace!("{indent}Returning translate results: {results_display:?}...");
    }

    // pop the identifier path
    context.identifier_path.pop();

    results
  }

  fn do_replace(
    &self,
    context: &mut Context,
    text: &str,
    identifier: &str,
    level: usize,
    hint: ReplacerMatchHint<'_>,
  ) -> Vec<ResultTree> {
    let parts: Vec<&str> = identifier[1..].splitn(3, ':').collect();
    if parts.len() < 2 {
      log::error!("Invalid replacer identifier format: {identifier:?}");
      return Vec::new();
    }
    let name = parts[0];
    let base_namespace = parts[1];
    let config = parts.get(2).copied().unwrap_or("");
    let replacers = replacer::get_replacers();
    let Some(replacer) = replacers.get(name) else {
      log::error!("Replacer {name:?} not found for identifier {identifier:?}");
      return Vec::new();
    };
    log::trace!(
      "{}Using replacer {name} with config {config:?} on text {text:?}...",
      "  ".repeat(level)
    );
    replacer.replace(context, identifier, base_namespace, config, text, self, level + 1, hint)
  }

  fn resolve_captures(
    &self,
    context: &mut Context,
    mut result: ResultTree,
    depth: usize,
    capture_path: &mut Vec<String>,
  ) -> ResultTree {
    if is_translatable_wildcard(&result.identifier) {
      let key = (result.matched.clone(), depth);
      if let Some(cached) = context.capture_cache.get(&key) {
        result.translated = cached.clone().unwrap_or_else(|| result.matched.clone());
        return result;
      }
      if depth >= MAX_CAPTURE_DEPTH || capture_path.contains(&result.matched) {
        result.translated = result.matched.clone();
        context.capture_cache.insert(key, None);
        return result;
      }

      capture_path.push(result.matched.clone());
      let inner = self
        .do_translate(context, &result.matched, "::", 0)
        .into_iter()
        .filter(|candidate| candidate.remaining.is_empty())
        .min_by_key(ResultTree::specificity)
        .map(|candidate| self.resolve_captures(context, candidate, depth + 1, capture_path));
      capture_path.pop();
      result.translated = inner.map(|inner| inner.translated).unwrap_or_else(|| result.matched.clone());
      context.capture_cache.insert(key, Some(result.translated.clone()));
      return result;
    }

    for child in result.children.values_mut() {
      *child = self.resolve_captures(context, child.clone(), depth, capture_path);
    }
    if !result.translation_tokens.is_empty() {
      result.translated = compose_translation(&result.translation_tokens, &result.children, &result.identifier);
    }
    result
  }

  // Parse all ruleset files in a directory
  fn parse_dir(
    &mut self,
    base: impl AsRef<path::Path>,
    curr: impl AsRef<path::Path>,
    visited_root: &mut Option<String>,
  ) -> Result<()> {
    let mut sorted_paths =
      fs::read_dir(curr.as_ref())?.map(|res| res.map(|e| e.path())).collect::<Result<Vec<_>, std::io::Error>>()?;
    sorted_paths.sort();

    for path in sorted_paths {
      // recursively parse directories
      if path.is_dir() {
        self.parse_dir(base.as_ref(), path, visited_root)?;
        continue;
      }

      // for files, only parse .toml files
      if let Some(ext) = path.extension() {
        if path.is_file() && ext == "toml" {
          self.parse_file(base.as_ref(), &path, visited_root).context(format!("failed to parse file {:?}", path))?;
        }
      }
    }

    Ok(())
  }

  // Parse a single ruleset file
  fn parse_file(
    &mut self,
    base: impl AsRef<path::Path>,
    curr: impl AsRef<path::Path>,
    visited_root: &mut Option<String>,
  ) -> Result<()> {
    let base = base.as_ref();
    let curr = curr.as_ref();

    // read and parse the file
    let content = fs::read_to_string(&curr)?;
    let ruleset_file: RuleSetFile = toml::from_str(&content)?;

    // ensure only one root base exists for a ruleset directory
    // note this prevents translate authors from accidentally missing the base field
    if ruleset_file.base.is_none() {
      if let Some(file_with_root) = visited_root {
        return Err(anyhow!("Multiple root bases found: {file_with_root:?} and {curr:?}"));
      }

      *visited_root = Some(curr.to_string_lossy().into_owned());
    }

    // determine the base namespace for this file
    let base_namespace = ruleset_file.base.clone().unwrap_or(String::new());

    // validate base namespace matches file path
    // note this prevents translate authors from accidentally misnaming the base field
    {
      let relative_path_str =
        curr.strip_prefix(base).expect(&format!("path {curr:?} should be under base {base:?}")).to_string_lossy();
      let mut expected_base_namespace =
        relative_path_str.trim_end_matches(".toml").split(std::path::MAIN_SEPARATOR).collect::<Vec<_>>();
      if expected_base_namespace.last() == Some(&"index") {
        expected_base_namespace.pop();
      }
      let expected_base_namespace = expected_base_namespace.join("::");
      if base_namespace != expected_base_namespace {
        return Err(anyhow!(
          "Base namespace mismatch in file {curr:?}: expected {expected_base_namespace:?}, found {base_namespace:?}"
        ));
      }
    }

    // parse all rulesets in the file
    for entry in ruleset_file.rulesets {
      // construct the canonical identifier for this ruleset
      let mut identifier = String::from("::");
      identifier.push_str(&base_namespace);
      if let Some(name) = &entry.name {
        identifier.push_str("::");
        identifier.push_str(name);
      }
      validate_identifier_format(&identifier).context(format!("failed to parse ruleset {identifier:?}"))?;

      // get or create the ruleset entry
      let ruleset = self.rulesets.entry(identifier.clone()).or_insert_with(|| IndexMap::new());

      // append empty rule if optional is set
      if entry.optional {
        ruleset.insert(
          vec![Token::Literal("".to_string())],
          vec![Token::Literal("".to_string())],
        );
      }

      // parse all rules in the ruleset
      for (original, translated) in entry.rules {
        // parse the original and translated tokens
        let original_tokens = parse_tokens(&base_namespace, &original).context(format!(
          "failed to parse the original entry {original:?} in ruleset {identifier:?}"
        ))?;
        let translated_tokens = parse_tokens(&base_namespace, &translated).context(format!(
          "failed to parse the translated entry {translated:?} in ruleset {identifier:?}"
        ))?;
        validate_wildcard_layout(&original_tokens).context(format!(
          "unsafe wildcard layout in original entry {original:?} in ruleset {identifier:?}"
        ))?;

        // ensure no duplicate reference tokens in the original entry
        let original_reference_tokens: Vec<String> = original_tokens
          .iter()
          .filter_map(|token| {
            if let Token::Reference(identifier) = token {
              Some(identifier.to_owned())
            } else {
              None
            }
          })
          .collect();
        let original_reference_tokens_set = original_reference_tokens.iter().collect::<BTreeSet<_>>();
        if original_reference_tokens.len() != original_reference_tokens_set.len() {
          return Err(anyhow!(
            "Original entry {original:?} has duplicate references in ruleset {identifier:?}"
          ));
        }

        // ensure all reference tokens in the translated entry exist in the original entry
        for token in &translated_tokens {
          if let Token::Reference(reference) = token {
            if !original_reference_tokens_set.contains(reference) {
              return Err(anyhow!(
                "Translated reference {reference:?} in translated entry {translated:?} not found in original entry {original:?} in ruleset {identifier:?}"
              ));
            }
          }
        }

        // insert the parsed rule tokens pair into the ruleset
        ruleset.insert(original_tokens, translated_tokens);
      }
    }

    Ok(())
  }

  // Validate that all references in the rulesets are valid
  fn validate_references(&self) -> Result<()> {
    for (ruleset_name, ruleset) in &self.rulesets {
      for (original_tokens, _) in ruleset {
        for token in original_tokens {
          if let Token::Reference(reference) = token {
            if reference.starts_with('%') {
              let name = replacer_name(reference).unwrap_or_default();
              if !replacer::get_replacers().contains_key(name) {
                return Err(anyhow!(
                  "In ruleset {ruleset_name:?}, replacer {name:?} is not registered for reference {reference:?}"
                ));
              }
            } else if !self.rulesets.contains_key(reference) {
              return Err(anyhow!(
                "In ruleset {ruleset_name:?}, reference {reference:?} not found for original tokens {original_tokens:?}"
              ));
            }
          }
        }
      }
    }

    Ok(())
  }

  pub fn dump(&self) -> &IndexMap<String, RuleSet> {
    &self.rulesets
  }
}

// Mapping of ruleset names to their corresponding RuleSet
pub type RuleSets = IndexMap<String, RuleSet>;

// A single ruleset mapping original text tokens to translated text tokens
pub type RuleSet = IndexMap<Tokens, Tokens>;

// A sequence of text tokens
pub type Tokens = Vec<Token>;

// A single text token
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Token {
  // A literal string token
  Literal(String),
  // A reference to another RuleSet by its identifier
  Reference(String),
}

fn replacer_name(identifier: &str) -> Option<&str> {
  identifier.strip_prefix('%')?.split(':').next()
}

fn is_wildcard_reference(identifier: &str) -> bool {
  matches!(replacer_name(identifier), Some("word" | "any"))
}

fn is_translatable_wildcard(identifier: &str) -> bool {
  is_wildcard_reference(identifier)
}

fn is_wildcard_token(token: &Token) -> bool {
  matches!(token, Token::Reference(reference) if is_wildcard_reference(reference))
}

fn compose_translation(tokens: &Tokens, children: &IndexMap<String, ResultTree>, identifier: &str) -> String {
  let mut translated = String::new();
  for token in tokens {
    match token {
      Token::Literal(literal) => translated.push_str(literal),
      Token::Reference(reference) => {
        let child = children.get(reference).unwrap_or_else(|| {
          panic!("translated reference {reference:?} should exist in children for ruleset {identifier:?}")
        });
        translated.push_str(&child.translated);
      }
    }
  }
  translated
}

fn validate_wildcard_layout(tokens: &Tokens) -> Result<()> {
  for (index, token) in tokens.iter().enumerate() {
    let Token::Reference(reference) = token else {
      continue;
    };
    let Some(name @ ("word" | "any")) = replacer_name(reference) else {
      continue;
    };
    let next = tokens.get(index + 1);
    if matches!(next, Some(Token::Reference(next_reference)) if is_wildcard_reference(next_reference)) {
      return Err(anyhow!(
        "adjacent %{name} and wildcard references require a non-empty literal boundary"
      ));
    }
    if name == "any" && next.is_some() && !matches!(next, Some(Token::Literal(literal)) if !literal.is_empty()) {
      return Err(anyhow!(
        "%any must be terminal or immediately followed by a non-empty literal boundary"
      ));
    }
  }
  Ok(())
}

// Regex for splitting tokens
static R_TOKEN_SPLIT: OnceLock<regex::Regex> = OnceLock::new();

// Parse a token string into Tokens, resolving references with the given base namespace
fn parse_tokens(base_namespace: &str, input: &str) -> Result<Tokens> {
  let rts = R_TOKEN_SPLIT.get_or_init(|| regex::Regex::new(r"\{([^\{\}]+)\}").unwrap());

  // validate matching braces
  let mut left_brace_count = 0;
  let mut right_brace_count = 0;
  for c in input.chars() {
    if c == '{' {
      left_brace_count += 1;
    }
    if c == '}' {
      right_brace_count += 1;
    }
  }
  if left_brace_count != right_brace_count {
    return Err(anyhow!(
      "Token string {input:?} has mismatched braces: {left_brace_count} '{{' and {right_brace_count} '}}'"
    ));
  }

  // find all positions for splitting
  let mut pos = BTreeSet::new();
  pos.insert(0);
  for m in rts.find_iter(input) {
    pos.insert(m.start());
    pos.insert(m.end());
  }
  pos.insert(input.len());

  // collect the splitted tokens
  let tokens: Vec<Token> = pos
    .into_iter()
    .collect::<Vec<usize>>()
    .windows(2)
    .flat_map(|it| {
      let l = it[0];
      let r = it[1];

      // extract the reference
      let str = &input[l..r];
      if str.chars().next() == Some('{') && str.chars().last() == Some('}') {
        // reference token
        let inner = &str[1..str.len() - 1];
        // convert the reference to a canonical identifier
        let reference = to_canonical_identifier(inner, base_namespace);
        return Some(Token::Reference(reference));
      }

      // extract the literal
      Some(Token::Literal(str.to_owned()))
    })
    .collect();

  // validate all reference tokens
  for token in &tokens {
    if let Token::Reference(identifier) = token {
      validate_identifier_format(identifier)
        .context(format!("failed to validate the identifier format for token {token:?}"))?;
    }
  }

  Ok(tokens)
}

// Convert an identifier to its canonical form using the base namespace
pub fn to_canonical_identifier(identifier: &str, base_namespace: &str) -> String {
  if identifier.starts_with("::") {
    identifier.to_owned()
  } else if identifier.starts_with("%") {
    let mut parts: Vec<&str> = identifier.splitn(2, ':').collect();
    parts.insert(1, base_namespace);
    parts.join(":")
  } else {
    if base_namespace.is_empty() {
      format!("::{}", identifier)
    } else {
      format!("::{}::{}", base_namespace, identifier)
    }
  }
}

// Regex for getting consecutive colons
static R_CONSECUTIVE_COLONS: OnceLock<regex::Regex> = OnceLock::new();

// Validate the format of a ruleset identifier
fn validate_identifier_format(identifier: &str) -> Result<()> {
  if identifier.starts_with("%") {
    if !identifier.contains(":") {
      return Err(anyhow!(
        "Replacer identifier {identifier:?} must contain a colon separating name and config"
      ));
    }

    // replacer identifiers can be in any format
    return Ok(());
  }

  let rcc = R_CONSECUTIVE_COLONS.get_or_init(|| regex::Regex::new(r":+").unwrap());

  // identifier cannot end with "::" unless it is the root base
  if identifier != "::" && identifier.ends_with("::") {
    return Err(anyhow!("Identifier {identifier:?} cannot end with ::"));
  }

  // identifier can only have two consecutive colons
  if let Some(invalid_colons) =
    rcc.find_iter(identifier).find_map(|m| if m.as_str().len() == 2 { None } else { Some(m.as_str()) })
  {
    return Err(anyhow!("Identifier {identifier:?} contains {invalid_colons:?}"));
  }

  Ok(())
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuleNode {
  pub identifier: String,
  pub rule: Tokens,
}

#[derive(Default)]
pub struct Context {
  pub identifier_path: Vec<String>,
  pub cyclic_rules: BTreeSet<RuleNode>,
  capture_cache: HashMap<(String, usize), Option<String>>,
}

// A candidate during token matching
#[derive(Clone)]
struct Candidate {
  pub results: IndexMap<String, ResultTree>,
  pub remaining: String,
}

impl Candidate {
  // Create a new candidate
  pub fn new(results: IndexMap<String, ResultTree>, remaining: &str) -> Self {
    let remaining = remaining.to_owned();
    Self { results, remaining }
  }
}

// A tree representing the result of a successful translation attempt
#[derive(Debug, Clone)]
pub struct ResultTree {
  // The identifier of the ruleset used
  pub identifier: String,
  // The original token string used for matching
  pub original: String,
  // The matched text
  pub matched: String,
  // The translated text for the matched text
  pub translated: String,
  // The remaining text after matching
  pub remaining: String,
  // The child result trees for referenced rulesets
  pub children: IndexMap<String, ResultTree>,
  // Tokens used to rank the specificity of wildcard rules
  match_tokens: Tokens,
  // Tokens used to rebuild the translation after wildcard captures are resolved
  translation_tokens: Tokens,
}

impl ResultTree {
  // Get the weight of the result tree (number of matched rules, the smaller the better)
  pub fn weight(&self) -> usize {
    let mut weight = 0;
    if self.matched.len() > 0 {
      weight += 1;
    }
    for (_, child) in &self.children {
      weight += child.weight();
    }
    weight
  }

  fn specificity(&self) -> (usize, Reverse<usize>, usize, usize, usize, Reverse<usize>) {
    let mut any = usize::from(matches!(replacer_name(&self.identifier), Some("any")));
    let mut word = usize::from(matches!(replacer_name(&self.identifier), Some("word")));
    let mut legacy_weight = usize::from(!self.matched.is_empty());
    let mut literal_bytes = self
      .match_tokens
      .iter()
      .filter_map(|token| match token {
        Token::Literal(literal) => Some(literal.len()),
        Token::Reference(_) => None,
      })
      .sum::<usize>();
    for child in self.children.values() {
      let child_score = child.specificity();
      literal_bytes += child_score.1.0;
      any += child_score.2;
      word += child_score.3;
      legacy_weight += child_score.4;
    }
    let has_wildcard = any + word > 0;
    (
      usize::from(has_wildcard),
      Reverse(if has_wildcard { literal_bytes } else { 0 }),
      any,
      word,
      legacy_weight,
      Reverse(self.matched.len()),
    )
  }

  // Create a new ResultTree
  fn new(
    identifier: String,
    original: String,
    matched: String,
    translated: String,
    remaining: String,
    children: IndexMap<String, ResultTree>,
    match_tokens: Tokens,
    translation_tokens: Tokens,
  ) -> Self {
    Self {
      identifier,
      original,
      matched,
      translated,
      remaining,
      children,
      match_tokens,
      translation_tokens,
    }
  }
}

// A TOML file containing multiple rulesets
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSetFile {
  // The base namespace for the rulesets in this file
  pub base: Option<String>,
  // The rulesets defined in this file
  pub rulesets: Vec<RuleSetEntry>,
}

// A single ruleset entry in a ruleset
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSetEntry {
  // The name of the ruleset
  pub name: Option<String>,
  // Whether to include an empty rule (matching empty string)
  #[serde(default = "bool::default")]
  pub optional: bool, // default to false
  // The mapping of original token string to translated token string
  pub rules: IndexMap<String, String>,
}
