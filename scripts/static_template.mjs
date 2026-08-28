#!/usr/bin/env node

import fs from "node:fs";
import http from "node:http";
import path from "node:path";

const projectRoot = process.cwd();
const mode = process.argv[2];
const publicFiles = ["index.html", "styles.css", "app.js"];

function fail(message) {
  console.error(`static template: ${message}`);
  process.exit(1);
}

function requireProjectFile(relativePath) {
  const absolutePath = path.join(projectRoot, relativePath);
  if (!fs.statSync(absolutePath, { throwIfNoEntry: false })?.isFile()) {
    fail(`missing ${relativePath}`);
  }
  return absolutePath;
}

function copyIfPresent(relativePath, outputRoot) {
  const source = path.join(projectRoot, relativePath);
  if (!fs.existsSync(source)) return;
  fs.cpSync(source, path.join(outputRoot, relativePath), { recursive: true });
}

function build() {
  publicFiles.forEach(requireProjectFile);
  const outputRoot = path.join(projectRoot, "dist");
  fs.rmSync(outputRoot, { recursive: true, force: true });
  fs.mkdirSync(outputRoot, { recursive: true });

  for (const relativePath of publicFiles) {
    fs.copyFileSync(path.join(projectRoot, relativePath), path.join(outputRoot, relativePath));
  }
  copyIfPresent("assets", outputRoot);

  console.log(`Static template built: ${path.relative(projectRoot, outputRoot)}`);
}

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  return index === -1 ? fallback : process.argv[index + 1];
}

function contentType(filePath) {
  return {
    ".css": "text/css; charset=utf-8",
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".json": "application/json; charset=utf-8",
    ".svg": "image/svg+xml",
    ".webp": "image/webp",
  }[path.extname(filePath).toLowerCase()] || "application/octet-stream";
}

function serve() {
  publicFiles.forEach(requireProjectFile);
  const host = option("--host", "127.0.0.1");
  const port = Number(option("--port", "5173"));
  if (host !== "127.0.0.1" || !Number.isInteger(port) || port < 1024 || port > 65535) {
    fail("serve expects --host 127.0.0.1 and a port between 1024 and 65535");
  }

  const allowedRoots = new Set([...publicFiles, "assets"]);
  const server = http.createServer((request, response) => {
    let pathname;
    try {
      pathname = decodeURIComponent(new URL(request.url || "/", `http://${host}:${port}`).pathname);
    } catch {
      response.writeHead(400).end("Bad request");
      return;
    }

    const relativePath = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
    const firstSegment = relativePath.split("/")[0];
    if (!allowedRoots.has(firstSegment) || relativePath.includes("..") || relativePath.includes("\\")) {
      response.writeHead(404).end("Not found");
      return;
    }

    const absolutePath = path.resolve(projectRoot, relativePath);
    if (!absolutePath.startsWith(`${projectRoot}${path.sep}`) || !fs.statSync(absolutePath, { throwIfNoEntry: false })?.isFile()) {
      response.writeHead(404).end("Not found");
      return;
    }

    response.writeHead(200, {
      "Cache-Control": "no-store",
      "Content-Type": contentType(absolutePath),
      "X-Content-Type-Options": "nosniff",
    });
    fs.createReadStream(absolutePath).pipe(response);
  });

  server.listen(port, host, () => {
    console.log(`Static template ready at http://${host}:${port}`);
  });

  const close = () => server.close(() => process.exit(0));
  process.on("SIGINT", close);
  process.on("SIGTERM", close);
}

if (mode === "build") build();
else if (mode === "serve") serve();
else fail("usage: node scripts/static_template.mjs <build|serve> [--host 127.0.0.1] [--port 5173]");
