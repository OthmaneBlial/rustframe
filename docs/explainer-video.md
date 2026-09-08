# RustFrame — narrated explainer video

The repository now has a second local video deliverable: a narrated explainer of approximately 3 minutes 12 seconds. It combines source-verified explanation cards with the accepted native macOS Research Desk recording. The interaction chapter is the same real package used by the short demo; no browser bridge or simulated product interaction is inserted.

## Watch locally

The ignored delivery workspace contains:

- `target/delivery/research-desk-preview/media/rustframe-explainer.mp4`: H.264/yuv420p web export, 1920 × 1080, AAC narration, 3:11.99.
- `target/delivery/research-desk-preview/media/rustframe-explainer-master.mp4`: higher-quality concatenated master.
- `target/delivery/research-desk-preview/media/rustframe-explainer-poster.jpg`: poster frame.
- `target/delivery/research-desk-preview/media/rustframe-explainer.en.vtt` and `.fr.vtt`: English and French chapter captions, with SRT equivalents.
- `target/delivery/research-desk-preview/media/rustframe-explainer-receipt.json`: source commit, chapter timeline, encoding and claim boundary.

Serve the assembled local site and open `http://127.0.0.1:4319/explainer.html`:

```sh
python3 -m http.server 4319 --bind 127.0.0.1 --directory target/delivery/site
```

The checked-in site manifest keeps the inline player disabled because GitHub release assets are served as downloads. The verified public MP4 is attached to the [`v0.1.0-rc.4` release](https://github.com/OthmaneBlial/rustframe/releases/tag/v0.1.0-rc.4), with [English captions](https://github.com/OthmaneBlial/rustframe/releases/download/v0.1.0-rc.4/rustframe-explainer.en.vtt), [French captions](https://github.com/OthmaneBlial/rustframe/releases/download/v0.1.0-rc.4/rustframe-explainer.fr.vtt) and a poster.

## Rebuild the media

The reproducible local edit script is [`scripts/build_explainer_video.py`](../scripts/build_explainer_video.py). It uses the FFmpeg workflow, the macOS `say` voice, Pillow for 16:9 cards, and the accepted native source under `target/roadmap-evidence/media/accepted/`. The source recordings and generated voice files remain outside Git.

The cards explain the manifest boundary, developer loop, local data/recovery behavior, and release evidence. The receipt is the authority for the final duration and segment boundaries. Public registry installation, signing, and inline site media hosting remain separate release gates.
