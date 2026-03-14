const state = {
    notes: [],
    search: "",
    filter: "all",
    draftMood: "idea",
    dbInfo: null
};

const elements = {
    noteCount: document.getElementById("note-count"),
    dbStatus: document.getElementById("db-status"),
    filterLabel: document.getElementById("filter-label"),
    noteTitle: document.getElementById("note-title"),
    noteBody: document.getElementById("note-body"),
    moodPicker: document.getElementById("mood-picker"),
    saveNote: document.getElementById("save-note"),
    syncTitle: document.getElementById("sync-title"),
    statusLog: document.getElementById("status-log"),
    searchInput: document.getElementById("search-input"),
    filterGroup: document.getElementById("filter-group"),
    notesGrid: document.getElementById("notes-grid")
};

boot().catch((error) => {
    writeStatus(`Daybreak Notes failed to boot.\n${formatError(error)}`);
});

async function boot() {
    bindEvents();
    state.dbInfo = await window.RustFrame.db.info();
    elements.dbStatus.textContent = state.dbInfo.databasePath;
    await refreshNotes();
    render();
    writeStatus(
        `Library connected.\n` +
        `Path: ${state.dbInfo.databasePath}\n` +
        `Schema version: ${state.dbInfo.schemaVersion}\n` +
        `Tables: ${state.dbInfo.tables.join(", ")}`
    );
}

function bindEvents() {
    elements.moodPicker.addEventListener("click", (event) => {
        const button = event.target.closest("[data-mood]");
        if (!button) {
            return;
        }

        state.draftMood = button.dataset.mood;
        renderMoodPicker();
    });

    elements.filterGroup.addEventListener("click", (event) => {
        const button = event.target.closest("[data-filter]");
        if (!button) {
            return;
        }

        state.filter = button.dataset.filter;
        render();
    });

    elements.searchInput.addEventListener("input", () => {
        state.search = elements.searchInput.value.trim().toLowerCase();
        render();
    });

    elements.saveNote.addEventListener("click", saveNote);
    elements.syncTitle.addEventListener("click", async () => {
        await runNative(async () => {
            await window.RustFrame.window.setTitle(`Daybreak Notes · ${visibleNotes().length} open cards`);
            writeStatus("Window title synced to the visible note count.");
        });
    });

    elements.notesGrid.addEventListener("click", async (event) => {
        const button = event.target.closest("[data-action]");
        if (!button) {
            return;
        }

        const id = Number(button.dataset.id);
        const note = state.notes.find((entry) => entry.id === id);
        if (!note) {
            return;
        }

        if (button.dataset.action === "pin") {
            await runNative(async () => {
                await window.RustFrame.db.update("notes", id, { pinned: !note.pinned });
                await refreshNotes();
                render();
                writeStatus(`${note.pinned ? "Unpinned" : "Pinned"} "${note.title}".`);
            });
        }

        if (button.dataset.action === "delete") {
            await runNative(async () => {
                await window.RustFrame.db.delete("notes", id);
                await refreshNotes();
                render();
                writeStatus(`Deleted "${note.title}".`);
            });
        }
    });
}

async function refreshNotes() {
    state.notes = await window.RustFrame.db.list("notes", {
        orderBy: [
            { field: "pinned", direction: "desc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });
}

async function saveNote() {
    const title = elements.noteTitle.value.trim();
    const body = elements.noteBody.value.trim();

    if (!title) {
        writeStatus("Give the note a title before saving it.");
        return;
    }

    elements.saveNote.disabled = true;

    try {
        const created = await window.RustFrame.db.insert("notes", {
            title,
            body,
            mood: state.draftMood,
            pinned: false
        });
        elements.noteTitle.value = "";
        elements.noteBody.value = "";
        state.draftMood = "idea";
        await refreshNotes();
        render();
        writeStatus(`Saved "${created.title}" to the local library.`);
    } catch (error) {
        writeStatus(formatError(error));
    } finally {
        elements.saveNote.disabled = false;
    }
}

function render() {
    const notes = visibleNotes();
    elements.noteCount.textContent = `${state.notes.length} ${state.notes.length === 1 ? "note" : "notes"}`;
    elements.filterLabel.textContent = state.filter === "all" ? "All moods" : capitalize(state.filter);
    renderMoodPicker();
    renderFilterGroup();
    renderNotes(notes);
}

function visibleNotes() {
    return state.notes.filter((note) => {
        const matchesFilter = state.filter === "all" || note.mood === state.filter;
        const haystack = `${note.title}\n${note.body}\n${note.mood}`.toLowerCase();
        const matchesSearch = !state.search || haystack.includes(state.search);
        return matchesFilter && matchesSearch;
    });
}

function renderMoodPicker() {
    elements.moodPicker.querySelectorAll("[data-mood]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.mood === state.draftMood);
    });
}

function renderFilterGroup() {
    elements.filterGroup.querySelectorAll("[data-filter]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.filter === state.filter);
    });
}

function renderNotes(notes) {
    if (!notes.length) {
        elements.notesGrid.innerHTML = `<div class="empty-state">No notes match the current search.</div>`;
        return;
    }

    elements.notesGrid.innerHTML = notes.map((note) => `
        <article class="note-card">
            <div class="note-topline">
                <div>
                    <p class="eyebrow">${escapeHtml(note.mood)}</p>
                    <h3>${escapeHtml(note.title)}</h3>
                </div>
                <span class="tag ${note.pinned ? "is-pinned" : ""}">${note.pinned ? "Pinned" : "Filed"}</span>
            </div>

            <p class="note-body">${escapeHtml(note.body || "No body text yet.")}</p>

            <div class="note-footer">
                <span class="tag">${new Date(note.updatedAt).toLocaleDateString()}</span>

                <div class="note-actions">
                    <button type="button" data-action="pin" data-id="${note.id}">
                        ${note.pinned ? "Unpin" : "Pin"}
                    </button>
                    <button type="button" data-action="delete" data-id="${note.id}">
                        Delete
                    </button>
                </div>
            </div>
        </article>
    `).join("");
}

function writeStatus(message) {
    elements.statusLog.textContent = message;
}

async function runNative(action) {
    try {
        await action();
    } catch (error) {
        writeStatus(formatError(error));
    }
}

function formatError(error) {
    if (error && typeof error === "object") {
        return `${error.code ?? "error"}: ${error.message ?? JSON.stringify(error)}`;
    }

    return String(error);
}

function capitalize(value) {
    return value.charAt(0).toUpperCase() + value.slice(1);
}

function escapeHtml(value) {
    return String(value)
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;");
}
