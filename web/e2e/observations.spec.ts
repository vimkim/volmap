import { expect, test } from "@playwright/test";

// Firefox can keep document readiness pending alongside the live watch.
// Wait for navigation commit, then assert rendered controls and observations.

// Port 41741 uses producer-fixture.py, not an actual CUBRID server.
test("selected-page observation uses a scripted socket producer, real HTTP and non-color detail", async ({ page, browserName }) => {
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
  // Firefox can render this page while goto still waits on the live watch.
  // Navigate in the document and let the bounded UI assertions prove readiness.
  await other.evaluate(() => { window.location.href = "http://127.0.0.1:41741/page/0/10"; });
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

for (const sector of [false, true]) {
test(`${sector ? "Sector" : "Page"} producer restart while paused clears evidence and requires an explicit retry`, async ({ page }) => {
  const { readFile } = await import("node:fs/promises");
  await page.clock.install();
  await page.goto(`http://127.0.0.1:41741/${sector ? "sector/0/0" : "page/0/10"}`, { waitUntil: "commit" });
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  const detail = page.getByRole("region", { name: sector ? "Visible-page buffer observations" : "Selected-page buffer observation" });
  await source.getByRole("button", { name: "Enable observations" }).click();
  await expect(detail).toContainText(/observed resident/i);
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  const pid = Number((await readFile("/tmp/volmap-browser-producer-41741.pid", "utf8")).trim());
  process.kill(pid, "SIGHUP");
  await page.clock.runFor(5000);
  await expect(source).toContainText("incarnation-changed");
  await expect(detail).not.toContainText(/observed resident/i);
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  await expect(source).toContainText("Automatic retry stopped");
  await source.getByRole("button", { name: sector ? "Refresh visible-page observations" : "Refresh selected-page observation" }).click();
  await expect(detail).toContainText(/observed resident/i);
  await expect(page.getByRole("heading", { name: sector ? "Sector 0" : "Page facts", exact: true })).toBeVisible();
});
}

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
  expect(visible.scope.kind).toBe("volume");
  expect(visible.scope.sectorids.length).toBeGreaterThan(0);
  expect(visible.scope.sectorids.length).toBeLessThanOrEqual(64);
  expect(visible.pages).toBeUndefined();
  const coverage = page.getByRole("region", { name: "Visible-page buffer observations" });
  await expect(coverage).toContainText("Evaluated");
  await expect(coverage).toContainText("producer scan complete");
  await expect(coverage).toContainText("2000 ms interval");
  const selected = await context.newPage();
  await selected.evaluate(() => { window.location.href = "http://127.0.0.1:41741/page/0/10"; });
  await selected.getByRole("button", { name: "Enable observations" }).click();
  await expect(selected.getByRole("region", { name: "Selected-page buffer observation" })).toContainText("Observed resident");
  await page.bringToFront();
  await expect(coverage).toContainText("Evaluated");
  await page.screenshot({ path: `../.scratch/pgbuf-overlay-implementation/verification/visible-volume-${browserName}.png` });
  const sectorRequest = page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe") && request.postDataJSON().scope?.kind === "sector");
  await page.getByRole("button", { name: /^Sector 0,/ }).click();
  const sectorBody = (await sectorRequest).postDataJSON();
  expect(sectorBody.scope).toEqual({ kind: "sector", volid: 0, sectorid: 0 });
  expect(sectorBody.pages).toBeUndefined();
  await expect(page.getByRole("heading", { name: "Sector 0", exact: true })).toBeVisible();
  await expect(coverage).toContainText("Evaluated 64 / requested 64");
  await expect(page.getByRole("gridcell", { name: /^Page 10,/ })).toContainText("Observed resident");
  await page.screenshot({ path: `../.scratch/pgbuf-overlay-implementation/verification/visible-sector-${browserName}.png`, fullPage: true });
  await selected.close();
});


