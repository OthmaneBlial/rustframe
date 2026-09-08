# Research Desk — real native demonstration

The local delivery includes a short, silent demonstration recorded from the packaged macOS ARM application built from `13926ad`. It shows the real native filesystem bridge, SQLite search, saved review notes, reader synchronization and JSONL export. No browser test bridge or simulated product screen is used.

## Watch locally

The delivery workspace is `target/delivery/research-desk-preview/media/`:

- `rustframe-demo.mp4`: H.264/yuv420p, 1920 × 1080, 30 fps, fast-start web export.
- `rustframe-demo-master.mp4`: higher-quality H.264 master.
- `rustframe-demo.en.vtt` and `.srt`: English captions; `.fr.vtt` and `.fr.srt`: French captions.
- `edit-receipt.json`: source revision, cuts and timings.

The assembled local site in `target/delivery/site/` contains a working player with controls, captions and a transcript. Serve it with `python3 -m http.server 4319 --bind 127.0.0.1 --directory target/delivery/site` and open `http://127.0.0.1:4319/demo.html`.

The source site's media manifest deliberately keeps the inline player disabled because GitHub release assets are served as downloads. The verified public MP4 is attached to the [`v0.1.0-rc.4` release](https://github.com/OthmaneBlial/rustframe/releases/tag/v0.1.0-rc.4), with [English captions](https://github.com/OthmaneBlial/rustframe/releases/download/v0.1.0-rc.4/rustframe-demo.en.vtt), [French captions](https://github.com/OthmaneBlial/rustframe/releases/download/v0.1.0-rc.4/rustframe-demo.fr.vtt) and a poster. The README links to the release asset and this provenance page.

## Reproduce the workflow

1. Build/package Research Desk using the local-source instructions in [the developer loop](developer-loop.md).
2. Copy two Markdown documents from `apps/research-desk/workspace/field-briefs/` into a dedicated folder: `port-entry-queue-observations.md` and `regional-returns-hotspots.md`. The recording uses the latter renamed to `regional-returns-reviewed.md` after the native watcher test.
3. Select the folder with the native picker, index it, and search for `returns`.
4. Open the reader, save `Review west-corridor packaging on Friday.`, and close the reader. Confirm the note appears in the main window.
5. Select the same folder again. The retained grant and note remain the same.
6. Open My data & privacy, export the visible queue as JSONL, and inspect the resulting file. It must contain the exact note above.

## Capture and edit provenance

Raw takes, probes, edit scripts and visual review frames are retained outside Git in `target/roadmap-evidence/media/`. The accepted recordings are in `accepted/`; use the edit receipt to identify the actual chosen takes. Earlier takes exposed defects and were rejected.

Capture used macOS `screencapture` on a 1280 × 713 point rectangle, yielding 2560 × 1426 video. Native controls were operated through macOS accessibility, including the real Open and Save dialogs. FFmpeg inspected every source, normalized timestamps and frame rate, scaled/padded the window to a 16:9 canvas and encoded the deliverables. Actions are not accelerated; cuts remove idle time. Personal system labels are masked and marked as hidden. TextEdit's display font was enlarged; its displayed JSON was compared with the original export and matched.

The video is an unsigned local preview, not an installation tutorial from public registries, proof of code signing, or a Windows/Linux native validation. The public-install tutorial remains blocked by the coordinated API/runtime publications. The separate offline test constrains the native process; this film itself makes no claim of recording under a system-wide network block.

The longer narrated explanation is documented in [the explainer video](explainer-video.md). It connects the manifest, build loop, native capture, data boundary, and release evidence while keeping the public-media URL unset until hosting is verified.

See [the validation journal](validation/roadmap-execution.md) for test and package scopes, and [release preparation](launch/release-notes-draft.md) for publication gates.
