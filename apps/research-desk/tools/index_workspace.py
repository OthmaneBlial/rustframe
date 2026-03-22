#!/usr/bin/env python3
import datetime
import json
import pathlib
import re
import sys


def parse_frontmatter(text):
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}, text

    metadata = {}
    body_start = 0
    for index in range(1, len(lines)):
        line = lines[index].strip()
        if line == "---":
            body_start = index + 1
            break
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        metadata[key.strip()] = value.strip()

    body = "\n".join(lines[body_start:]).strip()
    return metadata, body


def coerce_tags(value):
    if not value:
        return []
    return [part.strip() for part in value.split(",") if part.strip()]


def extract_title(metadata, body, path):
    if metadata.get("title"):
        return metadata["title"]

    for line in body.splitlines():
        stripped = line.strip()
        if stripped.startswith("# "):
            return stripped[2:].strip()
        if stripped:
            return stripped[:90]

    return path.stem.replace("-", " ").replace("_", " ").title()


def extract_summary(metadata, body):
    if metadata.get("summary"):
        return metadata["summary"]

    snippets = []
    for line in body.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or stripped.startswith("-"):
            continue
        snippets.append(stripped)
        if len(" ".join(snippets)) > 180:
            break

    summary = " ".join(snippets)
    return summary[:220]


def normalize_kind(metadata, path):
    if metadata.get("kind"):
        return metadata["kind"]
    return path.parent.name.replace("-", " ").replace("_", " ")


def normalize_collection(metadata, path):
    if metadata.get("collection"):
        return metadata["collection"]
    return path.parent.name.replace("-", " ").replace("_", " ").title()


def normalize_priority(metadata):
    priority = metadata.get("priority", "watch").strip().lower()
    return priority if priority in {"critical", "watch", "reference"} else "watch"


def normalize_status(metadata):
    status = metadata.get("status", "queued").strip().lower()
    return status if status in {"queued", "reviewing", "ready", "archived"} else "queued"


def normalize_reviewer(metadata):
    return metadata.get("reviewer", "").strip()


def estimate_reading_minutes(metadata, body):
    if metadata.get("readingMinutes", "").isdigit():
        return max(1, int(metadata["readingMinutes"]))

    words = len(re.findall(r"\w+", body))
    return max(1, round(words / 220))


def build_record(root, path):
    text = path.read_text(encoding="utf-8")
    metadata, body = parse_frontmatter(text)
    relative_path = path.relative_to(root).as_posix()
    stat = path.stat()

    return {
        "path": relative_path,
        "title": extract_title(metadata, body, path),
        "collection": normalize_collection(metadata, path),
        "kind": normalize_kind(metadata, path),
        "summary": extract_summary(metadata, body),
        "reviewer": normalize_reviewer(metadata),
        "status": normalize_status(metadata),
        "priority": normalize_priority(metadata),
        "tags": coerce_tags(metadata.get("tags", "")),
        "readingMinutes": estimate_reading_minutes(metadata, body),
        "lineCount": len(text.splitlines()),
        "fileSize": stat.st_size,
        "sourceModifiedAt": datetime.datetime.fromtimestamp(stat.st_mtime).isoformat()
    }


def main():
    root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
    if not root.exists():
        raise SystemExit(f"workspace root does not exist: {root}")

    records = []
    for extension in ("*.md", "*.txt"):
        for path in sorted(root.rglob(extension)):
            if not path.is_file():
                continue
            records.append(build_record(root, path))

    records.sort(key=lambda record: (record["collection"], record["title"], record["path"]))
    print(json.dumps(records, indent=2))


if __name__ == "__main__":
    main()
