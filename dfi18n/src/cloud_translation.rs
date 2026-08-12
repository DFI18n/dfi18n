use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::{Arc, OnceLock, RwLock},
  time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use lua53_sys as lua;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::tasks;

const CLIENT_HEADER_NAME: &str = "x-dfi18n-client";
const CLIENT_HEADER_VALUE: &str = "dfi18n-runtime-v1";

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
struct Config {
  enabled: bool,
  endpoint: String,
  min_words: usize,
  max_characters: usize,
  require_terminal_punctuation: bool,
  allow_quoted_text: bool,
  max_concurrent_requests: usize,
  request_timeout_seconds: u64,
  failure_retry_seconds: u64,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      enabled: false,
      endpoint: String::new(),
      min_words: 6,
      max_characters: 2_000,
      require_terminal_punctuation: true,
      allow_quoted_text: true,
      max_concurrent_requests: 4,
      request_timeout_seconds: 30,
      failure_retry_seconds: 300,
    }
  }
}

impl Config {
  fn validate(mut self) -> Self {
    self.endpoint = self.endpoint.trim().to_owned();
    self.min_words = self.min_words.max(1);
    self.max_characters = self.max_characters.clamp(1, 20_000);
    self.max_concurrent_requests = self.max_concurrent_requests.clamp(1, 32);
    self.request_timeout_seconds = self.request_timeout_seconds.clamp(1, 120);
    self.failure_retry_seconds = self.failure_retry_seconds.max(1);
    self
  }

  fn is_ready(&self) -> bool {
    self.enabled && self.has_supported_endpoint()
  }

  fn has_supported_endpoint(&self) -> bool {
    self.endpoint.starts_with("https://") || self.endpoint.starts_with("http://")
  }
}

enum CacheEntry {
  Pending,
  Success(String),
  Failed { retry_at: Instant },
}

struct State {
  config_path: Option<PathBuf>,
  config: Config,
  client: reqwest::Client,
  semaphore: Arc<Semaphore>,
  cache: HashMap<String, CacheEntry>,
  generation: u64,
}

impl Default for State {
  fn default() -> Self {
    let config = Config::default();
    Self {
      config_path: None,
      semaphore: Arc::new(Semaphore::new(config.max_concurrent_requests)),
      client: reqwest::Client::new(),
      config,
      cache: HashMap::new(),
      generation: 0,
    }
  }
}

static STATE: OnceLock<RwLock<State>> = OnceLock::new();

fn state() -> &'static RwLock<State> {
  STATE.get_or_init(|| RwLock::new(State::default()))
}

pub fn load(path: PathBuf) {
  let config = read_config(&path).unwrap_or_else(|error| {
    log::warn!("cloud translation is disabled: {error:#}");
    Config::default()
  });
  let ready = config.is_ready();
  let mut state = state().write().unwrap();
  state.config_path = Some(path);
  state.semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests));
  state.config = config;
  state.cache.clear();
  state.generation = state.generation.wrapping_add(1);
  if ready {
    log::info!("cloud translation enabled");
  } else {
    log::info!("cloud translation disabled or missing HTTP(S) endpoint");
  }
}

pub fn reload() {
  let path = state().read().unwrap().config_path.clone();
  if let Some(path) = path {
    load(path);
  }
}

fn set_enabled(enabled: bool) {
  let (current, config_path, endpoint_supported) = {
    let state = state().read().unwrap();
    (
      state.config.enabled,
      state.config_path.clone(),
      state.config.has_supported_endpoint(),
    )
  };

  if enabled && !endpoint_supported {
    log::warn!("cloud translation cannot be enabled without an HTTP(S) endpoint");
    return;
  }
  if current == enabled {
    return;
  }

  let Some(config_path) = config_path else {
    log::warn!("cloud translation configuration has not been loaded");
    return;
  };
  if let Err(error) = write_enabled(&config_path, enabled) {
    log::warn!("failed to persist cloud translation state: {error:#}");
    return;
  }

  let mut state = state().write().unwrap();
  state.config.enabled = enabled;
  state.cache.clear();
  state.generation = state.generation.wrapping_add(1);

  if enabled {
    log::info!("cloud translation enabled");
  } else {
    log::info!("cloud translation disabled");
  }
}

