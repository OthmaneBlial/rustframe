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

setupCommandTabs();
setupCopyButtons();
setupRevealObserver();
setupHomePage();
setupDocsPage();
setupShowcasePage();

function setupCommandTabs() {
    if (!HOME_COMMAND_TABS.length) {
        return;
    }

    HOME_COMMAND_TABS.forEach((button) => {
        button.addEventListener("click", () => {
            const target = button.dataset.commandTab;
            HOME_COMMAND_TABS.forEach((item) => {
                item.classList.toggle("is-active", item === button);
            });
            HOME_COMMAND_PANELS.forEach((panel) => {
                panel.classList.toggle("is-hidden", panel.dataset.commandPanel !== target);
            });
        });
    });
}

function setupCopyButtons() {
    if (!COPY_BUTTONS.length) {
        return;
    }

    COPY_BUTTONS.forEach((button) => {
        bindCopyButton(button, () => button.dataset.copy || "");
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
    if (navRoot) {
        navRoot.innerHTML = renderDocsNav();
    }
    if (countNode) {
        countNode.textContent = `${Object.keys(DOCS).length} mirrored guides`;
    }
    const navLinks = Array.from(document.querySelectorAll("[data-doc-link]"));

    const params = new URLSearchParams(window.location.search);
    const requested = params.get("doc");
    const activeId = normalizeDocId(requested);

    if (searchInput) {
        searchInput.addEventListener("input", () => {
            const query = searchInput.value.trim().toLowerCase();
            navLinks.forEach((link) => {
                const label = link.textContent.toLowerCase();
                link.classList.toggle("is-hidden", Boolean(query) && !label.includes(query));
            });
            Array.from(document.querySelectorAll(".docs-nav-group")).forEach((group) => {
                const visibleLinks = group.querySelectorAll("a:not(.is-hidden)");
                group.classList.toggle("is-hidden", !visibleLinks.length);
            });
        });
    }

    function activateDoc(docId, { pushState = false } = {}) {
        const resolved = normalizeDocId(docId);
        navLinks.forEach((link) => {
            link.classList.toggle("is-active", link.dataset.docLink === resolved);
        });
        loadDoc(resolved, docsContent, titleNode, sourceNode);
        if (pushState) {
            const next = new URL(window.location.href);
            next.searchParams.set("doc", resolved);
            window.history.pushState({ doc: resolved }, "", next);
        }
        const reducedMotion =
            window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
        docsContent.scrollIntoView({ behavior: reducedMotion ? "auto" : "smooth", block: "start" });
    }

    navLinks.forEach((link) => {
        link.addEventListener("click", (event) => {
            event.preventDefault();
            activateDoc(link.dataset.docLink, { pushState: true });
        });
    });

    docsContent.addEventListener("click", (event) => {
        const link = event.target.closest("a[data-doc-target]");
        if (!link) {
            return;
        }
        event.preventDefault();
        activateDoc(link.dataset.docTarget, { pushState: true });
    });

    window.addEventListener("popstate", () => {
        const next = new URLSearchParams(window.location.search).get("doc");
        activateDoc(next, { pushState: false });
    });

    activateDoc(activeId, { pushState: false });
}

async function loadDoc(docId, docsContent, titleNode, sourceNode) {
    const meta = DOCS[docId];
    titleNode.textContent = meta.title;
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
        ? `<div class="showcase-visual"><img src="${escapeHtml(item.screenshot)}" alt="${title} screenshot"></div>`
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
            const level = heading[1].length;
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
            return `<a href="docs.html?doc=${target}" data-doc-target="${target}">${label}</a>`;
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
    return DOC_SOURCE_TO_ID.get(normalized) || null;
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
