const state = {
    items: [],
    filter: "all",
    search: "",
    dbInfo: null
};

const elements = {
    assetMode: document.getElementById("asset-mode"),
    originLabel: document.getElementById("origin-label"),
    dbPath: document.getElementById("db-path"),
    unitsCount: document.getElementById("units-count"),
    lowCount: document.getElementById("low-count"),
    skuCount: document.getElementById("sku-count"),
    locationCount: document.getElementById("location-count"),
    itemCount: document.getElementById("item-count"),
    visibleCount: document.getElementById("visible-count"),
    nameInput: document.getElementById("name-input"),
    skuInput: document.getElementById("sku-input"),
    locationInput: document.getElementById("location-input"),
    categoryInput: document.getElementById("category-input"),
    supplierInput: document.getElementById("supplier-input"),
    quantityInput: document.getElementById("quantity-input"),
    reorderInput: document.getElementById("reorder-input"),
    filterGroup: document.getElementById("filter-group"),
    searchInput: document.getElementById("search-input"),
    saveItemButton: document.getElementById("save-item-button"),
    syncTitleButton: document.getElementById("sync-title-button"),
    closeButton: document.getElementById("close-button"),
    categoryList: document.getElementById("category-list"),
    inventoryList: document.getElementById("inventory-list"),
    logOutput: document.getElementById("log-output")
};

elements.assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
elements.originLabel.textContent = window.location.origin || `${window.location.protocol}//`;

boot().catch((error) => {
    writeLog(`Meridian Inventory failed to boot.\n${formatError(error)}`);
});

async function boot() {
    bindEvents();
    state.dbInfo = await window.RustFrame.db.info();
    elements.dbPath.textContent = state.dbInfo.databasePath;
    await refreshItems();
    render();
    writeLog(
        `Meridian Inventory online.\n` +
        `Path: ${state.dbInfo.databasePath}\n` +
        `Schema version: ${state.dbInfo.schemaVersion}\n` +
        `Tables: ${state.dbInfo.tables.join(", ")}`
    );
}

function bindEvents() {
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

    elements.saveItemButton.addEventListener("click", saveItem);
    elements.syncTitleButton.addEventListener("click", async () => {
        await runNative(async () => {
            const low = state.items.filter((item) => stockState(item) !== "healthy").length;
            await window.RustFrame.window.setTitle(`Meridian Inventory · ${low} low or out`);
            writeLog("Window title synced to low-stock pressure.");
        });
    });
    elements.closeButton.addEventListener("click", async () => {
        await runNative(async () => {
            await window.RustFrame.window.close();
        });
    });

    elements.inventoryList.addEventListener("click", async (event) => {
        const button = event.target.closest("[data-action]");
        if (!button) {
            return;
        }

        const id = Number(button.dataset.id);
        const item = state.items.find((entry) => entry.id === id);
        if (!item) {
            return;
        }

        if (button.dataset.action === "increment") {
            await updateQuantity(item, item.quantity + 1, `${item.name} increased to ${item.quantity + 1}.`);
        }

        if (button.dataset.action === "decrement") {
            await updateQuantity(item, Math.max(0, item.quantity - 1), `${item.name} reduced to ${Math.max(0, item.quantity - 1)}.`);
        }

        if (button.dataset.action === "restock") {
            await updateQuantity(item, item.quantity + Math.max(1, item.reorderPoint), `${item.name} restocked.`);
        }

        if (button.dataset.action === "delete") {
            await runNative(async () => {
                await window.RustFrame.db.delete("items", id);
                await refreshItems();
                render();
                writeLog(`Deleted ${item.name}.`);
            });
        }
    });
}

