(function () {
    const pending = new Map();
    let nextId = 1;
    const bridgeConfig = (() => {
        const raw = window.__RUSTFRAME_BRIDGE_CONFIG__;
        const config = raw && typeof raw === "object" ? raw : {};
        const asBoolean = (value, fallback) => typeof value === "boolean" ? value : fallback;
        const currentWindow = config.currentWindow && typeof config.currentWindow === "object"
            ? config.currentWindow
            : { id: "main", route: "/", isPrimary: true };

        return Object.freeze({
            model: config.model === "networked" ? "networked" : "local-first",
            database: asBoolean(config.database, true),
            filesystem: asBoolean(config.filesystem, true),
            shell: asBoolean(config.shell, true),
            currentWindow: Object.freeze({
                id: typeof currentWindow.id === "string" ? currentWindow.id : "main",
                route: typeof currentWindow.route === "string" ? currentWindow.route : "/",
                isPrimary: asBoolean(currentWindow.isPrimary, true)
            })
        });
    })();

    function normalizeError(payload) {
        if (payload && typeof payload === "object") {
            return payload;
        }

        return {
            code: "unknown_error",
            message: String(payload ?? "Unknown RustFrame error")
        };
    }

    function invoke(method, params = {}) {
        if (!window.ipc || typeof window.ipc.postMessage !== "function") {
            return Promise.reject({
                code: "ipc_unavailable",
                message: "window.ipc.postMessage is not available in this WebView"
            });
        }

        return new Promise((resolve, reject) => {
            const id = nextId++;
            pending.set(id, { resolve, reject });
            window.ipc.postMessage(JSON.stringify({ id, method, params }));
        });
    }

    function rejectRestrictedBridge(message) {
        return Promise.reject({
            code: "permission_denied",
            message
        });
    }

    function resolveFromNative(message) {
        const callback = pending.get(message.id);
        if (!callback) {
            return;
        }

        pending.delete(message.id);

        if (message.ok) {
            callback.resolve(message.data);
            return;
        }

        callback.reject(normalizeError(message.error));
    }

    window.RustFrame = Object.freeze({
        __resolveFromNative: resolveFromNative,
        invoke,
        security: bridgeConfig,
        window: Object.freeze({
            id: bridgeConfig.currentWindow.id,
            route: bridgeConfig.currentWindow.route,
            isPrimary: bridgeConfig.currentWindow.isPrimary,
            close: () => invoke("window.close"),
            minimize: () => invoke("window.minimize"),
            maximize: () => invoke("window.maximize"),
            setTitle: (title) => invoke("window.setTitle", { title }),
            current: () => invoke("window.current"),
            list: () => invoke("window.list"),
            open: (route = "/", options = {}) => {
                if (route && typeof route === "object") {
                    return invoke("window.open", route);
                }

                return invoke("window.open", { route, ...options });
            }
        }),
        fs: Object.freeze({
            readText: (path) => bridgeConfig.filesystem
                ? invoke("fs.readText", { path })
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend")
        }),
        shell: Object.freeze({
            exec: (command, args = []) => bridgeConfig.shell
                ? invoke("shell.exec", { command, args })
                : rejectRestrictedBridge("shell bridge is disabled for this frontend")
        }),
        db: Object.freeze({
            info: () => bridgeConfig.database
                ? invoke("db.info")
                : rejectRestrictedBridge("database bridge is disabled for this frontend"),
            get: (table, id) => bridgeConfig.database
                ? invoke("db.get", { table, id })
                : rejectRestrictedBridge("database bridge is disabled for this frontend"),
            list: (table, options = {}) => bridgeConfig.database
                ? invoke("db.list", { table, ...options })
                : rejectRestrictedBridge("database bridge is disabled for this frontend"),
            count: (table, options = {}) => bridgeConfig.database
                ? invoke("db.count", { table, ...options })
                : rejectRestrictedBridge("database bridge is disabled for this frontend"),
            insert: (table, record) => bridgeConfig.database
                ? invoke("db.insert", { table, record })
                : rejectRestrictedBridge("database bridge is disabled for this frontend"),
            update: (table, id, patch) => bridgeConfig.database
                ? invoke("db.update", { table, id, patch })
                : rejectRestrictedBridge("database bridge is disabled for this frontend"),
            delete: (table, id) => bridgeConfig.database
                ? invoke("db.delete", { table, id })
                : rejectRestrictedBridge("database bridge is disabled for this frontend")
        })
    });
})();
