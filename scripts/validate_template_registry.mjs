#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(scriptPath), "..");
const catalogPath = path.join(repoRoot, "examples/community-templates/catalog.json");
const showcasePath = path.join(repoRoot, "site/showcase.json");
const templateSchemaUrl = "https://othmaneblial.github.io/rustframe/schemas/templates/v1/template.schema.json";
const catalogSchemaUrl = "https://othmaneblial.github.io/rustframe/schemas/templates/v1/catalog.schema.json";
const repositoryUrl = "https://github.com/OthmaneBlial/rustframe";
const requiredWorkflows = new Set([
  "document-desk",
  "media-review",
  "offline-inventory",
  "evidence-tracker",
  "batch-operations",
]);
const allowedWorkflows = new Set([...requiredWorkflows, "queue-starter", "editorial-planning"]);
const allowedKinds = new Set(["flagship", "template", "starter", "reference"]);
const allowedProvenance = new Set(["first-party", "community"]);
const allowedCapabilities = new Set(["database", "filesystem", "shell", "window", "multi-window", "clipboard", "file-open"]);
const allowedPlatforms = new Set(["macos", "windows", "linux"]);
const allowedStates = new Set(["verified", "reference", "archived"]);
const allowedProfiles = new Set(["rustframe-static-v1", "rustframe-flagship-v1"]);
const versionPattern = /^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$/u;
const slugPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/u;

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function currentRustFrameVersion() {
  const cargo = fs.readFileSync(path.join(repoRoot, "crates/rustframe-cli/Cargo.toml"), "utf8");
  const version = cargo.match(/^version = "([^"]+)"/mu)?.[1];
  if (!version) throw new Error("could not read the RustFrame CLI version");
  return version;
}

function exactKeys(value, required, optional, label, errors) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    errors.push(`${label} must be an object`);
    return false;
  }
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) errors.push(`${label}.${key} is not allowed`);
  }
  for (const key of required) {
    if (!(key in value)) errors.push(`${label}.${key} is required`);
  }
  return true;
}

function text(value, minimum, maximum, label, errors) {
  if (typeof value !== "string" || value.length < minimum || value.length > maximum) {
    errors.push(`${label} must contain ${minimum}-${maximum} characters`);
    return false;
  }
  return true;
}

function relativePath(value, label, errors) {
  if (typeof value !== "string" || !value || path.isAbsolute(value) || value.includes("\\")) {
    errors.push(`${label} must be a non-empty POSIX relative path`);
    return false;
  }
  if (value.split("/").some((segment) => segment === ".." || segment === "." || segment === "")) {
    errors.push(`${label} must not contain traversal or empty segments`);
    return false;
  }
  return true;
}

function stringList(value, { minimum = 1, maximum = Infinity, allowed, label }, errors) {
  if (!Array.isArray(value) || value.length < minimum || value.length > maximum) {
    errors.push(`${label} must contain ${minimum}-${maximum === Infinity ? "many" : maximum} items`);
    return false;
  }
  const seen = new Set();
  for (const item of value) {
    if (typeof item !== "string" || !item) errors.push(`${label} contains an invalid item`);
    if (seen.has(item)) errors.push(`${label} contains duplicate '${item}'`);
    if (allowed && !allowed.has(item)) errors.push(`${label} contains unsupported '${item}'`);
    seen.add(item);
  }
  return true;
}

function httpsUrl(value, label, errors) {
  try {
    if (new URL(value).protocol !== "https:") throw new Error("not HTTPS");
  } catch {
    errors.push(`${label} must be an HTTPS URL`);
  }
}

function imageDimensions(filePath) {
  const data = fs.readFileSync(filePath);
  if (data.toString("ascii", 0, 4) !== "RIFF" || data.toString("ascii", 8, 12) !== "WEBP") {
    throw new Error("expected a WebP RIFF image");
  }
  const chunk = data.toString("ascii", 12, 16);
  if (chunk === "VP8 ") {
    return { width: data.readUInt16LE(26) & 0x3fff, height: data.readUInt16LE(28) & 0x3fff };
  }
  if (chunk === "VP8X") {
    return { width: 1 + data.readUIntLE(24, 3), height: 1 + data.readUIntLE(27, 3) };
  }
  if (chunk === "VP8L") {
    const bits = data.readUInt32LE(21);
    return { width: 1 + (bits & 0x3fff), height: 1 + ((bits >>> 14) & 0x3fff) };
  }
  throw new Error(`unsupported WebP chunk ${chunk}`);
}

