const assetMode = document.getElementById("asset-mode");
const originLabel = document.getElementById("origin-label");
const titleInput = document.getElementById("title-input");
const renameButton = document.getElementById("rename-button");
const closeButton = document.getElementById("close-button");
const logOutput = document.getElementById("log-output");

assetMode.textContent = window.location.protocol.startsWith("http") ? "Dev server" : "Embedded";
originLabel.textContent = window.location.origin || `${window.location.protocol}//`;

function log(message) {
    logOutput.textContent = message;
}

function formatError(error) {
    if (error && typeof error === "object") {
        return `${error.code ?? "error"}: ${error.message ?? JSON.stringify(error)}`;
    }

    return String(error);
}

renameButton.addEventListener("click", async () => {
    const nextTitle = titleInput.value.trim() || "Hello Rustframe";
    renameButton.disabled = true;

    try {
        await window.RustFrame.window.setTitle(nextTitle);
        log(`Window title updated to "${nextTitle}".\n\nStart replacing this starter UI with your app.`);
    } catch (error) {
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    } finally {
        renameButton.disabled = false;
    }
});

closeButton.addEventListener("click", async () => {
    closeButton.disabled = true;

    try {
        await window.RustFrame.window.close();
    } catch (error) {
        closeButton.disabled = false;
        log(`RustFrame error\n---------------\n${formatError(error)}`);
    }
});
