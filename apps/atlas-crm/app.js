const STAGES = ["lead", "qualified", "proposal", "won"];

const state = {
    deals: [],
    draftStage: "lead",
    draftHeat: 2,
    filter: "all",
    search: "",
    dbInfo: null
};

const currency = new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0
});

const elements = {
    assetMode: document.getElementById("asset-mode"),
    originLabel: document.getElementById("origin-label"),
    dbPath: document.getElementById("db-path"),
    pipelineValue: document.getElementById("pipeline-value"),
    wonValue: document.getElementById("won-value"),
    openCount: document.getElementById("open-count"),
    hotCount: document.getElementById("hot-count"),
    dealCount: document.getElementById("deal-count"),
    visibleCount: document.getElementById("visible-count"),
    companyInput: document.getElementById("company-input"),
    contactInput: document.getElementById("contact-input"),
    ownerInput: document.getElementById("owner-input"),
    valueInput: document.getElementById("value-input"),
    nextStepInput: document.getElementById("next-step-input"),
    stageGroup: document.getElementById("stage-group"),
    heatGroup: document.getElementById("heat-group"),
    filterGroup: document.getElementById("filter-group"),
    searchInput: document.getElementById("search-input"),
    saveDealButton: document.getElementById("save-deal-button"),
    syncTitleButton: document.getElementById("sync-title-button"),
    closeButton: document.getElementById("close-button"),
    logOutput: document.getElementById("log-output"),
    lanes: {
        lead: document.getElementById("lane-lead"),
        qualified: document.getElementById("lane-qualified"),
        proposal: document.getElementById("lane-proposal"),
        won: document.getElementById("lane-won")
    }
};

elements.assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
elements.originLabel.textContent = window.location.origin || `${window.location.protocol}//`;

boot().catch((error) => {
    writeLog(`Atlas CRM failed to boot.\n${formatError(error)}`);
});

async function boot() {
    bindEvents();
    state.dbInfo = await window.RustFrame.db.info();
    elements.dbPath.textContent = state.dbInfo.databasePath;
    await refreshDeals();
    render();
    writeLog(
        `Atlas CRM online.\n` +
        `Path: ${state.dbInfo.databasePath}\n` +
        `Schema version: ${state.dbInfo.schemaVersion}\n` +
        `Tables: ${state.dbInfo.tables.join(", ")}`
    );
}

