#!/usr/bin/env node

import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function fail(message) {
  console.error(`template screenshot: ${message}`);
  process.exit(1);
}

function option(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}

function safePath(relativePath, prefix) {
  if (!relativePath || path.isAbsolute(relativePath) || relativePath.includes("..") || relativePath.includes("\\")) {
    fail(`${relativePath || "path"} must be a safe repository-relative path`);
  }
  const absolutePath = path.resolve(repoRoot, relativePath);
  if (!absolutePath.startsWith(path.join(repoRoot, prefix, path.sep))) fail(`${relativePath} must stay under ${prefix}/`);
  return absolutePath;
}

function loadSeeds(projectRoot) {
  const seedRoot = path.join(projectRoot, "data/seeds");
  const tables = {};
  for (const name of fs.readdirSync(seedRoot).filter((item) => item.endsWith(".json")).sort()) {
    const seed = JSON.parse(fs.readFileSync(path.join(seedRoot, name), "utf8"));
    for (const entry of seed.entries || []) {
      tables[entry.table] ||= [];
      tables[entry.table].push(...entry.rows);
    }
  }
  return tables;
}

function contentType(filePath) {
  return {
    ".css": "text/css; charset=utf-8",
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".svg": "image/svg+xml",
  }[path.extname(filePath)] || "application/octet-stream";
}

function startServer(projectRoot) {
  const allowed = new Set(["index.html", "styles.css", "app.js", "assets"]);
  const server = http.createServer((request, response) => {
    const pathname = decodeURIComponent(new URL(request.url || "/", "http://127.0.0.1").pathname);
    const relativePath = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
    if (!allowed.has(relativePath.split("/")[0]) || relativePath.includes("..") || relativePath.includes("\\")) {
      response.writeHead(404).end("Not found");
      return;
    }
    const filePath = path.resolve(projectRoot, relativePath);
    if (!filePath.startsWith(`${projectRoot}${path.sep}`) || !fs.statSync(filePath, { throwIfNoEntry: false })?.isFile()) {
      response.writeHead(404).end("Not found");
      return;
    }
    response.writeHead(200, { "Content-Type": contentType(filePath), "Cache-Control": "no-store" });
    fs.createReadStream(filePath).pipe(response);
  });
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => resolve({ server, port: server.address().port }));
  });
}

const projectArgument = option("--project");
const outputArgument = option("--output");
const width = Number(option("--width"));
const height = Number(option("--height"));
if (!projectArgument || !outputArgument || !Number.isInteger(width) || !Number.isInteger(height)) {
  fail("usage: node scripts/capture_template_screenshot.mjs --project apps/example --output site/assets/screenshots/example.png --width 1440 --height 920");
}
if (width < 640 || height < 480 || width > 4096 || height > 4096) fail("invalid viewport dimensions");

const projectRoot = safePath(projectArgument, "apps");
const outputPath = safePath(outputArgument, "site/assets/screenshots");
for (const required of ["index.html", "styles.css", "app.js", "rustframe.json"]) {
  if (!fs.existsSync(path.join(projectRoot, required))) fail(`${projectArgument} is missing ${required}`);
}

const playwrightUrl = pathToFileURL(path.join(repoRoot, "site/node_modules/playwright/index.mjs"));
const { chromium } = await import(playwrightUrl.href);
const seedTables = loadSeeds(projectRoot);
const { server, port } = await startServer(projectRoot);
const browser = await chromium.launch({ headless: true });

try {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
  const browserErrors = [];
  page.on("pageerror", (error) => browserErrors.push(error.message));
  await page.addInitScript(({ tables, appId }) => {
    const fixedTime = "2026-08-28T10:00:00.000Z";
    const database = Object.fromEntries(Object.entries(tables).map(([table, rows]) => [
      table,
      rows.map((row, index) => ({ id: index + 1, createdAt: fixedTime, updatedAt: fixedTime, ...structuredClone(row) })),
    ]));

    const list = (table, options = {}) => {
      let rows = structuredClone(database[table] || []);
      if (options.where && typeof options.where === "object") {
        rows = rows.filter((row) => Object.entries(options.where).every(([key, value]) => row[key] === value));
      }
      for (const order of [...(options.orderBy || [])].reverse()) {
        rows.sort((left, right) => {
          const result = left[order.field] < right[order.field] ? -1 : left[order.field] > right[order.field] ? 1 : 0;
          return order.direction === "desc" ? -result : result;
        });
      }
      const offset = Number(options.offset || 0);
      return rows.slice(offset, options.limit ? offset + Number(options.limit) : undefined);
    };

    window.RustFrame = {
      security: { model: "local-first", currentWindow: { id: "main" } },
      db: {
        info: async () => ({ databasePath: `/local-data/${appId}/app.db`, schemaVersion: 1, tables: Object.keys(database) }),
        list: async (table, options) => list(table, options),
        search: async (table, query, options = {}) => {
          const needle = String(query || "").toLowerCase();
          return list(table, options).filter((row) => Object.values(row).some((value) => String(value).toLowerCase().includes(needle)));
        },
        insert: async (table, values) => {
          database[table] ||= [];
          const row = { id: database[table].length + 1, createdAt: fixedTime, updatedAt: fixedTime, ...structuredClone(values) };
          database[table].push(row);
          return structuredClone(row);
        },
        update: async (table, id, values) => {
          const row = (database[table] || []).find((item) => item.id === Number(id));
          if (!row) throw new Error(`missing ${table} row ${id}`);
          Object.assign(row, structuredClone(values), { updatedAt: fixedTime });
          return structuredClone(row);
        },
        delete: async (table, id) => {
          database[table] = (database[table] || []).filter((item) => item.id !== Number(id));
          return true;
        },
      },
      window: {
        setTitle: async (title) => { document.title = title; },
        close: async () => undefined,
        create: async () => ({ id: "preview" }),
      },
      clipboard: { writeText: async () => undefined },
    };
  }, { tables: seedTables, appId: path.basename(projectRoot) });

  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto(`http://127.0.0.1:${port}/`, { waitUntil: "networkidle" });
  await page.waitForTimeout(250);
  if (browserErrors.length) fail(`browser error: ${browserErrors.join("; ")}`);
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  await page.screenshot({ path: outputPath, type: "png", fullPage: false });
  console.log(`Captured ${path.relative(repoRoot, outputPath)} at ${width}x${height}`);
} finally {
  await browser.close();
  await new Promise((resolve) => server.close(resolve));
}
