use crate::*;

use super::Replacer;

// Replacer for item designations
#[derive(Default)]
pub struct ItemDesignationReplacer {}

impl Replacer for ItemDesignationReplacer {
  fn replace(
    &self,
    context: &mut Context,
    identifier: &str,
    base_namespace: &str,
    config: &str,
    text: &str,
    translator: &Translator,
    level: usize,
    _hint: ReplacerMatchHint<'_>,
  ) -> Vec<ResultTree> {
    let reference = super::to_canonical_identifier(config, base_namespace);
    let mut results = Vec::new();
    // for each matched item designation
    for (prefix, suffix) in match_item_designation(text) {
      // attempt to translate the inner item
      let result_trees = translator.do_translate(context, &text[prefix.len()..], &reference, level + 1);
      // for each translation result, construct a result tree
      for result_tree in result_trees {
        if !result_tree.remaining.starts_with(suffix) {
          continue;
        }
        let remaining = &result_tree.remaining[suffix.len()..];
        let identifier = identifier.to_owned();
        let original = format!("{{{identifier}:{reference}}}");
        let matched = text[..text.len() - remaining.len()].to_owned();
        let translated = format!("{prefix}{}{suffix}", result_tree.translated);
        let remaining = remaining.to_owned();
        let mut children = IndexMap::new();
        children.insert("item".to_owned(), result_tree);
        let composed_tokens = vec![
          Token::Literal(prefix.to_owned()),
          Token::Reference("item".to_owned()),
          Token::Literal(suffix.to_owned()),
        ];
        results.push(ResultTree::new(
          identifier,
          original,
          matched,
          translated,
          remaining,
          children,
          composed_tokens.clone(),
          composed_tokens,
        ));
      }
    }
    results
  }
}

unsafe impl Send for ItemDesignationReplacer {}
unsafe impl Sync for ItemDesignationReplacer {}

const P_NOT_OWNED: (&str, &str) = ("$", "$");
const P_ON_FIRE: (&str, &str) = ("‼", "‼");
const P_WEAR_1: (&str, &str) = ("XX", "XX"); // "XX" must come before "X", or "X" will be returned early
const P_WEAR_2: (&str, &str) = ("X", "X");
const P_WEAR_3: (&str, &str) = ("x", "x");
const P_OFF_SITE: (&str, &str) = ("(", ")");
const P_UNCLAIMED: (&str, &str) = ("{", "}");
const P_QUALITY_1: (&str, &str) = ("-", "-");
const P_QUALITY_2: (&str, &str) = ("+", "+");
const P_QUALITY_3: (&str, &str) = ("*", "*");
const P_QUALITY_4: (&str, &str) = ("≡", "≡");
const P_QUALITY_5: (&str, &str) = ("☼", "☼");
const P_DECOR: (&str, &str) = ("«", "»");
const P_MAGIC: (&str, &str) = ("◄", "►");

// All designation patterns from outermost to innermost
static DESIGNATION_PATTERNS: OnceLock<Vec<Vec<(&'static str, &'static str)>>> = OnceLock::new();

// Gets the designation patterns
fn designation_patterns() -> &'static Vec<Vec<(&'static str, &'static str)>> {
  DESIGNATION_PATTERNS.get_or_init(|| {
    vec![
      // outermost to innermost

      // 1. not owned
      vec![P_NOT_OWNED],
      // 2. on fire
      vec![P_ON_FIRE],
      // 3. wear
      vec![P_WEAR_1, P_WEAR_2, P_WEAR_3],
      // 4. off site
      vec![P_OFF_SITE],
      // 5. unclaimed
      vec![P_UNCLAIMED],
      // 6. quality
      vec![P_QUALITY_1, P_QUALITY_2, P_QUALITY_3, P_QUALITY_4, P_QUALITY_5],
      // 7. decor
      vec![P_DECOR],
      // 8. magic
      vec![P_MAGIC],
      // 9. quality (again)
      vec![P_QUALITY_1, P_QUALITY_2, P_QUALITY_3, P_QUALITY_4, P_QUALITY_5],
    ]
  })
}

// Partially matches item designations in the input string and returns prefix and suffix pairs
pub fn match_item_designation(input: &str) -> BTreeSet<(&str, &str)> {
  // add default case for no designation
  let mut results = BTreeSet::new();
  results.insert(("", ""));

  // collect character and accumulated byte lengths in a parallel vector
  let chars: Vec<char> = input.chars().collect();
  let mut acc = 0;
  let slens: Vec<usize> = chars
    .iter()
    .map(|ch| {
      acc += ch.len_utf8();
      acc
    })
    .collect();

  // only process inputs longer than 2 characters
  if chars.len() <= 2 {
    // returns empty match
    return results;
  }

  // get the designation patterns
  let patterns = designation_patterns();

  // find all candidate prefix-suffix pairs
  let first_ch = chars[0];
  let mut candidates = Vec::new();
  // from outermost to innermost designation patterns pairs
  for pattern_pairs in patterns {
    // for each possible pattern pair
    for &(pl, pr) in pattern_pairs {
      // match the first character of the prefix
      let cl = pl.chars().next().unwrap();
      if cl != first_ch {
        continue;
      }

      // match the last character of the suffix to find all candidates
      let cr = pr.chars().last().unwrap();
      // starting from the third character
      for i in 2..chars.len() {
        let last_ch = chars[i];
        if last_ch != cr {
          continue;
        }

        // found a candidate
        let len = slens[i];
        let str = &input[..len];
        candidates.push(str);
      }

      // break here as we already matched the first character
      break;
    }
  }

  // for each candidate, try to match further inner patterns
  for candidate in candidates {
    // start with the outermost pattern pair again
    let mut wrapper_len = 0;

    // from outermost to innermost designation patterns pairs
    let mut remaining = &candidate[..];
    for pattern_pairs in patterns {
      // for each possible pattern pair
      for &(pl, pr) in pattern_pairs {
        if let (Some(l), Some(r)) = (remaining.get(..pl.len()), remaining.get(remaining.len() - pr.len()..)) {
          if l == pl && r == pr {
            remaining = &remaining[pl.len()..remaining.len() - pr.len()];

            // accumulate lengths
            wrapper_len += pl.len();

            // matched this level, continue to next inner pattern
            break;
          }
        }
      }
    }

    // append the result tuple constructed
    let prefix = &candidate[..wrapper_len];
    let suffix = &candidate[candidate.len() - wrapper_len..];
    results.insert((prefix, suffix));
  }

  results
}
