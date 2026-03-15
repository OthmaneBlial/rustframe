#!/usr/bin/env python3

import argparse
import json
import os
import re
import threading
import time
from dataclasses import dataclass
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from playwright.sync_api import sync_playwright


ROOT = Path(__file__).resolve().parent.parent
SCREENSHOT_DIR = ROOT / "assets" / "screenshots"
APP_CONFIGS = [
    {"id": "hello-rustframe", "path": "apps/hello-rustframe/index.html", "kind": "db"},
    {"id": "daybreak-notes", "path": "apps/daybreak-notes/index.html", "kind": "db"},
    {"id": "atlas-crm", "path": "apps/atlas-crm/index.html", "kind": "db"},
    {"id": "orbit-desk", "path": "apps/orbit-desk/index.html", "kind": "local"},
    {"id": "prism-gallery", "path": "apps/prism-gallery/index.html", "kind": "db"},
    {"id": "quill-studio", "path": "apps/quill-studio/index.html", "kind": "db"},
]


@dataclass
class AppShot:
    app_id: str
    source_path: Path
    kind: str
    width: int
    height: int
    schema: dict | None
    seeds: list[dict]


class RepoRequestHandler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT), **kwargs)

    def log_message(self, *_args):
        return


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Capture RustFrame example app screenshots with a mock native bridge."
    )
    parser.add_argument(
        "--output-dir",
        default=str(SCREENSHOT_DIR),
        help="Directory for generated PNG screenshots",
    )
    parser.add_argument(
        "--app",
        action="append",
        dest="apps",
        help="Capture only the named app id. Repeat to select more than one.",
    )
    return parser.parse_args()


def extract_meta_dimension(html: str, name: str, fallback: int) -> int:
    pattern = rf'<meta\s+name="{re.escape(name)}"\s+content="([^"]+)"'
    match = re.search(pattern, html)
    if not match:
        return fallback
    try:
        return int(float(match.group(1)))
    except ValueError:
        return fallback


def load_app_shot(config: dict) -> AppShot:
    source_path = ROOT / config["path"]
    html = source_path.read_text(encoding="utf-8")
    width = extract_meta_dimension(html, "rustframe:width", 1280)
    height = extract_meta_dimension(html, "rustframe:height", 820)

    schema_path = source_path.parent / "data" / "schema.json"
    seeds_dir = source_path.parent / "data" / "seeds"
    schema = json.loads(schema_path.read_text(encoding="utf-8")) if schema_path.exists() else None
    seeds = []
    if seeds_dir.exists():
        for seed_file in sorted(seeds_dir.glob("*.json")):
            seeds.append(json.loads(seed_file.read_text(encoding="utf-8")))

    return AppShot(
        app_id=config["id"],
        source_path=source_path,
        kind=config["kind"],
        width=width,
        height=height,
        schema=schema,
        seeds=seeds,
    )


