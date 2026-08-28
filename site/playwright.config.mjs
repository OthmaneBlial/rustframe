import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "tests",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://127.0.0.1:4318",
    trace: "retain-on-failure",
    permissions: ["clipboard-read", "clipboard-write"],
  },
  webServer: [
    {
      command: "python3 -m http.server 4318 --bind 127.0.0.1 --directory .",
      port: 4318,
      reuseExistingServer: !process.env.CI,
    },
    {
      command: "python3 -m http.server 4316 --bind 127.0.0.1 --directory ../apps/research-desk",
      port: 4316,
      reuseExistingServer: !process.env.CI,
    },
  ],
  projects: [
    { name: "desktop-chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "mobile-chromium", use: { browserName: "chromium", viewport: { width: 390, height: 844 } } },
  ],
});
