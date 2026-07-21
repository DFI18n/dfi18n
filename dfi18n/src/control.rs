use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;

use crate::{glyph, markup, text, translator};

// Atomic flag to enable the MOD on the end of the current frame
static ENABLING: OnceLock<AtomicBool> = OnceLock::new();

// Getting the enabling flag
fn enabling() -> &'static AtomicBool {
  ENABLING.get_or_init(|| AtomicBool::new(false))
}

// Atomic flag to disable the MOD on the end of the current frame
static DISABLING: OnceLock<AtomicBool> = OnceLock::new();

// Getting the disabling flag
fn disabling() -> &'static AtomicBool {
  DISABLING.get_or_init(|| AtomicBool::new(false))
}

// Atomic flag to indicate if the MOD is enabled
static ENABLED: OnceLock<AtomicBool> = OnceLock::new();

// Getting the enabled state
fn enabled() -> &'static AtomicBool {
  ENABLED.get_or_init(|| AtomicBool::new(false))
}

// Enable hooks to show translated text
#[unsafe(no_mangle)]
extern "C" fn enable() {
  enabling().store(true, std::sync::atomic::Ordering::SeqCst);
}

// Disable hooks to show original text
#[unsafe(no_mangle)]
extern "C" fn disable() {
  disabling().store(true, std::sync::atomic::Ordering::SeqCst);
}

// Toggle hooks based on current state
#[unsafe(no_mangle)]
extern "C" fn toggle() {
  if is_enabled() {
    disable();
  } else {
    enable();
  }
}

// Check if MOD is enabled
#[unsafe(no_mangle)]
pub fn is_enabled() -> bool {
  enabled().load(std::sync::atomic::Ordering::SeqCst)
}

// Toggle hooks based on enabling/disabling flags
pub fn toggle_enabled() {
  if enabling().load(std::sync::atomic::Ordering::SeqCst) {
    set_enabled(true);
    log::info!("Hooks enabled");
  }

  if disabling().load(std::sync::atomic::Ordering::SeqCst) {
    set_enabled(false);
    log::info!("Hooks disabled");
  }
}

// Set enabled state and reset flags
fn set_enabled(state: bool) {
  enabling().store(false, std::sync::atomic::Ordering::SeqCst);
  disabling().store(false, std::sync::atomic::Ordering::SeqCst);
  enabled().store(state, std::sync::atomic::Ordering::SeqCst);
}

// Atomic flag to reset the MOD on the end of the current frame
static RESETTING: OnceLock<AtomicBool> = OnceLock::new();

// Getting the resetting flag
fn resetting() -> &'static AtomicBool {
  RESETTING.get_or_init(|| AtomicBool::new(false))
}

// Reset the MOD
#[unsafe(no_mangle)]
extern "C" fn reset() {
  resetting().store(true, std::sync::atomic::Ordering::SeqCst);
}

// Perform the reset if requested
pub fn do_reset_if_requested() {
  if resetting().load(std::sync::atomic::Ordering::SeqCst) {
    resetting().store(false, std::sync::atomic::Ordering::SeqCst);

    glyph::reset();
    translator::reset();
    text::reset();
    markup::reset();
    crate::visual_block::reset();

    log::info!("MOD state reset");
  }
}
