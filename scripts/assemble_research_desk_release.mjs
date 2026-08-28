#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const options = parseArguments(process.argv.slice(2));
const inputDir = path.resolve(required(options.input, "--input"));
const outputDir = path.resolve(required(options.output, "--output"));
const tag = required(options.tag, "--tag");
const version = tag.replace(/^research-desk-v/u, "");
const platformDefinitions = {
  "macos-app": { host: "macOS", format: "app", primary: false },
  "macos-dmg": { host: "macOS", format: "dmg", primary: true },
  "windows-nsis": { host: "Windows", format: "nsis", primary: true },
  "windows-msi": { host: "Windows", format: "msi", primary: false },
  "linux-appimage": { host: "Linux", format: "appimage", primary: true },
  "linux-deb": { host: "Linux", format: "deb", primary: false },
};

fs.mkdirSync(outputDir, { recursive: true });
const downloads = [];
const evidence = [];
const sourceCommits = new Set();
const localFirstReportHashes = new Set();
const fileAssociationPolicies = new Set();
let localFirstReportSource = null;
let releaseFileAssociations = null;

for (const [label, definition] of Object.entries(platformDefinitions)) {
  const bundleDir = path.join(inputDir, label);
  if (!fs.statSync(bundleDir, { throwIfNoEntry: false })?.isDirectory()) {
    fail(`verified bundle is missing: ${label}`);
  }
  const packageManifest = readJson(path.join(bundleDir, "rustframe-package-manifest.json"));
  if (packageManifest.schemaVersion !== 1) fail(`${label} has an unsupported package manifest`);
  if (packageManifest.version !== version) {
    fail(`${label} contains version ${packageManifest.version}, expected ${version}`);
  }
  if (!Array.isArray(packageManifest.fileAssociations) || packageManifest.fileAssociations.length === 0) {
    fail(`${label} is missing Research Desk file association metadata`);
  }
  const associationPolicy = JSON.stringify(packageManifest.fileAssociations);
  fileAssociationPolicies.add(associationPolicy);
  releaseFileAssociations ||= packageManifest.fileAssociations;
  const localFirstReportFile = findTopLevelFile(bundleDir, (name) => name === "rustframe-local-first-report.json");
  if (!localFirstReportFile) fail(`${label} is missing the local-first conformance report`);
  const localFirstReport = readJson(localFirstReportFile);
  if (localFirstReport.schemaVersion !== 1 || localFirstReport.kind !== "rustframe.local-first-conformance") {
    fail(`${label} has an unsupported local-first report`);
  }
  if (!localFirstReport.conformant || localFirstReport.policyHash !== packageManifest.policyHash) {
    fail(`${label} local-first policy is not conformant or does not match package metadata`);
  }
  localFirstReportHashes.add(sha256File(localFirstReportFile));
  localFirstReportSource ||= localFirstReportFile;
  const evidenceFile = findTopLevelFile(bundleDir, (name) => name.endsWith("-evidence.json"));
  const sbomFile = findTopLevelFile(bundleDir, (name) => name.endsWith(".spdx.json"));
  if (!evidenceFile || !sbomFile) fail(`${label} is missing evidence or SBOM metadata`);
  const offlineReceiptFile = findTopLevelFile(
    bundleDir,
    (name) => name === `rustframe-offline-${definition.format}-receipt.json`,
  );
  if (!offlineReceiptFile) fail(`${label} is missing its packaged no-server receipt`);
  const offlineReceipt = readJson(offlineReceiptFile);
  if (offlineReceipt.schemaVersion !== 1
    || offlineReceipt.kind !== "rustframe.offline-package-receipt"
    || offlineReceipt.packageFormat !== definition.format
    || offlineReceipt.result !== "passed"
    || offlineReceipt.scope !== "packaged-runtime-without-production-server"
    || !offlineReceipt.checks
    || Object.values(offlineReceipt.checks).some((passed) => passed !== true)) {
    fail(`${label} packaged no-server receipt is incomplete or failed`);
  }
  const verified = readJson(evidenceFile);
  if (verified.version !== version || verified.product !== "Research Desk") {
    fail(`${label} verification evidence identifies a different product or version`);
  }
  if (!verified.sourceCommit || verified.sourceCommit !== packageManifest.sourceCommit) {
    fail(`${label} source commit evidence is missing or inconsistent`);
  }
  if (!verified.verifier?.testedOsVersion) fail(`${label} does not record the tested OS version`);
  sourceCommits.add(verified.sourceCommit);
  if (verified.integrity?.state !== "verified") fail(`${label} integrity is not verified`);
  if (["macOS", "Windows"].includes(definition.host) && verified.signature?.state !== "verified") {
    fail(`${label} native signature is not verified`);
  }

  const evidenceName = `research-desk-${version}-${label}-evidence.json`;
  const sbomName = `research-desk-${version}-${label}.spdx.json`;
  const offlineReceiptName = `research-desk-${version}-${label}-offline-receipt.json`;
  copy(evidenceFile, path.join(outputDir, evidenceName));
  copy(sbomFile, path.join(outputDir, sbomName));
  copy(offlineReceiptFile, path.join(outputDir, offlineReceiptName));
  evidence.push({
    host: definition.host,
    format: definition.format,
    evidence: evidenceName,
    sbom: sbomName,
    offlineReceipt: offlineReceiptName,
  });

  const nativeArtifact = findNativeArtifact(bundleDir, definition.format);
  if (nativeArtifact) {
    const name = path.basename(nativeArtifact);
    copy(nativeArtifact, path.join(outputDir, name));
    downloads.push(releaseRecord(name, definition));
  } else if (definition.format === "app") {
    const transport = findRecursiveFile(
      path.resolve(required(options.transportRoot, "--transport-root")),
      (name) => name === "research-desk-macos-app-verified.tar.gz",
    );
    if (!transport) fail("macOS .app transport archive is missing");
    const name = `Research-Desk-${version}-macOS-app.tar.gz`;
    copy(transport, path.join(outputDir, name));
    downloads.push(releaseRecord(name, definition));
  } else {
    fail(`${label} contains no releaseable native artifact`);
  }
}

