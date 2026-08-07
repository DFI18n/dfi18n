use std::{
  collections::HashMap,
  env, fs,
  path::{Path, PathBuf},
  process::ExitCode,
};

use anyhow::{Context, Result, anyhow};
use rule_based_translator::{Translator, compose_preference_block, split_preference_block};

const LANG_TAG: &str = "zh-Hans";

fn main() -> ExitCode {
  match run() {
    Ok(Some(translation)) => {
      println!("{translation}");
      ExitCode::SUCCESS
    }
    Ok(None) => {
      eprintln!("NO MATCH");
      ExitCode::from(1)
    }
    Err(error) => {
      eprintln!("ERROR: {error:#}");
      ExitCode::from(2)
    }
  }
}

fn run() -> Result<Option<String>> {
  let input = env::args().skip(1).collect::<Vec<_>>().join(" ");
  if input.trim().is_empty() {
    return Err(anyhow!("用法: df_matcher.exe \"要检查的英文文本\""));
  }

  let exe = env::current_exe().context("无法确定 df_matcher.exe 所在目录")?;
  let data_root = find_data_root(exe.parent().context("df_matcher.exe 没有父目录")?)?;
  let simple_translations = load_simple_translations(&data_root)?;

  let rulesets = data_root.join("rulesets").join(LANG_TAG);
  let translator = if rulesets.join("index.toml").is_file() {
    rule_based_translator::register_default_replacers();
    let mut translator = Translator::default();
    translator.load_from_dir(&rulesets).with_context(|| format!("无法加载规则目录 {rulesets:?}"))?;
    Some(translator)
  } else {
    None
  };

  let translate = |text: &str| {
    simple_translations
      .get(text)
      .cloned()
      .or_else(|| translator.as_ref().and_then(|translator| translator.translate(text)))
  };
  if let Some(block) = split_preference_block(&input) {
    return Ok(Some(compose_preference_block(&block, translate)));
  }
  Ok(translate(&input))
}

fn find_data_root(exe_dir: &Path) -> Result<PathBuf> {
  let nested = exe_dir.join("dfi18n-data");
  if is_data_root(&nested) {
    return Ok(nested);
  }
  if is_data_root(exe_dir) {
    return Ok(exe_dir.to_owned());
  }
  Err(anyhow!(
    "在 {:?} 或其 dfi18n-data 子目录中找不到 simple/rulesets",
    exe_dir
  ))
}

fn is_data_root(path: &Path) -> bool {
  path.join("simple").is_dir() || path.join("rulesets").is_dir()
}

fn load_simple_translations(data_root: &Path) -> Result<HashMap<String, String>> {
  let simple_root = data_root.join("simple").join(LANG_TAG);
  if !simple_root.is_dir() {
    return Ok(HashMap::new());
  }

  let mut csv_paths = Vec::new();
  collect_csv_paths(&simple_root, &mut csv_paths)?;
  csv_paths.sort();

  let mut translations = HashMap::new();
  for path in csv_paths {
    let mut reader = csv::Reader::from_path(&path).with_context(|| format!("无法读取 {path:?}"))?;
    let headers = reader.headers().with_context(|| format!("无法读取 {path:?} 的表头"))?.clone();
    let text_index = headers.iter().position(|header| header == "text").unwrap_or(0);
    let translation_index = headers.iter().position(|header| header == "translation").unwrap_or(1);
    for record in reader.records() {
      let record = record.with_context(|| format!("无法解析 {path:?}"))?;
      if let (Some(text), Some(translation)) = (record.get(text_index), record.get(translation_index)) {
        translations.insert(text.to_owned(), translation.to_owned());
      }
    }
  }
  Ok(translations)
}

fn collect_csv_paths(directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
  for entry in fs::read_dir(directory).with_context(|| format!("无法读取目录 {directory:?}"))? {
    let path = entry?.path();
    if path.is_dir() {
      collect_csv_paths(&path, output)?;
    } else if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("csv")) {
      output.push(path);
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn finds_data_root_beside_the_executable() {
    let root = env::temp_dir().join(format!("df-matcher-{}", std::process::id()));
    let data = root.join("dfi18n-data");
    fs::create_dir_all(data.join("simple")).unwrap();
    assert_eq!(find_data_root(&root).unwrap(), data);
    fs::remove_dir_all(root).unwrap();
  }
}
