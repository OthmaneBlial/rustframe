const STORAGE_KEY = "orbit-desk:v3";
const DEFAULT_FOCUS_MINUTES = 25;
const PRIORITIES = ["critical", "high", "medium", "low"];
const TODAY = startOfDay(new Date());
const TODAY_KEY = toDateKey(TODAY);

const state = loadState();
const ui = {
    draftPriority: "high",
    draftDue: "",
    calendarOpen: false,
    calendarCursor: startOfMonth(new Date())
};

let timerHandle = null;
let lastWindowTitle = "";

const elements = {
    assetMode: document.getElementById("asset-mode"),
    originLabel: document.getElementById("origin-label"),
    todayLabel: document.getElementById("today-label"),
    taskForm: document.getElementById("task-form"),
    taskTitle: document.getElementById("task-title"),
    taskProject: document.getElementById("task-project"),
    priorityPicker: document.getElementById("priority-picker"),
    dueField: document.getElementById("due-field"),
    clearDue: document.getElementById("clear-due"),
    taskDueTrigger: document.getElementById("task-due-trigger"),
    taskDueLabel: document.getElementById("task-due-label"),
    taskDueMeta: document.getElementById("task-due-meta"),
    taskDuePopover: document.getElementById("task-due-popover"),
    calendarMonthLabel: document.getElementById("calendar-month-label"),
    calendarGrid: document.getElementById("calendar-grid"),
    taskList: document.getElementById("task-list"),
    focusList: document.getElementById("focus-list"),
    searchInput: document.getElementById("search-input"),
    filterGroup: document.getElementById("filter-group"),
    metricOpen: document.getElementById("metric-open"),
    metricDone: document.getElementById("metric-done"),
    metricFocus: document.getElementById("metric-focus"),
    metricCritical: document.getElementById("metric-critical"),
    timerDisplay: document.getElementById("timer-display"),
    timerCaption: document.getElementById("timer-caption"),
    timerToggle: document.getElementById("timer-toggle"),
    timerReset: document.getElementById("timer-reset"),
    presetRow: document.getElementById("preset-row"),
    notesInput: document.getElementById("notes-input"),
    activityLog: document.getElementById("activity-log"),
    windowRename: document.getElementById("window-rename"),
    windowMinimize: document.getElementById("window-minimize"),
    windowClose: document.getElementById("window-close")
};

elements.assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
elements.originLabel.textContent = window.location.origin || `${window.location.protocol}//`;
elements.todayLabel.textContent = formatDateLabel(new Date());
elements.searchInput.value = state.search;
elements.notesInput.value = state.notes;

render();
bindEvents();
syncTimerLoop();
syncWindowTitle();
window.requestAnimationFrame(() => {
    document.body.classList.add("is-ready");
});

function loadState() {
    const fallback = createInitialState();

    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (!raw) {
            return fallback;
        }

        const parsed = JSON.parse(raw);
        return {
            ...fallback,
            ...parsed,
            tasks: Array.isArray(parsed.tasks)
                ? parsed.tasks.map((task, index) => sanitizeTask(task, index + 1))
                : fallback.tasks,
            notes: typeof parsed.notes === "string" ? parsed.notes : fallback.notes,
            filter: parsed.filter || fallback.filter,
            search: parsed.search || "",
            activity: Array.isArray(parsed.activity) ? parsed.activity : fallback.activity,
            timer: {
                ...fallback.timer,
                ...(parsed.timer || {})
            }
        };
    } catch {
        return fallback;
    }
}

function createInitialState() {
    return {
        lastId: 4,
        search: "",
        filter: "all",
        notes: "Topline:\n- Stabilize the launch narrative\n- Tighten the open criticals before end of day\n- Protect at least two deep-work blocks\n",
        activity: [
            "Orbit Desk booted.",
            "State restored from local storage."
        ],
        timer: {
            durationMinutes: DEFAULT_FOCUS_MINUTES,
            remainingSeconds: DEFAULT_FOCUS_MINUTES * 60,
            running: false,
            sessionsCompleted: 0
        },
        tasks: [
            createTask("Finalize launch checklist", "Launch", "critical", TODAY_KEY, false, 1),
            createTask("Review hero copy with design", "Brand", "high", TODAY_KEY, false, 2),
            createTask("Clear inbox triage", "Admin", "medium", "", true, 3),
            createTask("Draft customer follow-up", "Sales", "high", nextDayKey(), false, 4)
        ]
    };
}