function ensureStaticProfile(sourceRoot, label, errors) {
  const manifestPath = path.join(sourceRoot, "rustframe.json");
  const packagePath = path.join(sourceRoot, "package.json");
  if (!fs.existsSync(manifestPath)) errors.push(`${label} is missing rustframe.json`);
  if (!fs.existsSync(packagePath)) errors.push(`${label} is missing package.json`);
  if (!fs.existsSync(manifestPath) || !fs.existsSync(packagePath)) return;

  const appManifest = readJson(manifestPath);
  const packageJson = readJson(packagePath);
  if (appManifest.frontend?.devCommand !== "npm run dev" || appManifest.frontend?.buildCommand !== "npm run build") {
    errors.push(`${label} must use the fixed npm run dev/build frontend contract`);
  }
  const expectedScripts = {
    dev: "node ../../scripts/static_template.mjs serve --host 127.0.0.1 --port 5173",
    build: "node ../../scripts/static_template.mjs build",
  };
  if (!isDeepStrictEqual(packageJson.scripts, expectedScripts)) {
    errors.push(`${label} package scripts differ from the non-extensible static template profile`);
  }
}

export function validateTemplateManifest(manifest, options = {}) {
  const errors = [];
  const manifestPath = options.manifestPath ? path.resolve(options.manifestPath) : undefined;
  const root = options.repoRoot ? path.resolve(options.repoRoot) : repoRoot;
  const runtimeVersion = options.runtimeVersion || currentRustFrameVersion();
  const label = options.label || manifest?.id || "template";
  const required = ["$schema", "schemaVersion", "id", "title", "kind", "provenance", "workflow", "summary", "bestFor", "capabilities", "source", "author", "license", "platforms", "rustframe", "verification", "screenshot"];
  if (!exactKeys(manifest, required, ["caseStudy"], label, errors)) return errors;

  if (manifest.$schema !== templateSchemaUrl) errors.push(`${label} uses an unsupported schema URL`);
  if (manifest.schemaVersion !== 1) errors.push(`${label} uses an unsupported schema version`);
  if (!slugPattern.test(manifest.id || "")) errors.push(`${label}.id must be a lowercase slug`);
  text(manifest.title, 3, 80, `${label}.title`, errors);
  if (!allowedKinds.has(manifest.kind)) errors.push(`${label}.kind is unsupported`);
  if (!allowedProvenance.has(manifest.provenance)) errors.push(`${label}.provenance is unsupported`);
  if (!allowedWorkflows.has(manifest.workflow)) errors.push(`${label}.workflow is unsupported`);
  text(manifest.summary, 40, 240, `${label}.summary`, errors);
  stringList(manifest.bestFor, { minimum: 2, maximum: 5, label: `${label}.bestFor` }, errors);
  stringList(manifest.capabilities, { allowed: allowedCapabilities, label: `${label}.capabilities` }, errors);

  if (exactKeys(manifest.source, ["repository", "path"], [], `${label}.source`, errors)) {
    if (manifest.source.repository !== repositoryUrl) errors.push(`${label}.source.repository must use the canonical repository`);
    if (relativePath(manifest.source.path, `${label}.source.path`, errors)) {
      const sourceRoot = path.resolve(root, manifest.source.path);
      if (!sourceRoot.startsWith(`${root}${path.sep}`) || !fs.statSync(sourceRoot, { throwIfNoEntry: false })?.isDirectory()) {
        errors.push(`${label}.source.path does not resolve to a repository directory`);
      }
      if (manifestPath) {
        const owningSource = path.dirname(path.dirname(manifestPath));
        if (sourceRoot !== owningSource) errors.push(`${label}.source.path does not own its template manifest`);
      }
      if (manifest.verification?.profile === "rustframe-static-v1") ensureStaticProfile(sourceRoot, label, errors);
      if (manifest.verification?.profile === "rustframe-flagship-v1" && manifest.source.path !== "apps/research-desk") {
        errors.push(`${label} uses the flagship profile outside apps/research-desk`);
      }
    }
  }

  if (exactKeys(manifest.author, ["name", "url"], [], `${label}.author`, errors)) {
    text(manifest.author.name, 2, 80, `${label}.author.name`, errors);
    httpsUrl(manifest.author.url, `${label}.author.url`, errors);
  }
  if (exactKeys(manifest.license, ["spdx", "file"], [], `${label}.license`, errors)) {
    if (!/^[A-Za-z0-9-.+]+$/u.test(manifest.license.spdx || "")) errors.push(`${label}.license.spdx is invalid`);
    if (relativePath(manifest.license.file, `${label}.license.file`, errors) && !fs.existsSync(path.join(root, manifest.license.file))) {
      errors.push(`${label}.license.file does not exist`);
    }
  }
  stringList(manifest.platforms, { allowed: allowedPlatforms, label: `${label}.platforms` }, errors);

  if (exactKeys(manifest.rustframe, ["minimumVersion", "testedVersion"], [], `${label}.rustframe`, errors)) {
    for (const field of ["minimumVersion", "testedVersion"]) {
      if (!versionPattern.test(manifest.rustframe[field] || "")) errors.push(`${label}.rustframe.${field} is invalid`);
    }
    if (manifest.rustframe.testedVersion !== runtimeVersion) {
      errors.push(`${label} was not tested against current RustFrame ${runtimeVersion}`);
    }
  }

  if (exactKeys(manifest.verification, ["state", "profile", "lastVerifiedAt"], [], `${label}.verification`, errors)) {
    if (!allowedStates.has(manifest.verification.state)) errors.push(`${label}.verification.state is unsupported`);
    if (!allowedProfiles.has(manifest.verification.profile)) errors.push(`${label}.verification.profile is unsupported`);
    if (!/^\d{4}-\d{2}-\d{2}$/u.test(manifest.verification.lastVerifiedAt || "")) {
      errors.push(`${label}.verification.lastVerifiedAt must use YYYY-MM-DD`);
    }
    if (manifest.verification.state === "verified") {
      const expectedPlatforms = ["linux", "macos", "windows"];
      if (!isDeepStrictEqual([...manifest.platforms].sort(), expectedPlatforms)) {
        errors.push(`${label} cannot be verified without macOS, Windows, and Linux support`);
      }
    }
  }

  if (exactKeys(manifest.screenshot, ["path", "width", "height", "alt"], [], `${label}.screenshot`, errors)) {
    if (relativePath(manifest.screenshot.path, `${label}.screenshot.path`, errors)) {
      if (!/^site\/assets\/screenshots\/[a-z0-9-]+\.webp$/u.test(manifest.screenshot.path)) {
        errors.push(`${label}.screenshot.path must use the public WebP screenshot folder`);
      } else {
        const imagePath = path.join(root, manifest.screenshot.path);
        if (!fs.existsSync(imagePath)) errors.push(`${label}.screenshot.path does not exist`);
        else {
          try {
            const dimensions = imageDimensions(imagePath);
            if (dimensions.width !== manifest.screenshot.width || dimensions.height !== manifest.screenshot.height) {
              errors.push(`${label}.screenshot dimensions do not match the image`);
            }
          } catch (error) {
            errors.push(`${label}.screenshot is invalid: ${error.message}`);
          }
        }
      }
    }
    if (!Number.isInteger(manifest.screenshot.width) || !Number.isInteger(manifest.screenshot.height)) {
      errors.push(`${label}.screenshot dimensions must be integers`);
    }
    text(manifest.screenshot.alt, 20, 180, `${label}.screenshot.alt`, errors);
  }
  if (manifest.caseStudy !== undefined) httpsUrl(manifest.caseStudy, `${label}.caseStudy`, errors);

  return errors;
}

