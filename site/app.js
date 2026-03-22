const HOME_COMMAND_TABS = document.querySelectorAll("[data-command-tab]");
const HOME_COMMAND_PANELS = document.querySelectorAll("[data-command-panel]");
const COPY_BUTTONS = document.querySelectorAll("[data-copy]");
const REVEAL_ITEMS = document.querySelectorAll(".reveal");

const DOCS = {
    readme: {
        title: "Docs index",
        source: "docs/README.md",
        path: "docs/README.md",
    },
    "getting-started": {
        title: "Getting started",
        source: "docs/getting-started.md",
        path: "docs/getting-started.md",
    },
    "choosing-rustframe": {
        title: "Choosing RustFrame",
        source: "docs/choosing-rustframe.md",
        path: "docs/choosing-rustframe.md",
    },
    "architecture-overview": {
        title: "Architecture overview",
        source: "docs/architecture-overview.md",
        path: "docs/architecture-overview.md",
    },
    "runtime-and-capabilities": {
        title: "Runtime and capabilities",
        source: "docs/runtime-and-capabilities.md",
        path: "docs/runtime-and-capabilities.md",
    },
    "build-in-20-minutes": {
        title: "Build in 20 minutes",
        source: "docs/build-in-20-minutes.md",
        path: "docs/build-in-20-minutes.md",
    },
    cookbook: {
        title: "Cookbook",
        source: "docs/cookbook.md",
        path: "docs/cookbook.md",
    },
    "threat-model": {
        title: "Threat model",
        source: "docs/threat-model.md",
        path: "docs/threat-model.md",
    },
    "migrations-and-versioning": {
        title: "Migrations and versioning",
        source: "docs/migrations-and-versioning.md",
        path: "docs/migrations-and-versioning.md",
    },
    "platform-support": {
        title: "Platform support",
        source: "docs/platform-support.md",
        path: "docs/platform-support.md",
    },
    "signing-and-notarization": {
        title: "Signing and notarization",
        source: "docs/signing-and-notarization.md",
        path: "docs/signing-and-notarization.md",
    },
    "update-strategy": {
        title: "Update strategy",
        source: "docs/update-strategy.md",
        path: "docs/update-strategy.md",
    },
    "release-checklist": {
        title: "Release checklist",
        source: "docs/release-checklist.md",
        path: "docs/release-checklist.md",
    },
    "community-templates": {
        title: "Community templates",
        source: "docs/community-templates.md",
        path: "docs/community-templates.md",
    },
    "remote-sync-patterns": {
        title: "Remote sync patterns",
        source: "docs/remote-sync-patterns.md",
        path: "docs/remote-sync-patterns.md",
    },
    "capability-extension-patterns": {
        title: "Capability extension patterns",
        source: "docs/capability-extension-patterns.md",
        path: "docs/capability-extension-patterns.md",
    },
    "frontend-app-rules": {
        title: "Frontend app rules",
        source: "docs/frontend-app-rules.md",
        path: "docs/frontend-app-rules.md",
    },
    "example-apps": {
        title: "Example apps",
        source: "docs/example-apps.md",
        path: "docs/example-apps.md",
    },
};

setupCommandTabs();
setupCopyButtons();
setupRevealObserver();
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

function setupDocsPage() {
    const docsContent = document.getElementById("docs-content");
    if (!docsContent) {
        return;
    }

    const searchInput = document.getElementById("docs-search");
    const titleNode = document.getElementById("docs-title");
    const sourceNode = document.getElementById("docs-source");
    const navLinks = Array.from(document.querySelectorAll("[data-doc-link]"));

    const params = new URLSearchParams(window.location.search);
    const requested = params.get("doc");
    const activeId = DOCS[requested] ? requested : "readme";

    if (searchInput) {
        searchInput.addEventListener("input", () => {
            const query = searchInput.value.trim().toLowerCase();
            navLinks.forEach((link) => {
                const label = link.textContent.toLowerCase();
                link.classList.toggle("is-hidden", Boolean(query) && !label.includes(query));
            });
        });
    }

    navLinks.forEach((link) => {
        link.classList.toggle("is-active", link.dataset.docLink === activeId);
    });

    loadDoc(activeId, docsContent, titleNode, sourceNode);
}

async function loadDoc(docId, docsContent, titleNode, sourceNode) {
    const meta = DOCS[docId];
    titleNode.textContent = meta.title;
    sourceNode.textContent = meta.source;

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

    function flushCode() {
        if (!inCode) {
            return;
        }
        html.push(`<pre><code>${escapeHtml(codeLines.join("\n"))}</code></pre>`);
        inCode = false;
        codeLines = [];
    }

    lines.forEach((line) => {
        if (line.startsWith("```")) {
            flushParagraph();
            flushList();
            if (inCode) {
                flushCode();
            } else {
                inCode = true;
            }
            return;
        }

        if (inCode) {
            codeLines.push(line);
            return;
        }

        if (!line.trim()) {
            flushParagraph();
            flushList();
            return;
        }

        const heading = line.match(/^(#{1,3})\s+(.*)$/);
        if (heading) {
            flushParagraph();
            flushList();
            const level = heading[1].length;
            html.push(`<h${level}>${renderInline(heading[2])}</h${level}>`);
            return;
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
            return;
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
            return;
        }

        paragraph.push(line.trim());
    });

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
        if (href.endsWith(".md")) {
            const slug = href.split("/").pop().replace(/\.md$/, "");
            const target = slug === "README" ? "readme" : slug;
            return `<a href="docs.html?doc=${target}">${label}</a>`;
        }
        return `<a href="${href}">${label}</a>`;
    });
    return value;
}

function escapeHtml(value) {
    return String(value)
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;");
}
