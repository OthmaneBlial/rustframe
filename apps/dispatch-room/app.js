const LANES = ["triage", "active", "blocked", "resolved"];

const state = {
    incidents: [],
    draftSeverity: "high",
    draftStatus: "triage",
    filter: "all",
    search: "",
    dbInfo: null
};

const elements = {
    assetMode: document.getElementById("asset-mode"),
    originLabel: document.getElementById("origin-label"),
    dbPath: document.getElementById("db-path"),
    totalCount: document.getElementById("total-count"),
    openCount: document.getElementById("open-count"),
    criticalCount: document.getElementById("critical-count"),
    todayCount: document.getElementById("today-count"),
    incidentCount: document.getElementById("incident-count"),
    visibleCount: document.getElementById("visible-count"),
    incidentTitle: document.getElementById("incident-title"),
    incidentService: document.getElementById("incident-service"),
    incidentOwner: document.getElementById("incident-owner"),
    incidentDueOn: document.getElementById("incident-due-on"),
    incidentSummary: document.getElementById("incident-summary"),
    severityGroup: document.getElementById("severity-group"),
    statusGroup: document.getElementById("status-group"),
    filterGroup: document.getElementById("filter-group"),
    searchInput: document.getElementById("search-input"),
    saveIncidentButton: document.getElementById("save-incident-button"),
    syncTitleButton: document.getElementById("sync-title-button"),
    closeButton: document.getElementById("close-button"),
    logOutput: document.getElementById("log-output"),
    lanes: {
        triage: document.getElementById("lane-triage"),
        active: document.getElementById("lane-active"),
        blocked: document.getElementById("lane-blocked"),
        resolved: document.getElementById("lane-resolved")
    }
};

elements.assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
elements.originLabel.textContent = window.location.origin || `${window.location.protocol}//`;
elements.incidentDueOn.value = todayString();

boot().catch((error) => {
    writeLog(`Dispatch Room failed to boot.\n${formatError(error)}`);
});

async function boot() {
    bindEvents();
    state.dbInfo = await window.RustFrame.db.info();
    elements.dbPath.textContent = state.dbInfo.databasePath;
    await refreshIncidents();
    render();
    writeLog(
        `Dispatch Room online.\n` +
        `Path: ${state.dbInfo.databasePath}\n` +
        `Schema version: ${state.dbInfo.schemaVersion}\n` +
        `Tables: ${state.dbInfo.tables.join(", ")}`
    );
}

