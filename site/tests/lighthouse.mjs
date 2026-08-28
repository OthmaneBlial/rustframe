import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import zlib from "node:zlib";
import { fileURLToPath } from "node:url";
import lighthouse from "lighthouse";
import * as chromeLauncher from "chrome-launcher";

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const port = 4320;
const mimeTypes = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".md": "text/markdown; charset=utf-8",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".webp": "image/webp",
  ".woff2": "font/woff2",
};

const server = http.createServer((request, response) => {
  const requestPath = decodeURIComponent(new URL(request.url, `http://127.0.0.1:${port}`).pathname);
  const relative = requestPath === "/" ? "index.html" : requestPath.replace(/^\/+/, "");
  const resolved = path.resolve(siteRoot, relative);
  if (!resolved.startsWith(`${siteRoot}${path.sep}`) || relative.startsWith("node_modules/")) {
    response.writeHead(404).end("Not found");
    return;
  }
  fs.readFile(resolved, (error, contents) => {
    if (error) {
      response.writeHead(404).end("Not found");
      return;
    }
    const headers = { "Content-Type": mimeTypes[path.extname(resolved)] || "application/octet-stream", "Cache-Control": "public, max-age=300" };
    if (/\bgzip\b/.test(request.headers["accept-encoding"] || "") && !/\.(?:png|webp|woff2)$/.test(resolved)) {
      zlib.gzip(contents, (gzipError, compressed) => {
        if (gzipError) return response.writeHead(500).end("Compression failed");
        response.writeHead(200, { ...headers, "Content-Encoding": "gzip" }).end(compressed);
      });
      return;
    }
    response.writeHead(200, headers).end(contents);
  });
});

await new Promise((resolve) => server.listen(port, "127.0.0.1", resolve));
const chrome = await chromeLauncher.launch({
  chromePath: process.env.CHROME_PATH,
  chromeFlags: ["--headless", "--no-sandbox", "--disable-gpu"],
});

const pages = ["/", "/docs.html", "/showcase.html", "/benchmarks.html"];
const reports = [];
let failed = false;
try {
  for (const page of pages) {
    const result = await lighthouse(`http://127.0.0.1:${port}${page}`, {
      port: chrome.port,
      output: "json",
      logLevel: "error",
      onlyCategories: ["performance", "accessibility", "best-practices", "seo"],
    });
    const report = {
      page,
      performance: Math.round(result.lhr.categories.performance.score * 100),
      accessibility: Math.round(result.lhr.categories.accessibility.score * 100),
      bestPractices: Math.round(result.lhr.categories["best-practices"].score * 100),
      seo: Math.round(result.lhr.categories.seo.score * 100),
      lcpMs: Math.round(result.lhr.audits["largest-contentful-paint"].numericValue),
      cls: Number(result.lhr.audits["cumulative-layout-shift"].numericValue.toFixed(3)),
    };
    reports.push(report);
    if (report.performance < 95 || report.accessibility < 95 || report.bestPractices < 95 || report.seo < 95 || report.lcpMs > 2500 || report.cls > 0.1) {
      failed = true;
    }
  }
} finally {
  await chrome.kill();
  await new Promise((resolve) => server.close(resolve));
}

console.table(reports);
if (failed) {
  console.error("Lighthouse quality gate failed: every page needs 95+ category scores, LCP <= 2500 ms, and CLS <= 0.1.");
  process.exit(1);
}
