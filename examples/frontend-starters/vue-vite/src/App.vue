<script setup>
import { onMounted, ref } from "vue";

const title = ref("RustFrame Vue Starter");
const runtimeLabel = ref("Checking…");
const security = ref("-");
const databasePath = ref("-");
const log = ref("Waiting for RustFrame…");

function formatError(error) {
  if (error && typeof error === "object") {
    return `${error.code ?? "error"}: ${error.message ?? JSON.stringify(error)}`;
  }

  return String(error);
}

async function refreshRuntime() {
  if (!window.RustFrame) {
    runtimeLabel.value = "Browser only";
    security.value = "Unavailable";
    databasePath.value = "Unavailable";
    log.value = "Open this Vue app inside RustFrame to use runtime APIs.";
    return;
  }

  const info = await window.RustFrame.db.info();
  runtimeLabel.value = "Connected";
  security.value = window.RustFrame.security.model;
  databasePath.value = info.databasePath;
  log.value =
    `Window: ${window.RustFrame.security.currentWindow.id}\n` +
    `Schema version: ${info.schemaVersion}\n` +
    `Tables: ${info.tables.join(", ")}`;
}

async function renameWindow() {
  if (!window.RustFrame) {
    log.value = "Window controls are only available inside RustFrame.";
    return;
  }

  try {
    await window.RustFrame.window.setTitle(title.value.trim() || "RustFrame Vue Starter");
    log.value = `Window title updated to "${title.value.trim() || "RustFrame Vue Starter"}".`;
  } catch (error) {
    log.value = formatError(error);
  }
}

onMounted(() => {
  refreshRuntime().catch((error) => {
    log.value = formatError(error);
  });
});
</script>

<template>
  <main class="starter-shell">
    <section class="starter-card">
      <p class="eyebrow">Vue Vite</p>
      <h1>RustFrame Dev Server Starter</h1>
      <p class="lede">
        Pair this frontend with a RustFrame app directory. Vue owns the component layer.
        RustFrame still owns the window, SQLite, packaging, and capability boundaries.
      </p>
      <div class="meta-grid">
        <div>
          <span>Runtime</span>
          <strong>{{ runtimeLabel }}</strong>
        </div>
        <div>
          <span>Security</span>
          <strong>{{ security }}</strong>
        </div>
        <div>
          <span>Database</span>
          <strong>{{ databasePath }}</strong>
        </div>
      </div>
      <label class="field">
        <span>Window title</span>
        <input v-model="title">
      </label>
      <div class="actions">
        <button type="button" @click="renameWindow">Set title</button>
        <button type="button" class="ghost" @click="refreshRuntime">Refresh runtime info</button>
      </div>
      <pre>{{ log }}</pre>
    </section>
  </main>
</template>
