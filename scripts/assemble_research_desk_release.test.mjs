import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const assembler = path.resolve("scripts/assemble_research_desk_release.mjs");

test("release assembly exposes one primary download per host and every proof file", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "rustframe-release-assembly-"));
  const staging = path.join(root, "staging");
  const transport = path.join(root, "transport");
  const output = path.join(root, "output");
  const definitions = {
    "macos-app": null,
    "macos-dmg": "Research Desk.dmg",
    "windows-nsis": "Research Desk Setup.exe",
    "windows-msi": "Research Desk.msi",
    "linux-appimage": "Research-Desk.AppImage",
    "linux-deb": "research-desk.deb",
  };

  for (const [label, artifact] of Object.entries(definitions)) {
    const directory = path.join(staging, label);
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(path.join(directory, "rustframe-package-manifest.json"), `${JSON.stringify({
      schemaVersion: 1,
      version: "0.1.0-rc.1",
      sourceCommit: "a".repeat(40),
      policyHash: "sha256:fixture-policy",
    })}\n`);
    fs.writeFileSync(path.join(directory, "rustframe-local-first-report.json"), `${JSON.stringify({
      schemaVersion: 1,
      kind: "rustframe.local-first-conformance",
      conformant: true,
      policyHash: "sha256:fixture-policy",
    })}\n`);
    fs.writeFileSync(path.join(directory, `research-desk-${label}-evidence.json`), `${JSON.stringify({
      product: "Research Desk",
      version: "0.1.0-rc.1",
      sourceCommit: "a".repeat(40),
      verifier: { testedOsVersion: "Fixture OS 1.0" },
      integrity: { state: "verified" },
      signature: { state: label.startsWith("linux") ? "not-applicable" : "verified" },
    })}\n`);
    fs.writeFileSync(path.join(directory, `research-desk-${label}.spdx.json`), "{}\n");
    if (artifact) fs.writeFileSync(path.join(directory, artifact), `${label}\n`);
  }
  const transportDirectory = path.join(transport, "research-desk-verified-macos-app");
  fs.mkdirSync(transportDirectory, { recursive: true });
  fs.writeFileSync(path.join(transportDirectory, "research-desk-macos-app-verified.tar.gz"), "app archive\n");

  const result = spawnSync(process.execPath, [
    assembler,
    "--input", staging,
    "--output", output,
    "--transport-root", transport,
    "--tag", "research-desk-v0.1.0-rc.1",
  ], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);

  const index = JSON.parse(fs.readFileSync(path.join(output, "research-desk-release-index.json"), "utf8"));
  assert.equal(index.downloads.length, 6);
  for (const host of ["macOS", "Windows", "Linux"]) {
    assert.equal(index.downloads.filter((entry) => entry.host === host && entry.primary).length, 1);
  }
  assert.equal(index.verification.length, 6);
  assert.equal(index.localFirstReport, "research-desk-0.1.0-rc.1-local-first-report.json");
  assert.ok(fs.statSync(path.join(output, index.localFirstReport)).isFile());
  assert.match(fs.readFileSync(path.join(output, "SHA256SUMS"), "utf8"), /research-desk-release-index\.json/u);
});
