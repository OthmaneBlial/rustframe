#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { buildIndexedRecord, isSourceUnchanged } from "../apps/research-desk/indexing.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const appDir = path.join(repoRoot, "apps/research-desk");
const options = parseArguments(process.argv.slice(2));
const hostFormat = process.platform === "darwin" ? "app" : process.platform === "linux" ? "appimage" : null;

if (!hostFormat) fail(`benchmark preparation is not implemented for ${process.platform}`);
if (!options.skipPrepare) preparePackage(hostFormat);

const manifestPath = path.join(appDir, "dist/packages/rustframe-package-manifest.json");
const packageManifest = readJson(manifestPath);
const packageArtifact = selectPackageArtifact(packageManifest, hostFormat);
const binaryPath = resolveBenchmarkBinary(packageArtifact.path, hostFormat);

const coldStart = benchmarkColdStart(binaryPath, options.iterations);
const rebuild = benchmarkRebuild(options.iterations);
const indexing = benchmarkIndexing(options.documents, options.iterations);
const sourceCommit = runText("git", ["rev-parse", "HEAD"]);
const collectedAt = new Date().toISOString();
const result = {
  schemaVersion: 1,
  product: "Research Desk",
  version: packageManifest.version,
  sourceCommit,
  collectedAt,
  host: hostReceipt(),
  methodology: {
    iterations: options.iterations,
    packageCommand: `rustframe package --format ${hostFormat} --verify`,
    coldStart: "Packaged embedded-runtime process start with a fresh data directory and smoke receipt on every iteration.",
    peakMemory: "Maximum resident set size reported by the host time utility for the same fresh-data process starts.",
    indexing: `Exact Research Desk frontmatter, metadata, and fingerprint code over ${options.documents.toLocaleString("en-US")} deterministic Markdown documents; filesystem IPC and SQLite commit time are excluded.`,
    warmRebuild: "Repeated production Vite builds after one uncounted priming build; this is a full warm build, not HMR latency.",
  },
  metrics: {
    packageSize: {
      bytes: packageArtifact.bytes,
      mebibytes: toMebibytes(packageArtifact.bytes),
      artifact: path.basename(packageArtifact.path),
      format: hostFormat,
      signatureState: packageManifest.signatureState,
    },
    coldStart: {
      medianMs: median(coldStart.elapsedMs),
      p95Ms: percentile(coldStart.elapsedMs, 0.95),
      samplesMs: coldStart.elapsedMs,
    },
    peakMemory: {
      medianBytes: median(coldStart.peakMemoryBytes),
      medianMebibytes: toMebibytes(median(coldStart.peakMemoryBytes)),
      samplesBytes: coldStart.peakMemoryBytes,
    },
    indexing: {
      documents: options.documents,
      corpusBytes: indexing.corpusBytes,
      medianMs: median(indexing.elapsedMs),
      documentsPerSecond: round(options.documents / (median(indexing.elapsedMs) / 1000), 0),
      samplesMs: indexing.elapsedMs,
      checksum: indexing.checksum,
    },
    warmReindex: {
      documents: options.documents,
      medianMs: median(indexing.warmElapsedMs),
      documentsPerSecond: round(options.documents / (median(indexing.warmElapsedMs) / 1000), 0),
      samplesMs: indexing.warmElapsedMs,
    },
    warmRebuild: {
      medianMs: median(rebuild),
      p95Ms: percentile(rebuild, 0.95),
      samplesMs: rebuild,
    },
  },
  limitations: [
    "This is one transparent host receipt, not a universal performance claim.",
    "Fresh-data start does not flush the operating system file cache.",
    "The indexing metric isolates the exact application parser and fingerprint path; native filesystem IPC and SQLite transaction time require a separate end-to-end receipt.",
    "Unsigned local package size is recorded honestly and does not represent a trusted public download.",
  ],
};

for (const output of [options.output, options.siteOutput]) {
  const destination = path.resolve(repoRoot, output);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.writeFileSync(destination, `${JSON.stringify(result, null, 2)}\n`);
}

