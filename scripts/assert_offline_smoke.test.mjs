import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const verifier = path.resolve("scripts/assert_offline_smoke.mjs");

test("records an embedded packaged runtime with no production server", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rustframe-offline-smoke-"));
  const input = path.join(directory, "runtime.json");
  const output = path.join(directory, "receipt.json");
  fs.writeFileSync(input, `${JSON.stringify(validSmoke())}\n`);
  const result = run(input, output);
  assert.equal(result.status, 0, result.stderr);
  const receipt = JSON.parse(fs.readFileSync(output, "utf8"));
  assert.equal(receipt.kind, "rustframe.offline-package-receipt");
  assert.equal(receipt.result, "passed");
  assert.equal(receipt.scope, "packaged-runtime-without-production-server");
  assert.ok(Object.values(receipt.checks).every(Boolean));
});

test("refuses a dev-server launch or missing local database", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rustframe-offline-smoke-"));
  const input = path.join(directory, "runtime.json");
  const output = path.join(directory, "receipt.json");
  const smoke = validSmoke();
  smoke.launchMode = "dev-server";
  smoke.activeDevUrl = "http://127.0.0.1:5173";
  smoke.database = null;
  fs.writeFileSync(input, `${JSON.stringify(smoke)}\n`);
  const result = run(input, output);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /embeddedLaunch, noActiveDevUrl, sqliteOpened/u);
  assert.equal(fs.existsSync(output), false);
});

function run(input, output) {
  return spawnSync(process.execPath, [
    verifier,
    "--input", input,
    "--format", "appimage",
    "--output", output,
  ], { encoding: "utf8" });
}

function validSmoke() {
  return {
    appId: "research-desk",
    launchMode: "embedded",
    activeDevUrl: null,
    security: { model: "local-first", database: true },
    hasIndexHtml: true,
    bridgeInjected: true,
    database: { schemaVersion: 2, tables: ["documents", "settings"] },
  };
}
