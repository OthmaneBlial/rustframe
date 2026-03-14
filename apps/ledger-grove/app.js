const state = {
    entries: [],
    direction: "income",
    filter: "all",
    search: "",
    dbInfo: null
};

const currencyFormatter = new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 2
});

const elements = {
    assetMode: document.getElementById("asset-mode"),
    originLabel: document.getElementById("origin-label"),
    dbPath: document.getElementById("db-path"),
    balanceValue: document.getElementById("balance-value"),
    balanceCaption: document.getElementById("balance-caption"),
    incomeValue: document.getElementById("income-value"),
    expenseValue: document.getElementById("expense-value"),
    largestLine: document.getElementById("largest-line"),
    largestCaption: document.getElementById("largest-caption"),
    entryCount: document.getElementById("entry-count"),
    visibleCount: document.getElementById("visible-count"),
    entryLabel: document.getElementById("entry-label"),
    entryAmount: document.getElementById("entry-amount"),
    entryDate: document.getElementById("entry-date"),
    entryAccount: document.getElementById("entry-account"),
    entryCategory: document.getElementById("entry-category"),
    entryNote: document.getElementById("entry-note"),
    entryCleared: document.getElementById("entry-cleared"),
    directionGroup: document.getElementById("direction-group"),
    flowFilter: document.getElementById("flow-filter"),
    searchInput: document.getElementById("search-input"),
    saveEntryButton: document.getElementById("save-entry-button"),
    categoryBreakdown: document.getElementById("category-breakdown"),
    ledgerList: document.getElementById("ledger-list"),
    syncTitleButton: document.getElementById("sync-title-button"),
    closeButton: document.getElementById("close-button"),
    logOutput: document.getElementById("log-output")
};

elements.assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
elements.originLabel.textContent = window.location.origin || `${window.location.protocol}//`;
elements.entryDate.value = todayString();

boot().catch((error) => {
    writeLog(`Ledger Grove failed to boot.\n${formatError(error)}`);
});

async function boot() {
    bindEvents();
    state.dbInfo = await window.RustFrame.db.info();
    elements.dbPath.textContent = state.dbInfo.databasePath;
    await refreshEntries();
    render();
    writeLog(
        `Database ready.\n` +
        `Path: ${state.dbInfo.databasePath}\n` +
        `Schema version: ${state.dbInfo.schemaVersion}\n` +
        `Tables: ${state.dbInfo.tables.join(", ")}`
    );
}

