use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use rule_based_translator::{Translator, register_default_replacers};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

fn translator_with(rules: &str) -> Translator {
  register_default_replacers();
  let directory = std::env::temp_dir().join(format!(
    "dfi18n-wildcards-{}-{}",
    std::process::id(),
    NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
  ));
  let _ = fs::remove_dir_all(&directory);
  fs::create_dir_all(&directory).unwrap();
  fs::write(
    directory.join("index.toml"),
    format!("[[rulesets]]\n[rulesets.rules]\n{rules}"),
  )
  .unwrap();
  let mut translator = Translator::default();
  translator.load_from_dir(&directory).unwrap();
  fs::remove_dir_all(directory).unwrap();
  translator
}

#[test]
fn translates_word_and_any_captures_after_the_outer_rule_matches() {
  let translator = translator_with(
    r#"
"beer" = "啤酒"
"iron" = "铁"
"plump helmet" = "膨大头盔菇"
"wood" = "木材"
"beer Barrel" = "特制啤酒桶"
"{%word:item} Barrel" = "{%word:item}桶"
"{%word:item}." = "{%word:item}。"
"{%word:name} likes" = "{%word:name}喜欢"
"{%word:material} beer" = "{%word:material}啤酒"
"{%any:item} Barrel" = "{%any:item}桶"
"{%any:plant} seeds bag" = "{%any:plant}种子袋"
"Description: {%any:text}" = "描述：{%any:text}"
"{%any:left} made of {%any:right}" = "{%any:left}制成，材料为{%any:right}"
"prefix {%any:value}a" = "前缀{%any:value}甲"
"#,
  );

  assert_eq!(translator.translate("beer Barrel").as_deref(), Some("特制啤酒桶"));
  assert_eq!(translator.translate("iron Barrel").as_deref(), Some("铁桶"));
  assert_eq!(translator.translate("mystery Barrel").as_deref(), Some("mystery桶"));
  assert_eq!(translator.translate("beer.").as_deref(), Some("啤酒。"));
  assert_eq!(translator.translate("Dák-en's likes").as_deref(), Some("Dák-en's喜欢"));
  assert_eq!(
    translator.translate("plump helmet seeds bag").as_deref(),
    Some("膨大头盔菇种子袋")
  );
  assert_eq!(translator.translate("Description: beer").as_deref(), Some("描述：啤酒"));
  assert_eq!(
    translator.translate("beer made of wood").as_deref(),
    Some("啤酒制成，材料为木材")
  );
  assert_eq!(translator.translate("iron beer Barrel").as_deref(), Some("铁啤酒桶"));
  assert_eq!(translator.translate("Description: "), None);
  assert_eq!(translator.translate("Description: beer\nignored"), None);
  let repeated_anchor = format!("prefix {}", "a".repeat(100));
  let expected = format!("前缀{}甲", "a".repeat(99));
  assert_eq!(
    translator.translate(&repeated_anchor).as_deref(),
    Some(expected.as_str())
  );
}

#[test]
fn a_terminal_any_cycle_falls_back_to_the_original_text() {
  let translator = translator_with(r#""{%any:value}" = "{%any:value}""#);
  assert_eq!(translator.translate("unknown text").as_deref(), Some("unknown text"));
}

#[test]
fn rejects_adjacent_or_unbounded_any_placeholders() {
  register_default_replacers();
  for rule in [
    r#""{%any:left}{%any:right}" = "{%any:left}{%any:right}""#,
    r#""{%any:left}{::thing}" = "{%any:left}{::thing}""#,
  ] {
    let directory = std::env::temp_dir().join(format!(
      "dfi18n-invalid-wildcards-{}-{}",
      std::process::id(),
      NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    fs::write(
      directory.join("index.toml"),
      format!(
        "[[rulesets]]\n[rulesets.rules]\n{rule}\n\n[[rulesets]]\nname = \"thing\"\n[rulesets.rules]\n\"x\" = \"x\""
      ),
    )
    .unwrap();
    let error = format!("{:#}", Translator::default().load_from_dir(&directory).unwrap_err());
    fs::remove_dir_all(directory).unwrap();
    assert!(error.contains("unsafe wildcard layout"), "unexpected error: {error}");
  }
}