test("viewport selection stays fixed; scrolling revokes scope and HTTP overload keeps disk usable", async ({ page }) => {
  await page.goto("http://127.0.0.1:41741/volume/0", { waitUntil: "commit" });
  const pending = page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe"));
  await page.getByRole("button", { name: "Enable observations" }).click();
  const first = (await pending).postDataJSON();
  const coverage = page.getByRole("region", { name: "Visible-page buffer observations" });
  await expect(coverage).toContainText("selection does not rotate");
  await expect(coverage).toContainText("Evaluated");
  const next = page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe") && request.postDataJSON().epoch !== first.epoch);
  const second = (await next).postDataJSON();
  expect(second.scope).toEqual(first.scope);
  await page.getByRole("button", { name: /^Sector 20,/ }).scrollIntoViewIfNeeded();
  const churn = await page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe") && request.postDataJSON().scope?.sectorids?.includes(20));
  expect(churn.postDataJSON().epoch).not.toBe(second.epoch);
  await page.route("**/runtime/page-buffer/observe", (route) => route.fulfill({ status: 429, json: { code: "runtime-admission-refused" } }));
  await page.evaluate(() => window.scrollTo(0, 0));
  await expect(page.getByRole("region", { name: "CUBRID page-buffer observation" })).toContainText("Observation overloaded (HTTP 429)");
  const sectorRequest = page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe") && request.postDataJSON().scope?.kind === "sector");
  await page.getByRole("button", { name: /^Sector 0,/ }).click();
  const sectorBody = (await sectorRequest).postDataJSON();
  expect(sectorBody.scope).toEqual({ kind: "sector", volid: 0, sectorid: 0 });
  expect(sectorBody.pages).toBeUndefined();
  await expect(page.getByRole("heading", { name: "Sector 0", exact: true })).toBeVisible();
});

for (const scale of ["volume/0", "sector/0/0"]) {
  test(`${scale} composes state marks and switches to explicitly labelled LRU colors`, async ({ page }) => {
    await page.goto(`http://127.0.0.1:41741/${scale}`, { waitUntil: "commit" });
    await page.getByRole("button", { name: "Enable observations" }).click();
    const mode = page.getByRole("combobox", { name: "Runtime color mode" });
    await expect(mode).toHaveValue("state");
    const coverage = page.getByRole("region", { name: "Visible-page buffer observations" });
    await expect(coverage).toContainText("observed resident");
    const cell = page.locator('[data-observation-page="0:10"]');
    // Freeze adoption while testing color-mode presentation.
    await expect(cell).toHaveAttribute("data-runtime-state", "resident");
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    await expect(cell).toHaveClass(/runtime-resident/);
    await expect(cell).toHaveAttribute("title", /Observed resident/);
    const unadmitted = scale.startsWith("volume") ? page.locator("[data-observation-page]:not([data-runtime-state])").first() : null;
    const unadmittedStorage = await unadmitted?.evaluate((element) => getComputedStyle(element).backgroundColor);
    await mode.focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Tab");
    await expect(mode).toHaveValue("lru");
    await expect(cell).toHaveClass(/runtime-lru/);
    await expect(cell.locator(".runtime-glyph")).toHaveText(scale.startsWith("volume") ? "2P" : "2PD");
    if (unadmitted) {
      await expect(unadmitted).toHaveCSS("background-color", "rgb(55, 65, 81)");
      await expect(unadmitted).toHaveAttribute("title", /^Page \d+:/);
      await expect(unadmitted.locator("../..")).toHaveAccessibleName(/Not evaluated/);
      await expect(coverage).toContainText("not evaluated");
    }
    await coverage.getByText("Available page observations and capture limitations", { exact: true }).click();
    const observedRow = coverage.getByRole("row").filter({ has: page.getByRole("cell", { name: "0:10", exact: true }) });
    await expect(observedRow.getByRole("cell", { name: "lru2", exact: true })).toBeVisible();
    await expect(observedRow.getByRole("cell", { name: "private", exact: true })).toBeVisible();
    await expect(page.getByRole("region", { name: "Runtime overlay legend" })).toContainText("LRU topology colors replace storage colors");
    await expect(coverage).toContainText("Shared lists: 2");
    await expect(coverage).toContainText("incarnation-local");
    await expect(coverage).toContainText("Observed per-list counts in this batch only");
    await mode.selectOption("state");
    await expect(cell).not.toHaveClass(/runtime-lru/);
    if (unadmitted && unadmittedStorage) await expect(unadmitted).toHaveCSS("background-color", unadmittedStorage);
  });
}

