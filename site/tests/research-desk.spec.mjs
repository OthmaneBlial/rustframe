import { expect, test } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

const RESEARCH_DESK_URL = "http://127.0.0.1:4316/";

async function installRustFrameMock(page, { connected = false, fileCount = 2, renameFirst = false } = {}) {
  await page.addInitScript(({ connectedWorkspace, requestedFileCount, shouldRenameFirst }) => {
    const calls = { batches: [], saves: [], revoked: [], backups: 0 };
    let nextId = 20;
    const root = "grant://workspace-alpha";
    const documents = connectedWorkspace ? [
      {
        id: 1,
        createdAt: "2026-08-01T08:00:00Z",
        updatedAt: "2026-08-28T08:00:00Z",
        path: `${root}/launch-memo.md`,
        title: "Northwind launch memo",
        collection: "Launch",
        kind: "Brief",
        summary: "Launch research and customer evidence.",
        reviewer: "Mina",
        status: "reviewing",
        priority: "critical",
        tags: ["launch", "customer"],
        readingMinutes: 3,
        lineCount: 8,
        fileSize: 144,
        sourceModifiedAt: "2026-08-28T08:00:00Z",
        note: "Confirm the launch sequence.",
        pinned: true,
      },
      {
        id: 2,
        createdAt: "2026-08-02T08:00:00Z",
        updatedAt: "2026-08-27T08:00:00Z",
        path: `${root}/archive.md`,
        title: "Archive checklist",
        collection: "Operations",
        kind: "Checklist",
        summary: "Retention and archive steps.",
        reviewer: "Ops",
        status: "queued",
        priority: "reference",
        tags: ["archive"],
        readingMinutes: 2,
        lineCount: 5,
        fileSize: 96,
        sourceModifiedAt: "2026-08-27T08:00:00Z",
        note: "",
        pinned: false,
      },
    ] : [];
    const settings = connectedWorkspace ? [
      {
        id: 10,
        key: "workspaceProfile",
        value: {
          label: "Workspace Alpha",
          root,
          grantId: "grant-alpha",
          command: "RustFrame native indexer",
          fileCount: 2,
          lastIndexedAt: "2026-08-28T08:00:00Z",
        },
      },
      { id: 11, key: "recentWorkspaces", value: [] },
    ] : [];

    const clone = (value) => JSON.parse(JSON.stringify(value));
    const tableRows = (table) => table === "documents" ? documents : settings;
    const matchesFilters = (row, filters = []) => filters.every((filter) => row[filter.field] === filter.value);
    const indexFiles = Array.from({ length: requestedFileCount }, (_, index) => ({
      uri: `${root}/new-${index + 1}.md`,
      path: `${root}/new-${index + 1}.md`,
      name: `new-${index + 1}.md`,
      extension: "md",
      isFile: true,
      isDir: false,
      size: shouldRenameFirst && index === 0 ? 144 : 200 + index,
      modifiedAt: shouldRenameFirst && index === 0 ? "2026-08-28T08:00:00Z" : `2026-08-28T09:${String(index).padStart(2, "0")}:00Z`,
    }));

    window.__mockCalls = calls;
    window.RustFrame = {
      security: Object.freeze({ database: true, filesystem: true, shell: false }),
      window: {
        id: "main",
        route: "/",
        list: async () => [{ id: "main", route: "/", title: "Research Desk", isPrimary: true }],
        setTitle: async () => true,
        open: async () => ({ id: "reader-1" }),
        close: async () => true,
      },
      db: {
        info: async () => ({
          appId: "research-desk",
          dataDir: "/Users/example/Library/Application Support/research-desk",
          databasePath: "/Users/example/Library/Application Support/research-desk/app.db",
          schemaVersion: 1,
          tables: ["documents", "settings"],
        }),
        list: async (table, options = {}) => {
          let rows = tableRows(table).filter((row) => matchesFilters(row, options.filters));
          return clone(rows);
        },
        search: async (table, term, options = {}) => {
          const terms = term.toLowerCase().split(/\s+/u).filter(Boolean);
          return clone(tableRows(table).filter((row) => {
            const haystack = JSON.stringify(row).toLowerCase();
            return matchesFilters(row, options.filters) && terms.every((word) => haystack.includes(word));
          }));
        },
        get: async (table, id) => clone(tableRows(table).find((row) => row.id === id) || null),
        insert: async (table, record) => {
          const row = { id: nextId++, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString(), ...clone(record) };
          tableRows(table).push(row);
          return clone(row);
        },
        update: async (table, id, patch) => {
          const row = tableRows(table).find((entry) => entry.id === id);
          Object.assign(row, clone(patch), { updatedAt: new Date().toISOString() });
          return clone(row);
        },
        delete: async (table, id) => {
          const rows = tableRows(table);
          const index = rows.findIndex((row) => row.id === id);
          if (index >= 0) rows.splice(index, 1);
          return index >= 0;
        },
        batch: async (operations) => {
          calls.batches.push(clone(operations));
          for (const operation of operations) {
            const rows = tableRows(operation.table);
            if (operation.operation === "delete") {
              const index = rows.findIndex((row) => row.id === operation.id);
              if (index >= 0) rows.splice(index, 1);
            } else if (operation.operation === "insert") {
              rows.push({ id: nextId++, ...clone(operation.record) });
            } else if (operation.operation === "update") {
              Object.assign(rows.find((row) => row.id === operation.id), clone(operation.patch));
            }
          }
          return [];
        },
        backup: async () => { calls.backups += 1; return { backedUp: true, cancelled: false }; },
        restore: async () => ({ restored: false, cancelled: true }),
      },
      fs: {
        listGrants: async () => connectedWorkspace ? [{ id: "grant-alpha", uri: root, name: "Workspace Alpha" }] : [],
        requestGrant: async () => null,
        revokeGrant: async (id) => { calls.revoked.push(id); return true; },
        watch: async () => ({ id: "watch-1" }),
        unwatch: async () => true,
        listDir: async () => [],
        walk: async () => clone(indexFiles),
        readText: async (uri) => {
          if (uri.includes("new-")) await new Promise((resolve) => setTimeout(resolve, 120));
          return `---\ncollection: Launch\ntags: launch, local\n---\n# ${uri.split("/").pop()}\n\nLaunch evidence stays local.`;
        },
        metadata: async (path) => ({ path, size: 144, modifiedAt: "2026-08-28T08:00:00Z" }),
        openPath: async () => true,
        revealPath: async () => true,
      },
      dialog: {
        saveText: async (options) => {
          calls.saves.push(clone(options));
          return { path: `/tmp/${options.defaultName}` };
        },
      },
      clipboard: { writeText: async () => true },
      events: {
        onFileDrop: () => () => {},
        onDatabaseChange: () => () => {},
        onFilesystemChange: () => () => {},
        onRestore: () => () => {},
      },
    };
  }, { connectedWorkspace: connected, requestedFileCount: fileCount, shouldRenameFirst: renameFirst });
}

