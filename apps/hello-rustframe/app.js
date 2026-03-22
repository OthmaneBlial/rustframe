const laneOrder = ["Inbox", "Review", "Ready", "Shipped"];

const assetMode = document.getElementById("asset-mode");
const originLabel = document.getElementById("origin-label");
const securityModel = document.getElementById("security-model");
const windowId = document.getElementById("window-id");
const searchInput = document.getElementById("search-input");
const allCount = document.getElementById("all-count");
const activeCount = document.getElementById("active-count");
const doneCount = document.getElementById("done-count");
const queueList = document.getElementById("queue-list");
const itemForm = document.getElementById("item-form");
const itemTitle = document.getElementById("item-title");
const itemOwner = document.getElementById("item-owner");
const itemLane = document.getElementById("item-lane");
const itemPriority = document.getElementById("item-priority");
const itemSummary = document.getElementById("item-summary");
const saveItemButton = document.getElementById("save-item-button");
const titleInput = document.getElementById("title-input");
const renameButton = document.getElementById("rename-button");
const selectedTitle = document.getElementById("selected-title");
const selectedMeta = document.getElementById("selected-meta");
const selectedSummary = document.getElementById("selected-summary");
const advanceButton = document.getElementById("advance-button");
const toggleDoneButton = document.getElementById("toggle-done-button");
const dbPath = document.getElementById("db-path");
const copyDbPathButton = document.getElementById("copy-db-path-button");
const logOutput = document.getElementById("log-output");

const state = {
    items: [],
    selectedId: null,
    dbInfo: null
};

assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
originLabel.textContent = window.location.origin || `${window.location.protocol}//`;
securityModel.textContent = window.RustFrame.security.model;
windowId.textContent = window.RustFrame.security.currentWindow.id;

function formatError(error) {
    if (error && typeof error === "object") {
        return `${error.code ?? "error"}: ${error.message ?? JSON.stringify(error)}`;
    }

    return String(error);
}

function escapeHtml(value) {
    return String(value ?? "")
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;");
}

function formatDate(value) {
    if (!value) {
        return "Unknown";
    }

    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
        return String(value);
    }

    return date.toLocaleString();
}

function selectedItem() {
    return state.items.find((item) => item.id === state.selectedId) ?? null;
}

function nextLane(lane) {
    const index = laneOrder.indexOf(lane);
    return laneOrder[(index + 1) % laneOrder.length] ?? "Inbox";
}

function log(message) {
    logOutput.textContent = message;
}

async function loadItems() {
    const term = searchInput.value.trim();
    const options = {
        orderBy: [
            { field: "done", direction: "asc" },
            { field: "pinned", direction: "desc" },
            { field: "updatedAt", direction: "desc" }
        ],
        limit: 40
    };

    state.items = term
        ? await window.RustFrame.db.search("work_items", term, options)
        : await window.RustFrame.db.list("work_items", options);

    if (!state.selectedId || !state.items.some((item) => item.id === state.selectedId)) {
        state.selectedId = state.items[0]?.id ?? null;
    }
}

function renderCounts() {
    allCount.textContent = String(state.items.length);
    activeCount.textContent = String(state.items.filter((item) => !item.done).length);
    doneCount.textContent = String(state.items.filter((item) => item.done).length);
}

function renderQueue() {
    if (!state.items.length) {
        queueList.innerHTML = `
            <article class="queue-empty">
                <h3>No matching items</h3>
                <p>Try a different search or save a new work item from the form.</p>
            </article>
        `;
        return;
    }

    queueList.innerHTML = state.items
        .map((item) => {
            const isSelected = item.id === state.selectedId;
            const statusClass = item.done ? "is-done" : "";
            return `
                <button class="queue-card ${isSelected ? "is-selected" : ""} ${statusClass}" type="button" data-select-id="${item.id}">
                    <div class="queue-card-top">
                        <span class="lane-pill">${escapeHtml(item.done ? "Done" : item.lane)}</span>
                        <span class="priority-pill priority-${escapeHtml(item.priority)}">${escapeHtml(item.priority)}</span>
                    </div>
                    <h3>${escapeHtml(item.title)}</h3>
                    <p>${escapeHtml(item.summary || "No summary yet.")}</p>
                    <footer>
                        <span>${escapeHtml(item.owner || "Unassigned")}</span>
                        <span>${escapeHtml(formatDate(item.updatedAt))}</span>
                    </footer>
                </button>
            `;
        })
        .join("");
}

