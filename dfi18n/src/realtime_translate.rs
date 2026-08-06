//! Optional realtime translation inside the framework.
//!
//! Captures untranslated strings pushed by the hooks, batches them, calls an
//! OpenAI-compatible chat API (DeepSeek by default, thinking disabled) from a
//! background Tokio task, and writes the translations back into the in-memory
//! dictionary (so the composer rewrite / classic path pick them up immediately)
//! and appends them to `ai_fill.csv` for persistence.
//!
//! No external runtime is required. The API key is read from
//! `~/.dfi18n/api_key.txt` or the `DEEPSEEK_API_KEY` env var; model/settings
//! from `~/.dfi18n/config.json` (or defaults). If no key is configured the
//! feature is disabled gracefully.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;

use parking_lot::Mutex;

use crate::{tasks, translator};

// Queue of untranslated strings captured by the hooks
static QUEUE: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
// Strings already requested (dedup, so we don't re-translate within a session)
static REQUESTED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn queue() -> &'static Mutex<Vec<String>> {
  QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}
fn requested() -> &'static Mutex<HashSet<String>> {
  REQUESTED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Record a captured untranslated string for background translation.
/// Deduplicates against already-requested strings and bounds the queue so the
/// high-frequency composer hooks cannot overflow it.
pub fn push(content: String) {
  {
    let req = requested().lock();
    if req.contains(&content) {
      return;
    }
  }
  let mut q = queue().lock();
  if q.len() >= 8192 {
    return;
  }
  q.push(content);
}

#[derive(Clone)]
struct Config {
  base_url: String,
  model: String,
  thinking: String,
  temperature: f64,
  batch_max: usize,
  interval_ms: u64,
}

fn home_dir() -> Option<PathBuf> {
  std::env::var_os("USERPROFILE")
    .or_else(|| std::env::var_os("HOME"))
    .map(PathBuf::from)
}

fn read_key() -> Option<String> {
  if let Ok(k) = std::env::var("DEEPSEEK_API_KEY") {
    if !k.trim().is_empty() {
      return Some(k.trim().to_string());
    }
  }
  let path = home_dir()?.join(".dfi18n").join("api_key.txt");
  std::fs::read_to_string(path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn read_config() -> Config {
  let mut cfg = Config {
    base_url: "https://api.deepseek.com/chat/completions".to_string(),
    model: "deepseek-v4-flash".to_string(),
    thinking: "disabled".to_string(),
    temperature: 0.3,
    batch_max: 40,
    interval_ms: 3000,
  };
  if let Some(dir) = home_dir() {
    let path = dir.join(".dfi18n").join("config.json");
    if let Ok(txt) = std::fs::read_to_string(path) {
      if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
        if let Some(s) = v.get("base_url").and_then(|x| x.as_str()) {
          cfg.base_url = s.to_string();
        }
        if let Some(s) = v.get("model").and_then(|x| x.as_str()) {
          cfg.model = s.to_string();
        }
        if let Some(s) = v.get("thinking").and_then(|x| x.as_str()) {
          cfg.thinking = s.to_string();
        }
        if let Some(n) = v.get("temperature").and_then(|x| x.as_f64()) {
          cfg.temperature = n;
        }
        if let Some(n) = v.get("batch_max").and_then(|x| x.as_u64()) {
          cfg.batch_max = n as usize;
        }
        if let Some(n) = v.get("interval_ms").and_then(|x| x.as_u64()) {
          cfg.interval_ms = n;
        }
      }
    }
  }
  cfg
}

fn find_data_dir() -> Option<PathBuf> {
  // ~/.dfi18n/config.json may specify "data_dir"; otherwise auto-detect
  if let Some(dir) = home_dir() {
    let path = dir.join(".dfi18n").join("config.json");
    if let Ok(txt) = std::fs::read_to_string(path) {
      if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
        if let Some(s) = v.get("data_dir").and_then(|x| x.as_str()) {
          return Some(PathBuf::from(s));
        }
      }
    }
  }
  // Auto-detect: %APPDATA%/Bay 12 Games/Dwarf Fortress/mods/*/dfi18n-data/simple/zh-Hans
  if let Some(apd) = std::env::var_os("APPDATA") {
    let base = PathBuf::from(apd).join("Bay 12 Games").join("Dwarf Fortress").join("mods");
    if let Ok(entries) = std::fs::read_dir(&base) {
      let mut found = Vec::new();
      for e in entries.flatten() {
        let d = e.path().join("dfi18n-data").join("simple").join("zh-Hans");
        if d.is_dir() {
          found.push(d);
        }
      }
      // prefer a dir with ai_fill.csv
      for d in &found {
        if d.join("ai_fill.csv").is_file() {
          return Some(d.clone());
        }
      }
      if let Some(d) = found.first() {
        return Some(d.clone());
      }
    }
  }
  None
}

