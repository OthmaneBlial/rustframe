const statusElement = document.querySelector("[data-benchmark-status]");

loadBenchmark().catch((error) => {
    if (statusElement) statusElement.textContent = `Receipt unavailable: ${error.message}`;
});

document.querySelector("[data-benchmark-copy]")?.addEventListener("click", async (event) => {
    const button = event.currentTarget;
    const command = button.previousElementSibling?.textContent || "node scripts/benchmark_research_desk.mjs";
    const original = button.textContent;
    try {
        await navigator.clipboard.writeText(command);
        button.textContent = "Copied";
    } catch {
        button.textContent = "Copy failed";
    }
    window.setTimeout(() => { button.textContent = original; }, 1400);
});

async function loadBenchmark() {
    const response = await fetch("assets/data/research-desk-benchmark.json", { cache: "no-store" });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const receipt = await response.json();
    if (receipt.schemaVersion !== 1 || receipt.product !== "Research Desk") throw new Error("unsupported receipt schema");

    setText("package", `${formatNumber(receipt.metrics.packageSize.mebibytes, 2)} MiB`);
    setText("start", `${formatNumber(receipt.metrics.coldStart.medianMs, 0)} ms`);
    setText("memory", `${formatNumber(receipt.metrics.peakMemory.medianMebibytes, 2)} MiB`);
    setText("indexing", `${formatNumber(receipt.metrics.indexing.documentsPerSecond, 0)} docs/s`);
    setText("rebuild", `${formatNumber(receipt.metrics.warmRebuild.medianMs / 1000, 2)} s`);

    setReceipt("host", `${receipt.host.os} · ${receipt.host.architecture}`);
    setReceipt("cpu", `${receipt.host.cpu} · ${receipt.host.logicalCpus} logical CPUs`);
    setReceipt("runtime", `${receipt.host.node} · ${receipt.methodology.iterations} samples`);
    setReceipt("date", new Intl.DateTimeFormat("en", { dateStyle: "medium", timeStyle: "short", timeZone: "UTC" }).format(new Date(receipt.collectedAt)) + " UTC");
    const commit = document.querySelector('[data-receipt="commit"]');
    if (commit) {
        commit.textContent = receipt.sourceCommit.slice(0, 9);
        commit.href = `https://github.com/OthmaneBlial/rustframe/commit/${receipt.sourceCommit}`;
    }

    const methodology = document.querySelector("[data-methodology]");
    const labels = { coldStart: "Native init", peakMemory: "Peak memory", indexing: "Index parser", warmRebuild: "Warm build" };
    for (const [key, description] of Object.entries(receipt.methodology)) {
        if (["iterations", "packageCommand"].includes(key)) continue;
        const article = document.createElement("article");
        const heading = document.createElement("h3");
        const paragraph = document.createElement("p");
        heading.textContent = labels[key] || key;
        paragraph.textContent = description;
        article.append(heading, paragraph);
        methodology?.append(article);
    }

    const limitations = document.querySelector("[data-limitations]");
    for (const limitation of receipt.limitations) {
        const item = document.createElement("li");
        item.textContent = limitation;
        limitations?.append(item);
    }
    if (statusElement) statusElement.textContent = `Receipt loaded · Research Desk ${receipt.version} · ${receipt.metrics.indexing.documents.toLocaleString("en-US")} document corpus`;
}

function setText(metric, value) {
    const element = document.querySelector(`[data-metric="${metric}"]`);
    if (element) element.textContent = value;
}

function setReceipt(field, value) {
    const element = document.querySelector(`[data-receipt="${field}"]`);
    if (element) element.textContent = value;
}

function formatNumber(value, maximumFractionDigits) {
    return new Intl.NumberFormat("en", { maximumFractionDigits }).format(value);
}
