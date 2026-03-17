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
    "runtime-and-capabilities": {
        title: "Runtime and capabilities",
        source: "docs/runtime-and-capabilities.md",
        path: "docs/runtime-and-capabilities.md",
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
        button.addEventListener("click", async () => {
            const original = button.textContent;
            try {
                await navigator.clipboard.writeText(button.dataset.copy || "");
                button.textContent = "Copied";
            } catch {
                button.textContent = "Copy failed";
            }
            window.setTimeout(() => {
                button.textContent = original;
            }, 1200);
        });
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
    } catch (error) {
        docsContent.innerHTML = `<p>Unable to load the selected doc.</p><pre>${escapeHtml(String(error))}</pre>`;
    }
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