for (const scale of ["volume/0", "sector/0/0"]) {
  test(`${scale} separates partial unknowns, duplicate ambiguity, nonresidency and expired evidence`, async ({ page, browserName }) => {
    await page.clock.install();
    await page.emulateMedia({ reducedMotion: "reduce" });
    let complete = false;
    await page.route("**/runtime/page-buffer/observe", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      const observations = body.slots ?? body.observations;
      const resident = observations.find((row: { state: string } | null) => row?.state === "resident");
      // Retain the real normalized capture envelope and request echo. Only the
      // semantic row scenarios vary, through the production decoder/effects.
      if (resident) {
        body.producer_complete = complete;
        const rows = observations.map((vpid: { volid: number; pageid: number } | null, index: number) => {
          if (vpid === null) return null;
          const pageid = body.scope.kind === "volume" ? body.scope.sectorids[Math.floor(index / 64)] * 64 + index % 64 : vpid.pageid;
          if (!complete && [10, 11].includes(pageid)) return { ...vpid, state: "resident", reason: "observed-resident", evidence: { ...resident.evidence, ...(body.scope.kind === "volume" ? {} : { flushing: true }), lru_zone: "lru2", lru_list_kind: pageid === 10 ? "private" : "shared", lru_list_index: pageid === 10 ? 1 : 0 } };
          return { ...vpid, state: complete ? "not-resident" : "unknown", reason: complete ? "observed-not-resident" : pageid === 12 ? "duplicate-vpid" : pageid === 13 ? "unevaluated" : "partial-omission", evidence: null };
        });
        if (body.slots) body.slots = rows;
        else body.observations = rows;
        body.evaluated_count = rows.filter((row: { reason: string } | null) => row !== null && row.reason !== "unevaluated").length;
      }
      await route.fulfill({ response, json: body });
    });
    await page.goto(`http://127.0.0.1:41741/${scale}`, { waitUntil: "commit" });
    await expect(page.locator("#crumb")).toContainText("revision");
    const revision = await page.locator("#crumb").innerText();
    const cell = (id: number) => page.locator(`[data-observation-page="0:${id}"]`);
    const storage = await cell(10).evaluate((element) => getComputedStyle(element).background);
    await page.getByRole("button", { name: "Enable observations" }).click();
    await expect(cell(10)).toHaveClass(scale.startsWith("volume") ? /runtime-resident/ : /runtime-dirty runtime-flushing/);
    expect(await cell(10).evaluate((element) => getComputedStyle(element).background)).toBe(storage);
    await expect(cell(12)).toHaveAttribute("title", /duplicate-vpid/);
    await expect(cell(13)).toHaveAttribute("title", /unevaluated/);
    await expect(cell(14)).toHaveAttribute("title", /partial-omission/);
    const coverage = page.getByRole("region", { name: "Visible-page buffer observations" });
    await coverage.getByText("Observed per-list counts in this batch only", { exact: true }).click();
    await expect(coverage).toContainText("Partial producer scan; partial summary");
    await expect(coverage).toContainText("private list 1: 1 observed resident");
    await expect(coverage).toContainText("shared list 0: 1 observed resident");
    if (scale.startsWith("volume")) {
      await expect(page.getByRole("button", { name: /^Sector 0,/ })).toHaveAccessibleName(/duplicate-vpid/);
    } else {
      await expect(cell(10)).toHaveAccessibleName(/dirty true · flushing true/);
      await cell(10).focus();
      await page.clock.runFor(2000);
      await expect(cell(10)).toBeFocused();
      await page.keyboard.press("ArrowRight");
      await expect(cell(11)).toBeFocused();
    }
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    await page.clock.runFor(4500);
    await expect(cell(10)).toHaveAttribute("title", /stale/);
    await expect(coverage).toContainText("paused");
    await page.getByRole("combobox", { name: "Runtime color mode" }).selectOption("lru");
    await expect(cell(10)).toHaveClass(/lru-private/);
    await page.emulateMedia({ forcedColors: "active" });
    await expect(cell(10).locator(".runtime-glyph")).toHaveText(scale.startsWith("volume") ? "2" : "2DF");
    expect(await cell(10).evaluate((element) => getComputedStyle(element, "::before").borderStyle)).toBe("dashed");
    await page.emulateMedia({ forcedColors: "none" });
    await page.screenshot({ path: `../.scratch/pgbuf-overlay-implementation/verification/05-${scale.split("/")[0]}-${browserName}.png` });
    await page.clock.runFor(31000);
    await expect(cell(10)).toHaveAttribute("data-runtime-state", "expired");
    await expect(cell(10)).not.toHaveClass(/runtime-resident/);
    await expect(coverage).not.toContainText("private list 1: 1");
    complete = true;
    await page.getByRole("button", { name: "Resume", exact: true }).click();
    await expect(cell(10)).toHaveAttribute("title", /Observed not resident/);
    await expect(cell(10)).not.toHaveClass(/runtime-resident/);
    await expect(coverage).toContainText("No exact list membership observed");
    await expect(page.locator("#crumb")).toHaveText(revision);
  });
}

