#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const options = parseArguments(process.argv.slice(2));
const bundleDir = path.resolve(options.directory || ".");
const platform = options.platform || process.platform;
const evidencePath = path.resolve(options.output || path.join(bundleDir, "release-evidence.json"));
const checksumPath = path.join(bundleDir, "SHA256SUMS");
const manifestPath = path.join(bundleDir, "rustframe-package-manifest.json");

assertFile(checksumPath, "SHA256 checksum manifest");
assertFile(manifestPath, "RustFrame package manifest");
const packageManifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const checksums = parseChecksumManifest(fs.readFileSync(checksumPath, "utf8"));
validatePackageManifest(packageManifest, checksums);
const verifiedArtifacts = [];

for (const entry of checksums) {
  const artifactPath = path.join(bundleDir, entry.name);
  if (!fs.existsSync(artifactPath)) fail(`checksum target is missing: ${entry.name}`);
  const actual = digestArtifact(artifactPath);
  if (actual.sha256 !== entry.sha256) {
    fail(`checksum mismatch for ${entry.name}: expected ${entry.sha256}, received ${actual.sha256}`);
  }
  verifiedArtifacts.push({ name: entry.name, sha256: actual.sha256, bytes: actual.bytes });
}

const expectedSourceCommit = process.env.RUSTFRAME_SOURCE_COMMIT || process.env.GITHUB_SHA || null;
if (expectedSourceCommit && packageManifest.sourceCommit !== expectedSourceCommit) {
  fail(`package source commit ${packageManifest.sourceCommit || "is missing"}; expected ${expectedSourceCommit}`);
}

const requiresNativeSignature = platform === "darwin" || platform === "win32";
let signatureChecks;
let signatureState;
if (requiresNativeSignature && packageManifest.signed !== true) {
  if (!options.allowUnsignedLocal) fail(`package manifest says the ${platform} bundle is unsigned`);
  if (process.env.GITHUB_ACTIONS === "true") {
    fail("--allow-unsigned-local is forbidden in GitHub Actions");
  }
  signatureChecks = [{ kind: "unsigned-local-build", status: "not-for-distribution" }];
  signatureState = "unsigned-local";
} else {
  signatureChecks = verifyPlatformSignatures(bundleDir, platform);
  signatureState = requiresNativeSignature ? "verified" : "not-applicable";
}

const sbom = findFirst(bundleDir, (name) => name.endsWith(".spdx.json"));
if (options.requireSbom && !sbom) fail("release bundle is missing an SPDX SBOM");
if (sbom && fs.statSync(sbom).size === 0) fail("release bundle contains an empty SPDX SBOM");

const evidence = {
  schemaVersion: 1,
  product: packageManifest.productName,
  version: packageManifest.version,
  appId: packageManifest.appId,
  sourceCommit: packageManifest.sourceCommit || process.env.GITHUB_SHA || null,
  verifiedAt: new Date().toISOString(),
  verifier: {
    platform,
    architecture: process.arch,
    testedOsVersion: detectOsVersion(platform),
    githubRunnerImage: process.env.ImageOS || null,
  },
  integrity: {
    state: "verified",
    checksumAlgorithm: "SHA-256",
    artifacts: verifiedArtifacts,
  },
  signature: {
    state: signatureState,
    checks: signatureChecks,
  },
  sbom: sbom ? digestFileRecord(sbom, bundleDir) : null,
  provenance: {
    state: process.env.GITHUB_ACTIONS === "true" ? "attested-by-workflow" : "local-verification",
    verificationCommand: "gh attestation verify <artifact> --repo OthmaneBlial/rustframe",
  },
};

fs.writeFileSync(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
process.stdout.write(`Verified ${verifiedArtifacts.length} artifacts; evidence: ${evidencePath}\n`);

function parseArguments(args) {
  const parsed = { allowUnsignedLocal: false, requireSbom: false };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--dir") parsed.directory = args[++index];
    else if (arg === "--platform") parsed.platform = normalizePlatform(args[++index]);
    else if (arg === "--output") parsed.output = args[++index];
    else if (arg === "--require-sbom") parsed.requireSbom = true;
    else if (arg === "--allow-unsigned-local") parsed.allowUnsignedLocal = true;
    else fail(`unknown argument: ${arg}`);
  }
  return parsed;
}

function normalizePlatform(value) {
  const normalized = String(value || "").toLowerCase();
  if (["macos", "mac", "darwin"].includes(normalized)) return "darwin";
  if (["windows", "win32", "win"].includes(normalized)) return "win32";
  if (["linux"].includes(normalized)) return "linux";
  fail(`unsupported platform: ${value}`);
}

function parseChecksumManifest(source) {
  const entries = source.split(/\r?\n/u).filter(Boolean).map((line) => {
    const match = line.match(/^([a-f0-9]{64})  (.+)$/u);
    if (!match) fail(`invalid SHA256SUMS line: ${line}`);
    if (match[2].includes("/") || match[2].includes("\\") || match[2] === "..") {
      fail(`unsafe checksum target: ${match[2]}`);
    }
    return { sha256: match[1], name: match[2] };
  });
  if (!entries.length) fail("SHA256SUMS contains no artifacts");
  if (new Set(entries.map((entry) => entry.name)).size !== entries.length) {
    fail("SHA256SUMS contains duplicate artifact names");
  }
  return entries;
}

