import { defineConfig } from "@playwright/test";
import { readFileSync } from "node:fs";

if (!process.env.VOLMAP_PRODUCER_RUN) {
  throw new Error("VOLMAP_PRODUCER_RUN must name an explicit real-producer run JSON file");
}
const run = JSON.parse(readFileSync(process.env.VOLMAP_PRODUCER_RUN, "utf8"));
if (!Number.isInteger(run.listen_port) || run.listen_port < 1 || run.listen_port > 65535) {
  throw new Error("listen_port must be an explicit port from 1 to 65535");
}
const baseURL = `http://127.0.0.1:${run.listen_port}`;

export default defineConfig({
  testDir: "./e2e",
  testMatch: "real-producer.spec.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  forbidOnly: true,
  timeout: 30_000,
  outputDir: `${run.output_directory}-browser-artifacts`,
  reporter: [["line"], ["json", { outputFile: `${run.output_directory}-browser-results.json` }]],
  use: { baseURL, trace: "retain-on-failure" },
  webServer: {
    command: "python3 ../tools/serve-real-producer.py",
    url: baseURL,
    reuseExistingServer: false,
    timeout: 120_000,
    gracefulShutdown: { signal: "SIGTERM", timeout: 30_000 },
  },
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "firefox", use: { browserName: "firefox" } },
  ],
});
