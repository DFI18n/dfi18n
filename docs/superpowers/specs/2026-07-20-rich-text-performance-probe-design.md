# Rich Text Performance Probe

## Goal

Determine whether the rich-text hook path is called excessively and identify whether scanning or native markup synchronization consumes the most time, without materially adding to the slowdown.

## Scope

Instrument the existing rich-text path only. Do not change translation, caching, word ownership, synchronization, drawing, or collision behavior.

## Metrics

Accumulate process-wide counters for one-second windows:

- `top_addst`: calls entering the rich-text ownership check.
- `boxes_scanned`: markup text boxes examined.
- `words_examined`: native markup words compared with the incoming string pointer.
- `hits`: words whose string pointer matched.
- `first_hits`: matches for the first word that trigger complete-block handling.
- `syncs`: calls that rebuild a native markup text box.
- `rebuilt_words`: native words recreated by synchronization.
- `scan_us` and `max_scan_us`: total and maximum ownership-scan time.
- `sync_us` and `max_sync_us`: total and maximum synchronization time.

## Logging

Emit at most one debug line per second with the prefix `[rich_text_perf]`. Only emit a line when the window contains rich-text activity. Reset window counters after emission.

The hook must never write a log line for each invocation. Timing uses a monotonic clock. Counter updates must be bounded and must not hold existing markup locks while writing the log.

## Placement

- Count and time ownership scans in `handle_help_mtb`.
- Count boxes and words at the existing nested scan loops.
- Count and time synchronization through a single instrumented wrapper around `markup::sync`.
- Record rebuilt word counts from the managed markup before synchronization.

## Validation

- Unit-test counter accumulation, one-second snapshot/reset behavior, and idle-window suppression.
- Run formatting, clippy, and workspace tests.
- Build the release DLL.
- At runtime, compare a normal screen with a rich-text screen and inspect `[rich_text_perf]` lines for calls per second, words examined per call, synchronization frequency, and timing.

## Interpretation

- High `words_examined / top_addst` indicates repeated full-list ownership scans.
- High `syncs` near `first_hits` indicates the same markup is rebuilt every frame.
- High `sync_us` identifies native word reconstruction as the dominant cost.
- High scan time with low sync time identifies the ownership lookup as the dominant cost.