function sanitizeTask(task, fallbackId) {
    const priority = PRIORITIES.includes(task.priority) ? task.priority : "high";
    return {
        id: Number(task.id) || fallbackId,
        title: typeof task.title === "string" ? task.title : "Untitled task",
        project: typeof task.project === "string" ? task.project : "General",
        priority,
        due: typeof task.due === "string" ? task.due : "",
        done: Boolean(task.done),
        createdAt: typeof task.createdAt === "string" ? task.createdAt : new Date().toISOString(),
        completedAt: typeof task.completedAt === "string" ? task.completedAt : null
    };
}

function createTask(title, project, priority, due, done, id) {
    return {
        id,
        title,
        project,
        priority,
        due,
        done,
        createdAt: new Date().toISOString(),
        completedAt: done ? new Date().toISOString() : null
    };
}

function bindEvents() {
    elements.taskForm.addEventListener("submit", handleTaskSubmit);

    elements.priorityPicker.addEventListener("click", (event) => {
        const button = event.target.closest("[data-priority]");
        if (!button) {
            return;
        }

        ui.draftPriority = button.dataset.priority;
        renderComposer();
    });

    elements.taskDueTrigger.addEventListener("click", () => {
        ui.calendarOpen = !ui.calendarOpen;
        ui.calendarCursor = startOfMonth(ui.draftDue ? parseDateKey(ui.draftDue) : new Date());
        renderComposer();
    });

    elements.clearDue.addEventListener("click", () => {
        ui.draftDue = "";
        ui.calendarOpen = false;
        renderComposer();
    });

    elements.taskDuePopover.addEventListener("click", (event) => {
        const navButton = event.target.closest("[data-calendar-nav]");
        if (navButton) {
            const direction = Number(navButton.dataset.calendarNav);
            ui.calendarCursor = addMonths(ui.calendarCursor, direction);
            renderComposer();
            return;
        }

        const dayButton = event.target.closest("[data-date]");
        if (!dayButton) {
            return;
        }

        ui.draftDue = dayButton.dataset.date;
        ui.calendarCursor = startOfMonth(parseDateKey(ui.draftDue));
        ui.calendarOpen = false;
        renderComposer();
    });

    document.addEventListener("click", (event) => {
        if (!ui.calendarOpen || elements.dueField.contains(event.target)) {
            return;
        }

        ui.calendarOpen = false;
        renderComposer();
    });

    document.addEventListener("keydown", (event) => {
        if (event.key !== "Escape" || !ui.calendarOpen) {
            return;
        }

        ui.calendarOpen = false;
        renderComposer();
    });

    elements.searchInput.addEventListener("input", () => {
        state.search = elements.searchInput.value.trim();
        persistAndRender();
    });

    elements.filterGroup.addEventListener("click", (event) => {
        const button = event.target.closest("[data-filter]");
        if (!button) {
            return;
        }

        state.filter = button.dataset.filter;
        persistAndRender();
    });

    elements.taskList.addEventListener("click", handleTaskAction);
    elements.focusList.addEventListener("click", handleTaskAction);

    elements.notesInput.addEventListener("input", () => {
        state.notes = elements.notesInput.value;
        persistAndRender();
    });

    elements.timerToggle.addEventListener("click", handleTimerToggle);
    elements.timerReset.addEventListener("click", resetTimer);

    elements.presetRow.addEventListener("click", (event) => {
        const button = event.target.closest("[data-minutes]");
        if (!button) {
            return;
        }

        const minutes = Number(button.dataset.minutes);
        if (!minutes) {
            return;
        }

        state.timer.durationMinutes = minutes;
        state.timer.remainingSeconds = minutes * 60;
        state.timer.running = false;
        logActivity(`Focus timer set to ${minutes} minutes.`);
        syncTimerLoop();
        persistAndRender();
    });

    elements.windowRename.addEventListener("click", async () => {
        await runNativeAction(async () => {
            await window.RustFrame.window.setTitle(currentWindowTitle());
            logActivity("Window title synced.");
        });
    });

    elements.windowMinimize.addEventListener("click", async () => {
        await runNativeAction(async () => {
            await window.RustFrame.window.minimize();
            logActivity("Window minimized.");
        });
    });

    elements.windowClose.addEventListener("click", async () => {
        elements.windowClose.disabled = true;
        try {
            await window.RustFrame.window.close();
        } catch (error) {
            elements.windowClose.disabled = false;
            logActivity(`Close failed: ${formatError(error)}`);
            render();
        }
    });
}

