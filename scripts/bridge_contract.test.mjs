import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

const bridgeSource = await readFile(
  new URL("../crates/rustframe/src/bridge.js", import.meta.url),
  "utf8",
);

function loadBridge(openedFiles = []) {
  const browserEvents = [];
  class CustomEvent {
    constructor(type, options = {}) {
      this.type = type;
      this.detail = options.detail;
    }
  }
  const window = {
    __RUSTFRAME_BRIDGE_CONFIG__: {
      model: "local-first",
      database: true,
      filesystem: true,
      shell: false,
      openedFiles,
      currentWindow: { id: "main", route: "/", isPrimary: true },
    },
    dispatchEvent: (event) => browserEvents.push(event),
    setTimeout: (callback) => callback(),
  };

  vm.runInNewContext(bridgeSource, { window, CustomEvent }, { filename: "bridge.js" });
  return { rustframe: window.RustFrame, browserEvents };
}

test("bridge retains launch files and emits later single-instance opens", () => {
  const launchFile = { id: "launch", uri: "grant://launch/brief.md", name: "brief.md" };
  const laterFile = { id: "later", uri: "grant://later/notes.txt", name: "notes.txt" };
  const { rustframe, browserEvents } = loadBridge([launchFile]);
  const received = [];
  const unsubscribe = rustframe.app.onOpenFiles((event) => received.push(event));

  const launchSnapshot = rustframe.app.openedFiles();
  launchSnapshot.length = 0;
  assert.equal(rustframe.app.openedFiles().length, 1);

  rustframe.__emitOpenFiles({ files: [laterFile] });
  assert.deepEqual(
    Array.from(rustframe.app.openedFiles(), (file) => file.name),
    ["brief.md", "notes.txt"],
  );
  assert.equal(received.length, 1);
  assert.equal(received[0].files[0].uri, laterFile.uri);
  assert.equal(browserEvents.at(-1).type, "rustframe:open-files");

  unsubscribe();
  rustframe.__emitOpenFiles({ files: [] });
  assert.equal(received.length, 1);
});

test("file-drop listeners receive the documented event object", () => {
  const { rustframe } = loadBridge();
  let received;
  rustframe.events.onFileDrop((event) => { received = event; });
  rustframe.__emitFileDrop({ files: [{ uri: "grant://drop/report.md" }] });
  assert.equal(received.files[0].uri, "grant://drop/report.md");
});
