import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const verifier = path.resolve("scripts/verify_release_artifacts.mjs");

test("release verifier writes integrity, SBOM, and provenance evidence", () => {
  const directory = createFixture();
  const result = runVerifier(directory);
  assert.equal(result.status, 0, result.stderr);
  const evidence = JSON.parse(fs.readFileSync(path.join(directory, "evidence.json"), "utf8"));
  assert.equal(evidence.integrity.state, "verified");
  assert.equal(evidence.signature.state, "not-applicable");
  assert.match(evidence.sbom.name, /\.spdx\.json$/u);
  assert.equal(evidence.integrity.artifacts.length, 1);
});

test("release verifier rejects an artifact changed after checksums were written", () => {
  const directory = createFixture();
  fs.appendFileSync(path.join(directory, "research-desk.deb"), "tampered");
  const result = runVerifier(directory);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /checksum mismatch/u);
});

test("release verifier rejects package metadata that disagrees with checksums", () => {
  const directory = createFixture();
  const manifestPath = path.join(directory, "rustframe-package-manifest.json");
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  manifest.artifacts[0].sha256 = "0".repeat(64);
  fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  const result = runVerifier(directory);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /manifest checksum does not match/u);
});

test("release verifier fails closed for unsigned native-host bundles", () => {
  const directory = createFixture();
  const result = runVerifier(directory, ["--platform", "macos"]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /bundle is unsigned/u);
});

function createFixture() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rustframe-release-proof-"));
  const artifact = path.join(directory, "research-desk.deb");
  fs.writeFileSync(artifact, "native package fixture\n");
  const sha256 = crypto.createHash("sha256").update(fs.readFileSync(artifact)).digest("hex");
  fs.writeFileSync(path.join(directory, "SHA256SUMS"), `${sha256}  research-desk.deb\n`);
  fs.writeFileSync(path.join(directory, "rustframe-package-manifest.json"), `${JSON.stringify({
    schemaVersion: 1,
    appId: "research-desk",
    productName: "Research Desk",
    version: "0.1.0-rc.1",
    signed: false,
    signatureState: "unsigned",
    artifacts: [{
      format: "deb",
      path: artifact,
      sha256,
      bytes: fs.statSync(artifact).size,
    }],
  }, null, 2)}\n`);
  fs.writeFileSync(path.join(directory, "research-desk.spdx.json"), "{}\n");
  return directory;
}

function runVerifier(directory, platformArguments = ["--platform", "linux"]) {
  return spawnSync(process.execPath, [
    verifier,
    "--dir", directory,
    ...platformArguments,
    "--require-sbom",
    "--output", path.join(directory, "evidence.json"),
  ], {
    encoding: "utf8",
    env: {
      ...process.env,
      GITHUB_ACTIONS: "false",
      GITHUB_SHA: "",
      RUSTFRAME_SOURCE_COMMIT: "",
    },
  });
}
