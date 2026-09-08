# From a folder of Markdown to a desktop review queue

Draft for publication after the public-install gate passes. No post has been published.

I’m building RustFrame for frontend developers who need a desktop tool around local data. Research Desk is the application I use to test that promise: choose a folder, find a passage, save a review note and export the result.

The source documents stay in the selected directory. SQLite stores the index and review state. A native folder picker grants access, and the frontend receives an opaque URI instead of using arbitrary machine paths as its normal API. A reader window uses the same database as the main queue, so saving a note updates both views.

The current demonstration runs the packaged macOS ARM app from source `13926ad`. It uses two real Markdown files. The final export contains exactly the note saved in the reader, and the exported JSON was compared with the content displayed in TextEdit. The film is about 46 seconds, uses recorded interaction speed and has explicit privacy redactions. It does not show a simulated browser bridge or a shortened compilation presented as a fast build.

Recording uncovered useful defects. Re-selecting a retained directory used to allocate a new grant identity, making existing annotations appear missing. Repeated persistent directory grants now retain their identity when their access level matches. Another defect appeared after indexing 500 benchmark files: a global search limit could crowd out results from a smaller workspace. The query now applies the workspace restriction before ranking and limiting results.

The local validation includes 164 Rust tests, native file changes and rename handling, reader synchronization, note persistence after restart, exported data checks and temporary app/DMG installation on macOS ARM. Five isolated starter frontends were installed from a local API tarball and built natively. That last qualification matters: this is not yet proof of a successful public-registry quickstart.

The immediate distribution work is coordinating the API, runtime and CLI publication, then collecting exact native receipts on each supported host. The preview is unsigned. Building a native application still requires the Rust toolchain and platform prerequisites; RustFrame removes the application’s routine Rust backend code, not the native compiler.

Once those release gates pass, I want feedback from developers with a small tool they actually need. Can you follow the documentation, save your first local record and package the result without private help from me? That is the next useful milestone.

Before posting, attach the approved final video and verified download URLs, recheck the target community's current rules, and keep the creator disclosure. Sources: `docs/native-demo.md`, `docs/validation/roadmap-execution.md`, `docs/validation/native-benchmark.md` and the Research Desk application source.
