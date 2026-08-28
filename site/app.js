const HOME_COMMAND_TABS = document.querySelectorAll("[data-command-tab]");
const HOME_COMMAND_PANELS = document.querySelectorAll("[data-command-panel]");
const COPY_BUTTONS = document.querySelectorAll("[data-copy]");
const REVEAL_ITEMS = document.querySelectorAll(".reveal");
const GITHUB_REPO_BASE = "https://github.com/OthmaneBlial/rustframe/blob/main/";
const FEATURED_DOC_IDS = [
    "choosing-rustframe",
    "architecture-overview",
    "getting-started",
    "runtime-and-capabilities",
    "community-templates",
];

const DOCS = {
    "readme": {
        title: "Docs index",
        navLabel: "Docs index",
        section: "core",
        summary: "Start with the map of the guides, operator docs, and example references.",
        source: "docs/README.md",
        path: "docs/README.md",
    },
    "getting-started": {
        title: "Getting started",
        navLabel: "Getting started",
        section: "core",
        summary: "Run the starter app, scaffold a project, and walk the CLI from dev to package.",
        source: "docs/getting-started.md",
        path: "docs/getting-started.md",
    },
    "choosing-rustframe": {
        title: "Choosing RustFrame",
        navLabel: "Choosing RustFrame",
        section: "core",
        summary: "See the honest browser, RustFrame, Tauri, and Electron tradeoffs.",
        source: "docs/choosing-rustframe.md",
        path: "docs/choosing-rustframe.md",
    },
    "architecture-overview": {
        title: "Architecture overview",
        navLabel: "Architecture overview",
        section: "core",
        summary: "Understand the hidden-runner model, manifest contract, and runtime boundary.",
        source: "docs/architecture-overview.md",
        path: "docs/architecture-overview.md",
    },
    "runtime-and-capabilities": {
        title: "Runtime and capabilities",
        navLabel: "Runtime and capabilities",
        section: "core",
        summary: "Review the native surface, trust model, database lifecycle, and bridge rules.",
        source: "docs/runtime-and-capabilities.md",
        path: "docs/runtime-and-capabilities.md",
    },
    "build-in-20-minutes": {
        title: "Build in 20 minutes",
        navLabel: "Build in 20 minutes",
        section: "core",
        summary: "Build a small workflow app quickly without losing the runtime contract.",
        source: "docs/build-in-20-minutes.md",
        path: "docs/build-in-20-minutes.md",
    },
    "cookbook": {
        title: "Cookbook",
        navLabel: "Cookbook",
        section: "core",
        summary: "Jump straight to practical patterns for windows, SQLite, shell, and files.",
        source: "docs/cookbook.md",
        path: "docs/cookbook.md",
    },
    "workflow-guides": {
        title: "Workflow guides",
        navLabel: "Workflow guides",
        section: "core",
        summary: "Shape document desks, review queues, media libraries, and offline operations tools.",
        source: "docs/workflow-guides.md",
        path: "docs/workflow-guides.md",
    },
    "research-desk-architecture": {
        title: "Research Desk architecture case study",
        navLabel: "Research Desk case study",
        section: "core",
        summary: "Trace every flagship behavior to public APIs, least-privilege permissions, and repeatable tests.",
        source: "docs/research-desk-architecture.md",
        path: "docs/research-desk-architecture.md",
    },
    "local-first-and-capabilities": {
        title: "Local-first and capability inspection",
        navLabel: "Local-first inspection",
        section: "operations",
        summary: "Inspect bundled ownership, explain policy, and deny unreviewed privilege expansion.",
        source: "docs/local-first-and-capabilities.md",
        path: "docs/local-first-and-capabilities.md",
    },
    "portable-data-exports": {
        title: "Portable data exports",
        navLabel: "Portable data exports",
        section: "operations",
        summary: "Stream a consistent SQLite snapshot to checksummed JSON, JSONL, or CSV.",
        source: "docs/portable-data-exports.md",
        path: "docs/portable-data-exports.md",
    },
    "release-verification": {
        title: "Release verification",
        navLabel: "Release verification",
        section: "operations",
        summary: "Verify downloaded checksums, native trust, SPDX evidence, and GitHub provenance.",
        source: "docs/release-verification.md",
        path: "docs/release-verification.md",
    },
    "developer-loop": {
        title: "Developer loop and diagnostics",
        navLabel: "Developer loop",
        section: "operations",
        summary: "Keep rebuilds warm, open the debug inspector, and export redacted support evidence.",
        source: "docs/developer-loop.md",
        path: "docs/developer-loop.md",
    },
    "file-open-routing": {
        title: "Single-instance file-open routing",
        navLabel: "File-open routing",
        section: "operations",
        summary: "Route OS document opens to one app process without exposing absolute paths.",
        source: "docs/file-open-routing.md",
        path: "docs/file-open-routing.md",
    },
    "api-reference": {
        title: "Frontend API reference",
        navLabel: "Frontend API reference",
        section: "reference",
        summary: "Read the generated public TypeScript contract for data, files, windows, events, and errors.",
        source: "docs/api-reference.md",
        path: "docs/api-reference.md",
    },
    "manifest-reference": {
        title: "Manifest reference",
        navLabel: "Manifest reference",
        section: "reference",
        summary: "Inspect every schema v1 field and its machine-generated JSON Schema constraint.",
        source: "docs/manifest-reference.md",
        path: "docs/manifest-reference.md",
    },
    "threat-model": {
        title: "Threat model",
        navLabel: "Threat model",
        section: "operations",
        summary: "See what the runtime protects, what it delegates, and where the operator stays responsible.",
        source: "docs/threat-model.md",
        path: "docs/threat-model.md",
    },
    "migrations-and-versioning": {
        title: "Migrations and versioning",
        navLabel: "Migrations and versioning",
        section: "operations",
        summary: "Manage schema upgrades, shipped data changes, and app compatibility with intent.",
        source: "docs/migrations-and-versioning.md",
        path: "docs/migrations-and-versioning.md",
    },
    "platform-support": {
        title: "Platform support",
        navLabel: "Platform support",
        section: "operations",
        summary: "Read the support boundary for Linux, Windows, and macOS packaging.",
        source: "docs/platform-support.md",
        path: "docs/platform-support.md",
    },
    "signing-and-notarization": {
        title: "Signing and notarization",
        navLabel: "Signing and notarization",
        section: "operations",
        summary: "Prepare shipped bundles for the trust and identity layers outside the runtime.",
        source: "docs/signing-and-notarization.md",
        path: "docs/signing-and-notarization.md",
    },
    "update-strategy": {
        title: "Update strategy",
        navLabel: "Update strategy",
        section: "operations",
        summary: "Choose how packaged apps update without hand-waving the release process.",
        source: "docs/update-strategy.md",
        path: "docs/update-strategy.md",
    },
    "release-checklist": {
        title: "Release checklist",
        navLabel: "Release checklist",
        section: "operations",
        summary: "Use the operator checklist before calling a RustFrame app ready to ship.",
        source: "docs/release-checklist.md",
        path: "docs/release-checklist.md",
    },
    "troubleshooting": {
        title: "Troubleshooting",
        navLabel: "Troubleshooting",
        section: "operations",
        summary: "Resolve install, validation, development, grant, build, and packaging failures by stage.",
        source: "docs/troubleshooting.md",
        path: "docs/troubleshooting.md",
    },
    "community-templates": {
        title: "Community templates",
        navLabel: "Community templates",
        section: "ecosystem",
        summary: "See how starters and references should stay tied to credible workflow jobs.",
        source: "docs/community-templates.md",
        path: "docs/community-templates.md",
    },
    "remote-sync-patterns": {
        title: "Remote sync patterns",
        navLabel: "Remote sync patterns",
        section: "ecosystem",
        summary: "Layer sync onto local-first apps without turning the runtime into a backend framework.",
        source: "docs/remote-sync-patterns.md",
        path: "docs/remote-sync-patterns.md",
    },
    "capability-extension-patterns": {
        title: "Capability extension patterns",
        navLabel: "Capability extension patterns",
        section: "ecosystem",
        summary: "Extend the native surface deliberately instead of reopening the whole machine.",
        source: "docs/capability-extension-patterns.md",
        path: "docs/capability-extension-patterns.md",
    },
    "frontend-app-rules": {
        title: "Frontend app rules",
        navLabel: "Frontend app rules",
        section: "ecosystem",
        summary: "Keep frontend apps compatible with the runtime contract and packaging model.",
        source: "docs/frontend-app-rules.md",
        path: "docs/frontend-app-rules.md",
    },
    "example-apps": {
        title: "Example apps",
        navLabel: "Example apps",
        section: "ecosystem",
        summary: "Browse the example set as references, not equal product claims.",
        source: "docs/example-apps.md",
        path: "docs/example-apps.md",
    },
};

