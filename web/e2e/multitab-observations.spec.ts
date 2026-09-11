import { expect, test, type Page } from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";

const evidenceRoot = "../.scratch/volume-overlay-64-sectors-implementation/verification/03-lifecycle";

interface Exchange {
  request: { after_request?: boolean; cadence_ms: number; pages?: unknown; scope?: unknown; epoch: string; generation: string };
  body: { capture?: { sequence: string }; scope?: unknown; epoch: string; generation: string };
  status: number;
}

async function traceObservations(page: Page): Promise<Exchange[]> {
  const exchanges: Exchange[] = [];
  await page.route("**/runtime/page-buffer/observe", async (route) => {
    // Forward the original request through actual HTTP and return its exact
    // response. Retain bodies here because Chromium may discard a response
    // resource when a simultaneous route reload cancels its browser request.
    const response = await route.fetch();
    const body = await response.json();
    await route.fulfill({ response }).catch(() => {});
    exchanges.push({ request: route.request().postDataJSON(), body, status: response.status() });
  });
  return exchanges;
}

for (const count of [1, 8, 32]) {
  test(`${count} mixed tabs preserve request scope and disk navigation under simultaneous and staggered demand`, async ({ browser, browserName }) => {
    test.setTimeout(120_000);
    const tabs: { page: Page; route: string; index: number; exchanges: Exchange[] }[] = [];
    const traces: unknown[] = [];
    for (let index = 0; index < count; index++) {
      const page = await browser.newPage();
      page.setDefaultTimeout(10_000);
      await page.clock.install();
      const route = ["volume/0", "sector/0/0", "page/0/10"][index % 3]!;
      await page.goto(`http://127.0.0.1:41741/${route}`, { waitUntil: "commit" });
      await expect(page.getByRole("button", { name: "Enable observations" })).toBeVisible();
      tabs.push({ page, route, index, exchanges: await traceObservations(page) });
    }
    try {
      for (const phase of ["simultaneous", "staggered"]) {
        const demand = async ({ page, route, index, exchanges }: typeof tabs[number]) => {
          console.info(`demand ${count}/${index} ${phase} ${route}`);
          const sent = page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe"));
          const before = exchanges.length;
          await page.getByRole("button", { name: phase === "simultaneous" ? "Enable observations" : "Resume", exact: true }).click();
          const issued = (await sent).postDataJSON();
          if (phase === "staggered") expect(issued.after_request).toBe(true);
          await page.clock.runFor(50);
          const next = () => exchanges.slice(before).find((exchange) => phase === "simultaneous" || exchange.request.after_request);
          await expect.poll(next).toBeDefined();
          const { request, body, status } = next()!;
          console.info(`response ${count}/${index} ${phase} ${status} ${body.capture?.sequence ?? "none"}`);
          expect([200, 429]).toContain(status);
          expect(request.cadence_ms).toBe(route === "page/0/10" ? 500 : 2000);
          if (route === "page/0/10") expect(request.pages).toEqual([{ volid: 0, pageid: 10 }]);
          else expect(request.scope).toMatchObject({ kind: route.startsWith("volume") ? "volume" : "sector", volid: 0 });
          if (phase === "staggered") expect(request.after_request).toBe(true);
          if (status === 200) {
            expect(body.epoch).toBe(request.epoch);
            expect(body.generation).toBe(request.generation);
            if (request.scope) expect(body.scope).toEqual(request.scope);
            expect(body.capture).not.toBeNull();
            const detail = page.getByRole("region", { name: route === "page/0/10" ? "Selected-page buffer observation" : "Visible-page buffer observations" });
            await expect(detail).toContainText(`scan ${body.capture!.sequence} ·`);
            if (route === "page/0/10") await expect(page.getByRole("region", { name: "Selected-page buffer observation" })).toContainText("Observed resident");
            else await expect(page.locator('[data-observation-page="0:10"]')).toHaveAttribute("data-runtime-state", "resident");
          }
          await page.getByRole("button", { name: "Pause", exact: true }).click();
          traces.push({ phase, tab: index, route, status, request, capture: body.capture ?? null,
            diskHeading: await page.getByRole("heading").first().textContent(),
            resident: await page.locator('[data-runtime-state="resident"]').count() });
        };
        if (phase === "simultaneous") await Promise.all(tabs.map(demand));
        else for (const tab of tabs) await demand(tab);
      }
      // Each tab still follows its own disk route after runtime demand.
      for (const { page } of tabs) {
        await page.goto("http://127.0.0.1:41741/sector/0/0", { waitUntil: "commit" });
        await expect(page.getByRole("heading", { name: "Sector 0", exact: true })).toBeVisible();
      }
      await mkdir(evidenceRoot, { recursive: true });
      await writeFile(`${evidenceRoot}/multitab-${count}-${browserName}.json`, JSON.stringify(traces, null, 2) + "\n");
    } finally {
      await Promise.all(tabs.map(({ page }) => page.close()));
    }
  });
}

