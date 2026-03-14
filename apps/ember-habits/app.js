const state = {
    habits: [],
    draftZone: "body",
    filter: "all",
    search: "",
    dbInfo: null
};

const elements = {
    assetMode: document.getElementById("asset-mode"),
    originLabel: document.getElementById("origin-label"),
    dbPath: document.getElementById("db-path"),
    doneCount: document.getElementById("done-count"),
    averageStreak: document.getElementById("average-streak"),
    habitCount: document.getElementById("habit-count"),
    bestStreak: document.getElementById("best-streak"),
    rowCount: document.getElementById("row-count"),
    visibleCount: document.getElementById("visible-count"),
    nameInput: document.getElementById("name-input"),
    cadenceInput: document.getElementById("cadence-input"),
    targetInput: document.getElementById("target-input"),
    zoneGroup: document.getElementById("zone-group"),
    filterGroup: document.getElementById("filter-group"),
    searchInput: document.getElementById("search-input"),
    saveHabitButton: document.getElementById("save-habit-button"),
    syncTitleButton: document.getElementById("sync-title-button"),
    closeButton: document.getElementById("close-button"),
    zoneList: document.getElementById("zone-list"),
    habitsList: document.getElementById("habits-list"),
    logOutput: document.getElementById("log-output")
};

elements.assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
elements.originLabel.textContent = window.location.origin || `${window.location.protocol}//`;

boot().catch((error) => {
    writeLog(`Ember Habits failed to boot.\n${formatError(error)}`);
});

async function boot() {
    bindEvents();
    state.dbInfo = await window.RustFrame.db.info();
    elements.dbPath.textContent = state.dbInfo.databasePath;
    await refreshHabits();
    render();
    writeLog(
        `Ember Habits online.\n` +
        `Path: ${state.dbInfo.databasePath}\n` +
        `Schema version: ${state.dbInfo.schemaVersion}\n` +
        `Tables: ${state.dbInfo.tables.join(", ")}`
    );
}

