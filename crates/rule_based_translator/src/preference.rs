#[derive(Debug, PartialEq, Eq)]
pub struct PreferenceSection {
  pub prefixes: Vec<String>,
  pub items: Vec<String>,
  pub join_last_with_and: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PreferenceBlock {
  pub sections: Vec<PreferenceSection>,
}

pub fn split_semantic_sentences(text: &str) -> Vec<String> {
  let chars: Vec<char> = text.chars().collect();
  let mut sentences = Vec::new();
  let mut start = 0;
  let mut index = 0;

  while index < chars.len() {
    let punctuation = chars[index];
    let is_ellipsis_dot = punctuation == '.'
      && (index.checked_sub(1).is_some_and(|previous| chars[previous] == '.')
        || chars.get(index + 1).is_some_and(|next| *next == '.'));
    if !matches!(punctuation, '.' | '!' | '?') || is_ellipsis_dot {
      index += 1;
      continue;
    }

    let mut end = index + 1;
    while chars.get(end).is_some_and(|character| matches!(character, '"' | '\'' | ')' | ']' | '}' | '”' | '’')) {
      end += 1;
    }
    if end < chars.len() && !chars[end].is_whitespace() {
      index += 1;
      continue;
    }

    let sentence: String = chars[start..end].iter().collect::<String>().trim().to_owned();
    if !sentence.is_empty() {
      sentences.push(sentence);
    }
    start = end;
    while start < chars.len() && chars[start].is_whitespace() {
      start += 1;
    }
    index = start;
  }

  let remainder: String = chars[start..].iter().collect::<String>().trim().to_owned();
  if !remainder.is_empty() {
    sentences.push(remainder);
  }
  sentences
}

fn split_preference_items(text: &str, split_final_and: bool) -> Option<(Vec<String>, bool)> {
  let mut items: Vec<String> =
    text.split(',').map(str::trim).filter(|item| !item.is_empty()).map(str::to_owned).collect();
  if items.is_empty() {
    return None;
  }

  let mut join_last_with_and = false;
  if split_final_and && let Some(last) = items.pop() {
    if let Some((left, right)) = last.rsplit_once(" and ") {
      if !left.trim().is_empty() && !right.trim().is_empty() {
        items.push(left.trim().to_owned());
        items.push(right.trim().to_owned());
        join_last_with_and = true;
      } else {
        items.push(last);
      }
    } else {
      items.push(last);
    }
  }
  Some((items, join_last_with_and))
}

pub fn split_preference_block(text: &str) -> Option<PreferenceBlock> {
  let sentences = split_semantic_sentences(text);
  if sentences.len() != 3 {
    return None;
  }

  let likes_sentence = sentences[0].strip_suffix('.')?;
  if likes_sentence.matches(',').count() < 4 {
    return None;
  }
  let (subject, preferences) = likes_sentence.split_once(" likes ")?;
  let subject = subject.trim();
  if subject.is_empty()
    || subject.chars().any(|character| matches!(character, ',' | '.' | '!' | '?'))
    || matches!(subject.to_ascii_lowercase().as_str(), "he" | "she" | "it" | "they")
  {
    return None;
  }

  let (likes_items, likes_join_last_with_and) = split_preference_items(preferences, true)?;
  if likes_items.len() < 5 {
    return None;
  }

  let preference_sentence = sentences[1].strip_suffix('.')?.strip_prefix("When possible, ")?;
  let (pronoun, preference) = preference_sentence.split_once(' ')?;
  if !matches!(pronoun, "he" | "she" | "it" | "they") {
    return None;
  }
  let preference = preference.strip_prefix("prefers to ")?;
  let (preference_verb, preference_items) = preference.split_once(' ')?;
  let (preference_items, preference_join_last_with_and) = split_preference_items(preference_items, true)?;

  let detest_prefix = match pronoun {
    "he" => "He absolutely detests",
    "she" => "She absolutely detests",
    "it" => "It absolutely detests",
    "they" => "They absolutely detest",
    _ => return None,
  };
  let (detest_items, _) = split_preference_items(
    sentences[2].strip_suffix('.')?.strip_prefix(detest_prefix)?.trim_start(),
    false,
  )?;

  Some(PreferenceBlock {
    sections: vec![
      PreferenceSection {
        prefixes: vec![format!("{subject} likes")],
        items: likes_items,
        join_last_with_and: likes_join_last_with_and,
      },
      PreferenceSection {
        prefixes: vec![
          "When possible".to_owned(),
          format!("{pronoun} prefers to {preference_verb}"),
        ],
        items: preference_items,
        join_last_with_and: preference_join_last_with_and,
      },
      PreferenceSection {
        prefixes: vec![detest_prefix.to_owned()],
        items: detest_items,
        join_last_with_and: false,
      },
    ],
  })
}

fn append_prefix_item(output: &mut String, item: &str) {
  if output.chars().next_back().is_some_and(|character| character.is_ascii())
    && item.chars().next().is_some_and(|character| character.is_ascii())
  {
    output.push(' ');
  }
  output.push_str(item);
}

fn translate_prefix(prefix: &str, translate: &mut impl FnMut(&str) -> Option<String>) -> Option<String> {
  if let Some(translated) = translate(prefix) {
    return Some(translated);
  }

  let subject = prefix.strip_suffix(" likes")?;
  let translated_likes = translate("likes")?;
  let mut translated = subject.to_owned();
  append_prefix_item(&mut translated, &translated_likes);
  Some(translated)
}

pub fn preference_section_source(section: &PreferenceSection) -> String {
  let mut source = section.prefixes.join(", ");
  for (index, item) in section.items.iter().enumerate() {
    if index == 0 {
      source.push(' ');
    } else if section.join_last_with_and && index + 1 == section.items.len() {
      source.push_str(" and ");
    } else {
      source.push_str(", ");
    }
    source.push_str(item);
  }
  source.push('.');
  source
}

pub fn compose_preference_block(block: &PreferenceBlock, mut translate: impl FnMut(&str) -> Option<String>) -> String {
  let mut translated = String::new();
  for section in &block.sections {
    let mut translated_section = String::new();
    let mut final_prefix_was_translated = false;
    for prefix in &section.prefixes {
      if !translated_section.is_empty() {
        translated_section.push('，');
      }
      let translated_prefix = translate_prefix(prefix, &mut translate);
      final_prefix_was_translated = translated_prefix.is_some();
      translated_section.push_str(&translated_prefix.unwrap_or_else(|| prefix.clone()));
    }
    if let Some((first, remaining)) = section.items.split_first() {
      let first = translate(first).unwrap_or_else(|| first.clone());
      if final_prefix_was_translated {
        append_prefix_item(&mut translated_section, &first);
      } else {
        translated_section.push(' ');
        translated_section.push_str(&first);
      }
      for (index, item) in remaining.iter().enumerate() {
        let is_last = index + 1 == remaining.len();
        translated_section.push(if section.join_last_with_and && is_last {
          '和'
        } else {
          '，'
        });
        translated_section.push_str(&translate(item).unwrap_or_else(|| item.clone()));
      }
    }
    translated_section.push('。');
    translated.push_str(&translated_section);
  }
  translated
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE: &str = "Unib Olonasàs likes native gold, steel, banded agate, glumprong wood, giant gray squirrel bone, buckets and sloth bear men for their large floppy ears. When possible, he prefers to consume giant mongoose, bat ray, goat cheese, pearl millet beer and bitter melons. He absolutely detests lizards.";

  #[test]
  fn splits_all_three_preference_sections() {
    let block = split_preference_block(SAMPLE).unwrap();
    assert_eq!(block.sections[0].prefixes, ["Unib Olonasàs likes"]);
    assert_eq!(block.sections[0].items[0], "native gold");
    assert_eq!(
      &block.sections[0].items[block.sections[0].items.len() - 2..],
      ["buckets", "sloth bear men for their large floppy ears"]
    );
    assert!(block.sections[0].join_last_with_and);
    assert_eq!(block.sections[1].prefixes, ["When possible", "he prefers to consume"]);
    assert_eq!(
      block.sections[1].items,
      [
        "giant mongoose",
        "bat ray",
        "goat cheese",
        "pearl millet beer",
        "bitter melons"
      ]
    );
    assert!(block.sections[1].join_last_with_and);
    assert_eq!(block.sections[2].prefixes, ["He absolutely detests"]);
    assert_eq!(block.sections[2].items, ["lizards"]);
  }

  #[test]
  fn composes_partial_translations_without_losing_original_parts() {
    let block = split_preference_block(SAMPLE).unwrap();
    let translated = compose_preference_block(&block, |part| match part {
      "Unib Olonasàs likes" => Some("Unib Olonasàs喜欢".to_owned()),
      "native gold" => Some("自然金".to_owned()),
      "He absolutely detests" => Some("他极其厌恶".to_owned()),
      "lizards" => Some("蜥蜴".to_owned()),
      _ => None,
    });
    assert!(translated.starts_with("Unib Olonasàs喜欢自然金，steel"));
    assert!(translated.contains("buckets和sloth bear men for their large floppy ears"));
    assert!(translated.contains("When possible，he prefers to consume giant mongoose"));
    assert!(translated.contains("pearl millet beer和bitter melons"));
    assert!(translated.ends_with("他极其厌恶蜥蜴。"));
  }

  #[test]
  fn translates_the_likes_suffix_without_translating_the_subject() {
    let block = split_preference_block(SAMPLE).unwrap();
    let translated = compose_preference_block(&block, |part| match part {
      "likes" => Some("喜欢".to_owned()),
      "When possible" => Some("条件允许时".to_owned()),
      "he prefers to consume" => Some("他更喜欢食用".to_owned()),
      "He absolutely detests" => Some("他极其厌恶".to_owned()),
      _ => None,
    });

    assert!(translated.starts_with("Unib Olonasàs喜欢native gold"));
    assert!(translated.contains("条件允许时，他更喜欢食用giant mongoose"));
    assert!(translated.ends_with("他极其厌恶lizards。"));
  }
}
