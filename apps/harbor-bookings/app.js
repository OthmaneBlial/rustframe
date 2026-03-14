const STATUSES = ["pending", "confirmed", "in-house", "complete"];

const state = {
    bookings: [],
    draftStatus: "pending",
    filter: "all",
    search: "",
    dbInfo: null
};

const elements = {
    assetMode: document.getElementById("asset-mode"),
    originLabel: document.getElementById("origin-label"),
    dbPath: document.getElementById("db-path"),
    arrivalsCount: document.getElementById("arrivals-count"),
    inhouseCount: document.getElementById("inhouse-count"),
    pendingCount: document.getElementById("pending-count"),
    nightCount: document.getElementById("night-count"),
    bookingCount: document.getElementById("booking-count"),
    visibleCount: document.getElementById("visible-count"),
    guestInput: document.getElementById("guest-input"),
    suiteInput: document.getElementById("suite-input"),
    hostInput: document.getElementById("host-input"),
    arrivalInput: document.getElementById("arrival-input"),
    nightsInput: document.getElementById("nights-input"),
    noteInput: document.getElementById("note-input"),
    statusGroup: document.getElementById("status-group"),
    filterGroup: document.getElementById("filter-group"),
    searchInput: document.getElementById("search-input"),
    saveBookingButton: document.getElementById("save-booking-button"),
    syncTitleButton: document.getElementById("sync-title-button"),
    closeButton: document.getElementById("close-button"),
    bookingsList: document.getElementById("bookings-list"),
    hostList: document.getElementById("host-list"),
    logOutput: document.getElementById("log-output")
};

elements.assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
elements.originLabel.textContent = window.location.origin || `${window.location.protocol}//`;
elements.arrivalInput.value = todayString();

boot().catch((error) => {
    writeLog(`Harbor Bookings failed to boot.\n${formatError(error)}`);
});

async function boot() {
    bindEvents();
    state.dbInfo = await window.RustFrame.db.info();
    elements.dbPath.textContent = state.dbInfo.databasePath;
    await refreshBookings();
    render();
    writeLog(
        `Harbor Bookings online.\n` +
        `Path: ${state.dbInfo.databasePath}\n` +
        `Schema version: ${state.dbInfo.schemaVersion}\n` +
        `Tables: ${state.dbInfo.tables.join(", ")}`
    );
}

