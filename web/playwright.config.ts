import { defineConfig } from "@playwright/test";

const serverUrl = "http://127.0.0.1:41739";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  reporter: "line",
  timeout: 30_000,
  use: {
    baseURL: serverUrl,
    screenshot: "off",
    trace: "retain-on-failure",
    video: "off",
  },
  webServer: {
    command: "../release/run-browser-server.sh",
    cwd: ".",
    reuseExistingServer: false,
    timeout: 120_000,
    url: serverUrl,
  },
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "firefox", use: { browserName: "firefox" } },
  ],
});
