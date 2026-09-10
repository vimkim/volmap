import { defineConfig } from "@playwright/test";

const serverUrl = "http://127.0.0.1:41739";

export default defineConfig({
  testDir: "./e2e",
  testIgnore: "**/overlay-density.spec.ts",
  fullyParallel: false,
  // Fault scenarios share one process-wide producer incarnation.
  workers: 1,
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
  webServer: [{
    command: "../release/run-browser-server.sh",
    cwd: ".",
    reuseExistingServer: false,
    timeout: 120_000,
    url: serverUrl,
  }, {
    command: "VOLMAP_BROWSER_PORT=41740 VOLMAP_BROWSER_RUNTIME=1 ../release/run-browser-server.sh",
    cwd: ".",
    reuseExistingServer: false,
    timeout: 120_000,
    url: "http://127.0.0.1:41740",
  }, {
    command: "VOLMAP_BROWSER_PORT=41741 VOLMAP_BROWSER_PRODUCER=1 VOLMAP_BROWSER_PRODUCER_PID_FILE=/tmp/volmap-browser-producer-41741.pid ../release/run-browser-server.sh",
    cwd: ".",
    reuseExistingServer: false,
    timeout: 120_000,
    url: "http://127.0.0.1:41741",
  }],
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "firefox", use: { browserName: "firefox" } },
  ],
});