for (const route of ["volume/0", "sector/0/0"]) {
  test(`${route} paused evidence only sees other-tab availability and expires while hidden requests stay silent`, async ({ page, browser, browserName }) => {
    await page.clock.install();
    await page.goto(`http://127.0.0.1:41741/${route}`, { waitUntil: "commit" });
    const requests: string[] = [];
    page.on("request", (request) => { if (request.url().includes("/runtime/")) requests.push(request.url()); });
    await page.getByRole("button", { name: "Enable observations" }).click();
    const cell = page.locator('[data-observation-page="0:10"]');
    await expect(cell).toHaveAttribute("data-runtime-state", "resident");
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    const captureLabel = page.getByRole("region", { name: "Visible-page buffer observations" }).locator("details p").first();
    const pausedCapture = await captureLabel.textContent();
    const observations = () => requests.filter((url) => url.endsWith("/observe")).length;
    const pausedCount = observations();
    const other = await browser.newPage();
    try {
      await other.evaluate(() => { window.location.href = "http://127.0.0.1:41741/page/0/10"; });
      await other.getByRole("button", { name: "Enable observations" }).click();
      await expect(other.getByRole("region", { name: "Selected-page buffer observation" })).toContainText("Observed resident");
      await other.getByRole("button", { name: "Pause", exact: true }).click();
      const exchanges = await traceObservations(other);
      await other.getByRole("button", { name: "Resume", exact: true }).click();
      await expect.poll(() => exchanges.find((exchange) => exchange.request.after_request)).toBeDefined();
      const capture = exchanges.find((exchange) => exchange.request.after_request)!.body.capture;
      await other.getByRole("button", { name: "Pause", exact: true }).click();
      await page.clock.runFor(5000);
      await expect(page.getByRole("region", { name: "CUBRID page-buffer observation" })).toContainText("Newer observation available");
      expect(observations()).toBe(pausedCount);
      await expect(captureLabel).toHaveText(pausedCapture!);
      await expect(cell).toHaveAttribute("title", /stale/);
      await page.evaluate(() => {
        Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "hidden" });
        document.dispatchEvent(new Event("visibilitychange"));
      });
      const hiddenCount = requests.length;
      await page.clock.runFor(10000);
      expect(requests).toHaveLength(hiddenCount);
      await page.evaluate(() => {
        Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "visible" });
        document.dispatchEvent(new Event("visibilitychange"));
      });
      const resumed = page.waitForRequest((request) => request.url().endsWith("/observe"));
      await page.getByRole("button", { name: "Resume", exact: true }).click();
      expect((await resumed).postDataJSON().after_request).toBe(true);
      await expect(cell).toHaveAttribute("data-runtime-state", "resident");
      await page.getByRole("button", { name: "Pause", exact: true }).click();
      await page.clock.runFor(31000);
      await expect(cell).toHaveAttribute("data-runtime-state", "expired");
      await mkdir(evidenceRoot, { recursive: true });
      await page.screenshot({ path: `${evidenceRoot}/paused-${route.split("/")[0]}-${browserName}.png` });
      await writeFile(`${evidenceRoot}/paused-${route.split("/")[0]}-${browserName}.json`, JSON.stringify({ route, capture, requests, hiddenCount, pausedCount, expired: await cell.getAttribute("data-runtime-state") }, null, 2) + "\n");
    } finally { await other.close(); }
  });
}
