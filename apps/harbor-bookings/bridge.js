(function () {
    const pending = new Map();
    let nextId = 1;

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
        window: Object.freeze({
            close: () => invoke("window.close"),
            minimize: () => invoke("window.minimize"),
            maximize: () => invoke("window.maximize"),
            setTitle: (title) => invoke("window.setTitle", { title })
        }),
        fs: Object.freeze({
            readText: (path) => invoke("fs.readText", { path })
        }),
        shell: Object.freeze({
            exec: (command, args = []) => invoke("shell.exec", { command, args })
        }),
        db: Object.freeze({
            info: () => invoke("db.info"),
            get: (table, id) => invoke("db.get", { table, id }),
            list: (table, options = {}) => invoke("db.list", { table, ...options }),
            count: (table, options = {}) => invoke("db.count", { table, ...options }),
            insert: (table, record) => invoke("db.insert", { table, record }),
            update: (table, id, patch) => invoke("db.update", { table, id, patch }),
            delete: (table, id) => invoke("db.delete", { table, id })
        })
    });
})();
