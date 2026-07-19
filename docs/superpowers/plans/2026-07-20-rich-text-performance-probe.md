# Rich Text Performance Probe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a low-overhead once-per-second diagnostic line that quantifies rich-text hook frequency, ownership scanning, and native markup rebuilding time.

**Architecture:** A focused `rich_text_perf` module owns atomic counters, monotonic timing, snapshot/reset logic, and log formatting. Existing rich-text hooks record scan activity, while both markup synchronization entry points use one profiled synchronization helper.

**Tech Stack:** Rust 2024, standard-library atomics and `Instant`, existing `log` facade, Cargo unit tests.

## Global Constraints

- Do not change translation, caching, ownership, synchronization, drawing, or collision behavior.
- Emit at most one `[rich_text_perf]` debug line per second.
- Do not emit a line for an idle window.
- Do not log individual hook calls.

---

### Task 1: Atomic metrics collector

**Files:**
- Create: `dfi18n/src/rich_text_perf.rs`
- Modify: `dfi18n/src/lib.rs`
- Test: inline tests in `dfi18n/src/rich_text_perf.rs`

**Interfaces:**
- Produces: `record_top_addst()`, `record_box_scan()`, `record_word_examined()`, `record_hit(bool)`, `record_scan(Duration)`, and `record_sync(usize, Duration)`.
- Produces: a private `Snapshot` with `take()` and `has_activity()` used by the one-second logger.

- [ ] **Step 1: Write failing snapshot tests**

Add tests that record known counts and durations, assert the first snapshot contains those values, assert the next snapshot is zeroed, and assert an all-zero snapshot reports no activity.

- [ ] **Step 2: Run the focused test and verify failure**

Run: `cargo test -p dfi18n rich_text_perf --lib`

Expected: compilation fails because `rich_text_perf` does not exist yet.

- [ ] **Step 3: Implement the minimal atomic collector**

Use `AtomicU64` counters with relaxed ordering, `fetch_max` for peak timings, `swap(0, Relaxed)` for snapshot/reset, and a `OnceLock<Instant>` plus `AtomicU64` second bucket for rate-limited emission.

- [ ] **Step 4: Run the focused tests**

Run: `cargo test -p dfi18n rich_text_perf --lib`

Expected: all `rich_text_perf` tests pass.

- [ ] **Step 5: Commit the collector**

```powershell
git add -- dfi18n/src/rich_text_perf.rs dfi18n/src/lib.rs
git commit -m "feat: add rich text performance counters"
```

### Task 2: Hook instrumentation, verification, and deployment

**Files:**
- Modify: `dfi18n/src/hooks.rs`
- Modify: `dfi18n/src/markup.rs`
- Modify: `dfi18n/src/rich_text_perf.rs`

**Interfaces:**
- Consumes: Task 1 metric-recording functions.
- Produces: `[rich_text_perf]` lines containing `top_addst`, `boxes_scanned`, `words_examined`, `hits`, `first_hits`, `syncs`, `rebuilt_words`, `scan_us`, `max_scan_us`, `sync_us`, and `max_sync_us`.

- [ ] **Step 1: Add a failing integration-oriented unit test**

Add a formatting test that records one scan and one synchronization and asserts the formatted line contains every required metric name and value.

- [ ] **Step 2: Run the test and verify failure**

Run: `cargo test -p dfi18n rich_text_perf --lib`

Expected: the formatting assertion fails until all required fields are emitted.

- [ ] **Step 3: Instrument ownership scans**

In `top_addst`, increment `top_addst` before `handle_help_mtb`. In `handle_help_mtb`, time the full ownership scan, count each examined markup box and word, and record both ordinary and first-word pointer matches before every return.

- [ ] **Step 4: Instrument synchronization**

In `markup.rs`, route both `set_width_and_sync` and `sync` through one helper that records managed word count and elapsed time around the existing `sync_mtb` call.

- [ ] **Step 5: Run focused and full verification**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy -p dfi18n --all-targets -- -D warnings
cargo test --workspace
cargo build -p dfi18n --release
```

Expected: every command exits successfully.

- [ ] **Step 6: Commit and deploy**

Commit the source changes, locate the release `dfhooks.dll`, preserve the existing game DLL as a timestamped backup, and copy the verified release DLL to the Dwarf Fortress directory. If the running game locks the DLL, stop before replacement and request that the user exit the game.

- [ ] **Step 7: Runtime evidence**

Open a normal screen and a rich-text screen, then compare `[rich_text_perf]` lines in `dfi18n-data/logs/dfi18n.log`. High `words_examined / top_addst` confirms repeated linear ownership scans; high `syncs` and `sync_us` confirm repeated native word reconstruction.