test("selected detail exposes exact membership and null meanings; inconsistent topology is refused", async ({ page }) => {
  let variant = "private";
  await page.route("**/runtime/page-buffer/observe", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    const row = body.observations[0];
    if (row.evidence) {
      row.evidence.lru_list_kind = variant === "out-of-range" ? "private" : variant;
      row.evidence.lru_list_index = variant === "private" ? 1 : variant === "out-of-range" ? 3 : null;
      row.evidence.lru_zone = variant === "none" ? "void" : "lru2";
    }
    await route.fulfill({ response, json: body });
  });
  await page.goto("http://127.0.0.1:41741/page/0/10", { waitUntil: "commit" });
  await page.getByRole("button", { name: "Enable observations" }).click();
  const detail = page.getByRole("region", { name: "Selected-page buffer observation" });
  await expect(detail).toContainText("Observed resident");
  await expect(detail).toContainText("lru list index1");
  await expect(detail).toContainText("incarnation-local");
  for (const kind of ["none", "invalid"]) {
    variant = kind;
    await expect(detail).toContainText(`lru list kind${kind}`);
    await expect(detail).toContainText("lru list indexnone / not applicable");
  }
  variant = "out-of-range";
  await expect(page.getByRole("region", { name: "CUBRID page-buffer observation" })).toContainText("Incompatible observation response");
  await expect(detail).not.toContainText("Observed resident");
});