#[unsafe(no_mangle)]
extern "C" fn cloud_enable() {
  set_enabled(true);
}

#[unsafe(no_mangle)]
extern "C" fn cloud_disable() {
  set_enabled(false);
}

#[unsafe(no_mangle)]
extern "C" fn cloud_get_status(lua_state: *mut std::ffi::c_void) -> i32 {
  let state = state().read().unwrap();
  lua::push_boolean(lua_state, state.config.enabled);
  lua::push_boolean(lua_state, state.config.has_supported_endpoint());
  2
}

fn normalize_endpoint(input: &str) -> Result<String> {
  let input = input.trim();
  if input.is_empty() {
    return Err(anyhow!("cloud translation host cannot be empty"));
  }

  let candidate = if input.contains("://") {
    input.to_owned()
  } else {
    format!("https://{input}")
  };
  let mut endpoint = reqwest::Url::parse(&candidate).context("invalid cloud translation host")?;
  if !matches!(endpoint.scheme(), "http" | "https") || endpoint.host_str().is_none() {
    return Err(anyhow!("cloud translation host must use HTTP or HTTPS"));
  }
  if endpoint.path() == "/" {
    endpoint.set_path("/v1/translate");
  }
  endpoint.set_fragment(None);
  Ok(endpoint.to_string())
}

fn write_endpoint(path: &Path, endpoint: &str) -> Result<()> {
  let contents = std::fs::read_to_string(path).with_context(|| format!("failed to read {path:?}"))?;
  let newline = if contents.contains("\r\n") { "\r\n" } else { "\n" };
  let encoded = toml::Value::String(endpoint.to_owned()).to_string();
  let mut replaced = false;
  let mut lines: Vec<String> = contents
    .lines()
    .map(|line| {
      let trimmed = line.trim_start();
      let is_endpoint =
        trimmed.strip_prefix("endpoint").is_some_and(|remaining| remaining.trim_start().starts_with('='));
      if is_endpoint {
        replaced = true;
        format!("endpoint = {encoded}")
      } else {
        line.to_owned()
      }
    })
    .collect();
  if !replaced {
    lines.push(format!("endpoint = {encoded}"));
  }
  std::fs::write(path, lines.join(newline) + newline).with_context(|| format!("failed to write {path:?}"))
}

fn write_enabled(path: &Path, enabled: bool) -> Result<()> {
  let contents = std::fs::read_to_string(path).with_context(|| format!("failed to read {path:?}"))?;
  let newline = if contents.contains("\r\n") { "\r\n" } else { "\n" };
  let mut replaced = false;
  let mut lines: Vec<String> = contents
    .lines()
    .map(|line| {
      let trimmed = line.trim_start();
      let is_enabled = trimmed.strip_prefix("enabled").is_some_and(|remaining| remaining.trim_start().starts_with('='));
      if is_enabled {
        replaced = true;
        format!("enabled = {enabled}")
      } else {
        line.to_owned()
      }
    })
    .collect();
  if !replaced {
    lines.insert(0, format!("enabled = {enabled}"));
  }
  std::fs::write(path, lines.join(newline) + newline).with_context(|| format!("failed to write {path:?}"))
}

fn set_endpoint(input: &str) -> Result<String> {
  let endpoint = normalize_endpoint(input)?;
  let path =
    state().read().unwrap().config_path.clone().context("cloud translation configuration has not been loaded")?;
  write_endpoint(&path, &endpoint)?;

  let mut state = state().write().unwrap();
  state.config.endpoint.clone_from(&endpoint);
  state.cache.clear();
  state.generation = state.generation.wrapping_add(1);
  log::info!("cloud translation endpoint set to {endpoint}");
  Ok(endpoint)
}

#[unsafe(no_mangle)]
extern "C" fn cloud_set_endpoint(lua_state: *mut std::ffi::c_void) -> i32 {
  let input = lua::check_string(lua_state, 1);
  match set_endpoint(&input) {
    Ok(endpoint) => {
      lua::push_string(lua_state, &endpoint);
      1
    }
    Err(error) => {
      log::error!("failed to set cloud translation endpoint: {error:#}");
      lua::push_nil(lua_state);
      lua::push_string(lua_state, &format!("{error:#}"));
      2
    }
  }
}

