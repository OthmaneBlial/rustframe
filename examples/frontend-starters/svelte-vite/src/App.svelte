<script>
  let title = "RustFrame Svelte Starter";
  let runtimeLabel = "Checking…";
  let security = "-";
  let databasePath = "-";
  let log = "Waiting for RustFrame…";

  function formatError(error) {
    if (error && typeof error === "object") {
      return `${error.code ?? "error"}: ${error.message ?? JSON.stringify(error)}`;
    }

    return String(error);
  }

  async function refreshRuntime() {
    if (!window.RustFrame) {
      runtimeLabel = "Browser only";
      security = "Unavailable";
      databasePath = "Unavailable";
      log = "Open this Svelte app inside RustFrame to use runtime APIs.";
      return;
    }

    const info = await window.RustFrame.db.info();
    runtimeLabel = "Connected";
    security = window.RustFrame.security.model;
    databasePath = info.databasePath;
    log =
      `Window: ${window.RustFrame.security.currentWindow.id}\n` +
      `Schema version: ${info.schemaVersion}\n` +
      `Tables: ${info.tables.join(", ")}`;
  }

  async function renameWindow() {
    if (!window.RustFrame) {
      log = "Window controls are only available inside RustFrame.";
      return;
    }

    try {
      await window.RustFrame.window.setTitle(title.trim() || "RustFrame Svelte Starter");
      log = `Window title updated to "${title.trim() || "RustFrame Svelte Starter"}".`;
    } catch (error) {
      log = formatError(error);
    }
  }

  refreshRuntime().catch((error) => {
    log = formatError(error);
  });
</script>

<main class="starter-shell">
  <section class="starter-card">
    <p class="eyebrow">Svelte Vite</p>
    <h1>RustFrame Dev Server Starter</h1>
    <p class="lede">
      Pair this frontend with a RustFrame app directory. Svelte owns the UI layer.
      RustFrame still owns the window, SQLite, packaging, and capability boundaries.
    </p>
    <div class="meta-grid">
      <div>
        <span>Runtime</span>
        <strong>{runtimeLabel}</strong>
      </div>
      <div>
        <span>Security</span>
        <strong>{security}</strong>
      </div>
      <div>
        <span>Database</span>
        <strong>{databasePath}</strong>
      </div>
    </div>
    <label class="field">
      <span>Window title</span>
      <input bind:value={title}>
    </label>
    <div class="actions">
      <button type="button" on:click={renameWindow}>Set title</button>
      <button type="button" class="ghost" on:click={() => refreshRuntime().catch((error) => {
        log = formatError(error);
      })}>Refresh runtime info</button>
    </div>
    <pre>{log}</pre>
  </section>
</main>
