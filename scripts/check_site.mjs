import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const siteRoot = path.join(repoRoot, "site");
const htmlPages = ["index.html", "docs.html", "showcase.html", "benchmarks.html"];
const expectedUrls = new Set([
  "https://othmaneblial.github.io/rustframe/",
  "https://othmaneblial.github.io/rustframe/docs.html",
  "https://othmaneblial.github.io/rustframe/showcase.html",
  "https://othmaneblial.github.io/rustframe/benchmarks.html",
]);
const errors = [];

function fail(message) {
  errors.push(message);
}

function read(relativePath) {
  return fs.readFileSync(path.join(siteRoot, relativePath), "utf8");
}

function localPath(reference, page) {
  const cleaned = reference.split("#")[0].split("?")[0];
  if (!cleaned || cleaned.startsWith("#") || cleaned.startsWith("%23") || /^(?:[a-z]+:)?\/\//i.test(cleaned) || /^(?:mailto|tel|data):/i.test(cleaned)) {
    return null;
  }
  return path.resolve(siteRoot, path.dirname(page), cleaned);
}

for (const page of htmlPages) {
  const html = read(page);
  const title = html.match(/<title>([^<]+)<\/title>/i)?.[1]?.trim() || "";
  const description = html.match(/<meta\s+name="description"\s+content="([^"]+)"/i)?.[1]?.trim() || "";
  const canonical = html.match(/<link\s+rel="canonical"\s+href="([^"]+)"/i)?.[1] || "";
  const h1Count = (html.match(/<h1(?:\s|>)/gi) || []).length;

  if (title.length < 30 || title.length > 60) fail(`${page}: title length is ${title.length}, expected 30-60`);
  if (description.length < 120 || description.length > 160) fail(`${page}: description length is ${description.length}, expected 120-160`);
  if (!expectedUrls.has(canonical)) fail(`${page}: unexpected or missing canonical URL`);
  if (h1Count !== 1) fail(`${page}: expected exactly one static h1, found ${h1Count}`);
  if (!/<meta\s+name="viewport"/i.test(html)) fail(`${page}: viewport meta is missing`);
  if (!/<meta\s+property="og:image"/i.test(html)) fail(`${page}: social image is missing`);

  for (const match of html.matchAll(/<script\s+type="application\/ld\+json">([\s\S]*?)<\/script>/gi)) {
    try {
      JSON.parse(match[1]);
    } catch (error) {
      fail(`${page}: JSON-LD does not parse (${error.message})`);
    }
  }

  for (const match of html.matchAll(/<(?:a|link|script|img)\b[^>]*(?:href|src)="([^"]+)"[^>]*>/gi)) {
    const resolved = localPath(match[1], page);
    if (resolved && !fs.existsSync(resolved)) fail(`${page}: missing local target ${match[1]}`);
  }

  for (const match of html.matchAll(/<img\b([^>]*)>/gi)) {
    const attributes = match[1];
    if (!/\balt="[^"]*"/i.test(attributes)) fail(`${page}: image is missing alt text`);
    if (!/\bwidth="\d+"/i.test(attributes) || !/\bheight="\d+"/i.test(attributes)) {
      fail(`${page}: image is missing intrinsic dimensions`);
    }
  }
}

const css = read("styles.css");
for (const match of css.matchAll(/url\(["']?([^"')]+)["']?\)/gi)) {
  const resolved = localPath(match[1], "styles.css");
  if (resolved && !fs.existsSync(resolved)) fail(`styles.css: missing asset ${match[1]}`);
}