function bindEvents() {
    elements.severityGroup.addEventListener("click", (event) => {
        const button = event.target.closest("[data-severity]");
        if (!button) {
            return;
        }

        state.draftSeverity = button.dataset.severity;
        renderSeverityGroup();
    });

    elements.statusGroup.addEventListener("click", (event) => {
        const button = event.target.closest("[data-status]");
        if (!button) {
            return;
        }

        state.draftStatus = button.dataset.status;
        renderStatusGroup();
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

    elements.saveIncidentButton.addEventListener("click", saveIncident);
    elements.syncTitleButton.addEventListener("click", async () => {
        await runNative(async () => {
            const criticalOpen = state.incidents.filter((incident) => incident.severity === "critical" && incident.status !== "resolved").length;
            await window.RustFrame.window.setTitle(`Dispatch Room · ${criticalOpen} critical open`);
            writeLog("Window title synced to critical open incident count.");
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
            const incident = state.incidents.find((item) => item.id === id);
            if (!incident) {
                return;
            }

            if (button.dataset.action === "advance") {
                await runNative(async () => {
                    const nextStatus = nextLane(incident.status);
                    await window.RustFrame.db.update("incidents", id, { status: nextStatus });
                    await refreshIncidents();
                    render();
                    writeLog(`Moved "${incident.title}" to ${nextStatus}.`);
                });
            }

            if (button.dataset.action === "resolve") {
                await runNative(async () => {
                    const nextStatus = incident.status === "resolved" ? "active" : "resolved";
                    await window.RustFrame.db.update("incidents", id, { status: nextStatus });
                    await refreshIncidents();
                    render();
                    writeLog(`${nextStatus === "resolved" ? "Resolved" : "Reopened"} "${incident.title}".`);
                });
            }

            if (button.dataset.action === "delete") {
                await runNative(async () => {
                    await window.RustFrame.db.delete("incidents", id);
                    await refreshIncidents();
                    render();
                    writeLog(`Deleted "${incident.title}".`);
                });
            }
        });
    }
}

async function refreshIncidents() {
    state.incidents = await window.RustFrame.db.list("incidents", {
        orderBy: [
            { field: "severityRank", direction: "desc" },
            { field: "dueOn", direction: "asc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });
}

async function saveIncident() {
    const title = elements.incidentTitle.value.trim();
    const service = elements.incidentService.value.trim();
    const owner = elements.incidentOwner.value.trim();
    const dueOn = normalizeDate(elements.incidentDueOn.value.trim());
    const summary = elements.incidentSummary.value.trim();

    if (!title || !service || !owner) {
        writeLog("Title, service, and owner are required.");
        return;
    }

    if (!dueOn) {
        writeLog("Due by must use YYYY-MM-DD.");
        return;
    }

    elements.saveIncidentButton.disabled = true;

    try {
        const created = await window.RustFrame.db.insert("incidents", {
            title,
            service,
            owner,
            dueOn,
            summary,
            severity: state.draftSeverity,
            severityRank: severityRank(state.draftSeverity),
            status: state.draftStatus
        });
        resetComposer();
        await refreshIncidents();
        render();
        writeLog(`Created ${created.severity} incident "${created.title}" in ${created.status}.`);
    } catch (error) {
        writeLog(formatError(error));
    } finally {
        elements.saveIncidentButton.disabled = false;
    }
}

function render() {
    const visible = visibleIncidents();
    const open = state.incidents.filter((incident) => incident.status !== "resolved");
    const critical = open.filter((incident) => incident.severity === "critical");
    const dueToday = open.filter((incident) => incident.dueOn === todayString());

    elements.totalCount.textContent = String(state.incidents.length);
    elements.openCount.textContent = String(open.length);
    elements.criticalCount.textContent = String(critical.length);
    elements.todayCount.textContent = String(dueToday.length);
    elements.incidentCount.textContent = `${state.incidents.length} ${state.incidents.length === 1 ? "row" : "rows"}`;
    elements.visibleCount.textContent = `${visible.length} visible`;

    renderSeverityGroup();
    renderStatusGroup();
    renderFilterGroup();

    for (const lane of LANES) {
        const incidents = visible.filter((incident) => incident.status === lane);
        renderLane(lane, incidents);
    }
}

function visibleIncidents() {
    return state.incidents.filter((incident) => {
        const matchesFilter = state.filter === "all" || incident.severity === state.filter;
        const haystack = `${incident.title}\n${incident.service}\n${incident.owner}\n${incident.summary}`.toLowerCase();
        const matchesSearch = !state.search || haystack.includes(state.search);
        return matchesFilter && matchesSearch;
    });
}

function renderSeverityGroup() {
    elements.severityGroup.querySelectorAll("[data-severity]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.severity === state.draftSeverity);
    });
}

function renderStatusGroup() {
    elements.statusGroup.querySelectorAll("[data-status]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.status === state.draftStatus);
    });
}

function renderFilterGroup() {
    elements.filterGroup.querySelectorAll("[data-filter]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.filter === state.filter);
    });
}

function renderLane(lane, incidents) {
    const container = elements.lanes[lane];
    if (!incidents.length) {
        container.innerHTML = `<div class="empty-state">No incidents in ${lane}.</div>`;
        return;
    }

    container.innerHTML = incidents.map((incident) => `
        <article class="incident-card">
            <div class="incident-top">
                <div>
                    <h4>${escapeHtml(incident.title)}</h4>
                    <p class="incident-summary">${escapeHtml(incident.summary || "No summary yet.")}</p>
                </div>
                <span class="tag is-${escapeHtml(incident.severity)}">${escapeHtml(incident.severity)}</span>
            </div>

            <div class="incident-meta">
                <span class="tag">${escapeHtml(incident.service)}</span>
                <span class="tag">${escapeHtml(incident.owner)}</span>
                <span class="tag">Due ${escapeHtml(incident.dueOn)}</span>
            </div>

            <div class="incident-footer">
                <span class="incident-summary">Updated ${new Date(incident.updatedAt).toLocaleString()}</span>
                <div class="incident-actions">
                    <button type="button" data-action="advance" data-id="${incident.id}">
                        ${lane === "resolved" ? "Back to triage" : `Move to ${nextLane(lane)}`}
                    </button>
                    <button type="button" data-action="resolve" data-id="${incident.id}">
                        ${lane === "resolved" ? "Reopen" : "Resolve"}
                    </button>
                    <button type="button" data-action="delete" data-id="${incident.id}">Delete</button>
                </div>
            </div>
        </article>
    `).join("");
}

function nextLane(status) {
    const index = LANES.indexOf(status);
    if (index === -1 || index === LANES.length - 1) {
        return "triage";
    }

    return LANES[index + 1];
}

function severityRank(value) {
    return {
        critical: 4,
        high: 3,
        medium: 2,
        low: 1
    }[value] || 1;
}

function resetComposer() {
    elements.incidentTitle.value = "";
    elements.incidentService.value = "";
    elements.incidentOwner.value = "";
    elements.incidentDueOn.value = todayString();
    elements.incidentSummary.value = "";
    state.draftSeverity = "high";
    state.draftStatus = "triage";
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

function normalizeDate(value) {
    return /^\d{4}-\d{2}-\d{2}$/.test(value) ? value : "";
}

function todayString() {
    return new Date().toISOString().slice(0, 10);
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
