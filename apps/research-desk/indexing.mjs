const STATUS_ORDER = ["queued", "reviewing", "ready", "archived"];
const PRIORITY_ORDER = ["critical", "watch", "reference"];

export function isSourceUnchanged(current, entry) {
    return Boolean(current)
        && Number(current.fileSize) === Number(entry.size)
        && String(current.sourceModifiedAt || "") === String(entry.modifiedAt || "")
        && Boolean(current.contentFingerprint);
}

export function buildIndexedRecord(root, entry, text) {
    const { metadata, body } = parseFrontmatter(text);
    const relative = entry.uri.startsWith(`${root}/`) ? entry.uri.slice(root.length + 1) : entry.name;
    const segments = relative.split("/");
    const collectionSource = metadata.collection || (segments.length > 1 ? segments[0] : "Unsorted");
    const title = metadata.title
        || body.split("\n").map((line) => line.trim()).find((line) => line.startsWith("# "))?.slice(2).trim()
        || entry.name.replace(/\.[^.]+$/u, "").replace(/[-_]+/gu, " ");
    const summary = metadata.summary
        || body.split("\n").map((line) => line.trim()).filter((line) => line && !line.startsWith("#") && !line.startsWith("-")).join(" ").slice(0, 220);
    const wordCount = (body.match(/\b[\p{L}\p{N}_]+\b/gu) || []).length;
    return {
        path: entry.uri,
        title,
        collection: humanizeLabel(collectionSource),
        kind: metadata.kind || humanizeLabel(segments.length > 1 ? segments.at(-2) : "document"),
        summary,
        reviewer: metadata.reviewer || "",
        status: STATUS_ORDER.includes(metadata.status) ? metadata.status : "queued",
        priority: PRIORITY_ORDER.includes(metadata.priority) ? metadata.priority : "watch",
        tags: String(metadata.tags || "").split(",").map((tag) => tag.trim()).filter(Boolean),
        readingMinutes: Math.max(1, Number(metadata.readingMinutes) || Math.round(wordCount / 220)),
        lineCount: text.split(/\r?\n/u).length,
        fileSize: entry.size,
        sourceModifiedAt: entry.modifiedAt || "",
        contentFingerprint: fingerprintText(text),
    };
}

export function parseFrontmatter(text) {
    if (!text.startsWith("---")) return { metadata: {}, body: text };
    const lines = text.replace(/\r\n/gu, "\n").split("\n");
    const metadata = {};
    let end = 0;
    for (let index = 1; index < lines.length; index += 1) {
        if (lines[index].trim() === "---") {
            end = index;
            break;
        }
        const separator = lines[index].indexOf(":");
        if (separator > 0) metadata[lines[index].slice(0, separator).trim()] = lines[index].slice(separator + 1).trim();
    }
    return { metadata, body: end ? lines.slice(end + 1).join("\n").trim() : text };
}

function humanizeLabel(value) {
    return String(value || "Unsorted").replace(/[-_]+/gu, " ").replace(/\b\w/gu, (letter) => letter.toUpperCase());
}

function fingerprintText(value) {
    let hash = 0x811c9dc5;
    for (const character of String(value || "")) {
        hash ^= character.codePointAt(0);
        hash = Math.imul(hash, 0x01000193) >>> 0;
    }
    return `fnv1a32:${hash.toString(16).padStart(8, "0")}`;
}