fn read_config(path: &Path) -> Result<Config> {
  let contents = std::fs::read_to_string(path).with_context(|| format!("failed to read {path:?}"))?;
  toml::from_str::<Config>(&contents).with_context(|| format!("failed to parse {path:?}")).map(Config::validate)
}

pub fn get_or_submit(text: &str, sentence_complete: bool) -> Option<String> {
  let normalized = normalize(text);
  let (config, semaphore, client, generation) = {
    let state = state().read().unwrap();
    if !eligible(&state.config, &normalized, sentence_complete) {
      return None;
    }
    if let Some(entry) = state.cache.get(&normalized) {
      match entry {
        CacheEntry::Success(translation) => return Some(translation.clone()),
        CacheEntry::Pending => return None,
        CacheEntry::Failed { retry_at } if *retry_at > Instant::now() => return None,
        CacheEntry::Failed { .. } => {}
      }
    }
    (
      state.config.clone(),
      state.semaphore.clone(),
      state.client.clone(),
      state.generation,
    )
  };

  let Ok(permit) = semaphore.try_acquire_owned() else {
    return None;
  };
  {
    let mut state = state().write().unwrap();
    if let Some(entry) = state.cache.get(&normalized) {
      match entry {
        CacheEntry::Success(translation) => return Some(translation.clone()),
        CacheEntry::Pending => return None,
        CacheEntry::Failed { retry_at } if *retry_at > Instant::now() => return None,
        CacheEntry::Failed { .. } => {}
      }
    }
    state.cache.insert(normalized.clone(), CacheEntry::Pending);
  }

  tasks::spawn(async move {
    let _permit = permit;
    let result = request(&client, &config, &normalized).await;
    let mut state = state().write().unwrap();
    if state.generation != generation {
      return;
    }
    match result {
      Ok(translation) => {
        state.cache.insert(normalized, CacheEntry::Success(translation));
      }
      Err(error) => {
        log::warn!("cloud translation request failed: {error:#}");
        state.cache.insert(
          normalized,
          CacheEntry::Failed {
            retry_at: Instant::now() + Duration::from_secs(config.failure_retry_seconds),
          },
        );
      }
    }
  });
  None
}

#[derive(Serialize)]
struct CloudRequest<'a> {
  text: &'a str,
}

#[derive(Deserialize)]
struct CloudResponse {
  translation: String,
}

async fn request(client: &reqwest::Client, config: &Config, text: &str) -> Result<String> {
  let response = client
    .post(&config.endpoint)
    .header(CLIENT_HEADER_NAME, CLIENT_HEADER_VALUE)
    .timeout(Duration::from_secs(config.request_timeout_seconds))
    .json(&CloudRequest { text })
    .send()
    .await
    .context("failed to contact cloud translation service")?;
  let status = response.status();
  if !status.is_success() {
    return Err(anyhow!("cloud translation service returned HTTP {status}"));
  }
  let response = response.json::<CloudResponse>().await.context("invalid cloud translation response")?;
  let translation = response.translation.trim().to_owned();
  if translation.is_empty() {
    return Err(anyhow!("cloud translation service returned an empty translation"));
  }
  Ok(translation)
}

fn eligible(config: &Config, text: &str, sentence_complete: bool) -> bool {
  if !config.is_ready() || text.is_empty() || text.chars().count() > config.max_characters {
    return false;
  }
  let quoted = config.allow_quoted_text && is_quoted(text);
  if !sentence_complete && !quoted {
    return false;
  }
  if config.require_terminal_punctuation && !has_terminal_punctuation(text) && !quoted {
    return false;
  }
  word_count(text) >= config.min_words
}