function showcaseEntry(manifest) {
  const entry = {
    id: manifest.id,
    title: manifest.title,
    source: manifest.source.path,
    category: manifest.kind,
    provenance: manifest.provenance,
    workflow: manifest.workflow,
    summary: manifest.summary,
    bestFor: manifest.bestFor,
    capabilities: manifest.capabilities,
    author: manifest.author,
    license: manifest.license.spdx,
    platforms: manifest.platforms,
    rustframe: manifest.rustframe,
    verification: manifest.verification,
    screenshot: manifest.screenshot.path.replace(/^site\//u, ""),
    width: manifest.screenshot.width,
    height: manifest.screenshot.height,
    alt: manifest.screenshot.alt,
    href: `${manifest.source.repository}/tree/main/${manifest.source.path}`,
  };
  if (manifest.caseStudy) entry.caseStudy = manifest.caseStudy;
  return entry;
}

export function validateRegistry(options = {}) {
  const root = options.repoRoot ? path.resolve(options.repoRoot) : repoRoot;
  const selectedCatalogPath = options.catalogPath ? path.resolve(options.catalogPath) : path.join(root, "examples/community-templates/catalog.json");
  const catalog = readJson(selectedCatalogPath);
  const errors = [];
  if (!exactKeys(catalog, ["$schema", "schemaVersion", "templates"], [], "catalog", errors)) return { errors, manifests: [], showcase: { schemaVersion: 1, templates: [] } };
  if (catalog.$schema !== catalogSchemaUrl) errors.push("catalog uses an unsupported schema URL");
  if (catalog.schemaVersion !== 1) errors.push("catalog uses an unsupported schema version");
  if (!Array.isArray(catalog.templates) || catalog.templates.length < 5) errors.push("catalog must contain at least five templates");

  const manifests = [];
  const seenPaths = new Set();
  const seenIds = new Set();
  for (const [index, entry] of (catalog.templates || []).entries()) {
    if (!exactKeys(entry, ["manifest"], [], `catalog.templates[${index}]`, errors)) continue;
    if (!/^apps\/[a-z0-9-]+\/\.rustframe\/template\.json$/u.test(entry.manifest || "")) {
      errors.push(`catalog.templates[${index}].manifest is not an allowed in-repository manifest path`);
      continue;
    }
    if (seenPaths.has(entry.manifest)) errors.push(`catalog repeats ${entry.manifest}`);
    seenPaths.add(entry.manifest);
    const absolutePath = path.join(root, entry.manifest);
    if (!fs.existsSync(absolutePath)) {
      errors.push(`${entry.manifest} does not exist`);
      continue;
    }
    const manifest = readJson(absolutePath);
    errors.push(...validateTemplateManifest(manifest, { manifestPath: absolutePath, repoRoot: root, label: entry.manifest }));
    if (seenIds.has(manifest.id)) errors.push(`catalog repeats template id ${manifest.id}`);
    seenIds.add(manifest.id);
    manifests.push(manifest);
  }

  const verifiedWorkflows = new Set(manifests.filter((item) => item.verification?.state === "verified").map((item) => item.workflow));
  for (const workflow of requiredWorkflows) {
    if (!verifiedWorkflows.has(workflow)) errors.push(`catalog has no verified ${workflow} workflow`);
  }

  return { errors, manifests, showcase: { schemaVersion: 1, templates: manifests.map(showcaseEntry) } };
}

function verifyPublishedSchemas(errors) {
  for (const name of ["template.schema.json", "catalog.schema.json"]) {
    const canonical = readJson(path.join(repoRoot, "schemas/templates/v1", name));
    const published = readJson(path.join(repoRoot, "site/schemas/templates/v1", name));
    if (!isDeepStrictEqual(canonical, published)) errors.push(`site schema mirror is stale: ${name}`);
  }
}

function main() {
  const write = process.argv.includes("--write");
  const { errors, manifests, showcase } = validateRegistry();
  verifyPublishedSchemas(errors);
  const expected = `${JSON.stringify(showcase, null, 2)}\n`;
  if (write && errors.length === 0) fs.writeFileSync(showcasePath, expected);
  else if (!write && fs.readFileSync(showcasePath, "utf8") !== expected) errors.push("site/showcase.json is stale; run node scripts/validate_template_registry.mjs --write");

  if (errors.length) {
    console.error(`Template registry failed with ${errors.length} error(s):`);
    errors.forEach((error) => console.error(`- ${error}`));
    process.exitCode = 1;
    return;
  }
  console.log(`Template registry passed for ${manifests.length} manifests and ${requiredWorkflows.size} required workflows.`);
}

if (path.resolve(process.argv[1] || "") === scriptPath) main();
