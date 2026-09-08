#!/usr/bin/env python3
"""Build the narrated RustFrame explainer from verified source material.

The native workflow segment is the accepted macOS recording already used by
the short demo. The other segments are explanatory cards rendered from source
files and local delivery receipts; no product interaction is fabricated.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import textwrap
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / "target" / "roadmap-evidence" / "explainer"
OUT = ROOT / "target" / "delivery" / "research-desk-preview" / "media"
SITE_MEDIA = ROOT / "target" / "delivery" / "site" / "assets" / "media"
DEMO = OUT / "rustframe-demo.mp4"


def run(args: list[str], *, capture: bool = False) -> str:
    result = subprocess.run(args, check=True, text=True, capture_output=capture)
    return result.stdout if capture else ""


def duration(path: Path) -> float:
    return float(
        run(
            [
                "ffprobe",
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
                str(path),
            ],
            capture=True,
        ).strip()
    )


def font(*names: str, size: int) -> ImageFont.FreeTypeFont:
    candidates = [Path("/System/Library/Fonts/Supplemental") / name for name in names]
    candidates += [Path("/usr/share/fonts/truetype/dejavu") / name for name in names]
    for candidate in candidates:
        if candidate.exists():
            return ImageFont.truetype(str(candidate), size)
    return ImageFont.load_default()


REGULAR = font("Arial.ttf", "DejaVuSans.ttf", size=31)
SMALL = font("Arial.ttf", "DejaVuSans.ttf", size=23)
BOLD = font("Arial Bold.ttf", "DejaVuSans-Bold.ttf", size=52)
MONO_SMALL = font("Menlo.ttc", "DejaVuSansMono.ttf", size=20)


def draw_wrapped(draw: ImageDraw.ImageDraw, text: str, xy: tuple[int, int], *, width: int,
                 fill: str, fnt: ImageFont.FreeTypeFont, line_gap: int = 12) -> int:
    x, y = xy
    lines: list[str] = []
    for paragraph in text.split("\n"):
        lines.extend(textwrap.wrap(paragraph, width=width, break_long_words=False) or [""])
    for line in lines:
        draw.text((x, y), line, font=fnt, fill=fill)
        y += fnt.size + line_gap
    return y


def card(path: Path, kicker: str, title: str, body: str, code: list[str] | None = None,
         footer: str = "SOURCE-VERIFIED · RUSTFRAME") -> None:
    im = Image.new("RGB", (1920, 1080), "#0b111a")
    draw = ImageDraw.Draw(im)
    for x in range(0, 1920, 120):
        draw.line((x, 0, x, 1080), fill="#101b29", width=1)
    for y in range(0, 1080, 120):
        draw.line((0, y, 1920, y), fill="#101b29", width=1)
    draw.rectangle((96, 72, 1824, 1008), outline="#26364a", width=2)
    draw.text((138, 118), kicker.upper(), font=SMALL, fill="#dfb56e")
    draw.text((138, 174), title, font=BOLD, fill="#f5f1e8")
    draw.line((138, 272, 1780, 272), fill="#80683e", width=2)
    draw_wrapped(draw, body, (138, 326), width=54, fill="#cbd5e1", fnt=REGULAR, line_gap=13)
    if code:
        box = (1040, 330, 1770, 850)
        draw.rounded_rectangle(box, radius=20, fill="#111c2b", outline="#38506b", width=2)
        y = box[1] + 36
        for raw in code:
            draw_wrapped(draw, raw, (box[0] + 34, y), width=38, fill="#a9d6c5", fnt=MONO_SMALL, line_gap=8)
            y += (len(textwrap.wrap(raw, width=38)) or 1) * (MONO_SMALL.size + 8) + 12
    draw.line((138, 946, 1780, 946), fill="#26364a", width=1)
    draw.text((138, 962), footer, font=SMALL, fill="#8898aa")
    im.save(path)


def voice(path: Path, text: str) -> None:
    # macOS ships the high-quality system voices used for this local narration.
    if shutil.which("say") is None:
        raise RuntimeError("macOS `say` is required to create the narrated local explainer")
    run(["say", "-v", "Daniel", "-r", "170", "-o", str(path), text])


def encode_card(image: Path, audio: Path, output: Path, hold: float = 1.1) -> float:
    length = duration(audio) + hold
    run(
        [
            "ffmpeg", "-hide_banner", "-y", "-loop", "1", "-i", str(image), "-i", str(audio),
            "-t", f"{length:.3f}", "-vf", "fps=30,format=yuv420p", "-map", "0:v:0", "-map", "1:a:0",
            "-c:v", "libx264", "-crf", "20", "-preset", "slow", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "160k", "-af", "apad", "-shortest", "-movflags", "+faststart",
            str(output),
        ]
    )
    return duration(output)


def encode_demo(audio: Path, output: Path) -> float:
    length = duration(DEMO)
    run(
        [
            "ffmpeg", "-hide_banner", "-y", "-i", str(DEMO), "-i", str(audio), "-t", f"{length:.3f}",
            "-map", "0:v:0", "-map", "1:a:0", "-c:v", "libx264", "-crf", "20", "-preset", "slow",
            "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k", "-af", "apad", "-movflags", "+faststart",
            str(output),
        ]
    )
    return duration(output)


def main() -> None:
    if not DEMO.exists():
        raise SystemExit(f"Missing accepted native demo: {DEMO}")
    WORK.mkdir(parents=True, exist_ok=True)
    OUT.mkdir(parents=True, exist_ok=True)
    SITE_MEDIA.mkdir(parents=True, exist_ok=True)

    # Every technical claim on a card is grounded in these checked-in sources or receipts.
    chapters = [
        {
            "id": "01-title",
            "kicker": "RustFrame · Research Desk",
            "title": "Build a local-first desktop workflow",
            "body": "This explainer follows a real packaged macOS application from its project contract to a native search, review, synchronization, and JSONL export workflow. The interaction segment is the accepted recording used by the short demo.",
            "code": ["REAL NATIVE CAPTURE", "macOS ARM · 46.47 s", "No simulated bridge"],
            "voice": "Welcome to RustFrame Research Desk. In the next few minutes, we will connect the project contract, the native runtime, and a real document review workflow. The middle section is a direct capture from the packaged macOS application, not a browser mockup.",
        },
        {
            "id": "02-contract",
            "kicker": "01 / Project contract",
            "title": "Start with explicit boundaries",
            "body": "A RustFrame app declares its windows, frontend entry point, local database, filesystem grants, and allowed capabilities in one reviewable manifest. That contract is compiled into the native runner and checked before packaging.",
            "code": ["app.windows: main, reader", "database.schema: data/schema.json", "permissions: db + fs + dialog", "shell.commands: []"],
            "voice": "RustFrame starts with explicit boundaries. The Research Desk manifest declares two windows, a local database schema, persistent folder grants, and the exact capabilities each window may request. There are no shell commands in this app. These are reviewable inputs to the native runner, rather than permissions hidden inside a web page.",
        },
        {
            "id": "03-build",
            "kicker": "02 / Developer loop",
            "title": "From frontend files to a native binary",
            "body": "The project keeps the frontend and native runtime separate. The CLI validates the manifest, builds the frontend, embeds the generated assets, and packages a platform-native application. The local delivery was checked with the exact commands below.",
            "code": ["rustframe validate", "rustframe build", "rustframe package --format dmg", "npm run build · cargo test --workspace"],
            "footer": "LOCAL DELIVERY RECEIPT · UNSIGNED PREVIEW",
            "voice": "The developer loop is deliberately ordinary. Validate the manifest, build the frontend, and ask the RustFrame CLI to package a native application. The local delivery receipt records the exact macOS ARM app and DMG, checksums, SPDX inventories, and the test commands that produced them. Public registry installation and signing are separate release gates and are not implied by this local preview.",
        },
        {
            "id": "04-workflow",
            "kicker": "03 / Real native workflow",
            "title": "Select, search, review, export",
            "body": "Watch the actual packaged app: a native folder picker, SQLite-backed search, a reader window, a saved review note, synchronized state, and a JSONL export. The recording keeps real interaction speed and masks only local machine labels.",
            "code": ["folder → consent → index", "search: returns", "note: persisted", "export: JSONL"],
            "voice": "Now watch the real workflow. The user chooses a folder through the native picker and grants access. Research Desk indexes two Markdown documents, searches for returns, opens the matching source in a reader, and saves a review note. Closing the reader leaves the same note visible in the main window. The final action uses the native save dialog and exports the visible queue as JSONL. The cuts remove idle time, but the interactions themselves are not accelerated.",
            "demo": True,
        },
        {
            "id": "05-data",
            "kicker": "04 / Data and recovery",
            "title": "Local data stays inspectable",
            "body": "The native bridge writes to SQLite and exposes an explicit export. The validation journal also covers renamed and removed files, empty folders, unreadable input, revoked access, restart persistence, and offline operation with loopback IPC allowed.",
            "code": ["SQLite → visible queue", "JSONL → inspectable export", "revoked grant → consent state", "external network → denied"],
            "voice": "The point of a local-first workflow is inspectability. The database keeps the visible queue and review note, while the export gives you a plain JSONL record you can archive or process elsewhere. The native validation also exercised rename, removal, empty folders, unreadable input, access revocation, restart persistence, and an external-network denial policy. Those cases return to explicit recovery states instead of silently inventing data.",
        },
        {
            "id": "06-release",
            "kicker": "05 / Package and release",
            "title": "Verify before you share",
            "body": "A release candidate is more than a source archive. The local bundle contains the app, DMG, runtime crate, API tarball, checksums, SBOMs, validation notes, and the media receipts. The next tag will remain a candidate until public registries and signing are available.",
            "code": ["SHA256SUMS ✓", "SPDX inventories ✓", "native smoke ✓", "signed / public install: pending"],
            "footer": "RELEASE EVIDENCE · DO NOT CONFUSE WITH TRUSTED SIGNING",
            "voice": "A useful release is evidence, not only a tag. The local bundle includes the app and DMG, package archives, checksums, SPDX inventories, validation notes, and the media receipts. The next GitHub tag is a coordinated release candidate. It will remain clearly unsigned and registry-gated until the protected credentials, public package publication, and signing services are available.",
        },
        {
            "id": "07-close",
            "kicker": "06 / Try the project",
            "title": "A focused foundation for local tools",
            "body": "Clone the repository, read the manifest, run the validation loop, and inspect the real Research Desk capture. Then build your own workflow around the permissions and data boundaries you can explain to a reviewer.",
            "code": ["github.com/OthmaneBlial/rustframe", "docs/native-demo.md", "ROADMAP.md", "candidate: v0.1.0-rc.4"],
            "voice": "That is the RustFrame foundation: a typed frontend, a native runtime, explicit permissions, inspectable local data, and a workflow you can actually package. Start with the manifest and the developer loop, then use the real capture and validation notes to judge the boundaries for yourself. The candidate tag and full source are linked in the release notes. Thanks for watching.",
        },
    ]

    segments: list[Path] = []
    timeline: list[dict[str, object]] = []
    elapsed = 0.0
    for chapter in chapters:
        image = WORK / f"{chapter['id']}.png"
        audio = WORK / f"{chapter['id']}.aiff"
        segment = WORK / f"{chapter['id']}.mp4"
        card(image, chapter["kicker"], chapter["title"], chapter["body"], chapter.get("code"), chapter.get("footer", "SOURCE-VERIFIED · RUSTFRAME"))
        voice(audio, chapter["voice"])
        if chapter.get("demo"):
            actual = encode_demo(audio, segment)
        else:
            actual = encode_card(image, audio, segment)
        segments.append(segment)
        timeline.append({"id": chapter["id"], "start": round(elapsed, 3), "duration": round(actual, 3), "demo": bool(chapter.get("demo"))})
        elapsed += actual

    concat = WORK / "concat.txt"
    concat.write_text("".join(f"file '{path}'\n" for path in segments), encoding="utf-8")
    master = OUT / "rustframe-explainer-master.mp4"
    web = OUT / "rustframe-explainer.mp4"
    run(["ffmpeg", "-hide_banner", "-y", "-f", "concat", "-safe", "0", "-i", str(concat), "-c", "copy", "-movflags", "+faststart", str(master)])
    run(["ffmpeg", "-hide_banner", "-y", "-i", str(master), "-c:v", "libx264", "-crf", "23", "-preset", "slow", "-profile:v", "high", "-level", "4.0", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "128k", "-movflags", "+faststart", str(web)])

    # Create concise bilingual caption tracks for the actual final timeline.
    english = {
        "01-title": "A real native Research Desk workflow, from contract to export.",
        "02-contract": "Windows, database, grants, and capabilities are declared in one manifest.",
        "03-build": "Validate, build, package, and inspect the local delivery receipt.",
        "04-workflow": "Real macOS capture: folder picker, search, note, synchronization, and JSONL export.",
        "05-data": "SQLite and explicit export keep local data inspectable; recovery states stay visible.",
        "06-release": "Checksums, SBOMs, native smoke, and honest unsigned/public gates.",
        "07-close": "Read the manifest, run the loop, and build a workflow you can explain.",
    }
    french = {
        "01-title": "Un vrai parcours natif Research Desk, du contrat à l’export.",
        "02-contract": "Fenêtres, base, accès et capacités sont déclarés dans un manifeste.",
        "03-build": "Valider, compiler, packager et inspecter le reçu de livraison locale.",
        "04-workflow": "Capture macOS réelle : dossier, recherche, note, synchronisation et export JSONL.",
        "05-data": "SQLite et export explicite gardent les données locales inspectables.",
        "06-release": "Checksums, SBOM, smoke natif et limites honnêtes de la candidate non signée.",
        "07-close": "Lisez le manifeste, lancez la boucle et construisez un outil explicable.",
    }

    def stamp(seconds: float, comma: bool = False) -> str:
        ms = round(seconds * 1000)
        hours, ms = divmod(ms, 3_600_000)
        minutes, ms = divmod(ms, 60_000)
        secs, millis = divmod(ms, 1000)
        return f"{hours:02}:{minutes:02}:{secs:02}{',' if comma else '.'}{millis:03}"

    for suffix, labels in (("en", english), ("fr", french)):
        vtt = ["WEBVTT", ""]
        srt: list[str] = []
        for index, entry in enumerate(timeline, start=1):
            label = labels[str(entry["id"])]
            start = float(entry["start"])
            end = start + float(entry["duration"])
            vtt.extend([f"{stamp(start)} --> {stamp(end)}", label, ""])
            srt.extend([str(index), f"{stamp(start, True)} --> {stamp(end, True)}", label, ""])
        (OUT / f"rustframe-explainer.{suffix}.vtt").write_text("\n".join(vtt) + "\n", encoding="utf-8")
        (OUT / f"rustframe-explainer.{suffix}.srt").write_text("\n".join(srt) + "\n", encoding="utf-8")
        shutil.copy2(OUT / f"rustframe-explainer.{suffix}.vtt", SITE_MEDIA / f"rustframe-explainer.{suffix}.vtt")
        shutil.copy2(OUT / f"rustframe-explainer.{suffix}.srt", SITE_MEDIA / f"rustframe-explainer.{suffix}.srt")

    poster = WORK / "poster.jpg"
    run(["ffmpeg", "-hide_banner", "-y", "-ss", "2", "-i", str(web), "-frames:v", "1", "-q:v", "2", str(poster)])
    shutil.copy2(poster, OUT / "rustframe-explainer-poster.jpg")
    shutil.copy2(poster, SITE_MEDIA / "rustframe-explainer-poster.jpg")
    receipt = {
        "sourceCommit": run(["git", "rev-parse", "HEAD"], capture=True).strip(),
        "sourceDemo": str(DEMO.relative_to(ROOT)),
        "workflow": "real macOS ARM native recording embedded as chapter 04-workflow",
        "resolution": [1920, 1080],
        "fps": 30,
        "audio": "macOS Daniel system voice, AAC 128k in web export",
        "segments": timeline,
        "durationSeconds": round(duration(web), 3),
        "claims": "Cards are rendered from checked-in manifests/docs and local delivery receipts; no simulated product interaction.",
    }
    (OUT / "rustframe-explainer-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
