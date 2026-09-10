import { expect, test } from "@playwright/test";

// Firefox can keep document readiness pending alongside the live watch.
// Wait for navigation commit, then assert rendered controls and observations.

test("selected-page observation uses the real producer, HTTP boundary and non-color detail", async ({ page, browserName }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("http://127.0.0.1:41741/page/0/10", { waitUntil: "commit" });
  await expect(page.getByRole("heading", { name: "Page facts" })).toBeVisible();
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  await source.getByRole("button", { name: "Enable observations" }).click();
  const response = page.waitForResponse((response) => response.url().endsWith("/runtime/page-buffer/observe"));
  await source.getByRole("button", { name: "Refresh selected-page observation" }).click();
  const observed = await response;
  expect(observed.status()).toBe(200);
  expect(observed.headers()["cache-control"]).toBe("no-store");
  const detail = page.getByRole("region", { name: "Selected-page buffer observation" });
  await expect(detail).toContainText("◉ Observed resident");
  await expect(detail).toContainText("VPID 0:10");
  await expect(source).toContainText("Observation source: active");
  await expect(detail).toContainText("latch mode");
  await expect(detail).toContainText("oldest unflush lsa");
  await detail.getByText("Capture identity and limitations", { exact: true }).click();
  await expect(detail).toContainText("records and scans are not atomic");
  await expect(page.getByRole("heading", { name: "Page facts" })).toBeVisible();
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  await expect(source.getByRole("button", { name: "Refresh selected-page observation" })).toBeDisabled();
  await expect(detail).toContainText("paused");
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: `../.scratch/pgbuf-overlay-implementation/verification/selected-observation-${browserName}.png`, fullPage: true });
  await source.getByRole("button", { name: "Disable observations" }).click();
  await expect(detail).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("automatic polling, paused offers, hidden silence, resume and expiry preserve disk navigation", async ({ page, context, browserName }) => {
  await page.clock.install();
  await page.goto("http://127.0.0.1:41741/page/0/10", { waitUntil: "commit" });
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  const detail = page.getByRole("region", { name: "Selected-page buffer observation" });
  let observations = 0;
  let metadata = 0;
  page.on("request", (request) => {
    if (request.url().endsWith("/page-buffer/observe")) observations += 1;
    if (request.url().endsWith("/runtime/capabilities")) metadata += 1;
  });
  await source.getByRole("button", { name: "Enable observations" }).click();
  await expect(detail).toContainText("Observed resident");
  const first = observations;
  await page.clock.runFor(500);
  await expect.poll(() => observations).toBeGreaterThan(first);
  await expect(source.getByRole("button", { name: "Refresh selected-page observation" })).toBeEnabled();
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  const pausedRequests = observations;
  const beforeMetadata = metadata;
  const other = await context.newPage();
  await other.goto("http://127.0.0.1:41741/page/0/10", { waitUntil: "commit" });
  const otherSource = other.getByRole("region", { name: "CUBRID page-buffer observation" });
  await otherSource.getByRole("button", { name: "Enable observations" }).click();
  await expect(other.getByRole("region", { name: "Selected-page buffer observation" })).toContainText("Observed resident");
  await other.waitForResponse((response) => response.url().endsWith("/page-buffer/observe"));
  await page.clock.runFor(5000);
  await expect(source).toContainText("Newer observation available");
  expect(observations).toBe(pausedRequests);
  expect(metadata).toBeGreaterThan(beforeMetadata);
  await other.close();

  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "hidden" });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  const hiddenMetadata = metadata;
  await page.clock.runFor(10000);
  expect(metadata).toBe(hiddenMetadata);
  expect(observations).toBe(pausedRequests);
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "visible" });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  const resumedRequest = page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe"));
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  expect((await resumedRequest).postDataJSON().after_request).toBe(true);
  await expect(detail).toContainText("Observed resident");
  await expect(source.getByRole("button", { name: "Refresh selected-page observation" })).toBeEnabled();
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  await page.clock.runFor(31000);
  await expect(detail).toContainText("Observation expired");
  await expect(page.getByRole("heading", { name: "Page facts" })).toBeVisible();
  await page.screenshot({ path: `../.scratch/pgbuf-overlay-implementation/verification/lifecycle-expired-${browserName}.png`, fullPage: true });
});

test("producer restart while paused clears evidence and requires an explicit retry", async ({ page }) => {
  const { readFile } = await import("node:fs/promises");
  await page.clock.install();
  await page.goto("http://127.0.0.1:41741/page/0/10", { waitUntil: "commit" });
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  const detail = page.getByRole("region", { name: "Selected-page buffer observation" });
  await source.getByRole("button", { name: "Enable observations" }).click();
  await expect(detail).toContainText("Observed resident");
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  const pid = Number((await readFile("/tmp/volmap-browser-producer-41741.pid", "utf8")).trim());
  process.kill(pid, "SIGHUP");
  await page.clock.runFor(5000);
  await expect(source).toContainText("incarnation-changed");
  await expect(detail).not.toContainText("Observed resident");
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  await expect(source).toContainText("Automatic retry stopped");
  await source.getByRole("button", { name: "Refresh selected-page observation" }).click();
  await expect(detail).toContainText("Observed resident");
  await expect(page.getByRole("heading", { name: "Page facts" })).toBeVisible();
});

