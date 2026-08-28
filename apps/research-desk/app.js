import { buildIndexedRecord, isSourceUnchanged } from "./indexing.mjs";

const APP = document.getElementById("app");
const ROUTE = window.RustFrame?.window?.route || "/";
const [ROUTE_PATH, ROUTE_QUERY = ""] = ROUTE.split("?");
const ROUTE_PARAMS = new URLSearchParams(ROUTE_QUERY);
const STATUS_ORDER = ["queued", "reviewing", "ready", "archived"];
const PRIORITY_ORDER = ["critical", "watch", "reference"];
let latestSearchRequestId = 0;
let filterSaveTimer = null;
let bootComplete = false;
let pendingOpenedFiles = window.RustFrame?.app?.openedFiles?.() || [];

const state = {
    mode: ROUTE_PATH === "/reader" ? "reader" : "main",
    dbInfo: null,
    documents: [],
    visibleDocuments: [],
    settingsByKey: new Map(),
    workspaceEntries: [],
    windows: [],
    selectedId: null,
    selectedContent: "",
    selectedFileMeta: null,
    readerDocument: null,
    readerContent: "",
    readerFileMeta: null,
    search: "",
    collection: "all",
    status: "all",
    importBusy: false,
    indexCancelRequested: false,
    indexProgress: null,
    indexErrors: [],
    savedFilters: [],
    privacyPanelOpen: false,
    deleteConfirmationOpen: false,
    activeWatcherId: null,
    log: "Research Desk is booting."
};

document.body.dataset.mode = state.mode;
window.requestAnimationFrame(() => {
    document.body.classList.add("is-ready");
});

APP.addEventListener("click", handleClick);
APP.addEventListener("input", handleInput);
document.addEventListener("keydown", handleKeyboardShortcut);

if (window.RustFrame?.events?.onFileDrop) {
    window.RustFrame.events.onFileDrop((payload) => {
        if (state.mode !== "main") {
            return;
        }
        void importExternalFiles(payload?.files || [], "drag and drop");
    });
}

if (window.RustFrame?.app?.onOpenFiles) {
    window.RustFrame.app.onOpenFiles((payload) => {
        pendingOpenedFiles.push(...(payload?.files || []));
        if (bootComplete && state.mode === "main") {
            void importPendingOpenedFiles();
        }
    });
}

if (window.RustFrame?.events?.onDatabaseChange) {
    window.RustFrame.events.onDatabaseChange((event) => {
        if (event?.sourceWindowId === window.RustFrame.window.id) return;
        void refreshAfterExternalChange();
    });
}

if (window.RustFrame?.events?.onFilesystemChange) {
    let refreshTimer = null;
    window.RustFrame.events.onFilesystemChange(() => {
        window.clearTimeout(refreshTimer);
        refreshTimer = window.setTimeout(() => void indexWorkspace("filesystem change"), 250);
    });
}

if (window.RustFrame?.events?.onRestore) {
    window.RustFrame.events.onRestore(() => {
        void refreshAfterRestore();
    });
}

boot().catch((error) => {
    state.log = `Research Desk failed to boot.\n${formatError(error)}`;
    renderFatal();
});

async function boot() {
    state.dbInfo = await window.RustFrame.db.info();
    await loadSettings();
    restoreSavedFilterState();

    if (state.mode === "main") {
        await refreshDocuments();
        await loadWorkspaceEntries();
        await restoreWorkspaceWatcher();
        selectDefaultDocument();
        await refreshSelectedContent();
        await refreshWindows();
        const workspace = workspaceProfile();
        writeLog(workspace.root
            ? `Workspace connected: ${workspace.label}\n${state.documents.length} documents indexed.\nDatabase tables: ${state.dbInfo.tables.join(", ")}`
            : "Choose a Markdown or text-document folder to create your first private research workspace.");
        renderMain();
        bootComplete = true;
        await importPendingOpenedFiles();
    } else {
        const documentId = Number(ROUTE_PARAMS.get("doc"));
        if (!documentId) {
            throw new Error("Reader route is missing a document id.");
        }
        state.selectedId = documentId;
        await loadReaderDocument();
        await refreshWindows();
        if (state.readerDocument) {
            await window.RustFrame.window.setTitle(`${state.readerDocument.title} · Reader`);
        }
        renderReader();
        bootComplete = true;
    }
}

async function importPendingOpenedFiles() {
    const files = pendingOpenedFiles;
    pendingOpenedFiles = [];
    if (files.length) {
        await importExternalFiles(files, "the operating system");
    }
}

async function refreshAfterExternalChange() {
    if (state.mode === "reader") {
        await loadReaderDocument();
        renderReader();
        return;
    }
    await loadSettings();
    await refreshDocuments();
    selectDefaultDocument();
    await refreshSelectedContent();
    renderMain();
}

async function loadSettings() {
    const rows = await window.RustFrame.db.list("settings", {
        orderBy: [{ field: "key", direction: "asc" }]
    });
    state.settingsByKey = new Map(rows.map((row) => [row.key, row]));
}

function restoreSavedFilterState() {
    const saved = state.settingsByKey.get("savedFilters")?.value;
    state.savedFilters = Array.isArray(saved) ? saved.filter(isValidSavedFilter).slice(0, 8) : [];
    const active = state.settingsByKey.get("activeFilter")?.value;
    if (isValidSavedFilter(active)) {
        state.search = active.search;
        state.collection = active.collection;
        state.status = active.status;
    }
}

function isValidSavedFilter(value) {
    return Boolean(value)
        && typeof value === "object"
        && typeof value.search === "string"
        && typeof value.collection === "string"
        && typeof value.status === "string";
}

function currentFilterState() {
    return {
        search: state.search,
        collection: state.collection,
        status: state.status
    };
}

function scheduleActiveFilterSave() {
    window.clearTimeout(filterSaveTimer);
    filterSaveTimer = window.setTimeout(() => {
        void saveSetting("activeFilter", currentFilterState());
    }, 180);
}

async function saveCurrentFilter() {
    const filter = currentFilterState();
    const duplicate = state.savedFilters.find((entry) =>
        entry.search === filter.search
        && entry.collection === filter.collection
        && entry.status === filter.status
    );
    if (duplicate) {
        writeLog(`The saved view "${duplicate.label}" already matches these filters.`);
        return;
    }

    const parts = [
        filter.search ? `“${filter.search}”` : "All text",
        filter.status === "all" ? null : filter.status,
        filter.collection === "all" ? null : filter.collection
    ].filter(Boolean);
    const entry = {
        ...filter,
        id: `view-${Date.now()}`,
        label: parts.join(" · ").slice(0, 72),
        savedAt: new Date().toISOString()
    };
    state.savedFilters = [entry, ...state.savedFilters].slice(0, 8);
    await saveSetting("savedFilters", state.savedFilters);
    writeLog(`Saved the current filters as "${entry.label}".`);
    render();
}

async function applySavedFilter(id) {
    const filter = state.savedFilters.find((entry) => entry.id === id);
    if (!filter) return;
    state.search = filter.search;
    state.collection = filter.collection;
    state.status = filter.status;
    await saveSetting("activeFilter", currentFilterState());
    await refreshVisibleDocuments();
    selectDefaultDocument();
    await refreshSelectedContent();
    writeLog(`Applied saved view "${filter.label}".`);
    render();
}

async function deleteSavedFilter(id) {
    const removed = state.savedFilters.find((entry) => entry.id === id);
    state.savedFilters = state.savedFilters.filter((entry) => entry.id !== id);
    await saveSetting("savedFilters", state.savedFilters);
    writeLog(removed ? `Removed saved view "${removed.label}".` : "Saved view was already removed.");
    render();
}

async function clearActiveFilters() {
    state.search = "";
    state.collection = "all";
    state.status = "all";
    await saveSetting("activeFilter", currentFilterState());
    await refreshVisibleDocuments();
    selectDefaultDocument();
    await refreshSelectedContent();
    writeLog("Reset search, collection, and status filters.");
    render();
}