const DOC_SOURCE_TO_ID = new Map(
    Object.entries(DOCS).flatMap(([id, meta]) => [
        [normalizeDocSlug(meta.path), id],
        [normalizeDocSlug(meta.source), id],
    ])
);

setupNavigation();
setupCommandTabs();
setupCopyButtons();
setupPolicyLab();
setupSchemaLab();
setupRevealObserver();
setupHomePage();
setupDocsPage();
setupShowcasePage();

function setupNavigation() {
    const toggle = document.querySelector("[data-nav-toggle]");
    const nav = document.querySelector("[data-site-nav]");
    if (!toggle || !nav) {
        return;
    }

    const setOpen = (open, { restoreFocus = false } = {}) => {
        nav.classList.toggle("is-open", open);
        toggle.setAttribute("aria-expanded", String(open));
        toggle.querySelector("span").textContent = open ? "Close" : "Menu";
        if (restoreFocus) {
            toggle.focus();
        }
    };

    toggle.addEventListener("click", () => {
        setOpen(toggle.getAttribute("aria-expanded") !== "true");
    });
    nav.addEventListener("click", (event) => {
        if (event.target.closest("a")) {
            setOpen(false);
        }
    });
    document.addEventListener("keydown", (event) => {
        if (event.key === "Escape" && toggle.getAttribute("aria-expanded") === "true") {
            setOpen(false, { restoreFocus: true });
        }
    });
    window.addEventListener("resize", () => {
        if (window.matchMedia("(min-width: 861px)").matches) {
            setOpen(false);
        }
    });
}