test("first run explains the exact consent boundary and data controls", async ({ page }) => {
  await installRustFrameMock(page);
  await page.goto(RESEARCH_DESK_URL);

  await expect(page.getByRole("heading", { name: "Your archive stays where it is." })).toBeVisible();
  await expect(page.getByText("Only Markdown and text files inside the selected folder")).toBeVisible();
  await page.getByRole("button", { name: "My data & privacy" }).click();
  await expect(page.getByRole("dialog", { name: /See, export, back up/ })).toBeVisible();
  await expect(page.getByRole("button", { name: "JSONL" })).toBeVisible();
  await expect(page.getByText("No sync account, telemetry endpoint, or cloud database")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "My data & privacy" })).toBeFocused();
});

test("FTS results are highlighted and filter views persist", async ({ page }) => {
  await installRustFrameMock(page, { connected: true });
  await page.goto(RESEARCH_DESK_URL);

  const search = page.getByRole("searchbox", { name: "Search" });
  await search.fill("launch memo");
  await expect(page.locator(".document-card mark").first()).toHaveText(/launch/i);
  await page.getByRole("button", { name: "reviewing" }).first().click({ noWaitAfter: true });
  await expect(page.locator('[data-action="filter-status"][data-status="reviewing"].is-active')).toBeVisible();
  await page.getByRole("button", { name: "Save current view" }).click();
  await expect(page.locator(".saved-filter strong")).toContainText("launch memo");
  await page.getByRole("button", { name: "Reset filters" }).click();
  await expect(search).toHaveValue("");
  await page.locator('[data-action="apply-saved-filter"]').click();
  await expect(page.getByRole("searchbox", { name: "Search" })).toHaveValue("launch memo");
});