for (const scale of ["volume/0", "sector/0/0"]) {
  test(`${scale} source failure hides marks and paused incarnation change revokes every list`, async ({ page }) => {
    await page.clock.install();
    await page.goto(`http://127.0.0.1:41741/${scale}`, { waitUntil: "commit" });
    await page.getByRole("button", { name: "Enable observations" }).click();
    const cell = page.locator('[data-observation-page="0:10"]');
    await expect(cell).toHaveAttribute("data-runtime-state", "resident");
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    await page.route("**/runtime/capabilities", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      body.incarnation_binding = "a-new-server-incarnation";
      await route.fulfill({ response, json: body });
    });
    await page.clock.runFor(5000);
    const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
    await expect(source).toContainText("incarnation-changed");
    await expect(cell).not.toHaveClass(/runtime-resident/);
    await expect(page.getByRole("region", { name: "Visible-page buffer observations" })).not.toContainText("Shared lists:");
    await page.unroute("**/runtime/capabilities");
    await page.getByRole("button", { name: "Resume", exact: true }).click();
    await page.getByRole("button", { name: "Refresh visible-page observations" }).click();
    await expect(cell).toHaveAttribute("data-runtime-state", "resident");
    await page.route("**/runtime/page-buffer/observe", (route) => route.abort("failed"));
    await page.clock.runFor(2000);
    await expect(source).toContainText("Observation source: unavailable");
    await expect(cell).toHaveAttribute("data-runtime-state", "unavailable");
    await expect(cell).not.toHaveClass(/runtime-resident/);
    await expect(cell).not.toHaveAttribute("title", /Observed not resident/);
    await expect(page.getByRole("region", { name: "Runtime overlay legend" })).toContainText("Source: unavailable");
    await expect(page.getByRole("heading", { level: 1 })).toContainText(scale.startsWith("volume") ? "Volume 0" : "Sector 0");
  });
}

test("cached unavailable metadata does not turn a pending first capture into page failures", async ({ page }) => {
  await page.route("**/runtime/capabilities", (route) => route.fulfill({
    status: 200, headers: { "cache-control": "no-store" },
    json: { schema: "volmap.runtime", schema_version: 1, source: "cubrid-page-buffer-observation",
      state: "unavailable", verification: "unverified", reason: "observation-expired",
      incarnation_binding: null, capture_identity: null, revision: "0" },
  }));
  let releaseCapture!: () => void;
  const captureGate = new Promise<void>((resolve) => { releaseCapture = resolve; });
  await page.route("**/runtime/page-buffer/observe", async (route) => {
    await captureGate;
    await route.fulfill({ status: 503, json: { code: "runtime-unavailable" } });
  });
  await page.goto("http://127.0.0.1:41741/volume/0", { waitUntil: "commit" });
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  await expect(source).toContainText("Observation source: unavailable");
  const pending = page.waitForRequest("**/runtime/page-buffer/observe");
  await source.getByRole("button", { name: "Enable observations" }).click();
  await pending;
  try {
    await expect(source).toContainText("Observing visible pages");
    const refresh = source.getByRole("button", { name: "Refresh visible-page observations" });
    await expect(refresh).toBeDisabled();
    await expect(refresh).toHaveCSS("opacity", "1");
    await expect(refresh).toHaveCSS("outline-style", "dashed");
    await expect(page.locator('.preview-page[data-runtime-state="unavailable"]')).toHaveCount(0);
    await expect(page.locator(".sector-card").first()).toHaveAccessibleName(/source unavailable/);
  } finally {
    releaseCapture();
  }
  await expect(source).toContainText("Observation unavailable");
  await expect(page.locator('.preview-page[data-runtime-state="unavailable"]').first()).toHaveAttribute("title", /No usable observation/);
  await page.unroute("**/runtime/page-buffer/observe");
  await source.getByRole("button", { name: "Refresh visible-page observations" }).click();
  await expect(source).toContainText("Observation source: active");
  await expect(page.locator('.preview-page[data-runtime-state="resident"]').first()).toHaveAttribute("title", /Observed resident/);
});

