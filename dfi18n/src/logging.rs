use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

use crate::{tasks, translation, translator};

static VISITED: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();

pub fn log_text(request: &translation::TranslationRequest, backtrace: &str, ptr: *const std::ffi::c_void) {
  let key = request.key().to_owned();
  let context = request.context().clone();
  let content = context.original();

  if translator::should_skip_translation(content) {
    return;
  }

  // only log new translation requests once
  let visited = VISITED.get_or_init(|| RwLock::new(HashSet::new()));
  if visited.read().unwrap().contains(&key) {
    return;
  }

  visited.write().unwrap().insert(key.to_owned());

  // spawn a task to perform the translation
  let request = request.clone();
  let content = content.to_owned();
  let backtrace = backtrace.to_owned();
  let ptr = ptr as usize;
  tasks::spawn(async move {
    // ensure the translation is performed and cached
    translator::translate_task(request.clone()).await;
    let response = translator::translate(&request);
    if response.is_none() {
      let function = match &context {
        translation::TranslationContext::addst { .. } => "addst",
        translation::TranslationContext::addst_flag { .. } => "addst_flag",
        translation::TranslationContext::addcoloredst { .. } => "addcoloredst",
        translation::TranslationContext::top_addst { .. } => "top_addst",
        translation::TranslationContext::markup_text_box { .. } => "mtb_process_string_to_lines",
        translation::TranslationContext::visual_text_block { .. } => "visual_text_block",
        translation::TranslationContext::dfhack { .. } => "dfhack",
      };
      let mut lines = vec![format!("========== {key}"), format!("[{function}] {backtrace}")];

      let viewscreen = request.view_screen();
      lines.push(format!("viewscreen: {viewscreen}"));

      let coordinate = request.coordinate();
      lines.push(format!("coordinate: {coordinate:?}"));

      let color_pair = request.color_pair();
      if let Some(color_pair) = color_pair {
        lines.push(format!("color_pair: {color_pair:?}"));
      }

      match context {
        translation::TranslationContext::addst_flag { flag, .. } => {
          lines.push(format!("flag: {flag:#010b}"));
        }
        _ => {}
      }

      let is_markup = request.is_markup();
      lines.push(format!(
        "---- {} ({ptr:#x}) ----",
        if is_markup { "MarkupText" } else { "PlainText" }
      ));

      lines.push(content.to_owned());

      let debug_string = lines.join("\n");

      log::debug!("{debug_string}");
    }
  });
}