function setupCommandTabs() {
    if (!HOME_COMMAND_TABS.length) {
        return;
    }

    const activateTab = (button) => {
        const target = button.dataset.commandTab;
        HOME_COMMAND_TABS.forEach((item) => {
            const selected = item === button;
            item.setAttribute("aria-selected", String(selected));
            item.tabIndex = selected ? 0 : -1;
        });
        HOME_COMMAND_PANELS.forEach((panel) => {
            panel.hidden = panel.dataset.commandPanel !== target;
        });
    };

    HOME_COMMAND_TABS.forEach((button, index) => {
        button.addEventListener("click", () => activateTab(button));
        button.addEventListener("keydown", (event) => {
            if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
                return;
            }
            event.preventDefault();
            let nextIndex = index;
            if (event.key === "ArrowLeft") nextIndex = (index - 1 + HOME_COMMAND_TABS.length) % HOME_COMMAND_TABS.length;
            if (event.key === "ArrowRight") nextIndex = (index + 1) % HOME_COMMAND_TABS.length;
            if (event.key === "Home") nextIndex = 0;
            if (event.key === "End") nextIndex = HOME_COMMAND_TABS.length - 1;
            const next = HOME_COMMAND_TABS[nextIndex];
            activateTab(next);
            next.focus();
        });
    });

    activateTab(HOME_COMMAND_TABS[0]);
}

function setupCopyButtons() {
    if (!COPY_BUTTONS.length) {
        return;
    }

    COPY_BUTTONS.forEach((button) => {
        bindCopyButton(button, () => {
            if (button.dataset.copy) {
                return button.dataset.copy;
            }
            return button.closest("[data-command-panel]")?.querySelector("code")?.textContent || "";
        });
    });
}

function setupPolicyLab() {
    const lab = document.querySelector("[data-policy-lab]");
    if (!lab) {
        return;
    }

    const controls = Array.from(lab.querySelectorAll("[data-capability-control]"));
    const output = lab.querySelector("[data-policy-output]");
    const count = lab.querySelector("[data-policy-count]");
    const meter = lab.querySelector("[data-risk-meter]");
    const label = lab.querySelector("[data-risk-label]");

    const render = () => {
        const selected = controls.filter((control) => control.checked).map((control) => control.value);
        const elevated = selected.filter((permission) => permission.includes("write") || permission.startsWith("shell:"));
        const score = Math.min(100, 18 + selected.length * 11 + elevated.length * 13);
        output.textContent = JSON.stringify(
            { security: { permissions: [{ window: "main", allow: selected }] } },
            null,
            2
        );
        count.textContent = `${selected.length} permission${selected.length === 1 ? "" : "s"}`;
        meter.style.width = `${score}%`;
        meter.style.background = score >= 70 ? "#ff4d00" : score >= 48 ? "#ffd447" : "#c7f65b";
        label.textContent = score >= 70 ? "review broad native access" : score >= 48 ? "moderate local policy" : "narrow local policy";
    };

    controls.forEach((control) => control.addEventListener("change", render));
    render();
}

