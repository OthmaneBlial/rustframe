const APP = document.getElementById("app");
const ROUTE = window.RustFrame?.window?.route || "/";
const [ROUTE_PATH, ROUTE_QUERY = ""] = ROUTE.split("?");
const ROUTE_PARAMS = new URLSearchParams(ROUTE_QUERY);
const INDEX_COMMANDS = [
    { name: "indexWorkspacePy3", label: "python3" },
    { name: "indexWorkspacePython", label: "python" },
    { name: "indexWorkspacePyLauncher", label: "py -3" }
];
const STATUS_ORDER = ["queued", "reviewing", "ready", "archived"];
const PRIORITY_ORDER = ["critical", "watch", "reference"];
let latestSearchRequestId = 0;

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
    log: "Research Desk is booting."
};

document.body.dataset.mode = state.mode;
window.requestAnimationFrame(() => {
    document.body.classList.add("is-ready");
});

APP.addEventListener("click", handleClick);
APP.addEventListener("input", handleInput);

if (window.RustFrame?.events?.onFileDrop) {
    window.RustFrame.events.onFileDrop((payload) => {
        if (state.mode !== "main") {
            return;
        }
        void importExternalFiles(payload?.files || [], "drag and drop");
    });
}

boot().catch((error) => {
    state.log = `Research Desk failed to boot.\n${formatError(error)}`;
    renderFatal();
});

async function boot() {
    state.dbInfo = await window.RustFrame.db.info();
    await loadSettings();

    if (state.mode === "main") {
        await refreshDocuments();
        await loadWorkspaceEntries();
        if (!state.documents.length) {
            try {
                await indexWorkspace("first boot");
            } catch (error) {
                writeLog(
                    `Automatic indexing failed.\n` +
                    `${formatError(error)}\n\n` +
                    `Use "Index workspace" after installing one of the configured Python launchers.`
                );
            }
        }
        await refreshDocuments();
        selectDefaultDocument();
        await refreshSelectedContent();
        await refreshWindows();
        if (!state.log.startsWith("Automatic indexing failed")) {
            writeLog(
                `Bundled archive connected.\n` +
                `Database: ${state.dbInfo.databasePath}\n` +
                `Tables: ${state.dbInfo.tables.join(", ")}\n` +
                `Use "Index workspace" to re-scan the local archive.`
            );
        }
        renderMain();
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
    }
}