function handleTaskSubmit(event) {
    event.preventDefault();

    const title = elements.taskTitle.value.trim();
    if (!title) {
        return;
    }

    const project = elements.taskProject.value.trim() || "General";

    state.lastId += 1;
    state.tasks.unshift(
        createTask(title, project, ui.draftPriority, ui.draftDue, false, state.lastId)
    );
    logActivity(`Task added: ${title}`);

    elements.taskForm.reset();
    ui.draftPriority = "high";
    ui.draftDue = "";
    ui.calendarOpen = false;
    elements.taskTitle.focus();
    persistAndRender();
}

function handleTaskAction(event) {
    const button = event.target.closest("[data-action]");
    if (!button) {
        return;
    }

    const taskId = Number(button.dataset.id);
    const task = state.tasks.find((entry) => entry.id === taskId);
    if (!task) {
        return;
    }

    if (button.dataset.action === "toggle") {
        task.done = !task.done;
        task.completedAt = task.done ? new Date().toISOString() : null;
        logActivity(task.done ? `Completed: ${task.title}` : `Reopened: ${task.title}`);
    }

    if (button.dataset.action === "promote") {
        task.priority = promotePriority(task.priority);
        logActivity(`Priority changed: ${task.title} -> ${task.priority}`);
    }

    if (button.dataset.action === "delete") {
        state.tasks = state.tasks.filter((entry) => entry.id !== taskId);
        logActivity(`Deleted: ${task.title}`);
    }

    persistAndRender();
}

function handleTimerToggle() {
    state.timer.running = !state.timer.running;
    logActivity(state.timer.running ? "Focus timer started." : "Focus timer paused.");
    syncTimerLoop();
    persistAndRender();
}

function resetTimer() {
    state.timer.running = false;
    state.timer.remainingSeconds = state.timer.durationMinutes * 60;
    logActivity("Focus timer reset.");
    syncTimerLoop();
    persistAndRender();
}

function syncTimerLoop() {
    if (timerHandle) {
        clearInterval(timerHandle);
        timerHandle = null;
    }

    if (!state.timer.running) {
        return;
    }

    timerHandle = window.setInterval(() => {
        state.timer.remainingSeconds = Math.max(0, state.timer.remainingSeconds - 1);

        if (state.timer.remainingSeconds === 0) {
            state.timer.running = false;
            state.timer.sessionsCompleted += 1;
            logActivity("Focus block complete.");
            state.timer.remainingSeconds = state.timer.durationMinutes * 60;
            syncTimerLoop();
        }

        persistAndRender(false);
    }, 1000);
}

function persistAndRender(shouldSave = true) {
    if (shouldSave) {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    }

    render();
}

function render() {
    const filteredTasks = getFilteredTasks();
    const sortedTasks = [...filteredTasks].sort(taskSort);
    const openTasks = state.tasks.filter((task) => !task.done);
    const doneToday = state.tasks.filter(
        (task) => task.done && task.completedAt && toDateKey(new Date(task.completedAt)) === TODAY_KEY
    );
    const criticalQueue = openTasks.filter((task) => task.priority === "critical");

    elements.metricOpen.textContent = String(openTasks.length);
    elements.metricDone.textContent = String(doneToday.length);
    elements.metricFocus.textContent = String(state.timer.sessionsCompleted);
    elements.metricCritical.textContent = String(criticalQueue.length);
    elements.timerDisplay.textContent = formatTime(state.timer.remainingSeconds);
    elements.timerCaption.textContent = state.timer.running
        ? `Stay with the current block. ${state.timer.durationMinutes}-minute session in progress.`
        : `Ready for a ${state.timer.durationMinutes}-minute focus block.`;
    elements.timerToggle.textContent = state.timer.running ? "Pause" : "Start";
    if (document.activeElement !== elements.notesInput) {
        elements.notesInput.value = state.notes;
    }
    elements.activityLog.textContent = state.activity.slice(0, 10).join("\n");

    renderComposer();
    renderFilterState();
    renderPresetState();
    renderTaskList(
        elements.focusList,
        sortedTasks.filter((task) => !task.done).slice(0, 3),
        "No active tasks in the focus queue."
    );
    renderTaskList(
        elements.taskList,
        sortedTasks,
        "No tasks match the current filter."
    );
    syncWindowTitle();
}