if (sourceCommits.size !== 1) fail("release bundles do not share one source commit");
if (localFirstReportHashes.size !== 1 || !localFirstReportSource) {
  fail("release bundles do not share one local-first conformance report");
}
if (fileAssociationPolicies.size !== 1 || !releaseFileAssociations) {
  fail("release bundles do not share one file association policy");
}

const localFirstReportName = `research-desk-${version}-local-first-report.json`;
copy(localFirstReportSource, path.join(outputDir, localFirstReportName));

for (const host of ["macOS", "Windows", "Linux"]) {
  if (downloads.filter((entry) => entry.host === host && entry.primary).length !== 1) {
    fail(`${host} must expose exactly one primary download`);
  }
}

const index = {
  schemaVersion: 1,
  product: "Research Desk",
  tag,
  version,
  sourceCommit: [...sourceCommits][0],
  generatedAt: new Date().toISOString(),
  downloads,
  verification: evidence,
  fileAssociations: releaseFileAssociations,
  localFirstReport: localFirstReportName,
  offlineProof: {
    state: "packaged-runtime-without-production-server",
    receipts: evidence.map(({ host, format, offlineReceipt }) => ({ host, format, offlineReceipt })),
  },
  provenance: {
    state: "github-attested",
    command: `gh attestation verify <download> --repo OthmaneBlial/rustframe`,
  },
};
fs.writeFileSync(path.join(outputDir, "research-desk-release-index.json"), `${JSON.stringify(index, null, 2)}\n`);

const releaseFiles = fs.readdirSync(outputDir)
  .filter((name) => name !== "SHA256SUMS")
  .sort();
const checksumLines = releaseFiles.map((name) => `${sha256File(path.join(outputDir, name))}  ${name}`);
fs.writeFileSync(path.join(outputDir, "SHA256SUMS"), `${checksumLines.join("\n")}\n`);
process.stdout.write(`Assembled ${downloads.length} downloads and ${evidence.length} evidence sets in ${outputDir}\n`);

function parseArguments(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--input") parsed.input = args[++index];
    else if (arg === "--output") parsed.output = args[++index];
    else if (arg === "--tag") parsed.tag = args[++index];
    else if (arg === "--transport-root") parsed.transportRoot = args[++index];
    else fail(`unknown argument: ${arg}`);
  }
  return parsed;
}

function findNativeArtifact(directory, format) {
  const predicates = {
    dmg: (name) => name.endsWith(".dmg"),
    nsis: (name) => name.endsWith(".exe"),
    msi: (name) => name.endsWith(".msi"),
    appimage: (name) => name.endsWith(".AppImage"),
    deb: (name) => name.endsWith(".deb"),
  };
  return predicates[format] ? findTopLevelFile(directory, predicates[format]) : null;
}

function releaseRecord(name, definition) {
  return {
    host: definition.host,
    format: definition.format,
    primary: definition.primary,
    file: name,
    sha256: sha256File(path.join(outputDir, name)),
    bytes: fs.statSync(path.join(outputDir, name)).size,
  };
}

function findTopLevelFile(directory, predicate) {
  const name = fs.readdirSync(directory, { withFileTypes: true })
    .find((entry) => entry.isFile() && predicate(entry.name))?.name;
  return name ? path.join(directory, name) : null;
}

function findRecursiveFile(directory, predicate) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isFile() && predicate(entry.name)) return absolute;
    if (entry.isDirectory()) {
      const nested = findRecursiveFile(absolute, predicate);
      if (nested) return nested;
    }
  }
  return null;
}

function copy(source, destination) {
  if (fs.existsSync(destination)) fail(`release asset name collision: ${path.basename(destination)}`);
  fs.copyFileSync(source, destination);
}

function readJson(file) {
  if (!fs.statSync(file, { throwIfNoEntry: false })?.isFile()) fail(`JSON file is missing: ${file}`);
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function sha256File(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function required(value, label) {
  if (!value) fail(`${label} is required`);
  return value;
}

function fail(message) {
  process.stderr.write(`release assembly failed: ${message}\n`);
  process.exit(1);
}