function setupSchemaLab() {
    const lab = document.querySelector("[data-schema-lab]");
    const tools = globalThis.RustFrameSiteTools;
    if (!lab || !tools) {
        return;
    }

    const input = lab.querySelector("[data-schema-input]");
    const output = lab.querySelector("[data-types-output]");
    const status = lab.querySelector("[data-schema-status]");
    const reset = lab.querySelector("[data-schema-reset]");
    const download = lab.querySelector("[data-starter-download]");
    let example = "";
    let currentSchema = null;
    let renderTimer = null;

    const render = () => {
        status.classList.remove("is-valid", "is-invalid");
        try {
            const schema = JSON.parse(input.value);
            const errors = tools.validateSchema(schema);
            if (errors.length) {
                throw new Error(errors[0]);
            }
            output.textContent = tools.renderTypescript(schema);
            status.textContent = `${schema.tables.length} table${schema.tables.length === 1 ? "" : "s"} · valid schema v${schema.version}`;
            status.classList.add("is-valid");
            download.disabled = false;
            currentSchema = schema;
        } catch (error) {
            output.textContent = `Validation stopped\n\n${String(error.message || error)}`;
            status.textContent = "Fix the highlighted contract";
            status.classList.add("is-invalid");
            download.disabled = true;
            currentSchema = null;
        }
    };

    input.addEventListener("input", () => {
        window.clearTimeout(renderTimer);
        renderTimer = window.setTimeout(render, 120);
    });
    reset.addEventListener("click", () => {
        input.value = example;
        render();
        input.focus();
    });
    download.addEventListener("click", () => {
        if (currentSchema) {
            tools.downloadStarter(currentSchema);
        }
    });

    fetch("examples/schema.json")
        .then((response) => {
            if (!response.ok) throw new Error("Unable to load the verified schema fixture");
            return response.text();
        })
        .then((source) => {
            example = source.trim();
            input.value = example;
            render();
        })
        .catch((error) => {
            status.textContent = String(error.message || error);
            status.classList.add("is-invalid");
        });
}

function setupRevealObserver() {
    if (!REVEAL_ITEMS.length || !("IntersectionObserver" in window)) {
        REVEAL_ITEMS.forEach((item) => item.classList.add("is-visible"));
        return;
    }

    if (window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
        REVEAL_ITEMS.forEach((item) => item.classList.add("is-visible"));
        return;
    }

    document.documentElement.classList.add("has-reveal");

    const observer = new IntersectionObserver(
        (entries) => {
            entries.forEach((entry) => {
                if (!entry.isIntersecting) {
                    return;
                }
                entry.target.classList.add("is-visible");
                observer.unobserve(entry.target);
            });
        },
        { threshold: 0.18 }
    );

    REVEAL_ITEMS.forEach((item) => observer.observe(item));
}

function setupHomePage() {
    const container = document.getElementById("featured-docs");
    if (!container) {
        return;
    }

    const cards = FEATURED_DOC_IDS.map((docId) => renderFeaturedDocCard(docId)).join("");
    container.innerHTML =
        cards +
        `
        <a class="doc-card doc-card-showcase" href="showcase.html">
            <span>Showcase</span>
            <strong>Browse the map</strong>
            <p>See the flagship, starter, reference, and frontend-starter entries in one public surface.</p>
        </a>
        `;
}

