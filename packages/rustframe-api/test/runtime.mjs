import assert from "node:assert/strict";
import test from "node:test";

import { RustFrameError, getRustFrame } from "../dist/index.js";

test("RustFrameError exposes its stable public shape", () => {
  const details = { method: "db.restore" };
  const error = new RustFrameError("permission_denied", "Denied", details);

  assert.ok(error instanceof Error);
  assert.ok(error instanceof RustFrameError);
  assert.equal(error.name, "RustFrameError");
  assert.equal(error.code, "permission_denied");
  assert.equal(error.message, "Denied");
  assert.equal(error.details, details);
});

test("getRustFrame reports an unavailable runtime with a typed error", () => {
  assert.throws(
    () => getRustFrame(),
    (error) => error instanceof RustFrameError && error.code === "ipc_unavailable",
  );
});