async function refreshItems() {
    state.items = await window.RustFrame.db.list("items", {
        orderBy: [
            { field: "quantity", direction: "asc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });
}

async function saveItem() {
    const name = elements.nameInput.value.trim();
    const sku = elements.skuInput.value.trim();
    const location = elements.locationInput.value.trim();
    const category = elements.categoryInput.value.trim();
    const supplier = elements.supplierInput.value.trim();
    const quantity = Number(elements.quantityInput.value);
    const reorderPoint = Number(elements.reorderInput.value);

    if (!name || !sku || !location || !category || !supplier) {
        writeLog("Name, SKU, location, category, and supplier are required.");
        return;
    }

    if (!Number.isInteger(quantity) || quantity < 0 || !Number.isInteger(reorderPoint) || reorderPoint < 0) {
        writeLog("Quantity and reorder point must be whole numbers.");
        return;
    }

    elements.saveItemButton.disabled = true;

    try {
        const created = await window.RustFrame.db.insert("items", {
            name,
            sku,
            location,
            category,
            supplier,
            quantity,
            reorderPoint
        });
        resetComposer();
        await refreshItems();
        render();
        writeLog(`Saved item ${created.name} with SKU ${created.sku}.`);
    } catch (error) {
        writeLog(formatError(error));
    } finally {
        elements.saveItemButton.disabled = false;
    }
}

async function updateQuantity(item, nextQuantity, message) {
    await runNative(async () => {
        await window.RustFrame.db.update("items", item.id, { quantity: nextQuantity });
        await refreshItems();
        render();
        writeLog(message);
    });
}

function render() {
    const visible = visibleItems();
    const lowOrOut = state.items.filter((item) => stockState(item) !== "healthy");
    const locations = new Set(state.items.map((item) => item.location));

    elements.unitsCount.textContent = String(state.items.reduce((sum, item) => sum + item.quantity, 0));
    elements.lowCount.textContent = String(lowOrOut.length);
    elements.skuCount.textContent = String(state.items.length);
    elements.locationCount.textContent = String(locations.size);
    elements.itemCount.textContent = `${state.items.length} ${state.items.length === 1 ? "row" : "rows"}`;
    elements.visibleCount.textContent = `${visible.length} visible`;

    renderFilterGroup();
    renderCategories(visible);
    renderInventory(visible);
}

function visibleItems() {
    return state.items.filter((item) => {
        const stateKey = stockState(item);
        const matchesFilter = state.filter === "all" || stateKey === state.filter;
        const haystack = `${item.name}\n${item.sku}\n${item.category}\n${item.supplier}\n${item.location}`.toLowerCase();
        const matchesSearch = !state.search || haystack.includes(state.search);
        return matchesFilter && matchesSearch;
    });
}

function renderFilterGroup() {
    elements.filterGroup.querySelectorAll("[data-filter]").forEach((button) => {
        button.classList.toggle("is-active", button.dataset.filter === state.filter);
    });
}

function renderCategories(items) {
    if (!items.length) {
        elements.categoryList.innerHTML = `<div class="empty-state">No categories match the current filter.</div>`;
        return;
    }

    const categories = new Map();
    for (const item of items) {
        categories.set(item.category, (categories.get(item.category) || 0) + item.quantity);
    }

    elements.categoryList.innerHTML = [...categories.entries()]
        .sort((left, right) => right[1] - left[1])
        .map(([category, units]) => `
            <div class="category-row">
                <strong>${escapeHtml(category)}</strong>
                <p>${units} units currently in stock</p>
            </div>
        `)
        .join("");
}

function renderInventory(items) {
    if (!items.length) {
        elements.inventoryList.innerHTML = `<div class="empty-state">No inventory rows match the current filter.</div>`;
        return;
    }

    elements.inventoryList.innerHTML = items.map((item) => {
        const stateKey = stockState(item);
        return `
            <article class="item-card">
                <div class="item-top">
                    <div>
                        <h4>${escapeHtml(item.name)}</h4>
                        <p class="item-meta">${escapeHtml(item.sku)} · ${escapeHtml(item.location)}</p>
                    </div>
                    <strong>${item.quantity}</strong>
                </div>

                <div class="tags">
                    <span class="tag ${tagClass(stateKey)}">${labelForState(stateKey)}</span>
                    <span class="tag">${escapeHtml(item.category)}</span>
                    <span class="tag">${escapeHtml(item.supplier)}</span>
                    <span class="tag">Reorder ${item.reorderPoint}</span>
                </div>

                <div class="item-footer">
                    <span class="item-note">Updated ${new Date(item.updatedAt).toLocaleString()}</span>
                    <div class="item-actions">
                        <button type="button" data-action="increment" data-id="${item.id}">+1</button>
                        <button type="button" data-action="decrement" data-id="${item.id}">-1</button>
                        <button type="button" data-action="restock" data-id="${item.id}">Restock</button>
                        <button type="button" data-action="delete" data-id="${item.id}">Delete</button>
                    </div>
                </div>
            </article>
        `;
    }).join("");
}

function stockState(item) {
    if (item.quantity === 0) {
        return "out";
    }
    if (item.quantity <= item.reorderPoint) {
        return "low";
    }
    return "healthy";
}

function tagClass(stateKey) {
    return {
        low: "is-low",
        out: "is-out",
        healthy: "is-healthy"
    }[stateKey];
}

function labelForState(stateKey) {
    return {
        low: "Low stock",
        out: "Out",
        healthy: "Healthy"
    }[stateKey];
}

function resetComposer() {
    elements.nameInput.value = "";
    elements.skuInput.value = "";
    elements.locationInput.value = "";
    elements.categoryInput.value = "";
    elements.supplierInput.value = "";
    elements.quantityInput.value = "";
    elements.reorderInput.value = "";
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
