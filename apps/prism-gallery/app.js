const STATES = ["draft", "in-review", "approved", "archive"];

const state = {
    assets: [],
    draftPalette: "sunset",
    draftState: "draft",
    draftRating: 3,
    filter: "all",
    search: "",
    dbInfo: null
};

const elements = {
    assetMode: document.getElementById("asset-mode"),
    originLabel: document.getElementById("origin-label"),
    dbPath: document.getElementById("db-path"),
    assetCount: document.getElementById("asset-count"),
    favoriteCount: document.getElementById("favorite-count"),
    reviewCount: document.getElementById("review-count"),
    collectionCount: document.getElementById("collection-count"),
    rowCount: document.getElementById("row-count"),
    visibleCount: document.getElementById("visible-count"),
    titleInput: document.getElementById("title-input"),
    collectionInput: document.getElementById("collection-input"),
    formatInput: document.getElementById("format-input"),
    usageInput: document.getElementById("usage-input"),
    paletteGroup: document.getElementById("palette-group"),
    stateGroup: document.getElementById("state-group"),
    ratingGroup: document.getElementById("rating-group"),
    filterGroup: document.getElementById("filter-group"),
    searchInput: document.getElementById("search-input"),
    saveAssetButton: document.getElementById("save-asset-button"),
    syncTitleButton: document.getElementById("sync-title-button"),
    closeButton: document.getElementById("close-button"),
    galleryGrid: document.getElementById("gallery-grid"),
    logOutput: document.getElementById("log-output")
};

elements.assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
elements.originLabel.textContent = window.location.origin || `${window.location.protocol}//`;

boot().catch((error) => {
    writeLog(`Prism Gallery failed to boot.\n${formatError(error)}`);
});

async function boot() {
    bindEvents();
    state.dbInfo = await window.RustFrame.db.info();
    elements.dbPath.textContent = state.dbInfo.databasePath;
    await refreshAssets();
    render();
    writeLog(
        `Prism Gallery online.\n` +
        `Path: ${state.dbInfo.databasePath}\n` +
        `Schema version: ${state.dbInfo.schemaVersion}\n` +
        `Tables: ${state.dbInfo.tables.join(", ")}`
    );
}