function setupDocsPage() {
    const docsContent = document.getElementById("docs-content");
    if (!docsContent) {
        return;
    }

    const searchInput = document.getElementById("docs-search");
    const titleNode = document.getElementById("docs-title");
    const sourceNode = document.getElementById("docs-source");
    const navRoot = document.getElementById("docs-nav");
    const countNode = document.getElementById("docs-count");
    const searchStatus = document.getElementById("docs-search-status");
    const pager = document.getElementById("docs-pager");
    if (navRoot) {
        navRoot.innerHTML = renderDocsNav();
    }
    if (countNode) {
        countNode.textContent = `${Object.keys(DOCS).length} mirrored guides`;
    }
    const navLinks = Array.from(document.querySelectorAll("[data-doc-link]"));
    const searchIndex = new Map(
        Object.entries(DOCS).map(([id, meta]) => [id, `${meta.title} ${meta.summary}`.toLowerCase()])
    );

    const params = new URLSearchParams(window.location.search);
    const requested = params.get("doc");
    const activeId = normalizeDocId(requested);

    const applySearch = () => {
        const query = searchInput?.value.trim().toLowerCase() || "";
        let visible = 0;
        navLinks.forEach((link) => {
            const matches = !query || searchIndex.get(link.dataset.docLink)?.includes(query);
            link.classList.toggle("is-hidden", !matches);
            if (matches) visible += 1;
        });
        Array.from(document.querySelectorAll(".docs-nav-group")).forEach((group) => {
            const visibleLinks = group.querySelectorAll("a:not(.is-hidden)");
            group.classList.toggle("is-hidden", !visibleLinks.length);
        });
        if (searchStatus) {
            searchStatus.textContent = query
                ? `${visible} guide${visible === 1 ? "" : "s"} match “${query}”`
                : `${Object.keys(DOCS).length} guides indexed across full text`;
        }
    };

    searchInput?.addEventListener("input", applySearch);
    void hydrateDocsSearch(searchIndex).then(applySearch);

    async function activateDoc(docId, { pushState = false, scroll = true, anchor = "" } = {}) {
        const resolved = normalizeDocId(docId);
        navLinks.forEach((link) => {
            link.classList.toggle("is-active", link.dataset.docLink === resolved);
        });
        await loadDoc(resolved, docsContent, titleNode, sourceNode);
        if (pager) {
            pager.innerHTML = renderDocsPager(resolved);
        }
        if (pushState) {
            const next = new URL(window.location.href);
            next.searchParams.set("doc", resolved);
            next.hash = anchor ? `#${anchor}` : "";
            window.history.pushState({ doc: resolved }, "", next);
        }
        const requestedAnchor = anchor || (!pushState ? window.location.hash.slice(1) : "");
        if (requestedAnchor) {
            scrollDocsToAnchor(docsContent, requestedAnchor);
        } else if (scroll) {
            scrollDocsToTitle(docsContent);
        }
    }

    navLinks.forEach((link) => {
        link.addEventListener("click", async (event) => {
            event.preventDefault();
            await activateDoc(link.dataset.docLink, { pushState: true, scroll: true });
        });
    });

    docsContent.parentElement.addEventListener("click", async (event) => {
        const link = event.target.closest("a[data-doc-target]");
        if (!link) {
            return;
        }
        event.preventDefault();
        await activateDoc(link.dataset.docTarget, {
            pushState: true,
            scroll: true,
            anchor: link.dataset.docAnchor || "",
        });
    });

    window.addEventListener("popstate", async () => {
        const next = new URLSearchParams(window.location.search).get("doc");
        await activateDoc(next, { pushState: false, scroll: true });
    });

    void activateDoc(activeId, { pushState: false, scroll: false });
}

async function loadDoc(docId, docsContent, titleNode, sourceNode) {
    const meta = DOCS[docId];
    titleNode.textContent = meta.title;
    document.title = `${meta.title} — RustFrame Docs`;
    sourceNode.textContent = meta.source;
    sourceNode.href = `${GITHUB_REPO_BASE}${meta.source}`;

    try {
        const response = await fetch(meta.path);
        if (!response.ok) {
            throw new Error(`Failed to load ${meta.path}`);
        }

        const markdown = await response.text();
        docsContent.innerHTML = renderMarkdown(markdown);
        enhanceDocsCodeBlocks(docsContent);
    } catch (error) {
        docsContent.innerHTML = `<p>Unable to load the selected doc.</p><pre>${escapeHtml(String(error))}</pre>`;
        enhanceDocsCodeBlocks(docsContent);
    }
}

async function hydrateDocsSearch(searchIndex) {
    await Promise.all(
        Object.entries(DOCS).map(async ([id, meta]) => {
            try {
                const response = await fetch(meta.path);
                if (!response.ok) return;
                const markdown = await response.text();
                searchIndex.set(id, `${searchIndex.get(id)} ${markdown}`.toLowerCase());
            } catch {
                // Metadata search remains available if a mirrored guide cannot be indexed.
            }
        })
    );
}

