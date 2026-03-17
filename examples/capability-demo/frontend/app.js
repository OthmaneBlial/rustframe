const output = document.getElementById("output");
const assetMode = document.getElementById("asset-mode");
const originLabel = document.getElementById("origin-label");
const titleInput = document.getElementById("title-input");

assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
originLabel.textContent = window.location.origin || `${window.location.protocol}//`;

function writeBlock(title, body) {
    output.textContent = `${title}\n${"-".repeat(title.length)}\n${body}`;
}

function formatError(error) {
    if (error && typeof error === "object") {
        return `${error.code ?? "error"}: ${error.message ?? JSON.stringify(error)}`;
    }

    return String(error);
}

async function handleSetTitle() {
    const nextTitle = titleInput.value.trim() || "RustFrame Capability Demo";
    await window.RustFrame.window.setTitle(nextTitle);
    writeBlock("Window updated", `Title set to "${nextTitle}"`);
}

async function handleMinimize() {
    await window.RustFrame.window.minimize();
    writeBlock("Window updated", "Minimize request sent.");
}

async function handleMaximize() {
    await window.RustFrame.window.maximize();
    writeBlock("Window updated", "Maximize request sent.");
}

async function handleReadApp() {
    const content = await window.RustFrame.fs.readText("app.js");
    writeBlock("frontend/app.js", content.slice(0, 1200));
}

async function handleListFrontend() {
    const result = await window.RustFrame.shell.exec("listFrontend");
    writeBlock(
        "listFrontend",
        `exitCode: ${result.exitCode}\n\nstdout:\n${result.stdout || "(empty)"}\n\nstderr:\n${result.stderr || "(empty)"}`
    );
}

const actions = {
    "set-title": handleSetTitle,
    minimize: handleMinimize,
    maximize: handleMaximize,
    "read-app": handleReadApp,
    "list-frontend": handleListFrontend
};

document.querySelectorAll("[data-action]").forEach((button) => {
    button.addEventListener("click", async () => {
        const action = actions[button.dataset.action];
        if (!action) {
            return;
        }

        button.disabled = true;
        try {
            await action();
        } catch (error) {
            writeBlock("RustFrame error", formatError(error));
        } finally {
            button.disabled = false;
        }
    });
});