function bindEvents() {
    bindChipGroup(elements.paletteGroup, "palette", (value) => {
        state.draftPalette = value;
        renderPaletteGroup();
    });
    bindChipGroup(elements.stateGroup, "state", (value) => {
        state.draftState = value;
        renderStateGroup();
    });
    bindChipGroup(elements.ratingGroup, "rating", (value) => {
        state.draftRating = Number(value);
        renderRatingGroup();
    });
    bindChipGroup(elements.filterGroup, "filter", (value) => {
        state.filter = value;
        render();
    });

    elements.searchInput.addEventListener("input", () => {
        state.search = elements.searchInput.value.trim().toLowerCase();
        render();
    });

    elements.saveAssetButton.addEventListener("click", saveAsset);
    elements.syncTitleButton.addEventListener("click", async () => {
        await runNative(async () => {
            const favorites = state.assets.filter((asset) => asset.rating >= 4).length;
            await window.RustFrame.window.setTitle(`Prism Gallery · ${favorites} favorites`);
            writeLog("Window title synced to favorite asset count.");
        });
    });
    elements.closeButton.addEventListener("click", async () => {
        await runNative(async () => {
            await window.RustFrame.window.close();
        });
    });

    elements.galleryGrid.addEventListener("click", async (event) => {
        const button = event.target.closest("[data-action]");
        if (!button) {
            return;
        }

        const id = Number(button.dataset.id);
        const asset = state.assets.find((entry) => entry.id === id);
        if (!asset) {
            return;
        }

        if (button.dataset.action === "rate") {
            await runNative(async () => {
                const nextRating = asset.rating >= 5 ? 1 : asset.rating + 1;
                await window.RustFrame.db.update("assets", id, { rating: nextRating });
                await refreshAssets();
                render();
                writeLog(`Updated rating for "${asset.title}" to ${nextRating}.`);
            });
        }

        if (button.dataset.action === "state") {
            await runNative(async () => {
                const nextState = nextStateFor(asset.state);
                await window.RustFrame.db.update("assets", id, { state: nextState });
                await refreshAssets();
                render();
                writeLog(`Moved "${asset.title}" to ${nextState}.`);
            });
        }

        if (button.dataset.action === "delete") {
            await runNative(async () => {
                await window.RustFrame.db.delete("assets", id);
                await refreshAssets();
                render();
                writeLog(`Deleted "${asset.title}".`);
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

async function refreshAssets() {
    state.assets = await window.RustFrame.db.list("assets", {
        orderBy: [
            { field: "rating", direction: "desc" },
            { field: "updatedAt", direction: "desc" }
        ]
    });
}

async function saveAsset() {
    const title = elements.titleInput.value.trim();
    const collection = elements.collectionInput.value.trim();
    const format = elements.formatInput.value.trim();
    const usage = elements.usageInput.value.trim();

    if (!title || !collection || !format || !usage) {
        writeLog("Title, collection, format, and usage are required.");
        return;
    }

    elements.saveAssetButton.disabled = true;

    try {
        const created = await window.RustFrame.db.insert("assets", {
            title,
            collection,
            format,
            usage,
            palette: state.draftPalette,
            state: state.draftState,
            rating: state.draftRating
        });
        resetComposer();
        await refreshAssets();
        render();
        writeLog(`Saved asset "${created.title}".`);
    } catch (error) {
        writeLog(formatError(error));
    } finally {
        elements.saveAssetButton.disabled = false;
    }
}

function render() {
    const visible = visibleAssets();
    const favorites = state.assets.filter((asset) => asset.rating >= 4);
    const review = state.assets.filter((asset) => asset.state === "in-review");
    const collections = new Set(state.assets.map((asset) => asset.collection));

    elements.assetCount.textContent = String(state.assets.length);
    elements.favoriteCount.textContent = String(favorites.length);
    elements.reviewCount.textContent = String(review.length);
    elements.collectionCount.textContent = String(collections.size);
    elements.rowCount.textContent = `${state.assets.length} ${state.assets.length === 1 ? "row" : "rows"}`;
    elements.visibleCount.textContent = `${visible.length} visible`;

    renderPaletteGroup();
    renderStateGroup();
    renderRatingGroup();
    renderFilterGroup();
    renderGallery(visible);
}

function visibleAssets() {
    return state.assets.filter((asset) => {
        const matchesFilter = state.filter === "all" || asset.palette === state.filter;
        const haystack = `${asset.title}\n${asset.collection}\n${asset.format}\n${asset.usage}`.toLowerCase();
        const matchesSearch = !state.search || haystack.includes(state.search);
        return matchesFilter && matchesSearch;
    });
}

function renderPaletteGroup() {
    toggleGroup(elements.paletteGroup, "palette", state.draftPalette);
}

function renderStateGroup() {
    toggleGroup(elements.stateGroup, "state", state.draftState);
}

function renderRatingGroup() {
    toggleGroup(elements.ratingGroup, "rating", String(state.draftRating));
}

function renderFilterGroup() {
    toggleGroup(elements.filterGroup, "filter", state.filter);
}

function toggleGroup(container, key, expected) {
    container.querySelectorAll(`[data-${key}]`).forEach((button) => {
        button.classList.toggle("is-active", button.dataset[key] === expected);
    });
}

function renderGallery(assets) {
    if (!assets.length) {
        elements.galleryGrid.innerHTML = `<div class="empty-state">No assets match the current filter.</div>`;
        return;
    }

    elements.galleryGrid.innerHTML = assets.map((asset) => `
        <article class="asset-card">
            <div class="asset-preview palette-${escapeHtml(asset.palette)}"></div>
            <div class="asset-top">
                <div>
                    <h4>${escapeHtml(asset.title)}</h4>
                    <p class="asset-note">${escapeHtml(asset.collection)} · ${escapeHtml(asset.format)}</p>
                </div>
                <strong class="rating">${"★".repeat(asset.rating)}</strong>
            </div>

            <p class="asset-meta">${escapeHtml(asset.usage)}</p>

            <div class="tags">
                <span class="tag">${escapeHtml(asset.palette)}</span>
                <span class="tag">${escapeHtml(asset.state)}</span>
            </div>

            <div class="asset-footer">
                <span class="asset-meta">Updated ${new Date(asset.updatedAt).toLocaleString()}</span>
                <div class="asset-actions">
                    <button type="button" data-action="rate" data-id="${asset.id}">Rate</button>
                    <button type="button" data-action="state" data-id="${asset.id}">Cycle state</button>
                    <button type="button" data-action="delete" data-id="${asset.id}">Delete</button>
                </div>
            </div>
        </article>
    `).join("");
}

function nextStateFor(current) {
    const index = STATES.indexOf(current);
    if (index === -1 || index === STATES.length - 1) {
        return "draft";
    }
    return STATES[index + 1];
}

function resetComposer() {
    elements.titleInput.value = "";
    elements.collectionInput.value = "";
    elements.formatInput.value = "";
    elements.usageInput.value = "";
    state.draftPalette = "sunset";
    state.draftState = "draft";
    state.draftRating = 3;
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
