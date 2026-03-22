import "./styles.css";

const app = document.getElementById("app");

app.innerHTML = `
  <main class="starter-shell">
    <section class="starter-card">
      <p class="eyebrow">Vite Vanilla</p>
      <h1>RustFrame Dev Server Starter</h1>
      <p class="lede">
        Pair this frontend with a RustFrame app directory. The runtime still owns the window,
        SQLite, packaging, and any scoped native capabilities.
      </p>
      <div class="meta-grid">
        <div><span>Runtime</span><strong id="runtime-status">Checking…</strong></div>
        <div><span>Security</span><strong id="security-model">-</strong></div>
        <div><span>Database</span><strong id="database-path">-</strong></div>
      </div>
      <label class="field">
        <span>Window title</span>
        <input id="title-input" type="text" value="RustFrame Vite Starter">
      </label>
      <div class="actions">
        <button id="rename-button" type="button">Set title</button>
        <button id="refresh-button" type="button" class="ghost">Refresh runtime info</button>
      </div>
      <pre id="log-output">Waiting for RustFrame…</pre>
    </section>
  </main>
`;

const runtimeStatus = document.getElementById("runtime-status");
const securityModel = document.getElementById("security-model");
const databasePath = document.getElementById("database-path");
const titleInput = document.getElementById("title-input");
const renameButton = document.getElementById("rename-button");
const refreshButton = document.getElementById("refresh-button");
const logOutput = document.getElementById("log-output");

function runtime() {
  return window.RustFrame ?? null;
}

function formatError(error) {
  if (error && typeof error === "object") {
    return `${error.code ?? "error"}: ${error.message ?? JSON.stringify(error)}`;
  }

  return String(error);
}

async function refreshRuntime() {
  const bridge = runtime();
  if (!bridge) {
    runtimeStatus.textContent = "Browser only";
    securityModel.textContent = "Unavailable";
    databasePath.textContent = "Unavailable";
    logOutput.textContent = "Open this Vite app inside RustFrame to use the runtime bridge.";
    return;
  }

  const info = await bridge.db.info();
  runtimeStatus.textContent = "Connected";
  securityModel.textContent = bridge.security.model;
  databasePath.textContent = info.databasePath;
  logOutput.textContent =
    `Window: ${bridge.security.currentWindow.id}\n` +
    `Schema version: ${info.schemaVersion}\n` +
    `Tables: ${info.tables.join(", ")}`;
}

renameButton.addEventListener("click", async () => {
  const bridge = runtime();
  if (!bridge) {
    logOutput.textContent = "Window controls are only available inside RustFrame.";
    return;
  }

  try {
    await bridge.window.setTitle(titleInput.value.trim() || "RustFrame Vite Starter");
    logOutput.textContent = `Window title updated to "${titleInput.value.trim() || "RustFrame Vite Starter"}".`;
  } catch (error) {
    logOutput.textContent = formatError(error);
  }
});

refreshButton.addEventListener("click", () => {
  refreshRuntime().catch((error) => {
    logOutput.textContent = formatError(error);
  });
});

refreshRuntime().catch((error) => {
  logOutput.textContent = formatError(error);
});
