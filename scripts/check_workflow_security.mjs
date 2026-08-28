import { readdirSync, readFileSync } from "node:fs";
import { extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const workflowRoots = [".github/workflows", "examples/github-actions"];

function indentation(line) {
  return line.match(/^\s*/)[0].length;
}

function topLevelPermissions(lines) {
  const index = lines.findIndex((line) => /^permissions:\s*/.test(line));
  if (index === -1) return ["missing top-level read-only permissions"];

  const declaration = lines[index].trim();
  if (declaration === "permissions: read-all") return [];
  if (declaration !== "permissions:") {
    return ["top-level permissions must be `read-all` or an explicit map"];
  }

  const entries = [];
  for (let cursor = index + 1; cursor < lines.length; cursor += 1) {
    const line = lines[cursor];
    if (!line.trim() || line.trimStart().startsWith("#")) continue;
    if (indentation(line) === 0) break;
    const match = line.match(/^\s{2}["']?([a-z-]+)["']?:\s*["']?([a-z-]+)["']?\s*$/);
    if (match) entries.push([match[1], match[2]]);
  }

  const errors = [];
  if (!entries.some(([permission, access]) => permission === "contents" && access === "read")) {
    errors.push("top-level permissions must grant `contents: read`");
  }
  for (const [permission, access] of entries) {
    if (access === "write") errors.push(`top-level permission ${permission} must not be write`);
  }
  return errors;
}

export function analyzeWorkflow(source, label = "workflow") {
  const lines = source.split(/\r?\n/);
  const errors = topLevelPermissions(lines).map((message) => `${label}: ${message}`);

  lines.forEach((line, index) => {
    const match = line.match(/\buses:\s*["']?([^\s#"']+)["']?\s*(?:#\s*(.+))?$/);
    if (!match) return;
    const action = match[1];
    const versionNote = match[2]?.trim();
    if (action.startsWith("./")) return;

    const separator = action.lastIndexOf("@");
    const reference = separator === -1 ? "" : action.slice(separator + 1);
    if (!/^[0-9a-f]{40}$/i.test(reference)) {
      errors.push(`${label}:${index + 1}: external action must use a full immutable commit SHA`);
    }
    if (!versionNote) {
      errors.push(`${label}:${index + 1}: pinned action needs a human-readable version comment`);
    }

    if (action.startsWith("actions/checkout@")) {
      const baseIndent = indentation(line);
      let persisted = false;
      for (let cursor = index + 1; cursor < lines.length; cursor += 1) {
        const next = lines[cursor];
        if (next.trim() && indentation(next) < baseIndent) break;
        if (indentation(next) === baseIndent && /^\s*-\s+/.test(next)) break;
        if (/^\s*persist-credentials:\s*false\s*$/.test(next)) persisted = true;
      }
      if (!persisted) {
        errors.push(`${label}:${index + 1}: checkout must set persist-credentials: false`);
      }
    }
  });

  return errors;
}

function workflowFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return workflowFiles(path);
    return [".yml", ".yaml"].includes(extname(entry.name)) ? [path] : [];
  });
}

export function checkRepository() {
  return workflowRoots.flatMap((directory) =>
    workflowFiles(join(root, directory)).flatMap((path) =>
      analyzeWorkflow(readFileSync(path, "utf8"), relative(root, path)),
    ),
  );
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const errors = checkRepository();
  if (errors.length) {
    console.error(errors.join("\n"));
    process.exitCode = 1;
  } else {
    console.log("Workflow policy passed: immutable actions, version notes, read-only defaults, and ephemeral checkout credentials.");
  }
}