test("enabling Volume observations keeps the map in place and the legend available", async ({ page }) => {
  await page.goto("http://127.0.0.1:41741/volume/0", { waitUntil: "commit" });
  const map = page.locator("#volumeMap");
  await expect(map).toBeVisible();
  const top = await map.evaluate((element) => element.getBoundingClientRect().top + window.scrollY);
  await page.getByRole("button", { name: "Enable observations", exact: true }).click();
  const legend = page.getByRole("region", { name: "Runtime overlay legend" });
  await expect(legend).toBeVisible();
  await expect(legend).toContainText("Pages outside this batch have no runtime glyph");
  expect(await map.evaluate((element) => element.getBoundingClientRect().top + window.scrollY)).toBe(top);
  await page.getByRole("combobox", { name: "Runtime color mode" }).selectOption("lru");
  await expect(legend).toContainText("LRU topology colors replace storage colors");
  expect(await map.evaluate((element) => element.getBoundingClientRect().top + window.scrollY)).toBe(top);
});

test("Volume nonresident markers share an image and retain a forced-color glyph", async ({ page, browserName }) => {
  await page.clock.install();
  await page.goto("http://127.0.0.1:41741/volume/0", { waitUntil: "commit" });
  await page.getByRole("button", { name: "Enable observations", exact: true }).click();
  const cell = page.locator('.preview-page[data-runtime-state="not-resident"]').first();
  await expect(cell).toHaveAttribute("title", /Observed not resident/);
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  const bounds = await cell.boundingBox();
  await expect(cell).toHaveCSS("background-image", /data:image\/svg\+xml/);
  await expect(cell.locator(".runtime-glyph")).toHaveCount(0);
  await cell.locator("../..").screenshot({ path: `../.scratch/pgbuf-overlay-implementation/verification/06-shared-glyph/normal-sector-${browserName}.png` });
  await page.getByRole("combobox", { name: "Runtime color mode" }).selectOption("lru");
  await expect(cell).toHaveCSS("background-image", /data:image\/svg\+xml/);
  await expect(cell).toHaveCSS("background-color", "rgb(55, 65, 81)");
  await page.emulateMedia({ forcedColors: "active" });
  await expect(cell).toHaveCSS("background-image", "none");
  expect(await cell.evaluate((element) => getComputedStyle(element, "::after").content)).toBe('"○"');
  expect(await cell.boundingBox()).toEqual(bounds);
  await cell.locator("../..").screenshot({ path: `../.scratch/pgbuf-overlay-implementation/verification/06-shared-glyph/forced-sector-${browserName}.png` });
  await expect(page.getByRole("region", { name: "Runtime overlay legend" })).toContainText("○ observed not resident");
  const identity = await cell.getAttribute("data-observation-page");
  await page.getByRole("button", { name: "Disable observations", exact: true }).click();
  const cleared = page.locator(`[data-observation-page="${identity}"]`);
  await expect(cleared).not.toHaveAttribute("data-runtime-state");
  await expect(cleared).toHaveCSS("background-image", "none");
  expect(await cleared.evaluate((element) => getComputedStyle(element, "::after").content)).toBe("none");
});

test("Volume nonresident circles preserve known and unknown occupancy backgrounds", async ({ page }) => {
  await page.clock.install();
  await page.route(/\/api\/v1\/sectors\/0\?/, async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    // Exercise both storage projections through the actual HTTP decoder.
    for (const sector of body.data.items) {
      for (const item of sector.pages) {
        if (![11, 12].includes(item.page_id)) continue;
        item.allocation = "allocated";
        item.occupancy = item.page_id === 11
          ? { state: "known", occupied_percent: 35, free_percent: 65 }
          : { state: "unknown" };
      }
    }
    await route.fulfill({ response, json: body });
  });
  await page.goto("http://127.0.0.1:41741/volume/0", { waitUntil: "commit" });
  const cells = [11, 12].map((id) => page.locator(`[data-observation-page="0:${id}"]`));
  const storage = [];
  for (const cell of cells) {
    await expect(cell).toHaveClass(/allocated occupancy-/);
    storage.push(await cell.evaluate((element) => getComputedStyle(element).backgroundImage));
  }
  await page.getByRole("button", { name: "Enable observations", exact: true }).click();
  for (const cell of cells) await expect(cell).toHaveAttribute("data-runtime-state", "not-resident");
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  for (const [index, cell] of cells.entries()) {
    await expect(cell).toHaveCSS("background-image", /data:image\/svg\+xml/);
    expect(await cell.evaluate((element) => getComputedStyle(element).backgroundImage)).toContain(storage[index]);
    await expect(cell).toHaveCSS("background-size", /5px 9px, auto/);
  }
  await page.getByRole("combobox", { name: "Runtime color mode" }).selectOption("lru");
  for (const cell of cells) {
    await expect(cell).toHaveCSS("background-image", /data:image\/svg\+xml/);
    await expect(cell).not.toHaveCSS("background-image", /gradient/);
    await expect(cell).toHaveCSS("background-color", "rgb(55, 65, 81)");
  }
  await page.getByRole("button", { name: "Disable observations", exact: true }).click();
  for (const [index, cell] of cells.entries()) {
    await expect(cell).toHaveCSS("background-image", storage[index]!);
    await expect(cell).toHaveCSS("background-size", "auto");
  }
});