fn normalize(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn word_count(text: &str) -> usize {
  text.split_whitespace().filter(|word| word.chars().any(|character| character.is_ascii_alphabetic())).count()
}

fn has_terminal_punctuation(text: &str) -> bool {
  let trimmed = text.trim_end_matches(|character: char| {
    character.is_whitespace() || matches!(character, '"' | '\'' | ')' | ']' | '}' | '”' | '’')
  });
  !trimmed.ends_with("...") && trimmed.ends_with(['.', '!', '?'])
}

fn is_quoted(text: &str) -> bool {
  let text = text.trim();
  let mut characters = text.chars();
  let Some(first) = characters.next() else {
    return false;
  };
  let Some(last) = characters.next_back().or_else(|| characters.next()) else {
    return false;
  };
  matches!((first, last), ('"', '"') | ('\'', '\'') | ('“', '”') | ('‘', '’'))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn ready_config() -> Config {
    Config {
      enabled: true,
      endpoint: "https://example.com/v1/translate".to_owned(),
      ..Config::default()
    }
  }

  #[test]
  fn only_accepts_complete_long_sentences() {
    let config = ready_config();
    assert!(eligible(&config, "He is generally confident in his abilities.", true));
    assert!(!eligible(&config, "He is generally confident in his abilities", false));
    assert!(!eligible(&config, "No job.", true));
  }

  #[test]
  fn accepts_quoted_text_when_enabled() {
    let config = ready_config();
    assert!(eligible(
      &config,
      "\"A sufficiently long quoted piece of game text\"",
      false
    ));
  }

  #[test]
  fn normalizes_whitespace_for_cache_keys() {
    assert_eq!(normalize("  He is\n very   calm. "), "He is very calm.");
  }

  #[test]
  fn accepts_http_endpoint_when_explicitly_configured() {
    let mut config = ready_config();
    config.endpoint = "http://127.0.0.1/v1/translate".to_owned();
    assert!(config.is_ready());
  }

  #[test]
  fn normalizes_cloud_hosts_and_preserves_explicit_paths() {
    assert_eq!(
      normalize_endpoint("translate.example.com").unwrap(),
      "https://translate.example.com/v1/translate"
    );
    assert_eq!(
      normalize_endpoint("http://127.0.0.1:8080").unwrap(),
      "http://127.0.0.1:8080/v1/translate"
    );
    assert_eq!(
      normalize_endpoint("https://translate.example.com/custom").unwrap(),
      "https://translate.example.com/custom"
    );
  }

  #[test]
  fn persists_an_endpoint_without_rewriting_the_rest_of_the_config() {
    let path = std::env::temp_dir().join(format!("dfi18n-cloud-config-{}.toml", std::process::id()));
    std::fs::write(&path, "enabled = false\nendpoint = \"\"\nmin_words = 3\n").unwrap();

    write_endpoint(&path, "https://translate.example.com/v1/translate").unwrap();
    let contents = std::fs::read_to_string(&path).unwrap();
    let parsed: toml::Value = toml::from_str(&contents).unwrap();
    assert_eq!(
      parsed.get("endpoint").and_then(toml::Value::as_str),
      Some("https://translate.example.com/v1/translate")
    );
    assert_eq!(parsed.get("min_words").and_then(toml::Value::as_integer), Some(3));

    std::fs::remove_file(path).unwrap();
  }

  #[test]
  fn persists_enabled_state_without_rewriting_the_rest_of_the_config() {
    let path = std::env::temp_dir().join(format!("dfi18n-cloud-enabled-config-{}.toml", std::process::id()));
    std::fs::write(
      &path,
      "enabled = false\nendpoint = \"https://example.com/v1/translate\"\nmin_words = 3\n",
    )
    .unwrap();

    write_enabled(&path, true).unwrap();
    let contents = std::fs::read_to_string(&path).unwrap();
    let parsed: toml::Value = toml::from_str(&contents).unwrap();
    assert_eq!(parsed.get("enabled").and_then(toml::Value::as_bool), Some(true));
    assert_eq!(
      parsed.get("endpoint").and_then(toml::Value::as_str),
      Some("https://example.com/v1/translate")
    );
    assert_eq!(parsed.get("min_words").and_then(toml::Value::as_integer), Some(3));

    std::fs::remove_file(path).unwrap();
  }
}
