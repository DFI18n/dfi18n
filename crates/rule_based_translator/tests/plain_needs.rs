use std::fs;

use rule_based_translator::Translator;

#[test]
fn translates_plain_visual_need_sentences_without_color_markup() {
  let directory = std::env::temp_dir().join(format!("dfi18n-plain-needs-{}", std::process::id()));
  let _ = fs::remove_dir_all(&directory);
  fs::create_dir_all(&directory).unwrap();
  fs::write(
    directory.join("index.toml"),
    "[[rulesets]]\n[rulesets.rules]\n\"{plain_needs}\" = \"{plain_needs}\"\n",
  )
  .unwrap();
  fs::write(
    directory.join("plain_needs.toml"),
    include_str!("../../../mod/dfi18n/local-data/rulesets/zh-Hans/plain_needs.toml"),
  )
  .unwrap();

  let mut translator = Translator::default();
  translator.load_from_dir(&directory).unwrap();
  assert_eq!(
    translator.translate("He is not distracted after leading an unexciting life."),
    Some("他没有因为生活平淡无奇而分心。".to_owned())
  );
  assert_eq!(
    translator.translate("She is badly distracted after being unable to acquire something."),
    Some("她因为无法获得某些东西而严重分心。".to_owned())
  );
  assert_eq!(
    translator.translate("Overall, he is untroubled by unmet needs."),
    Some("总体而言，他没有受到未满足需求的困扰。".to_owned())
  );
  fs::remove_dir_all(directory).unwrap();
}
