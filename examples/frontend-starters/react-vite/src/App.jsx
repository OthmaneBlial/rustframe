import { useEffect, useState } from "react";

const defaultState = {
  runtimeLabel: "Checking…",
  security: "-",
  databasePath: "-",
  log: "Waiting for RustFrame…"
};

function formatError(error) {
  if (error && typeof error === "object") {
    return `${error.code ?? "error"}: ${error.message ?? JSON.stringify(error)}`;
  }

  return String(error);
}

export default function App() {
  const [title, setTitle] = useState("RustFrame React Starter");
  const [state, setState] = useState(defaultState);

  async function refreshRuntime() {
    if (!window.RustFrame) {
      setState({
        runtimeLabel: "Browser only",
        security: "Unavailable",
        databasePath: "Unavailable",
        log: "Open this React app inside RustFrame to use runtime APIs."
      });
      return;
    }

    const info = await window.RustFrame.db.info();
    setState({
      runtimeLabel: "Connected",
      security: window.RustFrame.security.model,
      databasePath: info.databasePath,
      log:
        `Window: ${window.RustFrame.security.currentWindow.id}\n` +
        `Schema version: ${info.schemaVersion}\n` +
        `Tables: ${info.tables.join(", ")}`
    });
  }

  async function renameWindow() {
    if (!window.RustFrame) {
      setState((current) => ({
        ...current,
        log: "Window controls are only available inside RustFrame."
      }));
      return;
    }

    try {
      await window.RustFrame.window.setTitle(title.trim() || "RustFrame React Starter");
      setState((current) => ({
        ...current,
        log: `Window title updated to "${title.trim() || "RustFrame React Starter"}".`
      }));
    } catch (error) {
      setState((current) => ({
        ...current,
        log: formatError(error)
      }));
    }
  }

  useEffect(() => {
    refreshRuntime().catch((error) => {
      setState((current) => ({
        ...current,
        log: formatError(error)
      }));
    });
  }, []);

  return (
    <main className="starter-shell">
      <section className="starter-card">
        <p className="eyebrow">React Vite</p>
        <h1>RustFrame Dev Server Starter</h1>
        <p className="lede">
          Pair this frontend with a RustFrame app directory. React owns the UI. RustFrame still
          owns the window, SQLite, packaging, and native capability boundaries.
        </p>
        <div className="meta-grid">
          <div>
            <span>Runtime</span>
            <strong>{state.runtimeLabel}</strong>
          </div>
          <div>
            <span>Security</span>
            <strong>{state.security}</strong>
          </div>
          <div>
            <span>Database</span>
            <strong>{state.databasePath}</strong>
          </div>
        </div>
        <label className="field">
          <span>Window title</span>
          <input value={title} onChange={(event) => setTitle(event.target.value)} />
        </label>
        <div className="actions">
          <button type="button" onClick={renameWindow}>Set title</button>
          <button type="button" className="ghost" onClick={() => refreshRuntime().catch((error) => {
            setState((current) => ({ ...current, log: formatError(error) }));
          })}>Refresh runtime info</button>
        </div>
        <pre>{state.log}</pre>
      </section>
    </main>
  );
}
