import assert from "node:assert/strict";
import test from "node:test";

import { buildIndexedRecord, isSourceUnchanged, parseFrontmatter } from "./indexing.mjs";

test("indexing turns frontmatter and content into the persisted public record shape", () => {
    const text = "---\ncollection: launch-notes\nstatus: reviewing\npriority: critical\ntags: local, proof\n---\n# Decision memo\n\nKeep source files close.";
    const record = buildIndexedRecord("grant://workspace", {
        uri: "grant://workspace/memos/decision.md",
        name: "decision.md",
        size: Buffer.byteLength(text),
        modifiedAt: "2026-08-28T10:00:00Z",
    }, text);

    assert.equal(record.title, "Decision memo");
    assert.equal(record.collection, "Launch Notes");
    assert.equal(record.status, "reviewing");
    assert.equal(record.priority, "critical");
    assert.deepEqual(record.tags, ["local", "proof"]);
    assert.match(record.contentFingerprint, /^fnv1a32:[a-f0-9]{8}$/u);
});

test("warm indexing skips only sources with matching metadata and a fingerprint", () => {
    const entry = { size: 42, modifiedAt: "2026-08-28T10:00:00Z" };
    assert.equal(isSourceUnchanged({ fileSize: 42, sourceModifiedAt: entry.modifiedAt, contentFingerprint: "fnv1a32:12345678" }, entry), true);
    assert.equal(isSourceUnchanged({ fileSize: 42, sourceModifiedAt: entry.modifiedAt, contentFingerprint: "" }, entry), false);
    assert.equal(isSourceUnchanged({ fileSize: 43, sourceModifiedAt: entry.modifiedAt, contentFingerprint: "fnv1a32:12345678" }, entry), false);
});

test("frontmatter parsing keeps plain Markdown intact", () => {
    assert.deepEqual(parseFrontmatter("# Plain\n\nBody"), { metadata: {}, body: "# Plain\n\nBody" });
});