fn append_to_csv(data_dir: &std::path::Path, entries: &[(String, String)]) -> usize {
  let path = data_dir.join("ai_fill.csv");
  if !path.is_file() {
    if let Err(e) = std::fs::write(&path, "text,translation,tags\n") {
      log::warn!("csv_append status=header_error path={:?} error={}", path, e);
      return 0;
    }
  }
  use std::io::Write;
  let mut f = match std::fs::OpenOptions::new().append(true).open(&path) {
    Ok(f) => f,
    Err(e) => {
      log::warn!("csv_append status=open_error path={:?} error={}", path, e);
      return 0;
    }
  };
  let mut written = 0usize;
  for (s, t) in entries {
    if s == t || t.is_empty() {
      log::debug!("csv_append status=skipped original={:?} translated={:?}", s, t);
      continue;
    }
    let esc = |v: &str| {
      if v.contains(',') || v.contains('"') || v.contains('\n') {
        format!("\"{}\"", v.replace('"', "\"\""))
      } else {
        v.to_string()
      }
    };
    match writeln!(f, "{},{},", esc(s), esc(t)) {
      Ok(_) => written += 1,
      Err(e) => log::warn!("csv_append status=write_error path={:?} original={:?} error={}", path, s, e),
    }
  }
  log::debug!("csv_append status=complete path={:?} requested={} written={}", path, entries.len(), written);
  written
}

// Translate a batch via the API (sync; call via spawn_blocking).
fn translate_batch(cfg: &Config, api_key: &str, batch: &[String]) -> Vec<(String, String)> {
  use serde_json::json;
  let started = std::time::Instant::now();
  log::debug!("realtime_batch status=request model={:?} count={} originals={:?}", cfg.model, batch.len(), batch);
  let prompt = format!(
    "你是矮人要塞(Dwarf Fortress)简体中文专业译者。把以下英文界面/描述/想法文本逐条翻译成简体中文。\n\
     规则：\n\
     - translations 是 JSON 数组, 数量与输入完全一致, 第 i 个元素是第 i 条翻译(严格按顺序)。\n\
     - 保留占位/标记/数字/标点(如 [R]、-->、*、括号、's、斜杠)。\n\
     - 保留所有 [C:r:g:b] 颜色标记及其顺序。\n\
     - 内部标识/无意义串原样保留不译。\n\
     - 只输出 JSON 对象 {{\"translations\": [...]}}。\n\n\
     输入(JSON数组, {n}条)：\n{src}",
    n = batch.len(),
    src = serde_json::to_string(batch).unwrap_or_default()
  );
  let body = json!({
    "model": cfg.model,
    "messages": [{"role": "user", "content": prompt}],
    "temperature": cfg.temperature,
    "thinking": {"type": cfg.thinking},
  });
  let resp = ureq::post(&cfg.base_url)
    .header("Content-Type", "application/json")
    .header("Authorization", &format!("Bearer {api_key}"))
    .send_json(body);
  let content = match resp {
    Ok(mut r) => r.body_mut().read_json::<serde_json::Value>().ok().and_then(|v| {
      v["choices"][0]["message"]["content"].as_str().map(|s| s.to_string())
    }),
    Err(e) => {
      log::warn!("realtime_batch status=api_error elapsed_ms={} error={}", started.elapsed().as_millis(), e);
      None
    }
  };
  let content = match content {
    Some(c) => c,
    None => {
      log::warn!("realtime_batch status=missing_content elapsed_ms={}", started.elapsed().as_millis());
      return Vec::new();
    }
  };
  let translations = content
    .find('[')
    .zip(content.rfind(']'))
    .filter(|(a, b)| b > a)
    .and_then(|(a, b)| serde_json::from_str::<Vec<String>>(&content[a..=b]).ok());
  let list = match translations {
    Some(list) if list.len() == batch.len() => list,
    Some(list) => {
      log::warn!("realtime_batch status=count_mismatch expected={} actual={} raw={:?}", batch.len(), list.len(), content);
      return Vec::new();
    }
    None => {
      log::warn!("realtime_batch status=parse_error raw={:?}", content);
      return Vec::new();
    }
  };
  let entries: Vec<_> = batch.iter().cloned().zip(list).filter(|(s, t)| !t.is_empty() && t != s).collect();
  for (original, translated) in &entries {
    log::debug!("realtime_entry original={:?} translated={:?}", original, translated);
  }
  log::debug!("realtime_batch status=complete elapsed_ms={} requested={} translated={}", started.elapsed().as_millis(), batch.len(), entries.len());
  entries
}