test("incremental indexing can cancel without a partial database commit", async ({ page }) => {
  await installRustFrameMock(page, { connected: true, fileCount: 24 });
  await page.goto(RESEARCH_DESK_URL);

  await page.getByRole("button", { name: "Index workspace" }).click({ noWaitAfter: true });
  await expect(page.getByRole("button", { name: "Cancel indexing" })).toBeVisible();
  await page.evaluate(() => document.querySelector('[data-action="cancel-index"]')?.click());
  await expect(page.getByText(/Indexing canceled safely/)).toBeVisible();
  const batches = await page.evaluate(() => window.__mockCalls.batches);
  expect(batches).toHaveLength(0);
});

test("a detected rename preserves the existing SQLite record", async ({ page }) => {
  await installRustFrameMock(page, { connected: true, fileCount: 1, renameFirst: true });
  await page.goto(RESEARCH_DESK_URL);

  await page.getByRole("button", { name: "Index workspace" }).click();
  await expect(page.getByText(/1 renamed/)).toBeVisible();
  const operations = await page.evaluate(() => window.__mockCalls.batches.flat());
  const rename = operations.find((operation) => operation.operation === "update" && operation.id === 1);
  expect(rename.patch.path).toBe("grant://workspace-alpha/new-1.md");
  expect(rename.patch).not.toHaveProperty("note");
  expect(rename.patch).not.toHaveProperty("pinned");
});

test("exports are explicit, diagnostics are redacted, and deletion requires a backup", async ({ page }) => {
  await installRustFrameMock(page, { connected: true });
  await page.goto(RESEARCH_DESK_URL);
  await page.getByRole("button", { name: "My data & privacy" }).click();

  await page.getByRole("button", { name: "Save redacted diagnostics" }).click();
  const diagnostic = await page.evaluate(() => window.__mockCalls.saves.at(-1).contents);
  expect(diagnostic).toContain("<redacted-path>");
  expect(diagnostic).toContain("grant://<redacted>");
  expect(diagnostic).not.toContain("/Users/example/");

  await page.getByRole("button", { name: "Review deletion" }).click();
  await expect(page.getByRole("alertdialog")).toContainText("never deletes source files");
  await page.getByRole("button", { name: "Save backup & delete" }).click();
  await expect(page.getByText(/Deleted Research Desk records/)).toBeVisible();
  const proof = await page.evaluate(() => ({
    backups: window.__mockCalls.backups,
    revoked: window.__mockCalls.revoked,
    batches: window.__mockCalls.batches,
  }));
  expect(proof.backups).toBe(1);
  expect(proof.revoked).toEqual(["grant-alpha"]);
  expect(proof.batches.flat()).toHaveLength(4);
});

test("the workbench has no horizontal overflow at its target viewport", async ({ page }) => {
  await installRustFrameMock(page, { connected: true });
  await page.goto(RESEARCH_DESK_URL);
  const dimensions = await page.evaluate(() => ({
    viewport: document.documentElement.clientWidth,
    content: document.documentElement.scrollWidth,
  }));
  expect(dimensions.content).toBeLessThanOrEqual(dimensions.viewport);
});

test("keyboard shortcuts and automated accessibility checks cover the main workbench", async ({ page }) => {
  await installRustFrameMock(page, { connected: true });
  await page.goto(RESEARCH_DESK_URL);

  await page.keyboard.press("/");
  await expect(page.getByRole("searchbox", { name: "Search" })).toBeFocused();
  const mainAudit = await new AxeBuilder({ page }).analyze();
  expect(mainAudit.violations).toEqual([]);

  await page.getByRole("button", { name: "My data & privacy" }).click();
  const privacyAudit = await new AxeBuilder({ page }).analyze();
  expect(privacyAudit.violations).toEqual([]);
});
