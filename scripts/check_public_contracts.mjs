import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import ts from "../packages/rustframe-api/node_modules/typescript/lib/typescript.js";

const repoRoot = path.resolve(import.meta.dirname, "..");
const markdownFiles = [
  path.join(repoRoot, "README.md"),
  ...fs.readdirSync(path.join(repoRoot, "docs"), { withFileTypes: true })
    .filter(entry => entry.isFile() && entry.name.endsWith(".md"))
    .map(entry => path.join(repoRoot, "docs", entry.name)),
  path.join(repoRoot, "packages/rustframe-api/README.md"),
];

const failures = [];

function relative(file) {
  return path.relative(repoRoot, file);
}

function expectSame(left, right, label) {
  if (!fs.existsSync(left) || !fs.existsSync(right)) {
    failures.push(`${label}: expected both files to exist`);
    return;
  }
  if (fs.readFileSync(left, "utf8") !== fs.readFileSync(right, "utf8")) {
    failures.push(`${label}: ${relative(right)} is stale; run ./scripts/sync_site_docs.sh`);
  }
}

for (const source of fs.readdirSync(path.join(repoRoot, "docs"))) {
  if (!source.endsWith(".md")) continue;
  expectSame(
    path.join(repoRoot, "docs", source),
    path.join(repoRoot, "site/docs", source),
    `site documentation ${source}`,
  );
}

expectSame(
  path.join(repoRoot, "schemas/v1/rustframe.schema.json"),
  path.join(repoRoot, "crates/rustframe-cli/schema/rustframe-v1.schema.json"),
  "embedded schema",
);
expectSame(
  path.join(repoRoot, "schemas/v1/rustframe.schema.json"),
  path.join(repoRoot, "site/schemas/v1/rustframe.schema.json"),
  "published schema",
);
expectSame(
  path.join(repoRoot, "schemas/file-associations/v1/file-associations.schema.json"),
  path.join(repoRoot, "site/schemas/file-associations/v1/file-associations.schema.json"),
  "published file associations schema",
);

for (const file of markdownFiles) {
  const source = fs.readFileSync(file, "utf8");
  if (/\bpersistent\s*:\s*true/.test(source)) {
    failures.push(`${relative(file)} uses retired requestGrant option 'persistent'; use 'persist'`);
  }
  for (const match of source.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    const target = match[1].split("#", 1)[0];
    if (!target || /^(?:https?:|mailto:)/.test(target)) continue;
    const resolved = path.resolve(path.dirname(file), decodeURIComponent(target));
    if (!fs.existsSync(resolved)) {
      failures.push(`${relative(file)} links to missing local target '${target}'`);
    }
  }
}

const temporaryRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rustframe-docs-"));
try {
  const snippetFiles = [];
  for (const file of markdownFiles) {
    const source = fs.readFileSync(file, "utf8");
    const blocks = [...source.matchAll(/```(?:ts|typescript)\s*\n([\s\S]*?)```/g)]
      .map(match => match[1].trim())
      .filter(Boolean);
    if (blocks.length === 0) continue;

    const directory = path.join(temporaryRoot, relative(file).replace(/[^a-zA-Z0-9]+/g, "-"));
    fs.mkdirSync(directory, { recursive: true });
    const snippet = path.join(directory, "examples.ts");
    fs.writeFileSync(snippet, `${blocks.join("\n\n")}\n`);
    fs.writeFileSync(
      path.join(directory, "rustframe.generated.ts"),
      'import type { RustFrameClient } from "rustframe-api";\nexport type AppRustFrameClient = RustFrameClient<any>;\n',
    );
    snippetFiles.push(snippet);
  }

  const program = ts.createProgram(snippetFiles, {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    strict: true,
    noEmit: true,
    skipLibCheck: true,
    lib: ["lib.es2022.d.ts", "lib.dom.d.ts"],
    baseUrl: repoRoot,
    paths: { "rustframe-api": ["packages/rustframe-api/src/index.ts"] },
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  if (diagnostics.length > 0) {
    failures.push(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
      getCanonicalFileName: value => value,
      getCurrentDirectory: () => repoRoot,
      getNewLine: () => "\n",
    }));
  }
} finally {
  fs.rmSync(temporaryRoot, { recursive: true, force: true });
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exit(1);
}

console.log(`Public contracts verified across ${markdownFiles.length} Markdown files.`);