test("transient browser retries back off and protocol incompatibility waits for explicit retry", async ({ page }) => {
  await page.addInitScript(() => { Math.random = () => 0.5; });
  await page.clock.install();
  let requests = 0;
  await page.route("**/runtime/page-buffer/observe", async (route) => {
    requests += 1;
    if (requests <= 2) await route.abort("failed");
    else if (requests === 3) await route.fulfill({ json: { schema: "unsupported" } });
    else await route.continue();
  });
  await page.goto("http://127.0.0.1:41741/page/0/10", { waitUntil: "commit" });
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  await source.getByRole("button", { name: "Enable observations" }).click();
  await expect(source).toContainText("Observation unavailable");
  expect(requests).toBe(1);
  await page.clock.runFor(500);
  await expect.poll(() => requests).toBe(2);
  await expect(source).toContainText("Observation unavailable");
  await page.clock.runFor(1000);
  await expect(source).toContainText("Automatic retry stopped");
  expect(requests).toBe(3);
  await page.clock.runFor(10000);
  expect(requests).toBe(3);
  await source.getByRole("button", { name: "Refresh selected-page observation" }).click();
  await expect(page.getByRole("region", { name: "Selected-page buffer observation" })).toContainText("Observed resident");
  await expect(page.getByRole("heading", { name: "Page facts" })).toBeVisible();
});

test("Volume and Sector show bounded coverage, semantic rows and independent cadences across tabs", async ({ page, context, browserName }) => {
  await page.goto("http://127.0.0.1:41741/volume/0", { waitUntil: "commit" });
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  const request = page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe"));
  await source.getByRole("button", { name: "Enable observations" }).click();
  const visible = (await request).postDataJSON();
  expect(visible.cadence_ms).toBe(2000);
  expect(visible.pages.length).toBeGreaterThan(1);
  expect(visible.pages.length).toBeLessThanOrEqual(512);
  const coverage = page.getByRole("region", { name: "Visible-page buffer observations" });
  await expect(coverage).toContainText("Evaluated");
  await expect(coverage).toContainText("producer scan complete");
  await expect(coverage).toContainText("2000 ms interval");
  const selected = await context.newPage();
  await selected.goto("http://127.0.0.1:41741/page/0/10", { waitUntil: "commit" });
  await selected.getByRole("button", { name: "Enable observations" }).click();
  await expect(selected.getByRole("region", { name: "Selected-page buffer observation" })).toContainText("Observed resident");
  await page.bringToFront();
  await expect(coverage).toContainText("Evaluated 512 / requested 512");
  await page.screenshot({ path: `../.scratch/pgbuf-overlay-implementation/verification/visible-volume-${browserName}.png` });
  await page.getByRole("button", { name: /^Sector 0,/ }).click();
  await expect(page.getByRole("heading", { name: "Sector 0", exact: true })).toBeVisible();
  await expect(coverage).toContainText("Evaluated 64 / requested 64");
  await expect(page.getByRole("gridcell", { name: /^Page 10,/ })).toContainText("Observed resident");
  await page.screenshot({ path: `../.scratch/pgbuf-overlay-implementation/verification/visible-sector-${browserName}.png`, fullPage: true });
  await selected.close();
});


test("viewport overflow rotates explicitly; scrolling revokes scope and HTTP overload keeps disk usable", async ({ page }) => {
  await page.goto("http://127.0.0.1:41741/volume/0", { waitUntil: "commit" });
  const pending = page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe"));
  await page.getByRole("button", { name: "Enable observations" }).click();
  const first = (await pending).postDataJSON();
  const coverage = page.getByRole("region", { name: "Visible-page buffer observations" });
  await expect(coverage).toContainText("Reduced admission");
  await expect(coverage).toContainText("Evaluated 512 / requested 512");
  const next = page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe") && request.postDataJSON().epoch !== first.epoch);
  const second = (await next).postDataJSON();
  expect(second.pages).toHaveLength(512);
  expect(second.pages).not.toEqual(first.pages);
  await page.getByRole("button", { name: /^Sector 20,/ }).scrollIntoViewIfNeeded();
  const churn = await page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe") && request.postDataJSON().pages.some((page: { pageid: number }) => page.pageid >= 20 * 64));
  expect(churn.postDataJSON().epoch).not.toBe(second.epoch);
  await page.route("**/runtime/page-buffer/observe", (route) => route.fulfill({ status: 429, json: { code: "runtime-admission-refused" } }));
  await page.evaluate(() => window.scrollTo(0, 0));
  await expect(page.getByRole("region", { name: "CUBRID page-buffer observation" })).toContainText("Observation overloaded (HTTP 429)");
  await page.getByRole("button", { name: /^Sector 0,/ }).click();
  await expect(page.getByRole("heading", { name: "Sector 0", exact: true })).toBeVisible();
});