function setupShowcasePage() {
    const grid = document.getElementById("showcase-grid");
    if (!grid) {
        return;
    }

    loadShowcase(grid);
}

async function loadShowcase(grid) {
    try {
        const response = await fetch("showcase.json");
        if (!response.ok) {
            throw new Error(`Failed to load showcase.json`);
        }

        const payload = await response.json();
        const items = Array.isArray(payload.templates) ? payload.templates : [];
        if (!items.length) {
            grid.innerHTML = `<article class="showcase-empty">No showcase entries have been published yet.</article>`;
            return;
        }

        grid.innerHTML = items.map(renderShowcaseCard).join("");
    } catch (error) {
        grid.innerHTML = `<article class="showcase-empty">Unable to load the showcase.<pre>${escapeHtml(String(error))}</pre></article>`;
    }
}

function renderShowcaseCard(item) {
    const title = escapeHtml(item.title || "Untitled");
    const category = escapeHtml(item.category || "reference");
    const source = escapeHtml(item.source || "");
    const summary = escapeHtml(item.summary || "");
    const href = escapeHtml(item.href || "#");
    const bestFor = Array.isArray(item.bestFor) ? item.bestFor : [];
    const capabilities = Array.isArray(item.capabilities) ? item.capabilities : [];
    const visual = item.screenshot
        ? `<div class="showcase-visual"><img src="${escapeHtml(item.screenshot)}" width="${Number(item.width) || 1460}" height="${Number(item.height) || 940}" loading="lazy" alt="${title} screenshot"></div>`
        : `<div class="showcase-visual showcase-placeholder"><span>${category}</span></div>`;
    const bestForHtml = bestFor.length
        ? `<p class="showcase-meta"><strong>Best for</strong> ${escapeHtml(bestFor.join(", "))}</p>`
        : "";
    const capabilityTags = capabilities.length
        ? `<div class="showcase-tags">${capabilities
              .map((value) => `<span class="showcase-tag">${escapeHtml(value)}</span>`)
              .join("")}</div>`
        : "";

    return `
        <article class="showcase-card">
            ${visual}
            <div class="showcase-copy">
                <span class="gallery-kicker">${category}</span>
                <h3>${title}</h3>
                <p>${summary}</p>
                <p class="showcase-meta"><strong>Source</strong> <code>${source}</code></p>
                ${bestForHtml}
                ${capabilityTags}
                <div class="showcase-actions">
                    <a class="button button-ghost" href="${href}" target="_blank" rel="noreferrer">Open source</a>
                </div>
            </div>
        </article>
    `;
}

function renderFeaturedDocCard(docId) {
    const meta = DOCS[docId];
    if (!meta) {
        return "";
    }

    return `
        <a class="doc-card" href="docs.html?doc=${docId}">
            <span>${escapeHtml(navSectionLabel(meta.section))}</span>
            <strong>${escapeHtml(meta.title)}</strong>
            <p>${escapeHtml(meta.summary)}</p>
        </a>
    `;
}

function renderDocsNav() {
    const sections = [
        { id: "core", label: "Start here" },
        { id: "reference", label: "Generated reference" },
        { id: "operations", label: "Ship and operate" },
        { id: "ecosystem", label: "Patterns and examples" },
    ];

    return sections
        .map(({ id, label }) => {
            const items = Object.entries(DOCS).filter(([, meta]) => meta.section === id);
            if (!items.length) {
                return "";
            }

            const links = items
                .map(
                    ([docId, meta]) =>
                        `<a href="docs.html?doc=${docId}" data-doc-link="${docId}">${escapeHtml(
                            meta.navLabel || meta.title
                        )}</a>`
                )
                .join("");

            return `
                <section class="docs-nav-group">
                    <p class="docs-nav-label">${escapeHtml(label)}</p>
                    <div class="docs-nav-links">
                        ${links}
                    </div>
                </section>
            `;
        })
        .join("");
}

function renderDocsPager(activeId) {
    const ids = Object.keys(DOCS);
    const index = ids.indexOf(activeId);
    const previous = index > 0 ? ids[index - 1] : null;
    const next = index >= 0 && index < ids.length - 1 ? ids[index + 1] : null;
    return `
        ${previous ? `<a href="docs.html?doc=${previous}" data-doc-target="${previous}"><span>Previous</span><strong>← ${escapeHtml(DOCS[previous].title)}</strong></a>` : "<span></span>"}
        ${next ? `<a href="docs.html?doc=${next}" data-doc-target="${next}"><span>Next</span><strong>${escapeHtml(DOCS[next].title)} →</strong></a>` : "<span></span>"}
    `;
}

