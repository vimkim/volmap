import { defineConfig } from "@playwright/test";
import base from "./playwright.config";
import { densityPort, densityUrl } from "./e2e/density-address";

export default defineConfig({
  ...base,
  testIgnore: [],
  // DOM snapshot tracing materially perturbs dense-view timing and RSS.
  // Raw measurements and failure metadata are retained by the diagnostic.
  use: { ...base.use, trace: "off" },
  testMatch: "**/overlay-density.spec.ts",
  timeout: 180_000,
  webServer: (Array.isArray(base.webServer) ? base.webServer : []).filter((server) => server.url?.endsWith(":41741")).map((server) => ({
    ...server,
    url: densityUrl,
    timeout: 300_000,
    command: `VOLMAP_BROWSER_DENSE=1 VOLMAP_BROWSER_RELEASE=1 ${server.command?.replaceAll("41741", String(densityPort))}`,
  })),
});