process.stdout.write(`${JSON.stringify({
  output: options.output,
  siteOutput: options.siteOutput,
  packageMiB: result.metrics.packageSize.mebibytes,
  coldStartMedianMs: result.metrics.coldStart.medianMs,
  peakMemoryMiB: result.metrics.peakMemory.medianMebibytes,
  indexingDocumentsPerSecond: result.metrics.indexing.documentsPerSecond,
  warmRebuildMedianMs: result.metrics.warmRebuild.medianMs,
}, null, 2)}\n`);

function parseArguments(args) {
  const parsed = {
    documents: 5000,
    iterations: 5,
    output: "benchmarks/research-desk/latest.json",
    siteOutput: "site/assets/data/research-desk-benchmark.json",
    skipPrepare: false,
  };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--documents") parsed.documents = positiveInteger(args[++index], arg);
    else if (arg === "--iterations") parsed.iterations = positiveInteger(args[++index], arg);
    else if (arg === "--output") parsed.output = required(args[++index], arg);
    else if (arg === "--site-output") parsed.siteOutput = required(args[++index], arg);
    else if (arg === "--skip-prepare") parsed.skipPrepare = true;
    else fail(`unknown argument: ${arg}`);
  }
  return parsed;
}

function preparePackage(format) {
  run("cargo", [
    "run", "-p", "rustframe-cli", "--",
    "--project", "apps/research-desk",
    "package", "--format", format, "--verify",
  ], {
    ...process.env,
    RUSTFRAME_RUNTIME_PATH: path.join(repoRoot, "crates/rustframe"),
  });
}

function benchmarkColdStart(binary, iterations) {
  const elapsedMs = [];
  const peakMemoryBytes = [];
  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const receiptRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rustframe-benchmark-start-"));
    const output = path.join(receiptRoot, "smoke.json");
    const environment = {
      ...process.env,
      RUSTFRAME_SMOKE_TEST: "1",
      RUSTFRAME_SMOKE_OUTPUT: output,
      RUSTFRAME_SMOKE_DATA_DIR: path.join(receiptRoot, "data"),
    };
    const start = performance.now();
    const timed = timedProcess(binary, environment);
    elapsedMs.push(round(performance.now() - start));
    peakMemoryBytes.push(timed.peakMemoryBytes);
    if (!fs.statSync(output, { throwIfNoEntry: false })?.isFile()) fail("native start produced no smoke receipt");
  }
  return { elapsedMs, peakMemoryBytes };
}

function timedProcess(binary, environment) {
  const timeArguments = process.platform === "darwin" ? ["-l", binary] : ["-v", binary];
  const result = spawnSync("/usr/bin/time", timeArguments, {
    cwd: repoRoot,
    env: environment,
    encoding: "utf8",
    maxBuffer: 10 * 1024 * 1024,
  });
  if (result.status !== 0) fail(`native start failed: ${(result.stderr || result.stdout).trim()}`);
  const pattern = process.platform === "darwin"
    ? /([0-9]+)\s+maximum resident set size/u
    : /Maximum resident set size \(kbytes\):\s*([0-9]+)/u;
  const match = result.stderr.match(pattern);
  if (!match) fail("host time utility did not report maximum resident memory");
  return { peakMemoryBytes: Number(match[1]) * (process.platform === "darwin" ? 1 : 1024) };
}

function benchmarkRebuild(iterations) {
  run("npm", ["--prefix", "apps/research-desk", "run", "build"]);
  const samples = [];
  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const start = performance.now();
    run("npm", ["--prefix", "apps/research-desk", "run", "build"], process.env, true);
    samples.push(round(performance.now() - start));
  }
  return samples;
}