function renderComposer() {
    elements.priorityPicker.querySelectorAll("[data-priority]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.priority === ui.draftPriority);
    });

    elements.clearDue.disabled = !ui.draftDue;
    elements.taskDueTrigger.setAttribute("aria-expanded", String(ui.calendarOpen));
    elements.taskDuePopover.hidden = !ui.calendarOpen;

    if (ui.draftDue) {
        const date = parseDateKey(ui.draftDue);
        elements.taskDueLabel.textContent = formatLongDate(date);
        elements.taskDueMeta.textContent = relativeDateLabel(date);
    } else {
        elements.taskDueLabel.textContent = "No deadline";
        elements.taskDueMeta.textContent = "Custom calendar";
    }

    elements.calendarMonthLabel.textContent = new Intl.DateTimeFormat(undefined, {
        month: "long",
        year: "numeric"
    }).format(ui.calendarCursor);

    elements.calendarGrid.innerHTML = buildCalendarDays(ui.calendarCursor)
        .map((date) => {
            const dateKey = toDateKey(date);
            const classes = ["calendar-day"];
            if (date.getMonth() !== ui.calendarCursor.getMonth()) {
                classes.push("is-other-month");
            }
            if (dateKey === TODAY_KEY) {
                classes.push("is-today");
            }
            if (dateKey === ui.draftDue) {
                classes.push("is-selected");
            }

            return `
                <button type="button" class="${classes.join(" ")}" data-date="${dateKey}">
                    ${date.getDate()}
                </button>
            `;
        })
        .join("");
}

function renderTaskList(container, tasks, emptyMessage) {
    if (!tasks.length) {
        container.innerHTML = `<div class="empty-state">${emptyMessage}</div>`;
        return;
    }

    container.innerHTML = tasks
        .map((task) => {
            const dueLabel = task.due ? formatTaskDue(task.due) : "No due date";
            return `
                <article class="task-card ${task.done ? "is-done" : ""}">
                    <div class="task-topline">
                        <h3 class="task-title">${escapeHtml(task.title)}</h3>
                        <span class="tag priority-${task.priority}">${escapeHtml(task.priority)}</span>
                    </div>
                    <div class="task-meta">
                        <span class="tag">${escapeHtml(task.project)}</span>
                        <span class="tag">${escapeHtml(dueLabel)}</span>
                    </div>
                    <div class="task-actions">
                        <button type="button" data-action="toggle" data-id="${task.id}">
                            ${task.done ? "Reopen" : "Complete"}
                        </button>
                        <button type="button" data-action="promote" data-id="${task.id}" class="ghost-button">Promote</button>
                        <button type="button" data-action="delete" data-id="${task.id}" class="ghost-button">Delete</button>
                    </div>
                </article>
            `;
        })
        .join("");
}

function renderFilterState() {
    elements.filterGroup.querySelectorAll("[data-filter]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.filter === state.filter);
    });
}

function renderPresetState() {
    elements.presetRow.querySelectorAll("[data-minutes]").forEach((button) => {
        button.classList.toggle(
            "is-active",
            Number(button.dataset.minutes) === state.timer.durationMinutes
        );
    });
}

function getFilteredTasks() {
    return state.tasks.filter((task) => {
        const matchesFilter =
            state.filter === "all" ||
            (state.filter === "active" && !task.done) ||
            (state.filter === "done" && task.done);

        const query = state.search.toLowerCase();
        const matchesSearch =
            !query ||
            task.title.toLowerCase().includes(query) ||
            task.project.toLowerCase().includes(query) ||
            task.priority.toLowerCase().includes(query);

        return matchesFilter && matchesSearch;
    });
}

function promotePriority(priority) {
    const index = PRIORITIES.indexOf(priority);
    return PRIORITIES[Math.max(0, index - 1)] || "critical";
}