for (const malformed of ["schema", "variant", "scope", "slot-order", "slot-count", "epoch", "generation"]) {
  test(`Sector refuses malformed ${malformed} response without losing disk navigation`, async ({ page }) => {
    await page.route("**/runtime/page-buffer/observe", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      if (malformed === "schema") body.schema_version = 2;
      if (malformed === "variant") body.variant = "volume-residency";
      if (malformed === "scope") body.scope.sectorid = 1;
      if (malformed === "slot-order") [body.slots[0], body.slots[1]] = [body.slots[1], body.slots[0]];
      if (malformed === "slot-count") body.slots.pop();
      if (malformed === "epoch") body.epoch = "99999";
      if (malformed === "generation") body.generation = "99999";
      await route.fulfill({ response, json: body });
    });
    await page.goto("http://127.0.0.1:41741/sector/0/0", { waitUntil: "commit" });
    await page.getByRole("button", { name: "Enable observations" }).click();
    await expect(page.getByRole("region", { name: "CUBRID page-buffer observation" })).toContainText("Incompatible observation response");
    await expect(page.locator('[data-observation-page="0:10"]')).not.toHaveClass(/runtime-resident/);
    await page.getByRole("gridcell", { name: /^Page 10,/ }).click();
    await expect(page.getByRole("heading", { name: "Page facts" })).toBeVisible();
  });
}

for (const scale of ["volume/0", "sector/0/0"]) {
test(`${scale} pause rejects a delayed response and resume starts fresh scope demand`, async ({ page }) => {
  await page.clock.install();
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  let arrived!: () => void;
  const captured = new Promise<void>((resolve) => { arrived = resolve; });
  let first = true;
  await page.route("**/runtime/page-buffer/observe", async (route) => {
    if (!first) { await route.continue(); return; }
    first = false;
    const response = await route.fetch();
    arrived();
    await held;
    await route.fulfill({ response }).catch(() => {});
  });
  await page.goto(`http://127.0.0.1:41741/${scale}`, { waitUntil: "commit" });
  await page.getByRole("button", { name: "Enable observations" }).click();
  await captured;
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  release();
  await page.clock.runFor(5000);
  const cell = page.locator('[data-observation-page="0:10"]');
  await expect(cell).not.toHaveClass(/runtime-resident/);
  const resumed = page.waitForRequest((request) => request.url().endsWith("/page-buffer/observe"));
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  expect((await resumed).postDataJSON()).toMatchObject({ scope: { kind: scale.startsWith("volume") ? "volume" : "sector", volid: 0 }, after_request: true });
  await expect(cell).toHaveAttribute("data-runtime-state", "resident");
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  await page.clock.runFor(31000);
  await expect(cell).toHaveAttribute("data-runtime-state", "expired");
  await expect(page.getByRole("heading", { name: scale.startsWith("volume") ? "Volume 0 · full map" : "Sector 0", exact: true })).toBeVisible();
});

}