def start_server() -> tuple[ThreadingHTTPServer, str]:
    server = ThreadingHTTPServer(("127.0.0.1", 0), RepoRequestHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    host, port = server.server_address
    return server, f"http://{host}:{port}"


def build_db_payload(app: AppShot) -> dict:
    tables = {}
    if app.schema:
        for table in app.schema.get("tables", []):
            tables[table["name"]] = []

    for seed in app.seeds:
        for entry in seed.get("entries", []):
            rows = tables.setdefault(entry["table"], [])
            for index, row in enumerate(entry.get("rows", []), start=1):
                enriched = dict(row)
                enriched.setdefault("id", len(rows) + index)
                enriched.setdefault(
                    "createdAt", f"2026-03-{10 + len(rows) + index:02d}T08:00:00Z"
                )
                enriched.setdefault(
                    "updatedAt", f"2026-03-{10 + len(rows) + index:02d}T12:30:00Z"
                )
                rows.append(enriched)

    return {
        "appId": app.app_id,
        "schemaVersion": (app.schema or {}).get("version", 1),
        "tables": tables,
    }


def build_mock_script(app: AppShot) -> str:
    if app.kind == "db":
        payload = build_db_payload(app)
    else:
        payload = {
            "appId": app.app_id,
            "schemaVersion": 0,
            "tables": {},
        }

    serialized = json.dumps(payload)
    return f"""
(() => {{
    const payload = {serialized};
    const state = JSON.parse(JSON.stringify(payload.tables));
    const nextIds = Object.fromEntries(
        Object.entries(state).map(([table, rows]) => [table, rows.reduce((maxId, row) => Math.max(maxId, Number(row.id) || 0), 0) + 1])
    );

    function clone(value) {{
        return JSON.parse(JSON.stringify(value));
    }}

    function normalizeField(field) {{
        if (field === "createdAt") return "createdAt";
        if (field === "updatedAt") return "updatedAt";
        return field;
    }}

    function compareValues(left, right) {{
        if (left === right) return 0;
        if (left == null) return -1;
        if (right == null) return 1;
        if (typeof left === "number" && typeof right === "number") return left - right;
        return String(left).localeCompare(String(right));
    }}

    function sortRows(rows, orderBy) {{
        if (!Array.isArray(orderBy) || !orderBy.length) {{
            return rows;
        }}

        return rows.sort((left, right) => {{
            for (const rule of orderBy) {{
                const field = normalizeField(rule.field);
                const direction = rule.direction === "desc" ? -1 : 1;
                const result = compareValues(left[field], right[field]);
                if (result !== 0) {{
                    return result * direction;
                }}
            }}
            return 0;
        }});
    }}

    function listRows(table, params) {{
        const rows = clone(state[table] || []);
        sortRows(rows, params.orderBy);
        const offset = Number(params.offset || 0);
        const limit = params.limit == null ? rows.length : Number(params.limit);
        return rows.slice(offset, offset + limit);
    }}

    function getRow(table, id) {{
        return clone((state[table] || []).find((row) => Number(row.id) === Number(id)) || null);
    }}

    function insertRow(table, record) {{
        const row = clone(record);
        row.id = nextIds[table] || 1;
        nextIds[table] = row.id + 1;
        row.createdAt = new Date().toISOString();
        row.updatedAt = row.createdAt;
        state[table] = state[table] || [];
        state[table].push(row);
        return clone(row);
    }}

    function updateRow(table, id, patch) {{
        const rows = state[table] || [];
        const row = rows.find((entry) => Number(entry.id) === Number(id));
        if (!row) {{
            throw new Error(`No record found in ${{table}} for id ${{id}}`);
        }}
        Object.assign(row, clone(patch));
        row.updatedAt = new Date().toISOString();
        return clone(row);
    }}

    function deleteRow(table, id) {{
        const rows = state[table] || [];
        const index = rows.findIndex((entry) => Number(entry.id) === Number(id));
        if (index === -1) {{
            return false;
        }}
        rows.splice(index, 1);
        return true;
    }}

    window.ipc = {{
        postMessage(raw) {{
            const request = JSON.parse(raw);
            const reply = (result) => setTimeout(() => {{
                if (window.RustFrame && typeof window.RustFrame.__resolveFromNative === "function") {{
                    window.RustFrame.__resolveFromNative(result);
                }}
            }}, 0);

            try {{
                let data = null;
                switch (request.method) {{
                    case "window.close":
                    case "window.minimize":
                    case "window.maximize":
                        data = null;
                        break;
                    case "window.setTitle":
                        document.title = request.params.title || document.title;
                        data = null;
                        break;
                    case "db.info":
                        data = {{
                            appId: payload.appId,
                            dataDir: `/tmp/rustframe/${{payload.appId}}`,
                            databasePath: `/tmp/rustframe/${{payload.appId}}/app.db`,
                            schemaVersion: payload.schemaVersion,
                            tables: Object.keys(state)
                        }};
                        break;
                    case "db.get":
                        data = getRow(request.params.table, request.params.id);
                        break;
                    case "db.list":
                        data = listRows(request.params.table, request.params);
                        break;
                    case "db.count":
                        data = listRows(request.params.table, request.params).length;
                        break;
                    case "db.insert":
                        data = insertRow(request.params.table, request.params.record);
                        break;
                    case "db.update":
                        data = updateRow(request.params.table, request.params.id, request.params.patch);
                        break;
                    case "db.delete":
                        data = {{ deleted: deleteRow(request.params.table, request.params.id) }};
                        break;
                    case "fs.readText":
                        data = `// Mock read for ${{request.params.path}}`;
                        break;
                    case "shell.exec":
                        data = {{
                            stdout: "mock shell output",
                            stderr: "",
                            exitCode: 0
                        }};
                        break;
                    default:
                        throw new Error(`Unsupported mock method: ${{request.method}}`);
                }}

                reply({{ id: request.id, ok: true, data }});
            }} catch (error) {{
                reply({{
                    id: request.id,
                    ok: false,
                    data: null,
                    error: {{
                        code: "mock_error",
                        message: String(error && error.message ? error.message : error)
                    }}
                }});
            }}
        }}
    }};
}})();
"""


def wait_for_app(page, app: AppShot) -> None:
    if app.app_id == "orbit-desk":
        page.wait_for_selector("body.is-ready")
        page.wait_for_timeout(250)
        return

    page.wait_for_load_state("networkidle")
    if app.app_id == "hello-rustframe":
        page.wait_for_selector("#notes-list .note-card")
    elif app.app_id == "daybreak-notes":
        page.wait_for_selector("#notes-grid .note-card")
    elif app.app_id == "atlas-crm":
        page.wait_for_selector(".deal-card, .lane-card, [data-action]")
    elif app.app_id == "prism-gallery":
        page.wait_for_selector("#gallery-grid .asset-card")
    elif app.app_id == "quill-studio":
        page.wait_for_selector(".story-card, [data-action]")
    page.wait_for_timeout(250)


def capture_app(base_url: str, app: AppShot, output_dir: Path) -> None:
    relative_path = app.source_path.relative_to(ROOT).as_posix()
    output_path = output_dir / f"{app.app_id}.png"
    mock_script = build_mock_script(app)

    with sync_playwright() as playwright:
        executable_path = os.environ.get("RUSTFRAME_CHROMIUM")
        launch_kwargs = {
            "headless": True,
            "args": ["--no-sandbox"],
        }
        if executable_path:
            launch_kwargs["executable_path"] = executable_path
        browser = playwright.chromium.launch(**launch_kwargs)
        context = browser.new_context(
            viewport={"width": app.width, "height": app.height},
            device_scale_factor=1,
        )
        context.add_init_script(mock_script)
        page = context.new_page()
        page.goto(f"{base_url}/{relative_path}", wait_until="domcontentloaded")
        wait_for_app(page, app)
        output_dir.mkdir(parents=True, exist_ok=True)
        page.screenshot(path=str(output_path), scale="css")
        context.close()
        browser.close()


def main() -> None:
    args = parse_args()
    output_dir = Path(args.output_dir).resolve()
    requested = set(args.apps or [])
    configs = [
        config for config in APP_CONFIGS if not requested or config["id"] in requested
    ]
    if requested:
        known = {config["id"] for config in APP_CONFIGS}
        unknown = requested - known
        if unknown:
            raise SystemExit(f"Unknown app ids: {', '.join(sorted(unknown))}")

    apps = [load_app_shot(config) for config in configs]

    server, base_url = start_server()
    try:
        time.sleep(0.15)
        for app in apps:
            capture_app(base_url, app, output_dir)
            print(f"captured {app.app_id} -> {(output_dir / f'{app.app_id}.png')}")
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    main()
