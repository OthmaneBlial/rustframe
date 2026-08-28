import { expect, test } from "@playwright/test";

function captureConsoleErrors(page) {
  const errors = [];
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  page.on("pageerror", (error) => errors.push(error.message));
  return errors;
}

test("homepage proof and controls stay functional", async ({ page }) => {
  const errors = captureConsoleErrors(page);
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toContainText(/your frontend/i);
  await expect(page.locator("#featured-docs .doc-card")).toHaveCount(6);

  await page.getByRole("checkbox", { name: "Run bounded indexer" }).check();
  await expect(page.locator("[data-policy-count]")).toHaveText("4 permissions");
  await expect(page.locator("[data-policy-output]")).toContainText("shell:index-workspace");

  await page.getByRole("tab", { name: "Windows" }).click();
  await expect(page.locator('[data-command-panel="windows"]')).toBeVisible();
  await page.locator('[data-command-panel="windows"] [data-copy]').click();
  await expect(page.locator('[data-command-panel="windows"] [data-copy]')).toHaveText("Copied");

  await expect(page.locator("[data-schema-status]")).toContainText("valid schema v1");
  await expect(page.locator("[data-types-output]")).toContainText("interface ProjectRecord");
  await page.locator("[data-schema-input]").fill('{"version":1,"tables":[]}');
  await expect(page.locator("[data-schema-status]")).toContainText("Fix the highlighted contract");
  await expect(page.locator("[data-starter-download]")).toBeDisabled();

  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
    await page.evaluate(() => window.innerWidth),
  );
  expect(errors).toEqual([]);
});

test("desktop hero headline stays clear of the product proof", async ({ page }, testInfo) => {
  test.skip(!testInfo.project.name.startsWith("desktop"), "desktop-only composition contract");
  for (const width of [1181, 1280, 1365, 1920]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto("/");

    const spacing = await page.evaluate(() => {
      const heading = document.querySelector(".hero h1");
      const proof = document.querySelector(".proof-terminal");
      if (!heading || !proof) throw new Error("Hero composition is incomplete");

      const textRange = document.createRange();
      textRange.selectNodeContents(heading);
      const headline = textRange.getBoundingClientRect();
      const terminal = proof.getBoundingClientRect();
      return { gap: terminal.left - headline.right };
    });

    expect(spacing.gap, `hero spacing at ${width}px`).toBeGreaterThanOrEqual(16);
  }
});

test("mobile navigation is keyboard complete and does not overflow", async ({ page }, testInfo) => {
  test.skip(!testInfo.project.name.startsWith("mobile"), "mobile-only contract");
  const errors = captureConsoleErrors(page);
  await page.goto("/");
  const menu = page.getByRole("button", { name: "Menu" });
  await menu.click();
  await expect(page.getByRole("navigation", { name: "Primary navigation" })).toBeVisible();
  await page.getByRole("button", { name: "Close" }).press("Escape");
  await expect(menu).toBeFocused();
  await expect(menu).toHaveAttribute("aria-expanded", "false");
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
    await page.evaluate(() => window.innerWidth),
  );
  expect(errors).toEqual([]);
});

test("documentation supports deep links, full-text search, and paging", async ({ page }) => {
  const errors = captureConsoleErrors(page);
  await page.goto("/docs.html?doc=api-reference#rustframefilesystem");
  await expect(page.locator("#rustframefilesystem")).toHaveText("RustFrameFilesystem");
  await expect(page).toHaveTitle("Frontend API reference — RustFrame Docs");
  await expect(page.locator("#docs-pager a")).toHaveCount(2);
  await expect(page.locator("#docs-search-status")).toContainText("guides indexed across full text");

  await page.getByPlaceholder("Try “packaging” or “security”").fill("lost-access state");
  await expect(page.locator("[data-doc-link]:not(.is-hidden)")).toHaveCount(1);
  await expect(page.locator("[data-doc-link]:not(.is-hidden)")).toHaveText("Troubleshooting");
  expect(errors).toEqual([]);
});

test("showcase renders verified source links and optimized images", async ({ page }) => {
  const errors = captureConsoleErrors(page);
  await page.goto("/showcase.html");
  await expect(page.locator(".showcase-card")).toHaveCount(7);
  await expect(page.locator(".showcase-visual img")).toHaveCount(7);
  await expect(page.locator('.showcase-proof[data-state="verified"]')).toHaveCount(7);
  await expect(page.locator(".showcase-contract")).toHaveCount(7);
  const images = await page.locator(".showcase-visual img").evaluateAll((nodes) =>
    nodes.map((node) => node.getAttribute("src")),
  );
  expect(images.every((source) => source?.endsWith(".webp"))).toBe(true);
  const sourceLinks = await page.locator("[data-source-link]").evaluateAll((nodes) =>
    nodes.map((node) => node.getAttribute("href")),
  );
  expect(sourceLinks.every((href) => href?.startsWith("https://github.com/OthmaneBlial/rustframe/"))).toBe(true);
  const workflows = await page.locator(".gallery-kicker").allTextContents();
  for (const workflow of ["document-desk", "media-review", "offline-inventory", "evidence-tracker", "batch-operations"]) {
    expect(workflows.some((label) => label.includes(workflow))).toBe(true);
  }
  expect(errors).toEqual([]);
});

test("benchmark receipt discloses measurements, host, source, and limitations", async ({ page }) => {
  const errors = captureConsoleErrors(page);
  await page.goto("/benchmarks.html");
  await expect(page.locator("[data-benchmark-status]")).toContainText("Receipt loaded");
  await expect(page.locator("[data-metric]")).toHaveCount(5);
  await expect(page.locator('[data-metric="package"]')).toContainText("MiB");
  await expect(page.locator('[data-metric="indexing"]')).toContainText("docs/s");
  await expect(page.locator("[data-methodology] article")).toHaveCount(4);
  await expect(page.locator("[data-limitations] li")).toHaveCount(4);
  await expect(page.locator('[data-receipt="commit"]')).toHaveAttribute("href", /\/commit\/[a-f0-9]{40}$/u);
  const copyButton = page.getByRole("button", { name: "Copy benchmark command" });
  await copyButton.click();
  await expect(copyButton).toHaveText("Copied");
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
    await page.evaluate(() => window.innerWidth),
  );
  expect(errors).toEqual([]);
});