function bindEvents() {
    bindChipGroup(elements.statusGroup, "status", (value) => {
        state.draftStatus = value;
        renderStatusGroup();
    });
    bindChipGroup(elements.filterGroup, "filter", (value) => {
        state.filter = value;
        render();
    });

    elements.searchInput.addEventListener("input", () => {
        state.search = elements.searchInput.value.trim().toLowerCase();
        render();
    });

    elements.saveBookingButton.addEventListener("click", saveBooking);
    elements.syncTitleButton.addEventListener("click", async () => {
        await runNative(async () => {
            const arrivals = state.bookings.filter((booking) => booking.arrival === todayString()).length;
            await window.RustFrame.window.setTitle(`Harbor Bookings · ${arrivals} arrivals today`);
            writeLog("Window title synced to today's arrivals.");
        });
    });
    elements.closeButton.addEventListener("click", async () => {
        await runNative(async () => {
            await window.RustFrame.window.close();
        });
    });

    elements.bookingsList.addEventListener("click", async (event) => {
        const button = event.target.closest("[data-action]");
        if (!button) {
            return;
        }

        const id = Number(button.dataset.id);
        const booking = state.bookings.find((entry) => entry.id === id);
        if (!booking) {
            return;
        }

        if (button.dataset.action === "advance") {
            await runNative(async () => {
                const nextStatus = nextStatusFor(booking.status);
                await window.RustFrame.db.update("reservations", id, { status: nextStatus });
                await refreshBookings();
                render();
                writeLog(`Moved ${booking.guest} to ${nextStatus}.`);
            });
        }

        if (button.dataset.action === "complete") {
            await runNative(async () => {
                const nextStatus = booking.status === "complete" ? "confirmed" : "complete";
                await window.RustFrame.db.update("reservations", id, { status: nextStatus });
                await refreshBookings();
                render();
                writeLog(`${nextStatus === "complete" ? "Checked out" : "Reopened"} ${booking.guest}.`);
            });
        }

        if (button.dataset.action === "delete") {
            await runNative(async () => {
                await window.RustFrame.db.delete("reservations", id);
                await refreshBookings();
                render();
                writeLog(`Deleted reservation for ${booking.guest}.`);
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

async function refreshBookings() {
    state.bookings = await window.RustFrame.db.list("reservations", {
        orderBy: [
            { field: "arrival", direction: "asc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });
}

async function saveBooking() {
    const guest = elements.guestInput.value.trim();
    const suite = elements.suiteInput.value.trim();
    const host = elements.hostInput.value.trim();
    const arrival = normalizeDate(elements.arrivalInput.value.trim());
    const nights = Number(elements.nightsInput.value);
    const note = elements.noteInput.value.trim();

    if (!guest || !suite || !host || !note) {
        writeLog("Guest, suite, host, and note are required.");
        return;
    }
    if (!arrival) {
        writeLog("Arrival must use YYYY-MM-DD.");
        return;
    }
    if (!Number.isInteger(nights) || nights <= 0) {
        writeLog("Nights must be a whole number greater than zero.");
        return;
    }

    elements.saveBookingButton.disabled = true;

    try {
        const created = await window.RustFrame.db.insert("reservations", {
            guest,
            suite,
            host,
            arrival,
            nights,
            note,
            status: state.draftStatus
        });
        resetComposer();
        await refreshBookings();
        render();
        writeLog(`Saved reservation for ${created.guest}.`);
    } catch (error) {
        writeLog(formatError(error));
    } finally {
        elements.saveBookingButton.disabled = false;
    }
}

function render() {
    const visible = visibleBookings();
    const arrivals = state.bookings.filter((booking) => booking.arrival === todayString());
    const inhouse = state.bookings.filter((booking) => booking.status === "in-house");
    const pending = state.bookings.filter((booking) => booking.status === "pending");
    const nights = state.bookings.reduce((sum, booking) => sum + booking.nights, 0);

    elements.arrivalsCount.textContent = String(arrivals.length);
    elements.inhouseCount.textContent = String(inhouse.length);
    elements.pendingCount.textContent = String(pending.length);
    elements.nightCount.textContent = String(nights);
    elements.bookingCount.textContent = `${state.bookings.length} ${state.bookings.length === 1 ? "row" : "rows"}`;
    elements.visibleCount.textContent = `${visible.length} visible`;

    renderStatusGroup();
    renderFilterGroup();
    renderBookings(visible);
    renderHosts(visible);
}

function visibleBookings() {
    return state.bookings.filter((booking) => {
        const matchesFilter = state.filter === "all" || booking.status === state.filter;
        const haystack = `${booking.guest}\n${booking.suite}\n${booking.host}\n${booking.note}`.toLowerCase();
        const matchesSearch = !state.search || haystack.includes(state.search);
        return matchesFilter && matchesSearch;
    });
}

function renderStatusGroup() {
    toggleGroup(elements.statusGroup, "status", state.draftStatus);
}

function renderFilterGroup() {
    toggleGroup(elements.filterGroup, "filter", state.filter);
}

function toggleGroup(container, key, expected) {
    container.querySelectorAll(`[data-${key}]`).forEach((button) => {
        button.classList.toggle("is-active", button.dataset[key] === expected);
    });
}

function renderBookings(bookings) {
    if (!bookings.length) {
        elements.bookingsList.innerHTML = `<div class="empty-state">No bookings match the current filter.</div>`;
        return;
    }

    elements.bookingsList.innerHTML = bookings.map((booking) => `
        <article class="booking-card">
            <div class="booking-top">
                <div>
                    <h4>${escapeHtml(booking.guest)}</h4>
                    <p class="booking-meta">${escapeHtml(booking.suite)} · Host ${escapeHtml(booking.host)}</p>
                </div>
                <strong>${booking.nights} night${booking.nights === 1 ? "" : "s"}</strong>
            </div>

            <p class="booking-note">${escapeHtml(booking.note)}</p>

            <div class="tags">
                <span class="tag is-${escapeHtml(booking.status)}">${escapeHtml(booking.status)}</span>
                <span class="tag">Arrival ${escapeHtml(booking.arrival)}</span>
            </div>

            <div class="booking-footer">
                <span class="booking-meta">Updated ${new Date(booking.updatedAt).toLocaleString()}</span>
                <div class="booking-actions">
                    <button type="button" data-action="advance" data-id="${booking.id}">
                        ${booking.status === "complete" ? "Back to pending" : `Move to ${nextStatusFor(booking.status)}`}
                    </button>
                    <button type="button" data-action="complete" data-id="${booking.id}">
                        ${booking.status === "complete" ? "Reopen" : "Checkout"}
                    </button>
                    <button type="button" data-action="delete" data-id="${booking.id}">Delete</button>
                </div>
            </div>
        </article>
    `).join("");
}

function renderHosts(bookings) {
    if (!bookings.length) {
        elements.hostList.innerHTML = `<div class="empty-state">No host workload to show.</div>`;
        return;
    }

    const hosts = new Map();
    for (const booking of bookings) {
        hosts.set(booking.host, (hosts.get(booking.host) || 0) + booking.nights);
    }

    elements.hostList.innerHTML = [...hosts.entries()]
        .sort((left, right) => right[1] - left[1])
        .map(([host, nights]) => `
            <div class="host-row">
                <strong>${escapeHtml(host)}</strong>
                <p>${nights} hosted night${nights === 1 ? "" : "s"} in view</p>
            </div>
        `)
        .join("");
}

function nextStatusFor(status) {
    const index = STATUSES.indexOf(status);
    if (index === -1 || index === STATUSES.length - 1) {
        return "pending";
    }
    return STATUSES[index + 1];
}

function resetComposer() {
    elements.guestInput.value = "";
    elements.suiteInput.value = "";
    elements.hostInput.value = "";
    elements.arrivalInput.value = todayString();
    elements.nightsInput.value = "";
    elements.noteInput.value = "";
    state.draftStatus = "pending";
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