function taskSort(left, right) {
    const priorityOrder = { critical: 0, high: 1, medium: 2, low: 3 };

    if (left.done !== right.done) {
        return Number(left.done) - Number(right.done);
    }

    const priorityDelta = priorityOrder[left.priority] - priorityOrder[right.priority];
    if (priorityDelta !== 0) {
        return priorityDelta;
    }

    const dueDelta = (left.due || "9999-12-31").localeCompare(right.due || "9999-12-31");
    if (dueDelta !== 0) {
        return dueDelta;
    }

    return left.createdAt.localeCompare(right.createdAt);
}

function buildCalendarDays(cursor) {
    const firstDay = startOfMonth(cursor);
    const dayOffset = (firstDay.getDay() + 6) % 7;
    const gridStart = addDays(firstDay, -dayOffset);

    return Array.from({ length: 42 }, (_, index) => addDays(gridStart, index));
}

function startOfMonth(date) {
    return new Date(date.getFullYear(), date.getMonth(), 1);
}

function startOfDay(date) {
    return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function addDays(date, amount) {
    const next = new Date(date);
    next.setDate(next.getDate() + amount);
    return next;
}

function addMonths(date, amount) {
    return new Date(date.getFullYear(), date.getMonth() + amount, 1);
}

function nextDayKey() {
    return toDateKey(addDays(TODAY, 1));
}

function parseDateKey(value) {
    const [year, month, day] = value.split("-").map(Number);
    return new Date(year, month - 1, day);
}

function toDateKey(date) {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
}

function formatTime(totalSeconds) {
    const minutes = Math.floor(totalSeconds / 60)
        .toString()
        .padStart(2, "0");
    const seconds = Math.floor(totalSeconds % 60)
        .toString()
        .padStart(2, "0");
    return `${minutes}:${seconds}`;
}

function formatDateLabel(date) {
    return new Intl.DateTimeFormat(undefined, {
        weekday: "long",
        month: "short",
        day: "numeric"
    }).format(date);
}

function formatLongDate(date) {
    return new Intl.DateTimeFormat(undefined, {
        weekday: "short",
        month: "short",
        day: "numeric"
    }).format(date);
}

function formatTaskDue(dateKey) {
    const date = parseDateKey(dateKey);
    const deltaDays = Math.round((startOfDay(date) - TODAY) / 86400000);

    if (deltaDays === 0) {
        return "Due today";
    }
    if (deltaDays === 1) {
        return "Due tomorrow";
    }
    if (deltaDays === -1) {
        return "Due yesterday";
    }

    return new Intl.DateTimeFormat(undefined, {
        month: "short",
        day: "numeric"
    }).format(date);
}

function relativeDateLabel(date) {
    const deltaDays = Math.round((startOfDay(date) - TODAY) / 86400000);

    if (deltaDays === 0) {
        return "Due today";
    }
    if (deltaDays === 1) {
        return "Due tomorrow";
    }
    if (deltaDays === -1) {
        return "Due yesterday";
    }
    if (deltaDays > 1) {
        return `In ${deltaDays} days`;
    }

    return `${Math.abs(deltaDays)} days ago`;
}

function currentWindowTitle() {
    return state.timer.running
        ? `Orbit Desk · Focus ${formatTime(state.timer.remainingSeconds)}`
        : "Orbit Desk";
}

function syncWindowTitle() {
    const title = currentWindowTitle();
    document.title = title;

    if (!window.RustFrame?.window?.setTitle || title === lastWindowTitle) {
        return;
    }

    lastWindowTitle = title;
    window.RustFrame.window.setTitle(title).catch(() => {});
}

async function runNativeAction(action) {
    try {
        await action();
        persistAndRender(false);
    } catch (error) {
        logActivity(`RustFrame error: ${formatError(error)}`);
        render();
    }
}

function logActivity(message) {
    state.activity.unshift(`${timestamp()} · ${message}`);
    state.activity = state.activity.slice(0, 12);
}

function timestamp() {
    return new Intl.DateTimeFormat(undefined, {
        hour: "2-digit",
        minute: "2-digit"
    }).format(new Date());
}

function formatError(error) {
    if (error && typeof error === "object") {
        return `${error.code ?? "error"}: ${error.message ?? JSON.stringify(error)}`;
    }

    return String(error);
}

function escapeHtml(value) {
    return value
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;")
        .replaceAll("'", "&#39;");
}