function bindEvents() {
    bindChipGroup(elements.zoneGroup, "zone", (value) => {
        state.draftZone = value;
        renderZoneGroup();
    });
    bindChipGroup(elements.filterGroup, "filter", (value) => {
        state.filter = value;
        render();
    });

    elements.searchInput.addEventListener("input", () => {
        state.search = elements.searchInput.value.trim().toLowerCase();
        render();
    });

    elements.saveHabitButton.addEventListener("click", saveHabit);
    elements.syncTitleButton.addEventListener("click", async () => {
        await runNative(async () => {
            const doneToday = state.habits.filter((habit) => habit.completedToday).length;
            await window.RustFrame.window.setTitle(`Ember Habits · ${doneToday} done today`);
            writeLog("Window title synced to today's completed habits.");
        });
    });
    elements.closeButton.addEventListener("click", async () => {
        await runNative(async () => {
            await window.RustFrame.window.close();
        });
    });

    elements.habitsList.addEventListener("click", async (event) => {
        const button = event.target.closest("[data-action]");
        if (!button) {
            return;
        }

        const id = Number(button.dataset.id);
        const habit = state.habits.find((entry) => entry.id === id);
        if (!habit) {
            return;
        }

        if (button.dataset.action === "toggle") {
            await runNative(async () => {
                const completedToday = !habit.completedToday;
                const streak = Math.max(0, habit.streak + (completedToday ? 1 : -1));
                await window.RustFrame.db.update("habits", id, { completedToday, streak });
                await refreshHabits();
                render();
                writeLog(`${completedToday ? "Completed" : "Unchecked"} ${habit.name}.`);
            });
        }

        if (button.dataset.action === "reset") {
            await runNative(async () => {
                await window.RustFrame.db.update("habits", id, { streak: 0, completedToday: false });
                await refreshHabits();
                render();
                writeLog(`Reset ${habit.name}.`);
            });
        }

        if (button.dataset.action === "delete") {
            await runNative(async () => {
                await window.RustFrame.db.delete("habits", id);
                await refreshHabits();
                render();
                writeLog(`Deleted ${habit.name}.`);
            });
        }
    });
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

async function refreshHabits() {
    state.habits = await window.RustFrame.db.list("habits", {
        orderBy: [
            { field: "completedToday", direction: "desc" },
            { field: "streak", direction: "desc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });
}

async function saveHabit() {
    const name = elements.nameInput.value.trim();
    const cadence = elements.cadenceInput.value.trim();
    const target = Number(elements.targetInput.value);

    if (!name || !cadence) {
        writeLog("Habit name and cadence are required.");
        return;
    }
    if (!Number.isInteger(target) || target <= 0) {
        writeLog("Target streak must be a whole number greater than zero.");
        return;
    }

    elements.saveHabitButton.disabled = true;

    try {
        const created = await window.RustFrame.db.insert("habits", {
            name,
            cadence,
            target,
            zone: state.draftZone,
            streak: 0,
            completedToday: false
        });
        resetComposer();
        await refreshHabits();
        render();
        writeLog(`Saved habit "${created.name}".`);
    } catch (error) {
        writeLog(formatError(error));
    } finally {
        elements.saveHabitButton.disabled = false;
    }
}

function render() {
    const visible = visibleHabits();
    const doneToday = state.habits.filter((habit) => habit.completedToday);
    const totalStreak = state.habits.reduce((sum, habit) => sum + habit.streak, 0);
    const bestStreak = state.habits.reduce((best, habit) => Math.max(best, habit.streak), 0);

    elements.doneCount.textContent = String(doneToday.length);
    elements.averageStreak.textContent = state.habits.length ? (totalStreak / state.habits.length).toFixed(1) : "0";
    elements.habitCount.textContent = String(state.habits.length);
    elements.bestStreak.textContent = String(bestStreak);
    elements.rowCount.textContent = `${state.habits.length} ${state.habits.length === 1 ? "row" : "rows"}`;
    elements.visibleCount.textContent = `${visible.length} visible`;

    renderZoneGroup();
    renderFilterGroup();
    renderZones(visible);
    renderHabits(visible);
}

function visibleHabits() {
    return state.habits.filter((habit) => {
        const matchesFilter = state.filter === "all" || habit.zone === state.filter;
        const haystack = `${habit.name}\n${habit.cadence}\n${habit.zone}`.toLowerCase();
        const matchesSearch = !state.search || haystack.includes(state.search);
        return matchesFilter && matchesSearch;
    });
}

function renderZoneGroup() {
    toggleGroup(elements.zoneGroup, "zone", state.draftZone);
}

function renderFilterGroup() {
    toggleGroup(elements.filterGroup, "filter", state.filter);
}

function toggleGroup(container, key, expected) {
    container.querySelectorAll(`[data-${key}]`).forEach((button) => {
        button.classList.toggle("is-active", button.dataset[key] === expected);
    });
}

function renderZones(habits) {
    if (!habits.length) {
        elements.zoneList.innerHTML = `<div class="empty-state">No zone energy to show.</div>`;
        return;
    }

    const zones = new Map();
    for (const habit of habits) {
        const current = zones.get(habit.zone) || { count: 0, streak: 0 };
        current.count += 1;
        current.streak += habit.streak;
        zones.set(habit.zone, current);
    }

    elements.zoneList.innerHTML = [...zones.entries()]
        .sort((left, right) => right[1].streak - left[1].streak)
        .map(([zone, info]) => `
            <div class="zone-row">
                <strong>${escapeHtml(zone)}</strong>
                <p>${info.count} habit${info.count === 1 ? "" : "s"} · ${info.streak} streak points</p>
            </div>
        `)
        .join("");
}

function renderHabits(habits) {
    if (!habits.length) {
        elements.habitsList.innerHTML = `<div class="empty-state">No habits match the current filter.</div>`;
        return;
    }

    elements.habitsList.innerHTML = habits.map((habit) => {
        const progress = Math.min(100, (habit.streak / habit.target) * 100);
        return `
            <article class="habit-card">
                <div class="habit-top">
                    <div>
                        <h4>${escapeHtml(habit.name)}</h4>
                        <p class="habit-meta">${escapeHtml(habit.cadence)} · ${escapeHtml(habit.zone)}</p>
                    </div>
                    <strong>${habit.streak}/${habit.target}</strong>
                </div>

                <div class="progress"><div class="progress-bar" style="width:${progress}%"></div></div>

                <div class="tags">
                    <span class="tag ${habit.completedToday ? "is-done" : ""}">
                        ${habit.completedToday ? "Done today" : "Not done yet"}
                    </span>
                    <span class="tag">Target ${habit.target}</span>
                </div>

                <div class="habit-footer">
                    <span class="habit-note">Updated ${new Date(habit.updatedAt).toLocaleString()}</span>
                    <div class="habit-actions">
                        <button type="button" data-action="toggle" data-id="${habit.id}">
                            ${habit.completedToday ? "Undo today" : "Mark done"}
                        </button>
                        <button type="button" data-action="reset" data-id="${habit.id}">Reset</button>
                        <button type="button" data-action="delete" data-id="${habit.id}">Delete</button>
                    </div>
                </div>
            </article>
        `;
    }).join("");
}

function resetComposer() {
    elements.nameInput.value = "";
    elements.cadenceInput.value = "";
    elements.targetInput.value = "";
    state.draftZone = "body";
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
