#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const appDirectory = path.resolve(process.argv[2] || "");
const format = process.argv[3] || "";
if (!appDirectory || !format) {
  throw new Error("usage: assert_file_associations.mjs <app-directory> <package-format>");
}

const source = readJson(path.join(appDirectory, ".rustframe/file-associations.json"));
const manifest = readJson(path.join(appDirectory, "dist/packages/rustframe-package-manifest.json"));
assert.equal(source.schemaVersion, 1);
assert.ok(Array.isArray(source.associations) && source.associations.length > 0);
assert.deepEqual(manifest.fileAssociations, source.associations);

const expectedExtensions = source.associations.flatMap((association) => association.extensions).sort();

if (format === "app") {
  const appBundle = findEntry(path.join(appDirectory, "dist/packages"), (entry) => (
    entry.isDirectory() && entry.name.endsWith(".app")
  ));
  assert.ok(appBundle, "macOS app bundle is missing");
  const plist = run("plutil", [
    "-extract", "CFBundleDocumentTypes", "json", "-o", "-",
    path.join(appBundle, "Contents/Info.plist"),
  ]);
  const documentTypes = JSON.parse(plist);
  const emitted = documentTypes.flatMap((entry) => entry.CFBundleTypeExtensions || []).sort();
  assert.deepEqual(emitted, expectedExtensions);
}

if (format === "deb") {
  const deb = findEntry(path.join(appDirectory, "dist/packages"), (entry) => (
    entry.isFile() && entry.name.endsWith(".deb")
  ));
  assert.ok(deb, "Debian package is missing");
  const extraction = fs.mkdtempSync(path.join(os.tmpdir(), "rustframe-deb-associations-"));
  try {
    run("dpkg-deb", ["--extract", deb, extraction]);
    const desktopFile = findEntry(extraction, (entry) => entry.isFile() && entry.name.endsWith(".desktop"));
    assert.ok(desktopFile, "Debian desktop metadata is missing");
    const desktop = fs.readFileSync(desktopFile, "utf8");
    for (const association of source.associations) {
      if (association.mimeType) assert.ok(desktop.includes(association.mimeType));
    }
    assert.match(desktop, /^Exec=.*%F$/mu);
  } finally {
    fs.rmSync(extraction, { recursive: true, force: true });
  }
}

process.stdout.write(`Verified ${expectedExtensions.length} file extensions in ${format} package metadata.\n`);

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function findEntry(directory, predicate) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (predicate(entry)) return absolute;
    if (entry.isDirectory() && !entry.isSymbolicLink()) {
      const nested = findEntry(absolute, predicate);
      if (nested) return nested;
    }
  }
  return null;
}

function run(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${command} failed: ${result.stderr || result.stdout}`);
  }
  return result.stdout;
}