function bindEvents() {
    elements.stageGroup.addEventListener("click", (event) => {
        const button = event.target.closest("[data-stage]");
        if (!button) {
            return;
        }

        state.draftStage = button.dataset.stage;
        renderStageGroup();
    });

    elements.heatGroup.addEventListener("click", (event) => {
        const button = event.target.closest("[data-heat]");
        if (!button) {
            return;
        }

        state.draftHeat = Number(button.dataset.heat);
        renderHeatGroup();
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

    elements.saveDealButton.addEventListener("click", saveDeal);
    elements.syncTitleButton.addEventListener("click", async () => {
        await runNative(async () => {
            const openDeals = state.deals.filter((deal) => deal.stage !== "won");
            await window.RustFrame.window.setTitle(`Atlas CRM · ${openDeals.length} open deals`);
            writeLog("Window title synced to open pipeline count.");
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
            const deal = state.deals.find((entry) => entry.id === id);
            if (!deal) {
                return;
            }

            if (button.dataset.action === "advance") {
                await runNative(async () => {
                    const nextStage = nextStageFor(deal.stage);
                    await window.RustFrame.db.update("deals", id, { stage: nextStage });
                    await refreshDeals();
                    render();
                    writeLog(`Moved ${deal.company} to ${nextStage}.`);
                });
            }

            if (button.dataset.action === "close") {
                await runNative(async () => {
                    const nextStage = deal.stage === "won" ? "proposal" : "won";
                    await window.RustFrame.db.update("deals", id, { stage: nextStage });
                    await refreshDeals();
                    render();
                    writeLog(`${nextStage === "won" ? "Closed" : "Reopened"} ${deal.company}.`);
                });
            }

            if (button.dataset.action === "delete") {
                await runNative(async () => {
                    await window.RustFrame.db.delete("deals", id);
                    await refreshDeals();
                    render();
                    writeLog(`Deleted ${deal.company}.`);
                });
            }
        });
    }
}

async function refreshDeals() {
    state.deals = await window.RustFrame.db.list("deals", {
        orderBy: [
            { field: "heat", direction: "desc" },
            { field: "value", direction: "desc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });
}

async function saveDeal() {
    const company = elements.companyInput.value.trim();
    const contact = elements.contactInput.value.trim();
    const owner = elements.ownerInput.value.trim();
    const nextStep = elements.nextStepInput.value.trim();
    const value = Number(elements.valueInput.value);

    if (!company || !contact || !owner || !nextStep) {
        writeLog("Company, contact, owner, and next step are required.");
        return;
    }

    if (!Number.isFinite(value) || value <= 0) {
        writeLog("Deal value must be greater than zero.");
        return;
    }

    elements.saveDealButton.disabled = true;

    try {
        const created = await window.RustFrame.db.insert("deals", {
            company,
            contact,
            owner,
            value,
            nextStep,
            stage: state.draftStage,
            heat: state.draftHeat
        });
        resetComposer();
        await refreshDeals();
        render();
        writeLog(`Captured ${created.company} at ${currency.format(created.value)}.`);
    } catch (error) {
        writeLog(formatError(error));
    } finally {
        elements.saveDealButton.disabled = false;
    }
}

function render() {
    const visible = visibleDeals();
    const openDeals = state.deals.filter((deal) => deal.stage !== "won");
    const wonDeals = state.deals.filter((deal) => deal.stage === "won");
    const hotDeals = openDeals.filter((deal) => deal.heat >= 3);

    elements.pipelineValue.textContent = currency.format(sum(openDeals));
    elements.wonValue.textContent = currency.format(sum(wonDeals));
    elements.openCount.textContent = String(openDeals.length);
    elements.hotCount.textContent = String(hotDeals.length);
    elements.dealCount.textContent = `${state.deals.length} ${state.deals.length === 1 ? "row" : "rows"}`;
    elements.visibleCount.textContent = `${visible.length} visible`;

    renderStageGroup();
    renderHeatGroup();
    renderFilterGroup();

    for (const stage of STAGES) {
        renderLane(stage, visible.filter((deal) => deal.stage === stage));
    }
}

function visibleDeals() {
    return state.deals.filter((deal) => {
        const matchesFilter = (
            state.filter === "all" ||
            (state.filter === "hot" && deal.heat >= 3) ||
            (state.filter === "warm" && deal.heat === 2) ||
            (state.filter === "cool" && deal.heat <= 1)
        );
        const haystack = `${deal.company}\n${deal.contact}\n${deal.owner}\n${deal.nextStep}`.toLowerCase();
        const matchesSearch = !state.search || haystack.includes(state.search);
        return matchesFilter && matchesSearch;
    });
}

function renderStageGroup() {
    elements.stageGroup.querySelectorAll("[data-stage]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.stage === state.draftStage);
    });
}

function renderHeatGroup() {
    elements.heatGroup.querySelectorAll("[data-heat]").forEach((button) => {
        button.classList.toggle("is-active", Number(button.dataset.heat) === state.draftHeat);
    });
}

function renderFilterGroup() {
    elements.filterGroup.querySelectorAll("[data-filter]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.filter === state.filter);
    });
}

function renderLane(stage, deals) {
    const container = elements.lanes[stage];
    if (!deals.length) {
        container.innerHTML = `<div class="empty-state">No deals in ${stage}.</div>`;
        return;
    }

    container.innerHTML = deals.map((deal) => `
        <article class="deal-card">
            <div class="deal-top">
                <div>
                    <h4>${escapeHtml(deal.company)}</h4>
                    <p class="deal-note">${escapeHtml(deal.contact)} · ${escapeHtml(deal.owner)}</p>
                </div>
                <strong>${currency.format(deal.value)}</strong>
            </div>

            <p class="deal-note">${escapeHtml(deal.nextStep)}</p>

            <div class="deal-tags">
                <span class="tag ${heatClass(deal.heat)}">${heatLabel(deal.heat)}</span>
                <span class="tag">${escapeHtml(deal.stage)}</span>
            </div>

            <div class="deal-footer">
                <span class="deal-meta">Updated ${new Date(deal.updatedAt).toLocaleString()}</span>
                <div class="deal-actions">
                    <button type="button" data-action="advance" data-id="${deal.id}">
                        ${stage === "won" ? "Back to lead" : `Move to ${nextStageFor(stage)}`}
                    </button>
                    <button type="button" data-action="close" data-id="${deal.id}">
                        ${stage === "won" ? "Reopen" : "Mark won"}
                    </button>
                    <button type="button" data-action="delete" data-id="${deal.id}">Delete</button>
                </div>
            </div>
        </article>
    `).join("");
}

function heatClass(heat) {
    if (heat >= 3) {
        return "is-hot";
    }
    if (heat === 2) {
        return "is-warm";
    }
    return "is-cool";
}

function heatLabel(heat) {
    if (heat >= 4) {
        return "Heat 4";
    }
    if (heat === 3) {
        return "Heat 3";
    }
    if (heat === 2) {
        return "Heat 2";
    }
    return "Heat 1";
}

function nextStageFor(stage) {
    const index = STAGES.indexOf(stage);
    if (index === -1 || index === STAGES.length - 1) {
        return "lead";
    }

    return STAGES[index + 1];
}

function sum(deals) {
    return deals.reduce((total, deal) => total + deal.value, 0);
}

function resetComposer() {
    elements.companyInput.value = "";
    elements.contactInput.value = "";
    elements.ownerInput.value = "";
    elements.valueInput.value = "";
    elements.nextStepInput.value = "";
    state.draftStage = "lead";
    state.draftHeat = 2;
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
