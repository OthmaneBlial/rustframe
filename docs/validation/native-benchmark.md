# Native workflow measurement — 2026-09-08

Host: macOS ARM, working source `685a1af`. One sample, warm application and filesystem cache. These are validation receipts, not a universal performance comparison.

## Full indexing path

500 deterministic Markdown files were created before selecting their directory. Timing started at the native Open-dialog confirmation and ended when a read-only SQLite connection observed all 500 committed records. This includes folder grant creation, native filesystem walk/read IPC, parsing and database commit, plus accessibility automation overhead. Completion was polled every 50 ms.

Observed time: **420.03 ms**. UI painting after the final database commit is not included. This is separate from the earlier parser-only benchmark, which excludes IPC and SQLite. Raw receipt: `target/roadmap-evidence/native-index-benchmark.json`.

## Memory scope

The native process reported **111,024 KiB RSS** after indexing. No OS-reported descendants were attributable through parent PID. System-managed WebKit/XPC processes may be parented outside this process tree, so this figure excludes them and **must not be presented as total application memory or peak memory**. Raw receipt: `target/roadmap-evidence/native-memory.json`.

## Remaining measurement work

Obtain repeated cold/warm samples on every supported OS, measure time to an interactive painted window, and attribute WebView service memory before publishing a complete memory comparison. The existing runtime-initialization smoke is not a first-interactive-window measurement.