function renderSelection() {
    const item = selectedItem();
    const hasItem = Boolean(item);

    advanceButton.disabled = !hasItem;
    toggleDoneButton.disabled = !hasItem;

    if (!item) {
        selectedTitle.textContent = "No item selected";
        selectedMeta.textContent = "Choose a row from the queue.";
        selectedSummary.textContent = "This panel is where you can add your own detail view, file preview, or action strip.";
        toggleDoneButton.textContent = "Mark done";
        return;
    }

    selectedTitle.textContent = item.title;
    selectedMeta.textContent = [
        `Lane: ${item.done ? "Done" : item.lane}`,
        `Owner: ${item.owner || "Unassigned"}`,
        `Priority: ${item.priority}`,
        `Updated: ${formatDate(item.updatedAt)}`
    ].join("  •  ");
    selectedSummary.textContent = item.summary || "No summary yet.";
    toggleDoneButton.textContent = item.done ? "Re-open item" : "Mark done";
}

function render() {
    renderCounts();
    renderQueue();
    renderSelection();
}

async function refresh(message) {
    await loadItems();
    render();
    if (message) {
        log(message);
    }
}

queueList.addEventListener("click", (event) => {
    const button = event.target.closest("[data-select-id]");
    if (!button) {
        return;
    }

    state.selectedId = Number(button.dataset.selectId);
    renderSelection();
});

searchInput.addEventListener("input", () => {
    refresh().catch((error) => {
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    });
});

itemForm.addEventListener("submit", async (event) => {
    event.preventDefault();

    const title = itemTitle.value.trim();
    if (!title) {
        log("Add a title before saving the work item.");
        return;
    }

    saveItemButton.disabled = true;

    try {
        const saved = await window.RustFrame.db.insert("work_items", {
            title,
            owner: itemOwner.value.trim(),
            lane: itemLane.value,
            priority: itemPriority.value,
            summary: itemSummary.value.trim(),
            pinned: itemLane.value === "Review",
            done: false
        });

        itemForm.reset();
        itemLane.value = "Inbox";
        itemPriority.value = "normal";
        state.selectedId = saved.id;
        await refresh(`Saved work item #${saved.id} to the local queue.`);
    } catch (error) {
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    } finally {
        saveItemButton.disabled = false;
    }
});

renameButton.addEventListener("click", async () => {
    const nextTitle = titleInput.value.trim() || "Hello Rustframe";
    renameButton.disabled = true;

    try {
        await window.RustFrame.window.setTitle(nextTitle);
        log(`Window title updated to "${nextTitle}".`);
    } catch (error) {
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    } finally {
        renameButton.disabled = false;
    }
});

copyDbPathButton.addEventListener("click", async () => {
    if (!state.dbInfo?.databasePath) {
        return;
    }

    copyDbPathButton.disabled = true;

    try {
        await window.RustFrame.clipboard.writeText(state.dbInfo.databasePath);
        log(`Copied database path:\n${state.dbInfo.databasePath}`);
    } catch (error) {
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    } finally {
        copyDbPathButton.disabled = false;
    }
});

advanceButton.addEventListener("click", async () => {
    const item = selectedItem();
    if (!item) {
        return;
    }

    advanceButton.disabled = true;

    try {
        const lane = nextLane(item.lane);
        await window.RustFrame.db.update("work_items", item.id, {
            lane,
            done: lane === "Shipped",
            pinned: lane === "Review"
        });
        await refresh(`Moved "${item.title}" to ${lane}.`);
    } catch (error) {
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    } finally {
        advanceButton.disabled = false;
    }
});

toggleDoneButton.addEventListener("click", async () => {
    const item = selectedItem();
    if (!item) {
        return;
    }

    toggleDoneButton.disabled = true;

    try {
        const nextDone = !item.done;
        await window.RustFrame.db.update("work_items", item.id, {
            done: nextDone,
            lane: nextDone ? "Shipped" : "Review",
            pinned: !nextDone
        });
        await refresh(
            nextDone
                ? `Marked "${item.title}" as done.`
                : `Re-opened "${item.title}" and moved it back to Review.`
        );
    } catch (error) {
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    } finally {
        toggleDoneButton.disabled = false;
    }
});

async function boot() {
    state.dbInfo = await window.RustFrame.db.info();
    dbPath.textContent = state.dbInfo.databasePath;
    titleInput.value = "Hello Rustframe";
    await refresh(
        `Workflow starter ready.\n` +
        `----------------------\n` +
        `Database: ${state.dbInfo.databasePath}\n` +
        `Schema version: ${state.dbInfo.schemaVersion}\n` +
        `Tables: ${state.dbInfo.tables.join(", ")}\n\n` +
        `Next steps:\n` +
        `1. Replace work_items with your own workflow table.\n` +
        `2. Add filesystem roots or shell commands in rustframe.json when the product needs them.\n` +
        `3. Add migrations before you ship non-additive schema changes.`
    );
}

boot().catch((error) => {
    dbPath.textContent = "Unavailable";
    render();
    log(`RustFrame error\n---------------\n${formatError(error)}`);
});
