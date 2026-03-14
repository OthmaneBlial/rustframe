const assetMode = document.getElementById("asset-mode");
const originLabel = document.getElementById("origin-label");
const titleInput = document.getElementById("title-input");
const renameButton = document.getElementById("rename-button");
const closeButton = document.getElementById("close-button");
const noteTitle = document.getElementById("note-title");
const noteBody = document.getElementById("note-body");
const saveNoteButton = document.getElementById("save-note-button");
const notesList = document.getElementById("notes-list");
const notesCount = document.getElementById("notes-count");
const dbStatus = document.getElementById("db-status");
const logOutput = document.getElementById("log-output");

assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
originLabel.textContent = window.location.origin || `${window.location.protocol}//`;

function log(message) {
    logOutput.textContent = message;
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

async function refreshNotes() {
    const notes = await window.RustFrame.db.list("notes", {
        orderBy: [
            { field: "pinned", direction: "desc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });

    notesCount.textContent = String(notes.length);
    notesList.innerHTML = notes.length
        ? notes.map((note) => `
            <article class="note-card">
                <h3>${escapeHtml(note.title)}</h3>
                <p>${escapeHtml(note.body || "No content yet.")}</p>
            </article>
        `).join("")
        : `<article class="note-card"><h3>No notes yet</h3><p>Create one with the form above.</p></article>`;

    return notes;
}

async function bootDatabaseDemo() {
    const info = await window.RustFrame.db.info();
    dbStatus.textContent = info.databasePath;

    const notes = await refreshNotes();
    log(
        `Database ready.\n` +
        `---------------\n` +
        `Path: ${info.databasePath}\n` +
        `Schema version: ${info.schemaVersion}\n` +
        `Tables: ${info.tables.join(", ")}\n` +
        `Rows in notes: ${notes.length}\n\n` +
        `Edit data/schema.json and data/seeds/*.json to change the app database.`
    );
}

renameButton.addEventListener("click", async () => {
    const nextTitle = titleInput.value.trim() || "Hello Rustframe";
    renameButton.disabled = true;

    try {
        await window.RustFrame.window.setTitle(nextTitle);
        log(`Window title updated to "${nextTitle}".\n\nThe database remains available through window.RustFrame.db.*`);
    } catch (error) {
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    } finally {
        renameButton.disabled = false;
    }
});

closeButton.addEventListener("click", async () => {
    closeButton.disabled = true;

    try {
        await window.RustFrame.window.close();
    } catch (error) {
        closeButton.disabled = false;
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    }
});

saveNoteButton.addEventListener("click", async () => {
    const title = noteTitle.value.trim();
    const body = noteBody.value.trim();

    if (!title) {
        log("Enter a note title before saving.");
        return;
    }

    saveNoteButton.disabled = true;

    try {
        const saved = await window.RustFrame.db.insert("notes", {
            title,
            body,
            pinned: false
        });

        noteTitle.value = "";
        noteBody.value = "";
        await refreshNotes();
        log(`Saved note #${saved.id} to the embedded SQLite database.`);
    } catch (error) {
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    } finally {
        saveNoteButton.disabled = false;
    }
});

bootDatabaseDemo().catch((error) => {
    dbStatus.textContent = "Unavailable";
    notesCount.textContent = "0";
    log(`RustFrame error\n---------------\n${formatError(error)}`);
});