/// Background loop: periodically drain the queue, translate, update dict + CSV.
async fn run_loop() {
  loop {
    tokio::time::sleep(std::time::Duration::from_millis(read_config().interval_ms)).await;

    let key = match read_key() {
      Some(k) => k,
      None => continue,
    };
    let cfg = read_config();

    // Drain up to batch_max unique strings that are not already in the exact
    // simple dictionary. Queue/request locks are held only for this short move.
    let mut batch = Vec::new();
    let mut skipped_requested = 0usize;
    let mut skipped_translated = 0usize;
    let remaining;
    {
      let mut q = queue().lock();
      let mut req = requested().lock();
      let mut keep = Vec::new();
      for s in q.drain(..) {
        if req.contains(&s) {
          skipped_requested += 1;
          continue;
        }
        if translator::is_translated(&s) {
          skipped_translated += 1;
          continue;
        }
        if batch.len() < cfg.batch_max {
          req.insert(s.clone());
          batch.push(s);
        } else {
          keep.push(s);
        }
      }
      remaining = keep.len();
      *q = keep;
    }
    if batch.is_empty() {
      continue;
    }
    log::debug!(
      "realtime_queue drained={} remaining={} skipped_requested={} skipped_translated={}",
      batch.len(),
      remaining,
      skipped_requested,
      skipped_translated
    );

    let batch_count = batch.len();
    let api_started = std::time::Instant::now();
    let cfg2 = cfg.clone();
    let key2 = key.clone();
    let entries = tokio::task::spawn_blocking(move || translate_batch(&cfg2, &key2, &batch))
      .await
      .unwrap_or_default();
    log::debug!(
      "realtime_batch status=worker_complete elapsed_ms={} requested={} returned={}",
      api_started.elapsed().as_millis(),
      batch_count,
      entries.len()
    );

    if entries.is_empty() {
      continue;
    }

    for (s, t) in &entries {
      translator::insert_translation(s, t);
    }
    translator::clear_cache();
    // Do not call text::reset/retranslate_all here. Rebuilding markup blocks
    // costs 1.6-2.5 seconds of CPU for a normal personality screen and can
    // still miss because the game changes its screen list concurrently. The
    // next addst/addcoloredst render observes the cleared cache and performs a
    // synchronous in-memory lookup, so translation becomes effective without a
    // multi-second CPU spike.
    log::debug!("dictionary_update inserted={} cache=cleared redraw=deferred_to_next_render", entries.len());

    if let Some(dir) = find_data_dir() {
      let written = append_to_csv(&dir, &entries);
      log::debug!("dictionary_update csv_written={} dir={:?}", written, dir);
    } else {
      log::warn!("dictionary_update csv status=data_dir_not_found inserted={}", entries.len());
    }
    log::info!("realtime_translate: translated {} new strings", entries.len());
  }
}

/// Start the background translation loop on the Tokio runtime.
pub fn setup() {
  if read_key().is_some() {
    tasks::spawn(run_loop());
    log::info!("realtime_translate: background translation enabled");
  } else {
    log::info!("realtime_translate: no API key, disabled");
  }
}