function benchmarkIndexing(documentCount, iterations) {
  const root = "grant://benchmark-workspace";
  const templateBody = "Local evidence should stay on the selected machine. ".repeat(24);
  const corpus = Array.from({ length: documentCount }, (_, index) => {
    const text = `---\ncollection: benchmark-${index % 12}\nstatus: queued\npriority: watch\ntags: local, receipt\n---\n# Evidence ${index + 1}\n\n${templateBody}`;
    return {
      entry: {
        uri: `${root}/collection-${index % 12}/evidence-${index + 1}.md`,
        name: `evidence-${index + 1}.md`,
        size: Buffer.byteLength(text),
        modifiedAt: `2026-08-28T10:${String(index % 60).padStart(2, "0")}:00Z`,
      },
      text,
    };
  });
  const corpusBytes = corpus.reduce((sum, item) => sum + item.entry.size, 0);
  const elapsedMs = [];
  const warmElapsedMs = [];
  let checksum = 0;
  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const start = performance.now();
    const records = corpus.map(({ entry, text }) => buildIndexedRecord(root, entry, text));
    elapsedMs.push(round(performance.now() - start));
    checksum ^= records.reduce((sum, record) => sum + record.contentFingerprint.charCodeAt(record.contentFingerprint.length - 1), 0);

    const warmStart = performance.now();
    const skipped = records.reduce((count, record, index) => count + Number(isSourceUnchanged(record, corpus[index].entry)), 0);
    warmElapsedMs.push(round(performance.now() - warmStart));
    if (skipped !== documentCount) fail("warm indexing benchmark did not skip the complete unchanged corpus");
  }
  return { corpusBytes, elapsedMs, warmElapsedMs, checksum };
}

function selectPackageArtifact(manifest, format) {
  const artifact = manifest.artifacts?.find((entry) => entry.format === format);
  if (!artifact || !artifact.path || !Number.isFinite(artifact.bytes)) {
    fail(`package manifest has no measured ${format} artifact`);
  }
  return artifact;
}

function resolveBenchmarkBinary(artifact, format) {
  const absolute = path.resolve(artifact);
  if (format === "app") {
    const directory = path.join(absolute, "Contents/MacOS");
    const binary = fs.readdirSync(directory).map((name) => path.join(directory, name))
      .find((candidate) => fs.statSync(candidate).isFile() && (fs.statSync(candidate).mode & 0o111));
    if (!binary) fail(`packaged app has no executable in ${directory}`);
    return binary;
  }
  fs.chmodSync(absolute, 0o755);
  return absolute;
}

function hostReceipt() {
  const version = process.platform === "darwin"
    ? `macOS ${runText("sw_vers", ["-productVersion"])} (${runText("sw_vers", ["-buildVersion"])})`
    : readLinuxVersion();
  const cpu = process.platform === "darwin"
    ? runText("sysctl", ["-n", "machdep.cpu.brand_string"])
    : fs.readFileSync("/proc/cpuinfo", "utf8").match(/^model name\s*:\s*(.+)$/mu)?.[1] || os.cpus()[0]?.model;
  return {
    os: version,
    architecture: process.arch,
    cpu,
    logicalCpus: os.cpus().length,
    memoryBytes: os.totalmem(),
    node: process.version,
  };
}

function readLinuxVersion() {
  const source = fs.readFileSync("/etc/os-release", "utf8");
  return source.match(/^PRETTY_NAME="?(.+?)"?$/mu)?.[1] || `${os.type()} ${os.release()}`;
}

function run(command, args, environment = process.env, quiet = false) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: environment,
    encoding: "utf8",
    stdio: quiet ? "pipe" : "inherit",
    maxBuffer: 20 * 1024 * 1024,
  });
  if (result.status !== 0) fail(`${command} ${args.join(" ")} failed${quiet ? `: ${(result.stderr || result.stdout).trim()}` : ""}`);
}

function runText(command, args) {
  const result = spawnSync(command, args, { cwd: repoRoot, encoding: "utf8" });
  if (result.status !== 0) fail(`${command} failed: ${(result.stderr || result.stdout).trim()}`);
  return result.stdout.trim();
}

function readJson(file) {
  if (!fs.statSync(file, { throwIfNoEntry: false })?.isFile()) fail(`required JSON is missing: ${file}`);
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return round(sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2);
}

function percentile(values, fraction) {
  const sorted = [...values].sort((left, right) => left - right);
  return round(sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)]);
}

function round(value, precision = 2) {
  const factor = 10 ** precision;
  return Math.round(value * factor) / factor;
}

function toMebibytes(bytes) {
  return round(bytes / 1024 / 1024);
}

function positiveInteger(value, label) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1) fail(`${label} requires a positive integer`);
  return parsed;
}

function required(value, label) {
  if (!value) fail(`${label} requires a value`);
  return value;
}

function fail(message) {
  process.stderr.write(`Research Desk benchmark failed: ${message}\n`);
  process.exit(1);
}