async function refreshDocuments() {
    state.documents = await window.RustFrame.db.list("documents", {
        orderBy: [
            { field: "pinned", direction: "desc" },
            { field: "collection", direction: "asc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });
    await refreshVisibleDocuments();
}

async function refreshWindows() {
    state.windows = await window.RustFrame.window.list();
}

async function loadWorkspaceEntries() {
    const root = workspaceProfile().root;
    if (!root) {
        state.workspaceEntries = [];
        return;
    }
    try {
        state.workspaceEntries = await window.RustFrame.fs.listDir(root);
    } catch {
        state.workspaceEntries = [];
    }
}

function selectDefaultDocument() {
    const visible = visibleDocuments();
    if (!visible.length) {
        state.selectedId = null;
        state.selectedContent = "";
        return;
    }

    if (!state.selectedId || !state.documents.some((entry) => entry.id === state.selectedId)) {
        state.selectedId = visible[0].id;
    }
}

async function refreshVisibleDocuments() {
    const requestId = ++latestSearchRequestId;
    const filters = buildActiveFilters();
    const searchTerm = state.search.trim();

    if (!searchTerm) {
        state.visibleDocuments = filterDocumentsLocally(filters);
        return;
    }

    const results = await window.RustFrame.db.search("documents", searchTerm, {
        filters,
        orderBy: [
            { field: "pinned", direction: "desc" },
            { field: "collection", direction: "asc" },
            { field: "updatedAt", direction: "desc" }
        ],
        limit: 250
    });

    if (requestId === latestSearchRequestId) {
        state.visibleDocuments = results;
    }
}

async function refreshSelectedContent() {
    const selected = selectedDocument();
    if (!selected) {
        state.selectedContent = "";
        state.selectedFileMeta = null;
        return;
    }

    try {
        state.selectedContent = await window.RustFrame.fs.readText(selected.path);
        state.selectedFileMeta = await window.RustFrame.fs.metadata(selected.path);
    } catch (error) {
        state.selectedContent = `Unable to load source document.\n\n${formatError(error)}`;
        state.selectedFileMeta = null;
    }
}

async function loadReaderDocument() {
    const row = await window.RustFrame.db.get("documents", state.selectedId);
    state.readerDocument = row;

    if (!row) {
        state.readerContent = "";
        state.readerFileMeta = null;
        return;
    }

    try {
        state.readerContent = await window.RustFrame.fs.readText(row.path);
        state.readerFileMeta = await window.RustFrame.fs.metadata(row.path);
    } catch (error) {
        state.readerContent = `Unable to load source document.\n\n${formatError(error)}`;
        state.readerFileMeta = null;
    }
}

async function indexWorkspace(reason) {
    const profile = workspaceProfile();
    if (!profile.root) {
        await chooseWorkspace();
        return;
    }
    state.importBusy = true;
    state.indexCancelRequested = false;
    state.indexErrors = [];
    state.indexProgress = { phase: "discovering", completed: 0, total: 0, skipped: 0 };
    render();

    try {
        const indexed = await scanWorkspace(profile.root);
        if (indexed.cancelled) {
            writeLog(
                `Indexing canceled safely after ${indexed.completed} of ${indexed.total} files.\n` +
                "No partial database changes were committed."
            );
            return;
        }
        state.indexProgress = { ...state.indexProgress, phase: "committing" };
        render();
        const changes = await mergeIndexedDocuments(indexed.records, {
            root: profile.root,
            seenPaths: indexed.seenPaths,
            removeMissing: true
        });
        await saveSetting("workspaceProfile", {
            ...profile,
            command: "RustFrame native indexer",
            fileCount: indexed.total,
            lastIndexedAt: new Date().toISOString()
        });

        await refreshDocuments();
        selectDefaultDocument();
        await refreshSelectedContent();
        await refreshWindows();
        await loadWorkspaceEntries();
        writeLog(
            `Indexed ${indexed.total} documents using RustFrame filesystem APIs.\n` +
            `Changes: ${changes.inserted} added, ${changes.updated} updated, ${changes.renamed} renamed, ${changes.deleted} removed, ${indexed.skipped} unchanged.\n` +
            `${indexed.errors.length} recoverable read error${indexed.errors.length === 1 ? "" : "s"}.\n` +
            `Reason: ${reason}\n` +
            `Workspace: ${profile.label}`
        );
    } finally {
        state.importBusy = false;
        state.indexCancelRequested = false;
        state.indexProgress = null;
        render();
    }
}

async function chooseWorkspace() {
    const grant = await window.RustFrame.fs.requestGrant({
        kind: "directory",
        access: "read",
        persist: true,
        title: "Choose a Markdown or text-document folder"
    });
    if (!grant) {
        writeLog("Workspace selection canceled. No folder access was retained.");
        return;
    }
    await saveSetting("workspaceProfile", {
        label: grant.name,
        root: grant.uri,
        grantId: grant.id,
        command: "RustFrame native indexer",
        fileCount: 0,
        lastIndexedAt: null
    });
    await rememberWorkspace(grant);
    await loadSettings();
    await restoreWorkspaceWatcher();
    await indexWorkspace("workspace selected");
}

async function rememberWorkspace(grant) {
    const existing = recentWorkspaces().filter((workspace) => workspace.grantId !== grant.id);
    await saveSetting("recentWorkspaces", [
        { grantId: grant.id, root: grant.uri, label: grant.name, lastOpenedAt: new Date().toISOString() },
        ...existing
    ].slice(0, 8));
}

async function switchWorkspace(grantId) {
    const grants = await window.RustFrame.fs.listGrants();
    const grant = grants.find((entry) => entry.id === grantId);
    if (!grant) {
        writeLog("That recent workspace grant has been revoked. Choose the folder again to reconnect it.");
        return;
    }
    await saveSetting("workspaceProfile", {
        label: grant.name,
        root: grant.uri,
        grantId: grant.id,
        command: "RustFrame native indexer",
        fileCount: 0,
        lastIndexedAt: null
    });
    await rememberWorkspace(grant);
    await loadSettings();
    await restoreWorkspaceWatcher();
    await indexWorkspace("recent workspace selected");
}

async function backupDatabase() {
    const dateLabel = new Date().toISOString().slice(0, 10);
    const result = await window.RustFrame.db.backup({
        suggestedName: `research-desk-${dateLabel}.db`
    });
    writeLog(result.cancelled ? "Database backup canceled." : "A consistent SQLite backup was created successfully.");
}

async function restoreDatabase() {
    const result = await window.RustFrame.db.restore();
    if (!result.restored) {
        writeLog("Database restore canceled. Existing data was not changed.");
        return;
    }
    writeLog("Database restored successfully. A safety backup was created before replacement.");
}

async function refreshAfterRestore() {
    await loadSettings();
    await refreshDocuments();
    await loadWorkspaceEntries();
    await restoreWorkspaceWatcher();
    selectDefaultDocument();
    await refreshSelectedContent();
    await refreshWindows();
    writeLog("Database restore completed. Every open window reloaded its state.");
    render();
}

async function restoreWorkspaceWatcher() {
    const profile = workspaceProfile();
    if (!profile.root || !window.RustFrame.fs.watch) return;
    const grants = await window.RustFrame.fs.listGrants();
    if (!grants.some((grant) => grant.uri === profile.root)) {
        writeLog("The saved workspace grant is unavailable. Choose the folder again to restore access.");
        return;
    }
    if (state.activeWatcherId) await window.RustFrame.fs.unwatch(state.activeWatcherId);
    const watcher = await window.RustFrame.fs.watch(profile.root, { recursive: true });
    state.activeWatcherId = watcher.id;
}

async function scanWorkspace(root) {
    const entries = await window.RustFrame.fs.walk(root, {
        recursive: true,
        extensions: ["md", "txt"],
        limit: 10000
    });
    const files = entries.filter((entry) => entry.isFile);
    const existing = new Map(state.documents.map((record) => [record.path, record]));
    const records = [];
    const errors = [];
    const seenPaths = new Set(files.map((entry) => entry.uri));
    let skipped = 0;
    state.indexProgress = { phase: "reading", completed: 0, total: files.length, skipped };
    render();

    for (let index = 0; index < files.length; index += 1) {
        const entry = files[index];
        if (state.indexCancelRequested) {
            return { records, errors, seenPaths, skipped, completed: index, total: files.length, cancelled: true };
        }

        const current = existing.get(entry.uri);
        const unchanged = isSourceUnchanged(current, entry);
        if (unchanged) {
            skipped += 1;
            state.indexProgress = { phase: "reading", completed: index + 1, total: files.length, skipped };
            updateIndexProgress();
            continue;
        }

        try {
            const text = await window.RustFrame.fs.readText(entry.uri);
            records.push(buildIndexedRecord(root, entry, text));
        } catch (error) {
            errors.push({ uri: redactUri(entry.uri), error: formatError(error) });
            records.push({
                path: entry.uri,
                title: entry.name,
                collection: "Unreadable",
                kind: "unreadable",
                summary: `The source could not be read: ${formatError(error)}`,
                status: "queued",
                priority: "watch",
                tags: ["unreadable"],
                readingMinutes: 1,
                lineCount: 0,
                fileSize: entry.size,
                sourceModifiedAt: entry.modifiedAt || "",
                contentFingerprint: ""
            });
        }
        state.indexProgress = { phase: "reading", completed: index + 1, total: files.length, skipped };
        updateIndexProgress();
    }
    state.indexErrors = errors;
    return {
        records: records.sort((left, right) => left.path.localeCompare(right.path)),
        errors,
        seenPaths,
        skipped,
        completed: files.length,
        total: files.length,
        cancelled: false
    };
}

async function mergeIndexedDocuments(indexedDocuments, options = {}) {
    const existing = await window.RustFrame.db.list("documents");
    const existingByPath = new Map(existing.map((row) => [row.path, row]));
    const root = options.root ?? workspaceProfile().root;
    const seenPaths = options.seenPaths ?? new Set(indexedDocuments.map((entry) => entry.path));
    const removeMissing = options.removeMissing ?? false;

    const operations = [];
    const changes = { inserted: 0, updated: 0, renamed: 0, deleted: 0 };
    const renamedByNewPath = detectRenamedDocuments(existing, indexedDocuments, root, seenPaths, removeMissing);
    const renamedIds = new Set([...renamedByNewPath.values()].map((row) => row.id));
    for (const documentRecord of indexedDocuments) {
        const normalized = normalizeIndexedDocument(documentRecord);
        const current = existingByPath.get(normalized.path) || renamedByNewPath.get(normalized.path);

        if (current) {
            const renamed = current.path !== normalized.path;
            changes[renamed ? "renamed" : "updated"] += 1;
            operations.push({ operation: "update", table: "documents", id: current.id, patch: {
                path: normalized.path,
                title: normalized.title,
                collection: normalized.collection,
                kind: normalized.kind,
                summary: normalized.summary,
                reviewer: normalized.reviewer,
                status: normalized.status,
                priority: normalized.priority,
                tags: normalized.tags,
                readingMinutes: normalized.readingMinutes,
                lineCount: normalized.lineCount,
                fileSize: normalized.fileSize,
                sourceModifiedAt: normalized.sourceModifiedAt,
                contentFingerprint: normalized.contentFingerprint
            }});
        } else {
            changes.inserted += 1;
            operations.push({ operation: "insert", table: "documents", record: {
                ...normalized,
                note: "",
                pinned: false
            }});
        }
    }
    if (removeMissing) {
        for (const current of existing) {
            if (root && current.path.startsWith(`${root}/`) && !seenPaths.has(current.path) && !renamedIds.has(current.id)) {
                changes.deleted += 1;
                operations.push({ operation: "delete", table: "documents", id: current.id });
            }
        }
    }
    for (let index = 0; index < operations.length; index += 500) {
        await window.RustFrame.db.batch(operations.slice(index, index + 500));
    }
    return changes;
}

function detectRenamedDocuments(existing, indexedDocuments, root, seenPaths, removeMissing) {
    const matches = new Map();
    if (!removeMissing || !root) return matches;

    const existingPaths = new Set(existing.map((row) => row.path));
    const missing = existing.filter((row) => row.path.startsWith(`${root}/`) && !seenPaths.has(row.path));
    const added = indexedDocuments.filter((row) => !existingPaths.has(row.path));
    const oldBySignature = new Map();
    for (const row of missing) {
        for (const signature of renameSignatures(row)) {
            const candidates = oldBySignature.get(signature) || [];
            candidates.push(row);
            oldBySignature.set(signature, candidates);
        }
    }

    const claimedIds = new Set();
    for (const row of added) {
        const candidates = renameSignatures(row)
            .map((signature) => (oldBySignature.get(signature) || []).filter((entry) => !claimedIds.has(entry.id)))
            .find((entries) => entries.length === 1) || [];
        if (candidates.length === 1 && (row.contentFingerprint || row.sourceModifiedAt)) {
            matches.set(row.path, candidates[0]);
            claimedIds.add(candidates[0].id);
        }
    }
    return matches;
}

function renameSignatures(row) {
    return [
        row.contentFingerprint ? `content|${row.contentFingerprint}` : null,
        row.sourceModifiedAt ? `metadata|${Number(row.fileSize) || 0}|${String(row.sourceModifiedAt)}` : null
    ].filter(Boolean);
}

function normalizeIndexedDocument(record) {
    return {
        path: String(record.path || "").trim(),
        title: String(record.title || "Untitled note").trim(),
        collection: String(record.collection || "Unsorted").trim(),
        kind: String(record.kind || "memo").trim(),
        summary: String(record.summary || "").trim(),
        reviewer: String(record.reviewer || "").trim(),
        status: STATUS_ORDER.includes(record.status) ? record.status : "queued",
        priority: PRIORITY_ORDER.includes(record.priority) ? record.priority : "watch",
        tags: Array.isArray(record.tags) ? record.tags.map((value) => String(value).trim()).filter(Boolean) : [],
        readingMinutes: Math.max(1, Number(record.readingMinutes) || 1),
        lineCount: Math.max(0, Number(record.lineCount) || 0),
        fileSize: Math.max(0, Number(record.fileSize) || 0),
        sourceModifiedAt: String(record.sourceModifiedAt || "").trim(),
        contentFingerprint: String(record.contentFingerprint || "").trim()
    };
}

async function saveSetting(key, value) {
    const existing = state.settingsByKey.get(key);
    if (existing) {
        const updated = await window.RustFrame.db.update("settings", existing.id, { value });
        state.settingsByKey.set(key, updated);
        return updated;
    }

    const inserted = await window.RustFrame.db.insert("settings", { key, value });
    state.settingsByKey.set(key, inserted);
    return inserted;
}

async function patchSelectedDocument(patch, message) {
    const selected = selectedDocument();
    if (!selected) {
        return;
    }

    await window.RustFrame.db.update("documents", selected.id, patch);
    await refreshDocuments();
    await refreshSelectedContent();
    await refreshWindows();
    writeLog(message);
    render();
}

async function patchReaderDocument(patch, message) {
    if (!state.readerDocument) {
        return;
    }

    state.readerDocument = await window.RustFrame.db.update("documents", state.readerDocument.id, patch);
    await refreshWindows();
    renderReader();
    writeLog(message);
}

function buildActiveFilters() {
    const filters = [];
    if (state.collection !== "all") {
        filters.push({ field: "collection", value: state.collection });
    }
    if (state.status !== "all") {
        filters.push({ field: "status", value: state.status });
    }
    return filters;
}

function filterDocumentsLocally(filters) {
    return state.documents.filter((documentRecord) => filters.every((filter) => {
        if (filter.field === "collection") {
            return documentRecord.collection === filter.value;
        }
        if (filter.field === "status") {
            return documentRecord.status === filter.value;
        }
        return true;
    }));
}

function visibleExportRecords() {
    return visibleDocuments().map((documentRecord) => ({
        path: documentRecord.path,
        title: documentRecord.title,
        collection: documentRecord.collection,
        kind: documentRecord.kind,
        status: documentRecord.status,
        priority: documentRecord.priority,
        tags: normalizeTags(documentRecord.tags),
        reviewer: documentRecord.reviewer,
        note: documentRecord.note,
        summary: documentRecord.summary,
        sourceModifiedAt: documentRecord.sourceModifiedAt
    }));
}

async function exportVisibleDocumentsAsJson() {
    const payload = {
        exportedAt: new Date().toISOString(),
        source: "research-desk",
        count: visibleDocuments().length,
        workspace: workspaceProfile(),
        documents: visibleExportRecords()
    };

    const text = `${JSON.stringify(payload, null, 2)}\n`;
    const dateLabel = new Date().toISOString().slice(0, 10);
    const saved = await window.RustFrame.dialog.saveText({
        title: "Export visible research queue",
        defaultName: `research-desk-export-${dateLabel}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
        contents: text
    });

    if (saved) {
        writeLog(`Exported ${payload.count} visible documents to ${saved.path}.`);
    } else {
        writeLog("Export canceled.");
    }
}

async function exportVisibleDocumentsAsCsv() {
    const rows = visibleExportRecords().map((documentRecord) => ({
        path: documentRecord.path,
        title: documentRecord.title,
        collection: documentRecord.collection,
        kind: documentRecord.kind,
        status: documentRecord.status,
        priority: documentRecord.priority,
        reviewer: documentRecord.reviewer || "",
        tags: documentRecord.tags.join(" | "),
        summary: documentRecord.summary || "",
        note: documentRecord.note || "",
        sourceModifiedAt: documentRecord.sourceModifiedAt || ""
    }));
    const header = [
        "path",
        "title",
        "collection",
        "kind",
        "status",
        "priority",
        "reviewer",
        "tags",
        "summary",
        "note",
        "sourceModifiedAt"
    ];
    const csv = serializeCsv(header, rows);
    const dateLabel = new Date().toISOString().slice(0, 10);
    const saved = await window.RustFrame.dialog.saveText({
        title: "Export visible research queue as CSV",
        defaultName: `research-desk-export-${dateLabel}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
        contents: csv
    });

    if (saved) {
        writeLog(`Exported ${rows.length} visible documents to ${saved.path}.`);
    } else {
        writeLog("Export canceled.");
    }
}

async function exportVisibleDocumentsAsJsonl() {
    const rows = visibleExportRecords();
    const jsonl = `${rows.map((row) => JSON.stringify(row)).join("\n")}${rows.length ? "\n" : ""}`;
    const dateLabel = new Date().toISOString().slice(0, 10);
    const saved = await window.RustFrame.dialog.saveText({
        title: "Export visible research queue as JSONL",
        defaultName: `research-desk-export-${dateLabel}.jsonl`,
        filters: [{ name: "JSON Lines", extensions: ["jsonl"] }],
        contents: jsonl
    });
    writeLog(saved ? `Exported ${rows.length} visible documents to ${saved.path}.` : "Export canceled.");
}

async function exportEverything() {
    const grants = await window.RustFrame.fs.listGrants();
    const payload = {
        format: "research-desk-full-export-v1",
        exportedAt: new Date().toISOString(),
        database: state.dbInfo,
        documents: state.documents,
        settings: [...state.settingsByKey.values()],
        filesystemGrants: grants
    };
    const dateLabel = new Date().toISOString().slice(0, 10);
    const saved = await window.RustFrame.dialog.saveText({
        title: "Export all Research Desk data",
        defaultName: `research-desk-everything-${dateLabel}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
        contents: `${JSON.stringify(payload, null, 2)}\n`
    });
    writeLog(saved
        ? `Exported all ${state.documents.length} records, settings, and grant metadata to ${saved.path}.`
        : "Full data export canceled.");
}

async function exportDiagnosticBundle() {
    const grants = await window.RustFrame.fs.listGrants();
    const payload = {
        format: "rustframe-diagnostic-bundle-v1",
        generatedAt: new Date().toISOString(),
        app: { id: "research-desk", version: "0.1.0-rc.2", route: ROUTE_PATH },
        runtime: {
            security: window.RustFrame.security,
            database: state.dbInfo,
            windows: state.windows
        },
        state: {
            documentCount: state.documents.length,
            visibleCount: visibleDocuments().length,
            settingKeys: [...state.settingsByKey.keys()],
            grantCount: grants.length,
            grants,
            activeFilter: currentFilterState(),
            lastIndexErrors: state.indexErrors,
            lastLog: state.log
        }
    };
    const redacted = redactPrivateValue(payload);
    const saved = await window.RustFrame.dialog.saveText({
        title: "Save redacted diagnostic bundle",
        defaultName: `research-desk-diagnostics-${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
        contents: `${JSON.stringify(redacted, null, 2)}\n`
    });
    writeLog(saved
        ? `Saved a diagnostic bundle with filesystem paths and grant identifiers redacted to ${saved.path}.`
        : "Diagnostic export canceled.");
}

async function revokeCurrentWorkspace() {
    const profile = workspaceProfile();
    if (!profile.grantId) {
        writeLog("There is no retained workspace grant to revoke.");
        return;
    }
    if (state.activeWatcherId) {
        await window.RustFrame.fs.unwatch(state.activeWatcherId);
        state.activeWatcherId = null;
    }
    const revoked = await window.RustFrame.fs.revokeGrant(profile.grantId);
    const recent = recentWorkspaces().filter((entry) => entry.grantId !== profile.grantId);
    await saveSetting("recentWorkspaces", recent);
    await saveSetting("workspaceProfile", {
        label: "No workspace selected",
        root: null,
        command: "RustFrame native indexer",
        fileCount: 0,
        lastIndexedAt: null,
        revokedAt: new Date().toISOString(),
        previousLabel: profile.label
    });
    await loadSettings();
    state.workspaceEntries = [];
    state.selectedContent = "Folder access was revoked. Indexed metadata and review notes remain in SQLite until you export or delete them.";
    writeLog(revoked
        ? `Revoked access to "${profile.label}". Source files were not changed; indexed metadata remains local.`
        : `The grant for "${profile.label}" was already unavailable. Source files were not changed.`);
    render();
}

async function deleteAllLocalData() {
    const backup = await window.RustFrame.db.backup({
        suggestedName: `research-desk-before-delete-${new Date().toISOString().slice(0, 10)}.db`
    });
    if (backup.cancelled) {
        writeLog("Deletion canceled because the safety backup was not saved. No local data changed.");
        return;
    }

    const profile = workspaceProfile();
    if (state.activeWatcherId) {
        await window.RustFrame.fs.unwatch(state.activeWatcherId);
        state.activeWatcherId = null;
    }
    if (profile.grantId) {
        await window.RustFrame.fs.revokeGrant(profile.grantId);
    }
    const operations = [
        ...state.documents.map((row) => ({ operation: "delete", table: "documents", id: row.id })),
        ...[...state.settingsByKey.values()].map((row) => ({ operation: "delete", table: "settings", id: row.id }))
    ];
    for (let index = 0; index < operations.length; index += 500) {
        await window.RustFrame.db.batch(operations.slice(index, index + 500));
    }
    state.documents = [];
    state.visibleDocuments = [];
    state.settingsByKey = new Map();
    state.savedFilters = [];
    state.workspaceEntries = [];
    state.selectedId = null;
    state.selectedContent = "";
    state.privacyPanelOpen = false;
    state.deleteConfirmationOpen = false;
    writeLog("Deleted Research Desk records, settings, and retained workspace access. Original source files were not touched. A safety backup was saved first.");
    render();
}

async function importExternalFiles(fileEntries, sourceLabel) {
    const provided = Array.isArray(fileEntries) ? fileEntries : [];
    if (!provided.length) {
        writeLog("Import canceled.");
        return;
    }

    const supported = provided
        .filter((entry) => entry && entry.isFile)
        .filter((entry) => ["md", "txt"].includes(normalizeExtension(entry.extension)));

    if (!supported.length) {
        writeLog("No supported Markdown or text files were provided for import.");
        return;
    }

    state.importBusy = true;
    render();

    try {
        const indexed = [];
        for (const fileEntry of supported) {
            const text = await window.RustFrame.fs.readText(fileEntry.uri);
            indexed.push(buildIndexedRecord(fileEntry.uri, fileEntry, text));
        }
        await mergeIndexedDocuments(indexed);
        await refreshDocuments();
        selectDefaultDocument();
        await refreshSelectedContent();
        writeLog(`Added ${supported.length} temporary-grant files from ${sourceLabel}.`);
    } finally {
        state.importBusy = false;
        render();
    }
}

function normalizeExtension(value) {
    return String(value || "").trim().replace(/^\./u, "").toLowerCase();
}

async function handleInput(event) {
    if (event.target.id === "search-input") {
        try {
            state.search = event.target.value;
            scheduleActiveFilterSave();
            await refreshVisibleDocuments();
            selectDefaultDocument();
            await refreshSelectedContent();
            render();
        } catch (error) {
            writeLog(formatError(error));
            render();
        }
    }
}

function handleKeyboardShortcut(event) {
    if (state.mode !== "main") return;
    const target = event.target;
    const isTyping = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;

    if ((event.key === "/" && !isTyping) || ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k")) {
        event.preventDefault();
        APP.querySelector("#search-input")?.focus();
        return;
    }

    if (event.key === "Escape" && (state.privacyPanelOpen || state.deleteConfirmationOpen)) {
        event.preventDefault();
        state.privacyPanelOpen = false;
        state.deleteConfirmationOpen = false;
        render();
        APP.querySelector('[data-action="show-data"]')?.focus();
    }
}

async function handleClick(event) {
    const button = event.target.closest("[data-action]");
    if (!button) {
        return;
    }

    const action = button.dataset.action;

    try {
        if (action === "index") {
            await indexWorkspace("manual refresh");
            return;
        }

        if (action === "cancel-index") {
            state.indexCancelRequested = true;
            updateIndexProgress();
            return;
        }

        if (action === "choose-workspace") {
            await chooseWorkspace();
            return;
        }

        if (action === "switch-workspace") {
            await switchWorkspace(button.dataset.grantId);
            return;
        }

        if (action === "backup-db") {
            await backupDatabase();
            return;
        }

        if (action === "restore-db") {
            await restoreDatabase();
            return;
        }

        if (action === "export-json") {
            await exportVisibleDocumentsAsJson();
            return;
        }

        if (action === "export-jsonl") {
            await exportVisibleDocumentsAsJsonl();
            return;
        }

        if (action === "export-csv") {
            await exportVisibleDocumentsAsCsv();
            return;
        }

        if (action === "export-everything") {
            await exportEverything();
            return;
        }

        if (action === "export-diagnostics") {
            await exportDiagnosticBundle();
            return;
        }

        if (action === "show-data") {
            state.privacyPanelOpen = true;
            state.deleteConfirmationOpen = false;
            render();
            APP.querySelector("#privacy-panel-title")?.focus();
            return;
        }

        if (action === "close-data-panel") {
            state.privacyPanelOpen = false;
            state.deleteConfirmationOpen = false;
            render();
            APP.querySelector('[data-action="show-data"]')?.focus();
            return;
        }

        if (action === "open-delete-confirmation") {
            state.deleteConfirmationOpen = true;
            render();
            APP.querySelector("#delete-panel-title")?.focus();
            return;
        }

        if (action === "cancel-delete") {
            state.deleteConfirmationOpen = false;
            render();
            APP.querySelector('[data-action="open-delete-confirmation"]')?.focus();
            return;
        }

        if (action === "confirm-delete") {
            await deleteAllLocalData();
            return;
        }

        if (action === "revoke-workspace") {
            await revokeCurrentWorkspace();
            return;
        }

        if (action === "save-filter") {
            await saveCurrentFilter();
            return;
        }

        if (action === "apply-saved-filter") {
            await applySavedFilter(button.dataset.filterId);
            return;
        }

        if (action === "delete-saved-filter") {
            await deleteSavedFilter(button.dataset.filterId);
            return;
        }

        if (action === "clear-filters") {
            await clearActiveFilters();
            return;
        }

        if (action === "sync-title") {
            await window.RustFrame.window.setTitle(
                `Research Desk · ${visibleDocuments().length} visible documents`
            );
            writeLog("Window title synced to the visible research queue.");
            return;
        }

        if (action === "close-window") {
            await window.RustFrame.window.close();
            return;
        }

        if (action === "filter-status") {
            state.status = button.dataset.status || "all";
            scheduleActiveFilterSave();
            await refreshVisibleDocuments();
            selectDefaultDocument();
            await refreshSelectedContent();
            render();
            return;
        }

        if (action === "filter-collection") {
            state.collection = button.dataset.collection || "all";
            scheduleActiveFilterSave();
            await refreshVisibleDocuments();
            selectDefaultDocument();
            await refreshSelectedContent();
            render();
            return;
        }

        if (action === "select-document") {
            state.selectedId = Number(button.dataset.id);
            state.selectedContent = "Loading source document…";
            render();
            await refreshSelectedContent();
            render();
            return;
        }

        if (action === "toggle-pin") {
            const documentRecord = documentById(Number(button.dataset.id));
            if (!documentRecord) {
                return;
            }
            await patchSelectedDocument(
                { pinned: !documentRecord.pinned },
                `${documentRecord.pinned ? "Unpinned" : "Pinned"} "${documentRecord.title}".`
            );
            return;
        }

        if (action === "set-status") {
            await patchSelectedDocument(
                { status: button.dataset.status },
                `Updated status for "${selectedDocument().title}" to ${button.dataset.status}.`
            );
            return;
        }

        if (action === "set-priority") {
            await patchSelectedDocument(
                { priority: button.dataset.priority },
                `Updated priority for "${selectedDocument().title}" to ${button.dataset.priority}.`
            );
            return;
        }

        if (action === "save-note") {
            const textarea = APP.querySelector("#note-input");
            await patchSelectedDocument(
                { note: textarea ? textarea.value.trim() : "" },
                `Saved review note for "${selectedDocument().title}".`
            );
            return;
        }

        if (action === "open-reader") {
            const documentRecord = documentById(Number(button.dataset.id)) || selectedDocument();
            if (!documentRecord) {
                return;
            }
            await window.RustFrame.window.open({
                route: `/reader?doc=${documentRecord.id}`,
                title: `${documentRecord.title} · Reader`,
                width: 1040,
                height: 780
            });
            await refreshWindows();
            render();
            return;
        }

        if (action === "open-source") {
            const documentRecord = documentById(Number(button.dataset.id)) || selectedDocument();
            if (!documentRecord) {
                return;
            }
            await window.RustFrame.fs.openPath(documentRecord.path);
            writeLog(`Opened source file for "${documentRecord.title}".`);
            return;
        }

        if (action === "reveal-source") {
            const documentRecord = documentById(Number(button.dataset.id)) || selectedDocument();
            if (!documentRecord) {
                return;
            }
            await window.RustFrame.fs.revealPath(documentRecord.path);
            writeLog(`Revealed source file for "${documentRecord.title}" in the file manager.`);
            return;
        }

        if (action === "copy-source-path") {
            const documentRecord = documentById(Number(button.dataset.id)) || selectedDocument();
            if (!documentRecord) {
                return;
            }
            await window.RustFrame.clipboard.writeText(documentRecord.path);
            writeLog(`Copied source path for "${documentRecord.title}".`);
            return;
        }

        if (action === "reader-set-status" && state.readerDocument) {
            await patchReaderDocument(
                { status: button.dataset.status },
                `Updated status for "${state.readerDocument.title}" to ${button.dataset.status}.`
            );
            return;
        }

        if (action === "reader-set-priority" && state.readerDocument) {
            await patchReaderDocument(
                { priority: button.dataset.priority },
                `Updated priority for "${state.readerDocument.title}" to ${button.dataset.priority}.`
            );
            return;
        }

        if (action === "reader-toggle-pin" && state.readerDocument) {
            await patchReaderDocument(
                { pinned: !state.readerDocument.pinned },
                `${state.readerDocument.pinned ? "Unpinned" : "Pinned"} "${state.readerDocument.title}".`
            );
            return;
        }

        if (action === "reader-save-note" && state.readerDocument) {
            const textarea = APP.querySelector("#reader-note-input");
            await patchReaderDocument(
                { note: textarea ? textarea.value.trim() : "" },
                `Saved review note for "${state.readerDocument.title}".`
            );
            return;
        }

        if (action === "reader-refresh" && state.readerDocument) {
            await loadReaderDocument();
            renderReader();
            writeLog(`Reloaded source file for "${state.readerDocument.title}".`);
            return;
        }

        if (action === "reader-open-source" && state.readerDocument) {
            await window.RustFrame.fs.openPath(state.readerDocument.path);
            writeLog(`Opened source file for "${state.readerDocument.title}".`);
            return;
        }

        if (action === "reader-reveal-source" && state.readerDocument) {
            await window.RustFrame.fs.revealPath(state.readerDocument.path);
            writeLog(`Revealed source file for "${state.readerDocument.title}" in the file manager.`);
            return;
        }

        if (action === "reader-copy-source-path" && state.readerDocument) {
            await window.RustFrame.clipboard.writeText(state.readerDocument.path);
            writeLog(`Copied source path for "${state.readerDocument.title}".`);
        }
    } catch (error) {
        writeLog(formatError(error));
        render();
    }
}

function render() {
    if (state.mode === "main") {
        renderMain();
    } else {
        renderReader();
    }
}

function renderMain() {
    const documents = visibleDocuments();
    if (state.selectedId && !state.documents.some((entry) => entry.id === state.selectedId)) {
        state.selectedId = documents.length ? documents[0].id : null;
    }

    const selected = selectedDocument();
    const workspace = workspaceProfile();
    const collectionNames = ["all", ...new Set(state.documents.map((entry) => entry.collection))];
    const readerWindows = state.windows.filter((entry) => !entry.isPrimary).length;
    const reviewQueue = state.documents.filter((entry) => entry.status === "queued" || entry.status === "reviewing").length;
    const workspaceFolders = state.workspaceEntries.filter((entry) => entry.isDir);
    const recent = recentWorkspaces();

    APP.innerHTML = `
        <section class="shell-frame masthead">
            <article class="headline">
                <p class="eyebrow">Research Desk</p>
                <h1>Review a local archive, store decisions in SQLite, and keep the source files close.</h1>
                <p class="section-copy">
                    Choose any Markdown or text-document folder. Research Desk indexes it natively,
                    keeps every source file in place, and synchronizes review state across focused reader windows.
                </p>
                <div class="action-row">
                    <button class="button button-primary" type="button" data-action="choose-workspace" ${state.importBusy ? "disabled" : ""}>${workspace.root ? "Change workspace" : "Choose workspace"}</button>
                    <button class="button" type="button" data-action="index" ${state.importBusy || !workspace.root ? "disabled" : ""}>
                        ${state.importBusy ? "Indexing archive…" : "Index workspace"}
                    </button>
                    <button class="button" type="button" data-action="show-data">My data &amp; privacy</button>
                    ${workspace.grantId ? `<button class="ghost-button" type="button" data-action="revoke-workspace" ${state.importBusy ? "disabled" : ""}>Revoke folder access</button>` : ""}
                </div>
                ${state.indexProgress ? `
                    <div class="index-progress" aria-live="polite">
                        <div>
                            <strong>${state.indexProgress.phase === "committing" ? "Committing one atomic SQLite batch" : "Reading changed files only"}</strong>
                            <span id="index-progress-label">${state.indexProgress.completed} of ${state.indexProgress.total} files · ${state.indexProgress.skipped} unchanged</span>
                        </div>
                        <progress id="index-progress" max="${Math.max(1, state.indexProgress.total)}" value="${state.indexProgress.completed}"></progress>
                        ${state.indexProgress.phase === "reading" ? `<button class="ghost-button" type="button" data-action="cancel-index">Cancel indexing</button>` : ""}
                    </div>
                ` : ""}
            </article>

            <aside class="status-meta">
                <div class="status-grid">
                    <article class="status-card">
                        <p class="eyebrow">Workspace</p>
                        <strong>${escapeHtml(workspace.label)}</strong>
                        <p class="status-detail">${workspace.root ? `Access: <code>${escapeHtml(workspace.root)}</code>` : "No folder access retained"}</p>
                    </article>
                    <article class="status-card">
                        <p class="eyebrow">Last import</p>
                        <strong>${escapeHtml(workspace.lastIndexedAt ? formatDateTime(workspace.lastIndexedAt) : "Not indexed yet")}</strong>
                        <p class="status-detail">${workspace.lastIndexedAt ? `Indexer: ${escapeHtml(workspace.command || "RustFrame native indexer")}` : "Ready for native indexing"}</p>
                    </article>
                    <article class="status-card">
                        <p class="eyebrow">Database</p>
                        <strong>Local SQLite</strong>
                        <p class="status-detail">Schema version ${escapeHtml(String(state.dbInfo.schemaVersion))} · private app data</p>
                    </article>
                    <article class="status-card">
                        <p class="eyebrow">Windows</p>
                        <strong>${escapeHtml(String(state.windows.length))} open</strong>
                        <p class="status-detail">${readerWindows} reader windows</p>
                    </article>
                </div>
            </aside>
        </section>

        ${!workspace.root ? renderConsentPanel() : ""}
        ${state.privacyPanelOpen ? renderPrivacyPanel() : ""}

        <section class="metric-grid">
            <article class="metric panel"><span>Documents</span><strong>${escapeHtml(String(state.documents.length))}</strong><p>Indexed archive records</p></article>
            <article class="metric panel"><span>Collections</span><strong>${escapeHtml(String(new Set(state.documents.map((entry) => entry.collection)).size))}</strong><p>Distinct research tracks</p></article>
            <article class="metric panel"><span>Needs review</span><strong>${escapeHtml(String(reviewQueue))}</strong><p>Queued or reviewing</p></article>
            <article class="metric panel"><span>Visible now</span><strong>${escapeHtml(String(documents.length))}</strong><p>Current filtered queue</p></article>
        </section>

        <section class="workspace-grid">
            <aside class="panel sidebar">
                <div class="section-head">
                    <p class="eyebrow">Queue filters</p>
                    <h2>Focus the archive</h2>
                </div>

                <label class="label" for="search-input">Search</label>
                <div class="search-field">
                    <input id="search-input" type="search" value="${escapeHtml(state.search)}" placeholder="Full-text search title, summary, note, tags, reviewer">
                </div>

                <div class="filter-stack">
                    <div>
                        <p class="label">Status</p>
                        <div class="filter-row">
                            ${renderStatusFilterButtons()}
                        </div>
                    </div>

                    <div>
                        <p class="label">Collection</p>
                        <div class="filter-row">
                            ${collectionNames.map((collection) => `
                                <button
                                    type="button"
                                    class="chip ${state.collection === collection ? "is-active" : ""}"
                                    data-action="filter-collection"
                                    data-collection="${escapeHtml(collection)}"
                                >${escapeHtml(collection === "all" ? "All collections" : collection)}</button>
                            `).join("")}
                        </div>
                    </div>
                </div>

                <div class="saved-filter-toolbar">
                    <button class="chip" type="button" data-action="save-filter">Save current view</button>
                    <button class="chip is-muted" type="button" data-action="clear-filters">Reset filters</button>
                </div>
                <div class="saved-filter-list" aria-label="Saved filter views">
                    ${state.savedFilters.length ? state.savedFilters.map((filter) => `
                        <div class="saved-filter">
                            <button type="button" data-action="apply-saved-filter" data-filter-id="${escapeHtml(filter.id)}">
                                <strong>${escapeHtml(filter.label)}</strong>
                                <small>${escapeHtml(formatDateTime(filter.savedAt))}</small>
                            </button>
                            <button class="saved-filter-remove" type="button" data-action="delete-saved-filter" data-filter-id="${escapeHtml(filter.id)}" aria-label="Remove saved view ${escapeHtml(filter.label)}">×</button>
                        </div>
                    `).join("") : `<p class="section-copy">Save a search and filter combination for one-click recall.</p>`}
                </div>

                <div class="section-divider"></div>

                <div class="window-list">
                    <p class="label">Recent workspaces</p>
                    ${recent.length ? recent.map((entry) => `
                        <button class="window-chip" type="button" data-action="switch-workspace" data-grant-id="${escapeHtml(entry.grantId)}">
                            <small>${escapeHtml(formatDateTime(entry.lastOpenedAt))}</small>
                            <strong>${escapeHtml(entry.label)}</strong>
                        </button>
                    `).join("") : `<p class="section-copy">Choose a folder to add your first recent workspace.</p>`}
                </div>

                <div class="section-divider"></div>

                <div class="meta-list">
                    <div class="meta-box">
                        <p class="label">Workflow proof</p>
                        <strong>Embedded SQLite + real source files</strong>
                        <p class="section-copy">The review state lives in SQLite. The source documents stay in the filesystem root and are read directly when selected.</p>
                    </div>
                    <div class="meta-box">
                        <p class="label">Native indexing</p>
                        <strong>No Python or external runtime</strong>
                        <p class="section-copy">RustFrame walks the selected grant, reads supported files, and commits changes in atomic SQLite batches.</p>
                    </div>
                    <div class="meta-box">
                        <p class="label">Private by default</p>
                        <strong>User-selected folder grant</strong>
                        <p class="section-copy">The frontend retains an opaque <code>grant://</code> URI. Absolute paths stay inside the native runtime.</p>
                    </div>
                </div>

                <div class="section-divider"></div>

                <div class="window-list">
                    <p class="label">Workspace folders</p>
                    ${workspaceFolders.length ? workspaceFolders.map((entry) => `
                        <div class="window-chip">
                            <small>${escapeHtml(entry.path)}</small>
                            <strong>${escapeHtml(entry.name)}</strong>
                        </div>
                    `).join("") : `<p class="section-copy">No workspace folders were discovered yet.</p>`}
                </div>

                <div class="section-divider"></div>

                <div class="window-list">
                    <p class="label">Open windows</p>
                    ${state.windows.map((entry) => `
                        <div class="window-chip">
                            <small>${escapeHtml(entry.route)}</small>
                            <strong>${escapeHtml(entry.title)}</strong>
                        </div>
                    `).join("")}
                </div>

                <div class="section-divider"></div>

                <div class="log-list">
                    <p class="label">Runtime log</p>
                    <pre class="log-box" aria-live="polite">${escapeHtml(state.log)}</pre>
                </div>
            </aside>

            <section class="panel document-panel">
                <div class="section-head">
                    <p class="eyebrow">Indexed documents</p>
                    <h2>Archive queue</h2>
                </div>
                <div class="document-list">
                    ${documents.length ? documents.map((documentRecord) => renderDocumentCard(documentRecord)).join("") : `
                        <div class="empty-state">
                            <div>
                                <h2>No documents match the current filters.</h2>
                                <p>Reset the filters or re-index the workspace to repopulate the queue.</p>
                            </div>
                        </div>
                    `}
                </div>
            </section>

            <section class="panel preview-panel">
                ${selected ? renderPreview(selected) : `
                    <div class="empty-state">
                        <div>
                            <h2>Select a document to inspect the source file.</h2>
                            <p>The preview pane reads the document body directly from the local workspace root through <code>window.RustFrame.fs.readText(...)</code>.</p>
                        </div>
                    </div>
                `}
            </section>
        </section>
    `;
}

function renderConsentPanel() {
    return `
        <section class="consent-panel" aria-labelledby="consent-title">
            <div class="consent-copy">
                <p class="eyebrow">First run · explicit consent</p>
                <h2 id="consent-title">Your archive stays where it is.</h2>
                <p>Research Desk asks for one read-only folder grant. It does not scan your home folder, upload files, or modify source documents.</p>
                <button class="button button-primary" type="button" data-action="choose-workspace">Choose a folder to review</button>
            </div>
            <ol class="consent-steps">
                <li><span>01</span><div><strong>You choose the boundary</strong><p>Only Markdown and text files inside the selected folder are visible to this app.</p></div></li>
                <li><span>02</span><div><strong>RustFrame stores an opaque grant</strong><p>The web UI receives a <code>grant://</code> URI, never an unrestricted filesystem path.</p></div></li>
                <li><span>03</span><div><strong>You stay in control</strong><p>Export everything, revoke access, or safely delete app data without touching the original archive.</p></div></li>
            </ol>
        </section>
    `;
}

function renderPrivacyPanel() {
    const workspace = workspaceProfile();
    return `
        <section class="privacy-overlay" role="dialog" aria-modal="true" aria-labelledby="privacy-panel-title">
            <div class="privacy-panel panel">
                <div class="privacy-heading">
                    <div>
                        <p class="eyebrow">Data control center</p>
                        <h2 id="privacy-panel-title" tabindex="-1">See, export, back up, or erase your local data.</h2>
                    </div>
                    <button class="ghost-button" type="button" data-action="close-data-panel" aria-label="Close data control center">Close</button>
                </div>

                <div class="data-map">
                    <article>
                        <span>SQLite records</span>
                        <strong>${escapeHtml(String(state.documents.length))} documents · ${escapeHtml(String(state.settingsByKey.size))} settings</strong>
                        <code>${escapeHtml(state.dbInfo.databasePath)}</code>
                    </article>
                    <article>
                        <span>Source archive</span>
                        <strong>${escapeHtml(workspace.root ? workspace.label : "No retained access")}</strong>
                        <code>${escapeHtml(workspace.root || "No grant:// URI retained")}</code>
                    </article>
                    <article>
                        <span>Network storage</span>
                        <strong>None</strong>
                        <p>No sync account, telemetry endpoint, or cloud database is configured.</p>
                    </article>
                </div>

                <div class="privacy-actions">
                    <div>
                        <p class="label">Visible queue</p>
                        <div class="action-row">
                            <button class="button" type="button" data-action="export-json">JSON</button>
                            <button class="button" type="button" data-action="export-jsonl">JSONL</button>
                            <button class="button" type="button" data-action="export-csv">CSV</button>
                        </div>
                    </div>
                    <div>
                        <p class="label">Everything</p>
                        <div class="action-row">
                            <button class="button button-primary" type="button" data-action="export-everything">Export all app data</button>
                            <button class="button" type="button" data-action="backup-db">Back up SQLite</button>
                            <button class="button" type="button" data-action="restore-db">Restore backup</button>
                        </div>
                    </div>
                    <div>
                        <p class="label">Support</p>
                        <div class="action-row">
                            <button class="button" type="button" data-action="export-diagnostics">Save redacted diagnostics</button>
                            ${workspace.grantId ? `<button class="button" type="button" data-action="revoke-workspace">Revoke folder access</button>` : ""}
                        </div>
                    </div>
                </div>

                <div class="danger-zone">
                    <div>
                        <p class="label">Danger zone</p>
                        <strong>Delete Research Desk local data</strong>
                        <p>This removes SQLite records, settings, and retained access. It never deletes source files.</p>
                    </div>
                    <button class="button button-danger" type="button" data-action="open-delete-confirmation">Review deletion</button>
                </div>

                ${state.deleteConfirmationOpen ? `
                    <div class="delete-confirmation" role="alertdialog" aria-modal="true" aria-labelledby="delete-panel-title">
                        <h3 id="delete-panel-title" tabindex="-1">Back up, then delete local app data?</h3>
                        <p>Research Desk will first require you to save a SQLite safety backup. Canceling that dialog cancels deletion. It never deletes source files.</p>
                        <div class="action-row">
                            <button class="button button-danger" type="button" data-action="confirm-delete">Save backup &amp; delete</button>
                            <button class="ghost-button" type="button" data-action="cancel-delete">Keep my data</button>
                        </div>
                    </div>
                ` : ""}
            </div>
        </section>
    `;
}

function renderReader() {
    if (!state.readerDocument) {
        APP.innerHTML = `
            <section class="panel empty-state">
                <div>
                    <p class="eyebrow">Reader</p>
                    <h2>The requested document could not be found.</h2>
                    <p>The source record may have been removed or the route did not include a valid document id.</p>
                    <button class="button button-primary" type="button" data-action="close-window">Close reader</button>
                </div>
            </section>
        `;
        return;
    }

    APP.innerHTML = `
        <section class="shell-frame">
            <div class="reader-header">
                <p class="eyebrow">Reader window</p>
                <h1 class="reader-title">${escapeHtml(state.readerDocument.title)}</h1>
                <p class="reader-note">${escapeHtml(state.readerDocument.summary || "No summary available.")}</p>
            </div>
            <div class="reader-toolbar">
                <button class="button button-primary" type="button" data-action="reader-refresh">Reload source</button>
                <button class="button" type="button" data-action="open-reader" data-id="${state.readerDocument.id}">Open another reader</button>
                <button class="button" type="button" data-action="reader-open-source">Open source</button>
                <button class="button" type="button" data-action="reader-reveal-source">Reveal in folder</button>
                <button class="ghost-button" type="button" data-action="reader-copy-source-path">Copy path</button>
                <button class="ghost-button" type="button" data-action="close-window">Close</button>
            </div>
        </section>

        <section class="reader-shell">
            <article class="panel reader-document">
                <div class="badge-row">
                    ${renderTag(state.readerDocument.collection, "")}
                    ${renderTag(state.readerDocument.kind, "")}
                    ${renderTag(state.readerDocument.status, `is-status-${state.readerDocument.status}`)}
                    ${renderTag(state.readerDocument.priority, `is-priority-${state.readerDocument.priority}`)}
                    ${normalizeTags(state.readerDocument.tags).map((tag) => renderTag(tag, "")).join("")}
                </div>

                <div class="reader-paper">
                    ${renderRichText(stripFrontmatter(state.readerContent))}
                </div>
            </article>

            <aside class="panel reader-sidebar">
                <div class="section-head">
                    <p class="eyebrow">Review controls</p>
                    <h2>Update this document in place</h2>
                </div>

                <div>
                    <p class="label">Status</p>
                    <div class="status-row">
                        ${STATUS_ORDER.map((status) => `
                            <button
                                type="button"
                                class="status-button ${state.readerDocument.status === status ? "is-active" : ""}"
                                data-action="reader-set-status"
                                data-status="${status}"
                            >${escapeHtml(status)}</button>
                        `).join("")}
                    </div>
                </div>

                <div>
                    <p class="label">Priority</p>
                    <div class="status-row">
                        ${PRIORITY_ORDER.map((priority) => `
                            <button
                                type="button"
                                class="status-button ${state.readerDocument.priority === priority ? "is-active" : ""}"
                                data-action="reader-set-priority"
                                data-priority="${priority}"
                            >${escapeHtml(priority)}</button>
                        `).join("")}
                    </div>
                </div>

                <button class="button" type="button" data-action="reader-toggle-pin">
                    ${state.readerDocument.pinned ? "Unpin from queue" : "Pin to top of queue"}
                </button>

                <div class="meta-list">
                    <div class="meta-box">
                        <p class="label">Source file</p>
                        <strong>${escapeHtml(state.readerFileMeta?.path || state.readerDocument.path)}</strong>
                        <p class="section-copy">${escapeHtml(formatBytes(state.readerFileMeta?.size ?? state.readerDocument.fileSize))} · ${escapeHtml(String(state.readerDocument.lineCount))} lines · ${escapeHtml(String(state.readerDocument.readingMinutes))} min read</p>
                    </div>
                    <div class="meta-box">
                        <p class="label">Last modified</p>
                        <strong>${escapeHtml(formatDateTime(state.readerFileMeta?.modifiedAt || state.readerDocument.sourceModifiedAt))}</strong>
                        <p class="section-copy">Reader windows share the same runtime and the same database as the main queue.</p>
                    </div>
                </div>

                <label class="label" for="reader-note-input">Review note</label>
                <div class="note-field">
                    <textarea id="reader-note-input" placeholder="Capture what to brief back to the team.">${escapeHtml(state.readerDocument.note || "")}</textarea>
                </div>
                <button class="button button-primary" type="button" data-action="reader-save-note">Save review note</button>

                <div class="section-divider"></div>
                <div class="log-list">
                    <p class="label">Runtime log</p>
                    <pre class="log-box">${escapeHtml(state.log)}</pre>
                </div>
            </aside>
        </section>
    `;
}

function renderPreview(documentRecord) {
    return `
        <div class="preview-shell">
            <div class="preview-header">
                <p class="eyebrow">Selected document</p>
                <h2>${renderHighlightedText(documentRecord.title)}</h2>
                <p class="section-copy">${renderHighlightedText(documentRecord.summary || "No summary available.")}</p>
            </div>

            <div class="badge-row">
                ${renderTag(documentRecord.collection, "")}
                ${renderTag(documentRecord.kind, "")}
                ${renderTag(documentRecord.status, `is-status-${documentRecord.status}`)}
                ${renderTag(documentRecord.priority, `is-priority-${documentRecord.priority}`)}
                ${normalizeTags(documentRecord.tags).map((tag) => renderTag(tag, "")).join("")}
            </div>

            <div class="document-actions">
                <button class="button button-primary" type="button" data-action="open-reader" data-id="${documentRecord.id}">Open reader window</button>
                <button class="button" type="button" data-action="open-source" data-id="${documentRecord.id}">Open source</button>
                <button class="button" type="button" data-action="reveal-source" data-id="${documentRecord.id}">Reveal in folder</button>
                <button class="ghost-button" type="button" data-action="copy-source-path" data-id="${documentRecord.id}">Copy path</button>
                <button class="button" type="button" data-action="toggle-pin" data-id="${documentRecord.id}">
                    ${documentRecord.pinned ? "Unpin" : "Pin"}
                </button>
            </div>

            <div>
                <p class="label">Status</p>
                <div class="status-row">
                    ${STATUS_ORDER.map((status) => `
                        <button
                            type="button"
                            class="status-button ${documentRecord.status === status ? "is-active" : ""}"
                            data-action="set-status"
                            data-status="${status}"
                        >${escapeHtml(status)}</button>
                    `).join("")}
                </div>
            </div>

            <div>
                <p class="label">Priority</p>
                <div class="status-row">
                    ${PRIORITY_ORDER.map((priority) => `
                        <button
                            type="button"
                            class="status-button ${documentRecord.priority === priority ? "is-active" : ""}"
                            data-action="set-priority"
                            data-priority="${priority}"
                        >${escapeHtml(priority)}</button>
                    `).join("")}
                </div>
            </div>

            <div class="meta-list">
                <div class="meta-box">
                    <p class="label">Source file</p>
                    <strong>${escapeHtml(state.selectedFileMeta?.path || documentRecord.path)}</strong>
                    <p class="section-copy">${escapeHtml(formatDateTime(state.selectedFileMeta?.modifiedAt || documentRecord.sourceModifiedAt))}</p>
                </div>
                <div class="meta-box">
                    <p class="label">Reviewer</p>
                    <strong>${escapeHtml(documentRecord.reviewer || "Unassigned")}</strong>
                    <p class="section-copy">${escapeHtml(formatBytes(state.selectedFileMeta?.size ?? documentRecord.fileSize))} · ${escapeHtml(String(documentRecord.lineCount))} lines · ${escapeHtml(String(documentRecord.readingMinutes))} min read</p>
                </div>
            </div>

            <label class="label" for="note-input">Review note</label>
            <div class="note-field">
                <textarea id="note-input" placeholder="Capture the callout, decision, or contradiction worth sharing.">${escapeHtml(documentRecord.note || "")}</textarea>
            </div>
            <p class="note-help">Review notes stay in SQLite, while the document body below comes from the filesystem bridge.</p>
            <button class="button button-primary" type="button" data-action="save-note">Save note</button>

            <div class="preview-paper">
                ${renderRichText(stripFrontmatter(state.selectedContent))}
            </div>
        </div>
    `;
}

function renderDocumentCard(documentRecord) {
    const selectedClass = documentRecord.id === state.selectedId ? "is-selected" : "";
    return `
        <article class="document-card ${selectedClass}" data-action="select-document" data-id="${documentRecord.id}">
            <div class="document-card-head">
                <div>
                    <p class="eyebrow">${escapeHtml(documentRecord.collection)}</p>
                    <h3>${renderHighlightedText(documentRecord.title)}</h3>
                </div>
                ${documentRecord.pinned ? renderTag("Pinned", "") : ""}
            </div>

            <div class="badge-row">
                ${renderTag(documentRecord.status, `is-status-${documentRecord.status}`)}
                ${renderTag(documentRecord.priority, `is-priority-${documentRecord.priority}`)}
                ${renderTag(documentRecord.kind, "")}
            </div>

            <p>${renderHighlightedText(documentRecord.summary || "No summary available.")}</p>

            <div class="document-meta">
                <span>${renderHighlightedText(documentRecord.reviewer || "Unassigned")} · ${escapeHtml(String(documentRecord.readingMinutes))} min read</span>
                <span>${renderHighlightedText(normalizeTags(documentRecord.tags).join(" · ") || "No tags")}</span>
            </div>

            <div class="document-actions">
                <button class="chip" type="button" data-action="open-reader" data-id="${documentRecord.id}">Open reader</button>
                <button class="chip ${documentRecord.id === state.selectedId ? "is-active" : ""}" type="button" data-action="select-document" data-id="${documentRecord.id}">Inspect</button>
            </div>
        </article>
    `;
}

function renderStatusFilterButtons() {
    return ["all", ...STATUS_ORDER].map((status) => `
        <button
            type="button"
            class="chip ${state.status === status ? "is-active" : ""}"
            data-action="filter-status"
            data-status="${status}"
        >${escapeHtml(status === "all" ? "All statuses" : status)}</button>
    `).join("");
}

function selectedDocument() {
    return state.documents.find((entry) => entry.id === state.selectedId) || null;
}

function documentById(id) {
    return state.documents.find((entry) => entry.id === id) || null;
}

function visibleDocuments() {
    return state.visibleDocuments;
}

function workspaceProfile() {
    const row = state.settingsByKey.get("workspaceProfile");
    return row?.value || {
        label: "No workspace selected",
        root: null,
        command: "RustFrame native indexer",
        fileCount: 0,
        lastIndexedAt: null
    };
}

function recentWorkspaces() {
    const rows = state.settingsByKey.get("recentWorkspaces")?.value;
    return Array.isArray(rows) ? rows : [];
}

function normalizeTags(value) {
    if (Array.isArray(value)) {
        return value.map((entry) => String(entry).trim()).filter(Boolean);
    }

    return [];
}

function stripFrontmatter(source) {
    if (!source.startsWith("---")) {
        return source;
    }

    const lines = source.split("\n");
    let closingIndex = -1;
    for (let index = 1; index < lines.length; index += 1) {
        if (lines[index].trim() === "---") {
            closingIndex = index;
            break;
        }
    }

    return closingIndex === -1 ? source : lines.slice(closingIndex + 1).join("\n").trim();
}

function renderRichText(source) {
    const body = String(source || "").trim();
    if (!body) {
        return `<p class="empty-copy">No source text is available yet.</p>`;
    }

    const lines = body.replace(/\r\n/g, "\n").split("\n");
    const html = [];
    let paragraph = [];
    let listOpen = false;

    function flushParagraph() {
        if (!paragraph.length) {
            return;
        }
        html.push(`<p>${escapeHtml(paragraph.join(" "))}</p>`);
        paragraph = [];
    }

    function closeList() {
        if (listOpen) {
            html.push("</ul>");
            listOpen = false;
        }
    }

    for (const line of lines) {
        const trimmed = line.trim();

        if (!trimmed) {
            flushParagraph();
            closeList();
            continue;
        }

        const heading = trimmed.match(/^(#{1,3})\s+(.*)$/);
        if (heading) {
            flushParagraph();
            closeList();
            const level = heading[1].length;
            html.push(`<h${level}>${escapeHtml(heading[2])}</h${level}>`);
            continue;
        }

        const bullet = trimmed.match(/^-\s+(.*)$/);
        if (bullet) {
            flushParagraph();
            if (!listOpen) {
                html.push("<ul>");
                listOpen = true;
            }
            html.push(`<li>${escapeHtml(bullet[1])}</li>`);
            continue;
        }

        closeList();
        paragraph.push(trimmed);
    }

    flushParagraph();
    closeList();

    return html.join("");
}

function renderTag(value, className) {
    return `<span class="tag ${className}">${escapeHtml(value)}</span>`;
}

function serializeCsv(header, rows) {
    const escapeCell = (value) => {
        const text = String(value ?? "");
        if (/[",\n]/u.test(text)) {
            return `"${text.replace(/"/gu, "\"\"")}"`;
        }
        return text;
    };

    const lines = [
        header.join(","),
        ...rows.map((row) => header.map((key) => escapeCell(row[key])).join(","))
    ];
    return `${lines.join("\n")}\n`;
}

function renderFatal() {
    APP.innerHTML = `
        <section class="panel empty-state">
            <div>
                <p class="eyebrow">Research Desk</p>
                <h2>Boot failed.</h2>
                <p>${escapeHtml(state.log)}</p>
            </div>
        </section>
    `;
}

function writeLog(message) {
    state.log = message;
    APP.querySelectorAll(".log-box").forEach((node) => {
        node.textContent = message;
    });
}

function updateIndexProgress() {
    const progress = state.indexProgress;
    if (!progress) return;
    const progressElement = APP.querySelector("#index-progress");
    const labelElement = APP.querySelector("#index-progress-label");
    const cancelButton = APP.querySelector('[data-action="cancel-index"]');
    if (progressElement) {
        progressElement.max = Math.max(1, progress.total);
        progressElement.value = progress.completed;
    }
    if (labelElement) {
        labelElement.textContent = state.indexCancelRequested
            ? "Finishing the current file before canceling…"
            : `${progress.completed} of ${progress.total} files · ${progress.skipped} unchanged`;
    }
    if (cancelButton) {
        cancelButton.disabled = state.indexCancelRequested;
        cancelButton.textContent = state.indexCancelRequested ? "Cancel requested" : "Cancel indexing";
    }
}

function redactUri(value) {
    const text = String(value || "");
    if (text.startsWith("grant://")) return "grant://<redacted>";
    if (/^(?:[A-Za-z]:\\|\/)/u.test(text)) return "<redacted-path>";
    return text;
}

function redactPrivateValue(value, key = "") {
    if (Array.isArray(value)) return value.map((entry) => redactPrivateValue(entry, key));
    if (value && typeof value === "object") {
        return Object.fromEntries(Object.entries(value).map(([entryKey, entryValue]) => [
            entryKey,
            redactPrivateValue(entryValue, entryKey)
        ]));
    }
    if (typeof value === "string") {
        if (/path|root|uri|grant.?id|data.?dir/iu.test(key)) return redactUri(value);
        return value
            .replace(/grant:\/\/[^\s"']+/gu, "grant://<redacted>")
            .replace(/(?:[A-Za-z]:\\|\/Users\/|\/home\/)[^\s"']+/gu, "<redacted-path>");
    }
    return value;
}

function formatDateTime(value) {
    if (!value) {
        return "Unavailable";
    }

    const parsed = new Date(value);
    if (Number.isNaN(parsed.getTime())) {
        return String(value);
    }

    return parsed.toLocaleString();
}

function formatBytes(value) {
    const size = Number(value) || 0;
    if (size >= 1024 * 1024) {
        return `${(size / (1024 * 1024)).toFixed(1)} MB`;
    }
    if (size >= 1024) {
        return `${Math.round(size / 1024)} KB`;
    }
    return `${size} B`;
}

function formatError(error) {
    if (error && typeof error === "object") {
        return `${error.code ?? "error"}: ${error.message ?? JSON.stringify(error)}`;
    }

    return String(error);
}

function renderHighlightedText(value) {
    const text = String(value ?? "");
    const terms = [...new Set((state.search.toLowerCase().match(/[\p{L}\p{N}_-]+/gu) || [])
        .filter((term) => term.length > 1))];
    if (!terms.length) return escapeHtml(text);
    const expression = new RegExp(`(${terms.map(escapeRegExp).join("|")})`, "giu");
    return text.split(expression).map((part) =>
        terms.includes(part.toLowerCase()) ? `<mark>${escapeHtml(part)}</mark>` : escapeHtml(part)
    ).join("");
}

function escapeRegExp(value) {
    return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function escapeHtml(value) {
    return String(value)
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;");
}
