(function () {
    const pending = new Map();
    const fileDropListeners = new Set();
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

    function normalizePath(input = ".") {
        const value = String(input || ".").replaceAll("\\", "/");
        const isAbsolute = value.startsWith("/");
        const parts = value.split("/");
        const normalized = [];

        for (const part of parts) {
            if (!part || part === ".") {
                continue;
            }

            if (part === "..") {
                if (normalized.length && normalized[normalized.length - 1] !== "..") {
                    normalized.pop();
                    continue;
                }

                if (!isAbsolute) {
                    normalized.push(part);
                }
                continue;
            }

            normalized.push(part);
        }

        const joined = normalized.join("/");
        if (isAbsolute) {
            return `/${joined}`.replace(/\/+$/u, "") || "/";
        }

        return joined || ".";
    }

    function joinPath(...parts) {
        return normalizePath(parts.filter((part) => part !== undefined && part !== null).join("/"));
    }

    function dirname(input = ".") {
        const normalized = normalizePath(input);
        if (normalized === "." || normalized === "/") {
            return normalized;
        }

        const parts = normalized.split("/");
        parts.pop();
        if (!parts.length) {
            return normalized.startsWith("/") ? "/" : ".";
        }
        return parts.join("/") || ".";
    }

    function basename(input = ".") {
        const normalized = normalizePath(input);
        if (normalized === "." || normalized === "/") {
            return normalized;
        }
        const parts = normalized.split("/");
        return parts[parts.length - 1] || normalized;
    }

    function extname(input = ".") {
        const name = basename(input);
        const index = name.lastIndexOf(".");
        if (index <= 0 || index === name.length - 1) {
            return "";
        }
        return name.slice(index);
    }

    function emitFileDrop(payload) {
        fileDropListeners.forEach((listener) => {
            try {
                listener(payload);
            } catch (error) {
                window.setTimeout(() => {
                    throw error;
                }, 0);
            }
        });

        window.dispatchEvent(new CustomEvent("rustframe:file-drop", { detail: payload }));
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
        __emitFileDrop: emitFileDrop,
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
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            readBinary: (path) => bridgeConfig.filesystem
                ? invoke("fs.readBinary", { path })
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            metadata: (path) => bridgeConfig.filesystem
                ? invoke("fs.metadata", { path })
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            listDir: (path = ".") => bridgeConfig.filesystem
                ? invoke("fs.listDir", { path })
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            writeText: (path, contents) => bridgeConfig.filesystem
                ? invoke("fs.writeText", { path, contents })
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            writeBinary: (path, base64) => bridgeConfig.filesystem
                ? invoke("fs.writeBinary", { path, base64 })
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            copyFrom: (sourcePath, destinationPath) => bridgeConfig.filesystem
                ? invoke("fs.copyFrom", { sourcePath, destinationPath })
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            openPath: (path) => bridgeConfig.filesystem
                ? invoke("fs.openPath", { path })
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            revealPath: (path) => bridgeConfig.filesystem
                ? invoke("fs.revealPath", { path })
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend")
        }),
        clipboard: Object.freeze({
            writeText: (text) => invoke("clipboard.writeText", { text })
        }),
        dialog: Object.freeze({
            openFile: (options = {}) => bridgeConfig.filesystem
                ? invoke("dialog.openFile", options)
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            openFiles: (options = {}) => bridgeConfig.filesystem
                ? invoke("dialog.openFiles", options)
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            openDirectory: (options = {}) => bridgeConfig.filesystem
                ? invoke("dialog.openDirectory", options)
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            saveText: (options = {}) => bridgeConfig.filesystem
                ? invoke("dialog.saveText", options)
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend"),
            saveBinary: (options = {}) => bridgeConfig.filesystem
                ? invoke("dialog.saveBinary", options)
                : rejectRestrictedBridge("filesystem bridge is disabled for this frontend")
        }),
        events: Object.freeze({
            onFileDrop: (listener) => {
                if (typeof listener !== "function") {
                    throw new TypeError("RustFrame.events.onFileDrop expects a function");
                }
                fileDropListeners.add(listener);
                return () => fileDropListeners.delete(listener);
            }
        }),
        path: Object.freeze({
            normalize: normalizePath,
            join: joinPath,
            dirname,
            basename,
            extname
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
            search: (table, term, options = {}) => bridgeConfig.database
                ? invoke("db.search", { table, term, ...options })
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
