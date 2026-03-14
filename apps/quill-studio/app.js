const STATUSES = ["pitch", "drafting", "editing", "scheduled"];

const state = {
    stories: [],
    draftChannel: "web",
    draftStatus: "pitch",
    draftPriority: 2,
    filter: "all",
    search: "",
    dbInfo: null
};

const elements = {
    assetMode: document.getElementById("asset-mode"),
    originLabel: document.getElementById("origin-label"),
    dbPath: document.getElementById("db-path"),
    openCount: document.getElementById("open-count"),
    todayCount: document.getElementById("today-count"),
    scheduledCount: document.getElementById("scheduled-count"),
    priorityCount: document.getElementById("priority-count"),
    storyCount: document.getElementById("story-count"),
    visibleCount: document.getElementById("visible-count"),
    titleInput: document.getElementById("title-input"),
    deskInput: document.getElementById("desk-input"),
    editorInput: document.getElementById("editor-input"),
    deadlineInput: document.getElementById("deadline-input"),
    angleInput: document.getElementById("angle-input"),
    channelGroup: document.getElementById("channel-group"),
    statusGroup: document.getElementById("status-group"),
    priorityGroup: document.getElementById("priority-group"),
    filterGroup: document.getElementById("filter-group"),
    searchInput: document.getElementById("search-input"),
    saveStoryButton: document.getElementById("save-story-button"),
    syncTitleButton: document.getElementById("sync-title-button"),
    closeButton: document.getElementById("close-button"),
    logOutput: document.getElementById("log-output"),
    lanes: {
        pitch: document.getElementById("lane-pitch"),
        drafting: document.getElementById("lane-drafting"),
        editing: document.getElementById("lane-editing"),
        scheduled: document.getElementById("lane-scheduled")
    }
};

elements.assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
elements.originLabel.textContent = window.location.origin || `${window.location.protocol}//`;
elements.deadlineInput.value = todayString();

boot().catch((error) => {
    writeLog(`Quill Studio failed to boot.\n${formatError(error)}`);
});

async function boot() {
    bindEvents();
    state.dbInfo = await window.RustFrame.db.info();
    elements.dbPath.textContent = state.dbInfo.databasePath;
    await refreshStories();
    render();
    writeLog(
        `Quill Studio online.\n` +
        `Path: ${state.dbInfo.databasePath}\n` +
        `Schema version: ${state.dbInfo.schemaVersion}\n` +
        `Tables: ${state.dbInfo.tables.join(", ")}`
    );
}

function bindEvents() {
    bindChipGroup(elements.channelGroup, "channel", (value) => {
        state.draftChannel = value;
        renderChannelGroup();
    });

    bindChipGroup(elements.statusGroup, "status", (value) => {
        state.draftStatus = value;
        renderStatusGroup();
    });

    bindChipGroup(elements.priorityGroup, "priority", (value) => {
        state.draftPriority = Number(value);
        renderPriorityGroup();
    });

    bindChipGroup(elements.filterGroup, "filter", (value) => {
        state.filter = value;
        render();
    });

    elements.searchInput.addEventListener("input", () => {
        state.search = elements.searchInput.value.trim().toLowerCase();
        render();
    });

    elements.saveStoryButton.addEventListener("click", saveStory);
    elements.syncTitleButton.addEventListener("click", async () => {
        await runNative(async () => {
            const scheduled = state.stories.filter((story) => story.status === "scheduled").length;
            await window.RustFrame.window.setTitle(`Quill Studio · ${scheduled} scheduled`);
            writeLog("Window title synced to scheduled story count.");
        });
    });
    elements.closeButton.addEventListener("click", async () => {
        await runNative(async () => {
            await window.RustFrame.window.close();
        });
    });

    for (const lane of Object.values(elements.lanes)) {
        lane.addEventListener("click", async (event) => {
            const button = event.target.closest("[data-action]");
            if (!button) {
                return;
            }

            const id = Number(button.dataset.id);
            const story = state.stories.find((entry) => entry.id === id);
            if (!story) {
                return;
            }

            if (button.dataset.action === "advance") {
                await runNative(async () => {
                    const nextStatus = nextStatusFor(story.status);
                    await window.RustFrame.db.update("stories", id, { status: nextStatus });
                    await refreshStories();
                    render();
                    writeLog(`Moved "${story.title}" to ${nextStatus}.`);
                });
            }

            if (button.dataset.action === "schedule") {
                await runNative(async () => {
                    const nextStatus = story.status === "scheduled" ? "editing" : "scheduled";
                    await window.RustFrame.db.update("stories", id, { status: nextStatus });
                    await refreshStories();
                    render();
                    writeLog(`${nextStatus === "scheduled" ? "Scheduled" : "Reopened"} "${story.title}".`);
                });
            }

            if (button.dataset.action === "delete") {
                await runNative(async () => {
                    await window.RustFrame.db.delete("stories", id);
                    await refreshStories();
                    render();
                    writeLog(`Deleted "${story.title}".`);
                });
            }
        });
    }
}

function bindChipGroup(container, key, handler) {
    container.addEventListener("click", (event) => {
        const button = event.target.closest(`[data-${key}]`);
        if (!button) {
            return;
        }
        handler(button.dataset[key]);
    });
}