function enhanceDocsCodeBlocks(container) {
    const blocks = container.querySelectorAll("pre");
    blocks.forEach((block) => {
        const code = block.querySelector("code");
        if (!code || block.querySelector(".docs-copy-button")) {
            return;
        }

        block.classList.add("has-copy-button");

        const button = document.createElement("button");
        button.type = "button";
        button.className = "docs-copy-button";
        button.textContent = "Copy";
        button.setAttribute("aria-label", "Copy code to clipboard");
        block.append(button);

        bindCopyButton(button, () => code.textContent || "");
    });
}

function bindCopyButton(button, getText) {
    button.addEventListener("click", async () => {
        const original = button.textContent;
        try {
            await navigator.clipboard.writeText(getText());
            button.textContent = "Copied";
        } catch {
            button.textContent = "Copy failed";
        }
        window.setTimeout(() => {
            button.textContent = original;
        }, 1200);
    });
}

function scrollDocsToTitle(docsContent) {
    const reducedMotion =
        window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const header = document.querySelector(".docs-topbar");
    const headerStyle = header ? window.getComputedStyle(header) : null;
    const headerOffset =
        header && headerStyle && headerStyle.position !== "static" ? header.getBoundingClientRect().height : 0;
    const target = docsContent.querySelector("h1, h2, p") || docsContent;
    const targetTop = window.scrollY + target.getBoundingClientRect().top - headerOffset - 20;

    window.scrollTo({
        top: Math.max(0, targetTop),
        behavior: reducedMotion ? "auto" : "smooth",
    });
}

function scrollDocsToAnchor(docsContent, anchor) {
    const target = docsContent.querySelector(`#${CSS.escape(anchor)}`);
    if (!target) {
        scrollDocsToTitle(docsContent);
        return;
    }
    const reducedMotion =
        window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const header = document.querySelector(".docs-topbar");
    const offset = header ? header.getBoundingClientRect().height + 20 : 20;
    window.scrollTo({
        top: Math.max(0, window.scrollY + target.getBoundingClientRect().top - offset),
        behavior: reducedMotion ? "auto" : "smooth",
    });
}

