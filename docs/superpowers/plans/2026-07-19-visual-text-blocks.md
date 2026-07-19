# Visual Text Blocks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconstruct fragmented DF draw calls into complete visual text blocks, translate each complete block once, erase its original cells only when a translation exists, and render the translated block in the original UI region.

**Architecture:** A pure `visual_block` module collects immutable fragments during one `render_things` pass and groups horizontal adjacency, consecutive wrapped lines, and `addst_flag` top/bottom halves. Hooks continue to capture their original arguments; frame finalization produces translation requests and screen clear regions before DFI18n's existing lower/upper render stages.

**Tech Stack:** Rust 2024, DFI18n hook framework, existing `TextBlock`, screen occupancy buffers, Cargo unit tests.

## Global Constraints

- Preserve the existing MTB translation path.
- Do not perform translation, allocation-heavy formatting, or SDL drawing from a grouping primitive.
- Never erase original text unless a complete-block translation is available.
- Rebuild visual blocks every frame so stale translations disappear naturally.
- Preserve lower/upper layers, `addst_flag` double-height semantics, and `addcoloredst` source colors.

---

### Task 1: Pure visual text block aggregation

**Files:**
- Create: `dfi18n/src/visual_block.rs`
- Modify: `dfi18n/src/lib.rs`
- Test: inline tests in `dfi18n/src/visual_block.rs`

**Interfaces:**
- Consumes: `Fragment { source, layer, coordinate, text, color_runs, flag, caller }`
- Produces: `Collector::finish() -> Vec<VisualTextBlock>`

- [ ] Write failing tests for horizontal `To recenter...` fragments, consecutive wrapped personality lines, blank-line separation, and `flag=8/16` double-height deduplication.
- [ ] Run `cargo test -p dfi18n visual_block --lib` and verify missing-module/API failures.
- [ ] Implement deterministic grouping by layer, source family, caller context, coordinate continuity, and frame-local insertion order.
- [ ] Run the focused tests and the full workspace tests.
- [ ] Commit the tested aggregation module.

### Task 2: Frame collection and deferred source clearing

**Files:**
- Modify: `dfi18n/src/hooks.rs`
- Modify: `dfi18n/src/screen.rs`
- Modify: `dfi18n/src/visual_block.rs`

**Interfaces:**
- Consumes: hook fragments collected between `visual_block::begin_frame()` and `visual_block::finish_frame()`
- Produces: translated `TextBlock` registrations plus exact original cell clear regions

- [ ] Write failing tests for clear-region union and the rule that untranslated blocks produce no clear regions.
- [ ] Run focused tests and verify the expected failures.
- [ ] Start collection before `call_render_things()`, feed `addst`, `addst_flag`, `addcoloredst`, and `top_addst` fragments, then finalize after the original call.
- [ ] Add screen APIs that mark exact source rectangles independently from translated layout occupancy.
- [ ] Keep original draw calls intact during collection; mark source cells only for blocks with cached translations before `screen::move_occupied()`.
- [ ] Run focused and full tests, then commit.

### Task 3: Complete-block translation and bounded layout

**Files:**
- Modify: `dfi18n/src/translation/types.rs`
- Modify: `dfi18n/src/text.rs`
- Modify: `dfi18n/src/visual_block.rs`
- Modify: `dfi18n/src/hooks.rs`

**Interfaces:**
- Produces: `TranslationInput::visual_text_block` keyed by complete original text and viewscreen
- Produces: a `TextBlock` constrained to the original block width

- [ ] Write failing tests proving complete source text forms one cache key regardless of its fragment boundaries and that layout width is retained.
- [ ] Run tests and verify the expected failures.
- [ ] Add the visual-block translation context without volatile addresses or coordinates in its reusable key.
- [ ] Build translated text with the original block width, base color, layer, and double-height property.
- [ ] Register one translated block at the aggregate origin and preserve existing collision checks.
- [ ] Run focused and full tests, then commit.

### Task 4: Build and runtime validation package

**Files:**
- Modify: `dfi18n/src/hooks.rs` diagnostics if needed
- Modify: `README.md` with visual-block behavior

**Interfaces:**
- Produces: a version-locked DFI18n DLL ready for manual DF 53.15 validation

- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo clippy -p dfi18n --all-targets -- -D warnings`.
- [ ] Run `cargo test --workspace`.
- [ ] Build the release DLL.
- [ ] Validate `To recenter...`, a full `addcoloredst` personality block, and `Health`/`Skills`/`Rooms` double-height labels in game.
- [ ] Confirm that untranslated blocks remain English and closing a panel removes its Chinese overlay on the next frame.