function bindEvents() {
    elements.directionGroup.addEventListener("click", (event) => {
        const button = event.target.closest("[data-direction]");
        if (!button) {
            return;
        }

        state.direction = button.dataset.direction;
        renderDirectionGroup();
    });

    elements.flowFilter.addEventListener("click", (event) => {
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

    elements.saveEntryButton.addEventListener("click", saveEntry);
    elements.syncTitleButton.addEventListener("click", async () => {
        await runNative(async () => {
            const visible = visibleEntries();
            await window.RustFrame.window.setTitle(`Ledger Grove · ${currencyFormatter.format(balanceFor(visible))}`);
            writeLog("Window title synced to current visible balance.");
        });
    });
    elements.closeButton.addEventListener("click", async () => {
        await runNative(async () => {
            await window.RustFrame.window.close();
        });
    });

    elements.ledgerList.addEventListener("click", async (event) => {
        const button = event.target.closest("[data-action]");
        if (!button) {
            return;
        }

        const id = Number(button.dataset.id);
        const entry = state.entries.find((item) => item.id === id);
        if (!entry) {
            return;
        }

        if (button.dataset.action === "toggle-cleared") {
            await runNative(async () => {
                await window.RustFrame.db.update("entries", id, { cleared: !entry.cleared });
                await refreshEntries();
                render();
                writeLog(`${entry.label} marked as ${entry.cleared ? "uncleared" : "cleared"}.`);
            });
        }

        if (button.dataset.action === "delete") {
            await runNative(async () => {
                await window.RustFrame.db.delete("entries", id);
                await refreshEntries();
                render();
                writeLog(`Deleted "${entry.label}" from the ledger.`);
            });
        }
    });
}

async function refreshEntries() {
    state.entries = await window.RustFrame.db.list("entries", {
        orderBy: [
            { field: "bookedOn", direction: "desc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });
}

async function saveEntry() {
    const label = elements.entryLabel.value.trim();
    const amount = Number(elements.entryAmount.value);
    const bookedOn = normalizeDate(elements.entryDate.value.trim());
    const account = elements.entryAccount.value.trim();
    const category = elements.entryCategory.value.trim();
    const note = elements.entryNote.value.trim();

    if (!label) {
        writeLog("Add a label before saving the ledger line.");
        return;
    }

    if (!Number.isFinite(amount) || amount <= 0) {
        writeLog("Amount must be a number greater than zero.");
        return;
    }

    if (!bookedOn) {
        writeLog("Booked on must use the YYYY-MM-DD format.");
        return;
    }

    if (!account || !category) {
        writeLog("Account and category are required.");
        return;
    }

    elements.saveEntryButton.disabled = true;

    try {
        const created = await window.RustFrame.db.insert("entries", {
            label,
            amount,
            direction: state.direction,
            bookedOn,
            account,
            category,
            note,
            cleared: elements.entryCleared.checked
        });
        resetComposer();
        await refreshEntries();
        render();
        writeLog(`Saved ${created.direction} line "${created.label}" for ${currencyFormatter.format(created.amount)}.`);
    } catch (error) {
        writeLog(formatError(error));
    } finally {
        elements.saveEntryButton.disabled = false;
    }
}

function render() {
    const visible = visibleEntries();
    const totals = summarize(state.entries);
    const largest = state.entries.reduce((current, item) => {
        if (!current || item.amount > current.amount) {
            return item;
        }

        return current;
    }, null);

    elements.entryCount.textContent = `${state.entries.length} ${state.entries.length === 1 ? "row" : "rows"}`;
    elements.visibleCount.textContent = `${visible.length} visible`;
    elements.balanceValue.textContent = currencyFormatter.format(totals.balance);
    elements.balanceCaption.textContent = `${state.entries.length} transactions tracked`;
    elements.incomeValue.textContent = currencyFormatter.format(totals.income);
    elements.expenseValue.textContent = currencyFormatter.format(totals.expense);
    elements.largestLine.textContent = largest ? currencyFormatter.format(largest.amount) : currencyFormatter.format(0);
    elements.largestCaption.textContent = largest ? `${largest.label} · ${capitalize(largest.direction)}` : "No entries yet";
    renderDirectionGroup();
    renderFilterGroup();
    renderCategoryBreakdown(visible);
    renderLedger(visible);
}

function visibleEntries() {
    return state.entries.filter((entry) => {
        const matchesFilter = state.filter === "all" || entry.direction === state.filter;
        const haystack = `${entry.label}\n${entry.account}\n${entry.category}\n${entry.note}`.toLowerCase();
        const matchesSearch = !state.search || haystack.includes(state.search);
        return matchesFilter && matchesSearch;
    });
}

function renderDirectionGroup() {
    elements.directionGroup.querySelectorAll("[data-direction]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.direction === state.direction);
    });
}

function renderFilterGroup() {
    elements.flowFilter.querySelectorAll("[data-filter]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.filter === state.filter);
    });
}

function renderCategoryBreakdown(entries) {
    if (!entries.length) {
        elements.categoryBreakdown.innerHTML = `<div class="empty-state">No categories yet. Add a ledger line to populate this panel.</div>`;
        return;
    }

    const categories = new Map();
    for (const entry of entries) {
        const signed = entry.direction === "income" ? entry.amount : -entry.amount;
        categories.set(entry.category, (categories.get(entry.category) || 0) + signed);
    }

    const rows = [...categories.entries()]
        .sort((left, right) => Math.abs(right[1]) - Math.abs(left[1]))
        .map(([name, total]) => `
            <div class="category-row">
                <strong>${escapeHtml(name)}</strong>
                <p>${total >= 0 ? "Net positive" : "Net spend"} · ${currencyFormatter.format(total)}</p>
            </div>
        `);

    elements.categoryBreakdown.innerHTML = rows.join("");
}

function renderLedger(entries) {
    if (!entries.length) {
        elements.ledgerList.innerHTML = `<div class="empty-state">No ledger lines match the current filter.</div>`;
        return;
    }

    elements.ledgerList.innerHTML = entries.map((entry) => `
        <article class="entry-card">
            <div class="entry-top">
                <div>
                    <strong>${escapeHtml(entry.label)}</strong>
                    <p class="entry-note">${escapeHtml(entry.note || "No memo on this line.")}</p>
                </div>
                <strong class="entry-amount ${entry.direction === "income" ? "is-income" : "is-expense"}">
                    ${entry.direction === "income" ? "+" : "-"}${currencyFormatter.format(entry.amount)}
                </strong>
            </div>

            <div class="entry-meta">
                <span class="chip ${entry.direction === "income" ? "is-income" : "is-expense"}">${escapeHtml(entry.direction)}</span>
                <span class="chip">${escapeHtml(entry.category)}</span>
                <span class="chip">${escapeHtml(entry.account)}</span>
                <span class="chip">${escapeHtml(entry.bookedOn)}</span>
                <span class="chip">${entry.cleared ? "Cleared" : "Pending"}</span>
            </div>

            <div class="entry-footer">
                <span class="entry-note">Updated ${new Date(entry.updatedAt).toLocaleString()}</span>
                <div class="entry-actions">
                    <button type="button" data-action="toggle-cleared" data-id="${entry.id}">
                        ${entry.cleared ? "Mark pending" : "Mark cleared"}
                    </button>
                    <button type="button" data-action="delete" data-id="${entry.id}">Delete</button>
                </div>
            </div>
        </article>
    `).join("");
}

function summarize(entries) {
    return entries.reduce((totals, entry) => {
        if (entry.direction === "income") {
            totals.income += entry.amount;
            totals.balance += entry.amount;
        } else {
            totals.expense += entry.amount;
            totals.balance -= entry.amount;
        }

        return totals;
    }, { income: 0, expense: 0, balance: 0 });
}

function balanceFor(entries) {
    return summarize(entries).balance;
}

function resetComposer() {
    elements.entryLabel.value = "";
    elements.entryAmount.value = "";
    elements.entryDate.value = todayString();
    elements.entryAccount.value = "";
    elements.entryCategory.value = "";
    elements.entryNote.value = "";
    elements.entryCleared.checked = true;
    state.direction = "income";
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

function capitalize(value) {
    return value.charAt(0).toUpperCase() + value.slice(1);
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