const showcase = JSON.parse(read("showcase.json"));
if (showcase.schemaVersion !== 1) fail("showcase.json: unsupported schema version");
const verifiedWorkflows = new Set();
for (const item of showcase.templates || []) {
  if (!item.id || !item.title || !item.source || !item.category || !item.provenance || !item.workflow || !item.summary || !item.href) {
    fail(`showcase.json: ${item.id || "unnamed entry"} is incomplete`);
  }
  if (item.verification?.state === "verified") verifiedWorkflows.add(item.workflow);
  if (!item.screenshot?.endsWith(".webp")) fail(`showcase.json: ${item.id} does not use WebP`);
  if (!fs.existsSync(path.join(siteRoot, item.screenshot || ""))) fail(`showcase.json: missing ${item.screenshot}`);
  if (!Number.isInteger(item.width) || !Number.isInteger(item.height)) fail(`showcase.json: ${item.id} needs image dimensions`);
  if (!item.alt || item.alt.length < 20) fail(`showcase.json: ${item.id} needs descriptive alt text`);
  if (!item.author?.name || !item.author?.url || !item.license) fail(`showcase.json: ${item.id} needs author and license metadata`);
  if (!Array.isArray(item.platforms) || item.platforms.length !== 3) fail(`showcase.json: ${item.id} needs all supported platforms`);
  if (!item.rustframe?.testedVersion || !item.verification?.lastVerifiedAt) fail(`showcase.json: ${item.id} needs versioned verification metadata`);
}
for (const workflow of ["document-desk", "media-review", "offline-inventory", "evidence-tracker", "batch-operations"]) {
  if (!verifiedWorkflows.has(workflow)) fail(`showcase.json: missing verified ${workflow} workflow`);
}

const benchmark = JSON.parse(read("assets/data/research-desk-benchmark.json"));
if (benchmark.schemaVersion !== 1 || benchmark.product !== "Research Desk") {
  fail("research-desk-benchmark.json: unsupported identity or schema");
}
if (!/^[a-f0-9]{40}$/u.test(benchmark.sourceCommit || "")) {
  fail("research-desk-benchmark.json: source commit is missing or invalid");
}
for (const [name, value] of [
  ["package size", benchmark.metrics?.packageSize?.bytes],
  ["cold start", benchmark.metrics?.coldStart?.medianMs],
  ["peak memory", benchmark.metrics?.peakMemory?.medianBytes],
  ["indexing", benchmark.metrics?.indexing?.documentsPerSecond],
  ["warm rebuild", benchmark.metrics?.warmRebuild?.medianMs],
]) {
  if (!Number.isFinite(value) || value <= 0) fail(`research-desk-benchmark.json: ${name} metric is invalid`);
}
const canonicalBenchmark = fs.readFileSync(path.join(repoRoot, "benchmarks/research-desk/latest.json"), "utf8");
if (canonicalBenchmark !== read("assets/data/research-desk-benchmark.json")) {
  fail("research-desk-benchmark.json: public receipt differs from the canonical benchmark");
}

const sitemap = read("sitemap.xml");
for (const url of expectedUrls) {
  if (!sitemap.includes(`<loc>${url}</loc>`)) fail(`sitemap.xml: missing ${url}`);
}
if (!read("robots.txt").includes("/rustframe/sitemap.xml")) fail("robots.txt: sitemap URL is missing");
if (!read("llms.txt").includes("## Primary pages")) fail("llms.txt: primary page map is missing");
if (fs.statSync(path.join(siteRoot, "assets/screenshots/research-desk.webp")).size > 250_000) {
  fail("research-desk.webp exceeds the 250 KB proof-image budget");
}

await import(pathToFileURL(path.join(siteRoot, "codegen.js")));
const codegen = globalThis.RustFrameSiteTools;
const exampleSchema = JSON.parse(read("examples/schema.json"));
const expectedTypes = read("examples/rustframe.generated.ts");
if (!codegen || codegen.renderTypescript(exampleSchema) !== expectedTypes) {
  fail("codegen.js: browser output does not match the CLI-owned golden fixture");
} else {
  const starterZip = codegen.buildStarterZip(exampleSchema);
  if (starterZip.length < 1024 || starterZip[0] !== 0x50 || starterZip[1] !== 0x4b) {
    fail("codegen.js: generated starter is not a valid ZIP payload");
  }
}

if (errors.length) {
  console.error(`Site contract failed with ${errors.length} error(s):`);
  errors.forEach((error) => console.error(`- ${error}`));
  process.exit(1);
}

console.log(`Site contract passed for ${htmlPages.length} pages and ${showcase.templates.length} showcase entries.`);
