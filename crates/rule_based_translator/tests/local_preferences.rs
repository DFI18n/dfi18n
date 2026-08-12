use std::fs;

use rule_based_translator::{Translator, compose_preference_block, split_preference_block};

const SAMPLE: &str = "Unib Olonasàs likes native gold, steel, banded agate, glumprong wood, giant gray squirrel bone, buckets and sloth bear men for their large floppy ears. When possible, he prefers to consume giant mongoose, bat ray, goat cheese, pearl millet beer and bitter melons. He absolutely detests lizards.";

#[test]
fn translates_local_preference_phrases_and_preserves_the_name() {
  let directory = std::env::temp_dir().join(format!("dfi18n-local-preferences-{}", std::process::id()));
  let _ = fs::remove_dir_all(&directory);
  fs::create_dir_all(&directory).unwrap();
  fs::write(
    directory.join("index.toml"),
    "[[rulesets]]\n[rulesets.rules]\n\"{preference}\" = \"{preference}\"\n\"{name}\" = \"{name}\"\n",
  )
  .unwrap();
  fs::write(
    directory.join("preference.toml"),
    include_str!("../../../mod/dfi18n/local-data/rulesets/zh-Hans/preference.toml"),
  )
  .unwrap();
  fs::write(
    directory.join("name.toml"),
    "base = \"name\"\n\n[[rulesets]]\n[rulesets.rules]\n\"Unib Olonasàs\" = \"乌尼布·奥隆阿萨斯\"\n",
  )
  .unwrap();

  let mut translator = Translator::default();
  translator.load_from_dir(&directory).unwrap();
  let block = split_preference_block(SAMPLE).unwrap();
  let translated = compose_preference_block(&block, |part| translator.translate(part));

  assert!(translated.starts_with("乌尼布·奥隆阿萨斯喜欢native gold"));
  assert!(translated.contains("条件允许时，他更喜欢食用giant mongoose"));
  assert!(translated.ends_with("他极其厌恶lizards。"));
  fs::remove_dir_all(directory).unwrap();
}