function renderMarkdown(markdown) {
    const lines = markdown.replace(/\r\n/g, "\n").split("\n");
    const html = [];
    let paragraph = [];
    let listType = null;
    let inCode = false;
    let codeLines = [];

    function flushParagraph() {
        if (!paragraph.length) {
            return;
        }
        html.push(`<p>${renderInline(paragraph.join(" "))}</p>`);
        paragraph = [];
    }

    function flushList() {
        if (!listType) {
            return;
        }
        html.push(`</${listType}>`);
        listType = null;
    }

    function flushTable(table) {
        if (!table) {
            return;
        }

        const rows = table
            .map((line) =>
                line
                    .trim()
                    .replace(/^\|/, "")
                    .replace(/\|$/, "")
                    .split("|")
                    .map((cell) => cell.trim())
            )
            .filter((row) => row.length);

        if (rows.length < 2) {
            rows.forEach((row) => {
                html.push(`<p>${renderInline(row.join(" | "))}</p>`);
            });
            return;
        }

        const [header, _separator, ...body] = rows;
        html.push("<div class=\"docs-table-wrap\"><table><thead><tr>");
        header.forEach((cell) => {
            html.push(`<th>${renderInline(cell)}</th>`);
        });
        html.push("</tr></thead><tbody>");
        body.forEach((row) => {
            html.push("<tr>");
            row.forEach((cell) => {
                html.push(`<td>${renderInline(cell)}</td>`);
            });
            html.push("</tr>");
        });
        html.push("</tbody></table></div>");
    }

    function flushCode() {
        if (!inCode) {
            return;
        }
        html.push(`<pre><code>${escapeHtml(codeLines.join("\n"))}</code></pre>`);
        inCode = false;
        codeLines = [];
    }

    for (let index = 0; index < lines.length; index += 1) {
        const line = lines[index];

        if (line.startsWith("```")) {
            flushParagraph();
            flushList();
            if (inCode) {
                flushCode();
            } else {
                inCode = true;
            }
            continue;
        }

        if (inCode) {
            codeLines.push(line);
            continue;
        }

        if (!line.trim()) {
            flushParagraph();
            flushList();
            continue;
        }

        const heading = line.match(/^(#{1,3})\s+(.*)$/);
        if (heading) {
            flushParagraph();
            flushList();
            const level = Math.min(heading[1].length + 1, 4);
            const text = heading[2].trim();
            const id = slugifyHeading(text);
            html.push(`<h${level} id="${id}">${renderInline(text)}</h${level}>`);
            continue;
        }

        if (
            line.trim().startsWith("|") &&
            index + 1 < lines.length &&
            /^\s*\|(?:\s*:?-{3,}:?\s*\|)+\s*$/.test(lines[index + 1])
        ) {
            flushParagraph();
            flushList();

            const tableLines = [line, lines[index + 1]];
            index += 2;
            while (index < lines.length && lines[index].trim().startsWith("|")) {
                tableLines.push(lines[index]);
                index += 1;
            }
            index -= 1;

            flushTable(tableLines);
            continue;
        }

        if (line.trim().startsWith(">")) {
            flushParagraph();
            flushList();

            const quoteLines = [];
            while (index < lines.length && lines[index].trim().startsWith(">")) {
                quoteLines.push(lines[index].replace(/^\s*>\s?/, "").trim());
                index += 1;
            }
            index -= 1;

            html.push(`<blockquote><p>${renderInline(quoteLines.join(" "))}</p></blockquote>`);
            continue;
        }

        const bullet = line.match(/^-\s+(.*)$/);
        if (bullet) {
            flushParagraph();
            if (listType !== "ul") {
                flushList();
                listType = "ul";
                html.push("<ul>");
            }
            html.push(`<li>${renderInline(bullet[1])}</li>`);
            continue;
        }

        const numbered = line.match(/^\d+\.\s+(.*)$/);
        if (numbered) {
            flushParagraph();
            if (listType !== "ol") {
                flushList();
                listType = "ol";
                html.push("<ol>");
            }
            html.push(`<li>${renderInline(numbered[1])}</li>`);
            continue;
        }

        paragraph.push(line.trim());
    }

    flushParagraph();
    flushList();
    flushCode();
    return html.join("");
}

function renderInline(text) {
    let value = escapeHtml(text);
    value = value.replace(/`([^`]+)`/g, "<code>$1</code>");
    value = value.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
    value = value.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_match, label, href) => {
        const target = resolveInternalDocHref(href);
        if (target) {
            const anchor = target.anchor ? `#${escapeHtml(target.anchor)}` : "";
            const anchorData = target.anchor ? ` data-doc-anchor="${escapeHtml(target.anchor)}"` : "";
            return `<a href="docs.html?doc=${target.id}${anchor}" data-doc-target="${target.id}"${anchorData}>${label}</a>`;
        }

        if (isExternalHref(href)) {
            return `<a href="${escapeHtml(href)}" target="_blank" rel="noreferrer">${label}</a>`;
        }

        return `<a href="${escapeHtml(href)}">${label}</a>`;
    });
    return value;
}

function normalizeDocId(value) {
    if (!value) {
        return "readme";
    }

    const normalized = normalizeDocSlug(value);
    return DOC_SOURCE_TO_ID.get(normalized) || (DOCS[normalized] ? normalized : "readme");
}

function navSectionLabel(section) {
    if (section === "reference") {
        return "Reference";
    }
    if (section === "operations") {
        return "Operations";
    }
    if (section === "ecosystem") {
        return "Patterns";
    }
    return "Guide";
}

function normalizeDocSlug(value) {
    return String(value || "")
        .split("#")[0]
        .split("/")
        .pop()
        .replace(/\.md$/i, "")
        .replace(/_/g, "-")
        .toLowerCase();
}

function resolveInternalDocHref(href) {
    if (isExternalHref(href) || href.startsWith("#") || !/\.md(?:#.*)?$/i.test(href)) {
        return null;
    }

    const normalized = normalizeDocSlug(href);
    const id = DOC_SOURCE_TO_ID.get(normalized) || null;
    if (!id) {
        return null;
    }
    return { id, anchor: href.split("#")[1] || "" };
}

function isExternalHref(href) {
    return /^[a-z]+:\/\//i.test(href) || /^(mailto|tel):/i.test(href);
}

function slugifyHeading(value) {
    return String(value || "")
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "");
}

function escapeHtml(value) {
    return String(value)
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;");
}