function validatePackageManifest(manifest, checksums) {
  if (manifest.schemaVersion !== 1) fail(`unsupported package manifest schema: ${manifest.schemaVersion}`);
  if (!manifest.productName || !manifest.version || !manifest.appId) {
    fail("package manifest identity is incomplete");
  }
  const expectedSignatureState = manifest.signed === true ? "signed" : "unsigned";
  if (manifest.signatureState !== expectedSignatureState) {
    fail(`package manifest signature state is inconsistent: ${manifest.signatureState}`);
  }
  if (!Array.isArray(manifest.artifacts) || manifest.artifacts.length !== checksums.length) {
    fail("package manifest artifacts do not match SHA256SUMS");
  }
  const records = new Map(manifest.artifacts.map((entry) => [portableBasename(entry.path), entry]));
  if (records.size !== manifest.artifacts.length) fail("package manifest contains duplicate artifact names");
  for (const checksum of checksums) {
    const record = records.get(checksum.name);
    if (!record || record.sha256 !== checksum.sha256) {
      fail(`package manifest checksum does not match SHA256SUMS for ${checksum.name}`);
    }
  }
}

function portableBasename(value) {
  return String(value || "").split(/[\\/]/u).at(-1);
}

function digestArtifact(artifactPath) {
  const stat = fs.statSync(artifactPath);
  if (stat.isFile()) return { sha256: sha256File(artifactPath), bytes: stat.size };
  if (!stat.isDirectory()) fail(`unsupported artifact type: ${artifactPath}`);
  const files = walkFiles(artifactPath).sort((left, right) => left.relative.localeCompare(right.relative));
  const digest = crypto.createHash("sha256");
  let bytes = 0;
  for (const file of files) {
    const contents = fs.readFileSync(file.absolute);
    digest.update(file.relative);
    digest.update(Buffer.from([0]));
    digest.update(contents);
    bytes += contents.length;
  }
  return { sha256: digest.digest("hex"), bytes };
}

function walkFiles(root, directory = root) {
  const files = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...walkFiles(root, absolute));
    else if (entry.isFile()) files.push({
      absolute,
      relative: path.relative(root, absolute).split(path.sep).join("/"),
    });
  }
  return files;
}

function verifyPlatformSignatures(directory, targetPlatform) {
  if (targetPlatform === "darwin") return verifyMacSignatures(directory);
  if (targetPlatform === "win32") return verifyWindowsSignatures(directory);
  return [{ kind: "linux-integrity", status: "checksums-and-provenance-required" }];
}

function verifyMacSignatures(directory) {
  const targets = fs.readdirSync(directory)
    .filter((name) => name.endsWith(".app") || name.endsWith(".dmg"))
    .map((name) => path.join(directory, name));
  if (!targets.length) fail("macOS bundle contains no .app or .dmg artifact");
  return targets.map((target) => {
    run("codesign", ["--verify", "--strict", "--verbose=2", target]);
    run("xcrun", ["stapler", "validate", target]);
    if (target.endsWith(".app")) run("spctl", ["--assess", "--type", "execute", "--verbose=2", target]);
    return { artifact: path.basename(target), kind: "apple-developer-id", status: "valid-and-stapled" };
  });
}

function verifyWindowsSignatures(directory) {
  const targets = fs.readdirSync(directory)
    .filter((name) => name.endsWith(".exe") || name.endsWith(".msi"))
    .map((name) => path.join(directory, name));
  if (!targets.length) fail("Windows bundle contains no .exe or .msi artifact");
  return targets.map((target) => {
    const escaped = target.replaceAll("'", "''");
    const command = `$signature = Get-AuthenticodeSignature -LiteralPath '${escaped}'; `
      + `if ($signature.Status -ne 'Valid') { Write-Error \"Authenticode status: $($signature.Status)\"; exit 1 }; `
      + `if ($null -eq $signature.TimeStamperCertificate) { Write-Error 'Authenticode timestamp is missing'; exit 1 }; `
      + `$signature.SignerCertificate.Subject`;
    const result = run("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", command]);
    return {
      artifact: path.basename(target),
      kind: "authenticode",
      status: "valid-and-timestamped",
      signer: result.stdout.trim(),
    };
  });
}

function run(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) {
    fail(`${command} failed (${result.status}): ${(result.stderr || result.stdout || "no output").trim()}`);
  }
  return result;
}

function detectOsVersion(targetPlatform) {
  if (targetPlatform === "darwin") return run("sw_vers", ["-productVersion"]).stdout.trim();
  if (targetPlatform === "win32") {
    return run("powershell.exe", ["-NoProfile", "-Command", "(Get-CimInstance Win32_OperatingSystem).Caption"]).stdout.trim();
  }
  try {
    const fields = Object.fromEntries(fs.readFileSync("/etc/os-release", "utf8")
      .split(/\r?\n/u)
      .map((line) => line.split("=", 2))
      .filter(([key, value]) => key && value));
    return String(fields.PRETTY_NAME || process.release.name).replace(/^"|"$/gu, "");
  } catch {
    return `${process.platform} ${process.release.name}`;
  }
}

function findFirst(directory, predicate) {
  return walkFiles(directory).find((entry) => predicate(entry.relative))?.absolute || null;
}

function digestFileRecord(file, root) {
  return {
    name: path.relative(root, file).split(path.sep).join("/"),
    sha256: sha256File(file),
    bytes: fs.statSync(file).size,
  };
}

function sha256File(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function assertFile(file, label) {
  if (!fs.statSync(file, { throwIfNoEntry: false })?.isFile()) fail(`${label} is missing: ${file}`);
}

function fail(message) {
  process.stderr.write(`release verification failed: ${message}\n`);
  process.exit(1);
}