async function refreshStories() {
    state.stories = await window.RustFrame.db.list("stories", {
        orderBy: [
            { field: "priority", direction: "desc" },
            { field: "deadline", direction: "asc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });
}

async function saveStory() {
    const title = elements.titleInput.value.trim();
    const desk = elements.deskInput.value.trim();
    const editor = elements.editorInput.value.trim();
    const deadline = normalizeDate(elements.deadlineInput.value.trim());
    const angle = elements.angleInput.value.trim();

    if (!title || !desk || !editor || !angle) {
        writeLog("Title, desk, editor, and angle are required.");
        return;
    }

    if (!deadline) {
        writeLog("Deadline must use YYYY-MM-DD.");
        return;
    }

    elements.saveStoryButton.disabled = true;

    try {
        const created = await window.RustFrame.db.insert("stories", {
            title,
            desk,
            editor,
            deadline,
            angle,
            channel: state.draftChannel,
            status: state.draftStatus,
            priority: state.draftPriority
        });
        resetComposer();
        await refreshStories();
        render();
        writeLog(`Filed "${created.title}" for the ${created.channel} desk.`);
    } catch (error) {
        writeLog(formatError(error));
    } finally {
        elements.saveStoryButton.disabled = false;
    }
}

function render() {
    const visible = visibleStories();
    const openStories = state.stories.filter((story) => story.status !== "scheduled");
    const dueToday = state.stories.filter((story) => story.deadline === todayString());
    const scheduled = state.stories.filter((story) => story.status === "scheduled");
    const priorityDesk = state.stories.filter((story) => story.priority >= 3);

    elements.openCount.textContent = String(openStories.length);
    elements.todayCount.textContent = String(dueToday.length);
    elements.scheduledCount.textContent = String(scheduled.length);
    elements.priorityCount.textContent = String(priorityDesk.length);
    elements.storyCount.textContent = `${state.stories.length} ${state.stories.length === 1 ? "row" : "rows"}`;
    elements.visibleCount.textContent = `${visible.length} visible`;

    renderChannelGroup();
    renderStatusGroup();
    renderPriorityGroup();
    renderFilterGroup();

    for (const status of STATUSES) {
        renderLane(status, visible.filter((story) => story.status === status));
    }
}

function visibleStories() {
    return state.stories.filter((story) => {
        const matchesFilter = state.filter === "all" || story.channel === state.filter;
        const haystack = `${story.title}\n${story.desk}\n${story.editor}\n${story.angle}`.toLowerCase();
        const matchesSearch = !state.search || haystack.includes(state.search);
        return matchesFilter && matchesSearch;
    });
}

function renderChannelGroup() {
    toggleGroup(elements.channelGroup, "channel", state.draftChannel);
}

function renderStatusGroup() {
    toggleGroup(elements.statusGroup, "status", state.draftStatus);
}

function renderPriorityGroup() {
    toggleGroup(elements.priorityGroup, "priority", String(state.draftPriority));
}

function renderFilterGroup() {
    toggleGroup(elements.filterGroup, "filter", state.filter);
}

function toggleGroup(container, key, expected) {
    container.querySelectorAll(`[data-${key}]`).forEach((button) => {
        button.classList.toggle("is-active", button.dataset[key] === expected);
    });
}

function renderLane(status, stories) {
    const container = elements.lanes[status];
    if (!stories.length) {
        container.innerHTML = `<div class="empty-state">No stories in ${status}.</div>`;
        return;
    }

    container.innerHTML = stories.map((story) => `
        <article class="story-card">
            <div class="story-top">
                <div>
                    <h4>${escapeHtml(story.title)}</h4>
                    <p class="story-note">${escapeHtml(story.angle)}</p>
                </div>
                <strong>P${story.priority}</strong>
            </div>

            <div class="story-tags">
                <span class="tag">${escapeHtml(story.channel)}</span>
                <span class="tag">${escapeHtml(story.desk)}</span>
                <span class="tag">${escapeHtml(story.editor)}</span>
                <span class="tag">Due ${escapeHtml(story.deadline)}</span>
            </div>

            <div class="story-footer">
                <span class="story-meta">Updated ${new Date(story.updatedAt).toLocaleString()}</span>
                <div class="story-actions">
                    <button type="button" data-action="advance" data-id="${story.id}">
                        ${status === "scheduled" ? "Back to pitch" : `Move to ${nextStatusFor(status)}`}
                    </button>
                    <button type="button" data-action="schedule" data-id="${story.id}">
                        ${status === "scheduled" ? "Reopen" : "Schedule"}
                    </button>
                    <button type="button" data-action="delete" data-id="${story.id}">Delete</button>
                </div>
            </div>
        </article>
    `).join("");
}

function nextStatusFor(status) {
    const index = STATUSES.indexOf(status);
    if (index === -1 || index === STATUSES.length - 1) {
        return "pitch";
    }
    return STATUSES[index + 1];
}

function resetComposer() {
    elements.titleInput.value = "";
    elements.deskInput.value = "";
    elements.editorInput.value = "";
    elements.deadlineInput.value = todayString();
    elements.angleInput.value = "";
    state.draftChannel = "web";
    state.draftStatus = "pitch";
    state.draftPriority = 2;
}

function normalizeDate(value) {
    return /^\d{4}-\d{2}-\d{2}$/.test(value) ? value : "";
}

function todayString() {
    return new Date().toISOString().slice(0, 10);
}

function writeLog(message) {
    elements.logOutput.textContent = message;
}

async function runNative(action) {
    try {
        await action();
    } catch (error) {
        writeLog(formatError(error));
    }
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
