# Native workflow measurement — 2026-09-08

Host: macOS ARM, working source `685a1af`. One sample, warm application and filesystem cache. These are validation receipts, not a universal performance comparison.

## Full indexing path

500 deterministic Markdown files were created before selecting their directory. Timing started at the native Open-dialog confirmation and ended when a read-only SQLite connection observed all 500 committed records. This includes folder grant creation, native filesystem walk/read IPC, parsing and database commit, plus accessibility automation overhead. Completion was polled every 50 ms.

Observed time: **420.03 ms**. UI painting after the final database commit is not included. This is separate from the earlier parser-only benchmark, which excludes IPC and SQLite. Raw receipt: `target/roadmap-evidence/native-index-benchmark.json`.

## Memory scope

The native process reported **111,024 KiB RSS** after indexing. No OS-reported descendants were attributable through parent PID. System-managed WebKit/XPC processes may be parented outside this process tree, so this figure excludes them and **must not be presented as total application memory or peak memory**. Raw receipt: `target/roadmap-evidence/native-memory.json`.

## Native interactive window and search

Final app source `13926ad`, one warm-cache sample: process spawn to an enabled native search accessibility node took **1,078.70 ms**. Setting a different query through native accessibility and observing the matching source in the accessibility tree took **898.30 ms**. This includes UI automation, asynchronous search/frontend updates and polling overhead; it is not a pure SQLite query benchmark or compositor-paint measurement. Receipt: `target/roadmap-evidence/native-interactive.json`.

## Remaining measurement work

Obtain repeated cold/warm samples on every supported OS, repeat interactive-window measurements and measure compositor paint, and attribute WebView service memory before publishing a complete memory comparison. The existing runtime-initialization smoke is not a first-interactive-window measurement.