async function loadSettings() {
    const rows = await window.RustFrame.db.list("settings", {
        orderBy: [{ field: "key", direction: "asc" }]
    });
    state.settingsByKey = new Map(rows.map((row) => [row.key, row]));
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
    try {
        state.workspaceEntries = await window.RustFrame.fs.listDir(".");
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

    if (!state.search) {
        state.visibleDocuments = filterDocumentsLocally(filters);
        return;
    }

    const results = await window.RustFrame.db.search("documents", state.search, {
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
    state.importBusy = true;
    render();

    try {
        const result = await runIndexAutomation();
        const indexed = JSON.parse(result.stdout);
        await mergeIndexedDocuments(indexed);
        await saveSetting("workspaceProfile", {
            label: "Bundled Sample Archive",
            root: "workspace",
            command: result.label,
            fileCount: indexed.length,
            lastIndexedAt: new Date().toISOString()
        });

        await refreshDocuments();
        selectDefaultDocument();
        await refreshSelectedContent();
        await refreshWindows();
        await loadWorkspaceEntries();
        writeLog(
            `Indexed ${indexed.length} archive documents using ${result.label}.\n` +
            `Reason: ${reason}\n` +
            `Workspace root: workspace/`
        );
    } finally {
        state.importBusy = false;
        render();
    }
}

async function runIndexAutomation() {
    let lastError = null;

    for (const command of INDEX_COMMANDS) {
        try {
            const result = await window.RustFrame.shell.exec(command.name);
            if (result.exitCode === 0) {
                return { label: command.label, stdout: result.stdout };
            }

            lastError = new Error(
                result.stderr.trim() ||
                result.stdout.trim() ||
                `${command.label} exited with code ${result.exitCode}`
            );
        } catch (error) {
            lastError = error;
        }
    }

    throw lastError || new Error("No indexing command succeeded.");
}

async function mergeIndexedDocuments(indexedDocuments) {
    const existing = await window.RustFrame.db.list("documents");
    const existingByPath = new Map(existing.map((row) => [row.path, row]));

    for (const documentRecord of indexedDocuments) {
        const normalized = normalizeIndexedDocument(documentRecord);
        const current = existingByPath.get(normalized.path);

        if (current) {
            await window.RustFrame.db.update("documents", current.id, {
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
                sourceModifiedAt: normalized.sourceModifiedAt
            });
        } else {
            await window.RustFrame.db.insert("documents", {
                ...normalized,
                note: "",
                pinned: false
            });
        }
    }
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
        sourceModifiedAt: String(record.sourceModifiedAt || "").trim()
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
        for (let index = 0; index < supported.length; index += 1) {
            const fileEntry = supported[index];
            const destination = buildImportDestination(fileEntry, index);
            await window.RustFrame.fs.copyFrom(fileEntry.path, destination);
        }

        await indexWorkspace(`${sourceLabel} import`);
        writeLog(
            `Imported ${supported.length} files from ${sourceLabel} into workspace/imports.\n` +
            `${state.log}`
        );
    } finally {
        state.importBusy = false;
        render();
    }
}

function buildImportDestination(fileEntry, index) {
    const extension = normalizeExtension(fileEntry.extension) || "md";
    const stem = String(fileEntry.name || "imported-note")
        .replace(/\.[^.]+$/u, "")
        .toLowerCase()
        .replace(/[^a-z0-9]+/gu, "-")
        .replace(/^-+|-+$/gu, "")
        .slice(0, 48) || "imported-note";
    const timestamp = new Date().toISOString().replace(/[-:TZ.]/gu, "").slice(0, 14);
    return window.RustFrame.path.join("imports", `${timestamp}-${index + 1}-${stem}.${extension}`);
}

function normalizeExtension(value) {
    return String(value || "").trim().replace(/^\./u, "").toLowerCase();
}

async function handleInput(event) {
    if (event.target.id === "search-input") {
        try {
            state.search = event.target.value.trim();
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

        if (action === "import-files") {
            const files = await window.RustFrame.dialog.openFiles({
                title: "Import archive files",
                filters: [
                    { name: "Markdown and text", extensions: ["md", "txt"] }
                ]
            });
            await importExternalFiles(files, "file picker");
            return;
        }

        if (action === "export-json") {
            await exportVisibleDocumentsAsJson();
            return;
        }

        if (action === "export-csv") {
            await exportVisibleDocumentsAsCsv();
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
            await refreshVisibleDocuments();
            selectDefaultDocument();
            await refreshSelectedContent();
            render();
            return;
        }

        if (action === "filter-collection") {
            state.collection = button.dataset.collection || "all";
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
            const absolutePath = state.selectedFileMeta?.absolutePath || documentRecord.path;
            await window.RustFrame.clipboard.writeText(absolutePath);
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
            const absolutePath = state.readerFileMeta?.absolutePath || state.readerDocument.path;
            await window.RustFrame.clipboard.writeText(absolutePath);
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

    APP.innerHTML = `
        <section class="shell-frame masthead">
            <article class="headline">
                <p class="eyebrow">Research Desk</p>
                <h1>Review a local archive, store decisions in SQLite, and keep the source files close.</h1>
                <p class="section-copy">
                    This flagship app indexes a bundled research workspace, opens the real source files
                    through the filesystem bridge, supports drag-and-drop intake, and uses reader windows for focused review passes.
                </p>
                <div class="action-row">
                    <button class="button button-primary" type="button" data-action="index" ${state.importBusy ? "disabled" : ""}>
                        ${state.importBusy ? "Indexing archive…" : "Index workspace"}
                    </button>
                    <button class="button" type="button" data-action="import-files" ${state.importBusy ? "disabled" : ""}>Import files</button>
                    <button class="button" type="button" data-action="export-json">Export JSON</button>
                    <button class="button" type="button" data-action="export-csv">Export CSV</button>
                    <button class="ghost-button" type="button" data-action="sync-title">Sync title</button>
                    <button class="ghost-button" type="button" data-action="close-window">Close</button>
                </div>
            </article>

            <aside class="status-meta">
                <div class="status-grid">
                    <article class="status-card">
                        <p class="eyebrow">Workspace</p>
                        <strong>${escapeHtml(workspace.label)}</strong>
                        <p class="status-detail">Root: <code>${escapeHtml(workspace.root)}</code></p>
                    </article>
                    <article class="status-card">
                        <p class="eyebrow">Last import</p>
                        <strong>${escapeHtml(workspace.lastIndexedAt ? formatDateTime(workspace.lastIndexedAt) : "Not indexed yet")}</strong>
                        <p class="status-detail">Command: ${escapeHtml(workspace.command || "pending")}</p>
                    </article>
                    <article class="status-card">
                        <p class="eyebrow">Database</p>
                        <strong>${escapeHtml(state.dbInfo.databasePath)}</strong>
                        <p class="status-detail">Schema version ${escapeHtml(String(state.dbInfo.schemaVersion))}</p>
                    </article>
                    <article class="status-card">
                        <p class="eyebrow">Windows</p>
                        <strong>${escapeHtml(String(state.windows.length))} open</strong>
                        <p class="status-detail">${readerWindows} reader windows</p>
                    </article>
                </div>
            </aside>
        </section>

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

                <div class="section-divider"></div>

                <div class="meta-list">
                    <div class="meta-box">
                        <p class="label">Workflow proof</p>
                        <strong>Embedded SQLite + real source files</strong>
                        <p class="section-copy">The review state lives in SQLite. The source documents stay in the filesystem root and are read directly when selected.</p>
                    </div>
                    <div class="meta-box">
                        <p class="label">Shell automation</p>
                        <strong>Workspace indexing</strong>
                        <p class="section-copy">The import button runs an allowlisted local indexing command and merges the results into the database.</p>
                    </div>
                    <div class="meta-box">
                        <p class="label">Desktop intake</p>
                        <strong>File picker and drag drop</strong>
                        <p class="section-copy">Import local <code>.md</code> and <code>.txt</code> files straight into <code>workspace/imports</code> without leaving the app.</p>
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
                    <pre class="log-box">${escapeHtml(state.log)}</pre>
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
                <h2>${escapeHtml(documentRecord.title)}</h2>
                <p class="section-copy">${escapeHtml(documentRecord.summary || "No summary available.")}</p>
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
                    <h3>${escapeHtml(documentRecord.title)}</h3>
                </div>
                ${documentRecord.pinned ? renderTag("Pinned", "") : ""}
            </div>

            <div class="badge-row">
                ${renderTag(documentRecord.status, `is-status-${documentRecord.status}`)}
                ${renderTag(documentRecord.priority, `is-priority-${documentRecord.priority}`)}
                ${renderTag(documentRecord.kind, "")}
            </div>

            <p>${escapeHtml(documentRecord.summary || "No summary available.")}</p>

            <div class="document-meta">
                <span>${escapeHtml(documentRecord.reviewer || "Unassigned")} · ${escapeHtml(String(documentRecord.readingMinutes))} min read</span>
                <span>${escapeHtml(normalizeTags(documentRecord.tags).join(" · ") || "No tags")}</span>
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
        label: "Bundled Sample Archive",
        root: "workspace",
        command: "pending",
        fileCount: 0,
        lastIndexedAt: null
    };
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

function escapeHtml(value) {
    return String(value)
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;");
}
