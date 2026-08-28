import assert from "node:assert/strict";
import test from "node:test";

import { analyzeWorkflow } from "./check_workflow_security.mjs";

const sha = "de0fac2e4500dabe0009e67214ff5f5447ce83dd";

test("accepts an immutable action with read-only defaults", () => {
  const errors = analyzeWorkflow(`permissions:\n  contents: read\njobs:\n  test:\n    steps:\n      - uses: actions/checkout@${sha} # v6.0.2\n        with:\n          persist-credentials: false\n`);
  assert.deepEqual(errors, []);
});

test("rejects mutable action references", () => {
  const errors = analyzeWorkflow("permissions: read-all\njobs:\n  test:\n    steps:\n      - uses: vendor/action@main # rolling\n");
  assert.match(errors.join("\n"), /full immutable commit SHA/);
});

test("requires a version note beside a pinned action", () => {
  const errors = analyzeWorkflow(`permissions: read-all\njobs:\n  test:\n    steps:\n      - uses: vendor/action@${sha}\n`);
  assert.match(errors.join("\n"), /human-readable version comment/);
});

test("rejects workflow-wide write permissions", () => {
  const errors = analyzeWorkflow("permissions:\n  contents: write\njobs: {}\n");
  assert.match(errors.join("\n"), /must not be write/);
});

test("does not allow checkout to persist a token by default", () => {
  const errors = analyzeWorkflow(`permissions: read-all\njobs:\n  test:\n    steps:\n      - uses: actions/checkout@${sha} # v6.0.2\n`);
  assert.match(errors.join("\n"), /persist-credentials: false/);
});
