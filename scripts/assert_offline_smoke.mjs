#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const options = parseArguments(process.argv.slice(2));
const input = path.resolve(required(options.input, "--input"));
const output = path.resolve(required(options.output, "--output"));
const packageFormat = required(options.format, "--format");
const supportedFormats = new Set(["app", "dmg", "nsis", "msi", "appimage", "deb"]);
if (!supportedFormats.has(packageFormat)) fail(`unsupported package format: ${packageFormat}`);

const smoke = readJson(input);
const checks = {
  embeddedLaunch: smoke.launchMode === "embedded",
  noActiveDevUrl: smoke.activeDevUrl === null,
  indexHtmlBundled: smoke.hasIndexHtml === true,
  nativeBridgeAvailable: smoke.bridgeInjected === true,
  localFirstSecurity: smoke.security?.model === "local-first",
  sqliteOpened: smoke.security?.database === true
    && Number.isInteger(smoke.database?.schemaVersion)
    && Array.isArray(smoke.database?.tables)
    && smoke.database.tables.length > 0,
};
const failures = Object.entries(checks)
  .filter(([, passed]) => !passed)
  .map(([name]) => name);
if (!smoke.appId || typeof smoke.appId !== "string") failures.push("appIdentityPresent");
if (failures.length) fail(`packaged no-server checks failed: ${failures.join(", ")}`);

const receipt = {
  schemaVersion: 1,
  kind: "rustframe.offline-package-receipt",
  appId: smoke.appId,
  packageFormat,
  verifier: {
    platform: process.platform,
    architecture: process.arch,
  },
  result: "passed",
  scope: "packaged-runtime-without-production-server",
  checks,
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
process.stdout.write(`Packaged no-server receipt: ${output}\n`);

function parseArguments(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--input") parsed.input = args[++index];
    else if (argument === "--output") parsed.output = args[++index];
    else if (argument === "--format") parsed.format = args[++index];
    else fail(`unknown argument: ${argument}`);
  }
  return parsed;
}

function readJson(file) {
  if (!fs.statSync(file, { throwIfNoEntry: false })?.isFile()) fail(`smoke report is missing: ${file}`);
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (error) {
    fail(`smoke report is invalid JSON: ${error.message}`);
  }
}

function required(value, label) {
  if (!value) fail(`${label} is required`);
  return value;
}

function fail(message) {
  process.stderr.write(`offline package verification failed: ${message}\n`);
  process.exit(1);
}
