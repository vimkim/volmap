import { expect, test, type Response } from "@playwright/test";
import { loadVolumeCells } from "./volume-fixture";

const evidenceRoot = "../.scratch/volume-overlay-64-sectors-implementation/verification/02-volume-scope";

// Header arrival may precede a scope cancellation. Only completed response
// bodies can be candidates for the successful adoption assertions below.
async function completedObservation(response: Response): Promise<boolean> {
  if (!response.url().endsWith("/page-buffer/observe") || response.status() !== 200) return false;
  try { await response.json(); return true; }
  catch { return false; }
}

test("12288 rendered cells adopt 4096 changing results from one capture with fixed central selection", async ({ page, browserName }) => {
  await page.setViewportSize({ width: 2560, height: 2160 });
  await page.goto("http://127.0.0.1:41743/volume/0", { waitUntil: "commit" });
  await loadVolumeCells(page, 12288);
  const renderedCells = await page.locator("[data-observation-page]").evaluateAll((elements) => elements.filter((element) => element.getClientRects().length > 0).length);
  expect(renderedCells).toBe(12288);
  await page.evaluate(() => window.scrollTo(0, 0));
  const expectedSelection = () => page.locator(".sector-card").evaluateAll((elements) => {
    const visible = elements.flatMap((element) => {
      const rect = element.getBoundingClientRect();
      if (rect.bottom <= 0 || rect.top >= innerHeight || rect.right <= 0 || rect.left >= innerWidth) return [];
      return [{ id: Number(element.id.replace("sector-", "")), distance: Math.hypot((rect.top + rect.bottom) / 2 - innerHeight / 2, (rect.left + rect.right) / 2 - innerWidth / 2) }];
    });
    return { visible: visible.length, ids: visible.sort((a, b) => a.distance - b.distance || a.id - b.id).slice(0, 64).map((sector) => sector.id).sort((a, b) => a - b) };
  });
  const responsePromise = page.waitForResponse(async (response) => await completedObservation(response) &&
    JSON.stringify(response.request().postDataJSON().scope.sectorids) === JSON.stringify((await expectedSelection()).ids));
  await page.getByRole("button", { name: "Enable observations" }).click();
  const firstResponse = await responsePromise;
  const first = await firstResponse.json();
  const expected = await expectedSelection();
  expect(expected.visible).toBeGreaterThan(64);
  expect(first.scope).toEqual({ kind: "volume", volid: 0, sectorids: expected.ids });
  expect(firstResponse.request().postDataJSON().pages).toBeUndefined();
  expect(first.slots).toHaveLength(4096);
  expect(first.requested_count).toBe(4096);
  expect(first.evaluated_count).toBe(4096);
  expect(first.slots.every((row: { state: string }) => row.state === "resident")).toBe(true);
  const residentCells = page.locator('[data-observation-page][data-runtime-state="resident"]');
  await expect(residentCells).toHaveCount(4096);
  const visibleResidentCells = await residentCells.evaluateAll((elements) => elements.filter((element) => {
    const rect = element.getBoundingClientRect();
    return rect.top >= 0 && rect.bottom <= innerHeight && rect.left >= 0 && rect.right <= innerWidth;
  }).length);
  expect(visibleResidentCells).toBe(4096);
  const coverage = page.getByRole("region", { name: "Visible-page buffer observations" });
  await expect(coverage).toContainText("selection does not rotate");
  const unqueried = page.locator(".sector-card").filter({ has: page.locator("[data-observation-page]:not([data-runtime-state])") }).first();
  await expect(unqueried).toHaveAccessibleName(/Not evaluated/);
  const focused = page.locator(`#sector-${expected.ids[0]}`);
  await focused.evaluate((element: HTMLElement) => element.focus({ preventScroll: true }));
  const nextResponse = await page.waitForResponse(async (response) => await completedObservation(response) && (await response.json()).capture?.identity !== first.capture.identity);
  const second = await nextResponse.json();
  expect(second.scope).toEqual(first.scope);
  expect(second.slots.every((row: { evidence: { lru_zone: string } }, index: number) => row.evidence.lru_zone !== first.slots[index].evidence.lru_zone)).toBe(true);
  await expect(coverage).toContainText(`scan ${second.capture.sequence} ·`);
  await expect(focused).toBeFocused();
  await expect(residentCells).toHaveCount(4096);
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  const mode = page.getByRole("combobox", { name: "Runtime color mode" });
  await mode.selectOption("lru");
  await expect(page.locator(`.runtime-resident.runtime-lru.zone-${second.slots[0].evidence.lru_zone}`)).toHaveCount(4096);
  await expect(residentCells.first()).toHaveAttribute("title", /membership · index/);
  await expect(residentCells.first()).not.toHaveAttribute("title", /dirty|flushing|undefined/);
  await coverage.getByText("Available page observations and capture limitations", { exact: true }).click();
  await expect(coverage.getByRole("row")).toHaveCount(4097);
  await expect(coverage.getByRole("columnheader", { name: "Dirty", exact: true })).toHaveCount(0);
  await expect(coverage.getByRole("columnheader", { name: "List index", exact: true })).toBeVisible();
  await expect(coverage).toContainText("records and scans are not atomic");
  await page.screenshot({ path: `${evidenceRoot}/volume-${browserName}.png` });
  await coverage.getByText("Available page observations and capture limitations", { exact: true }).click();
  await mode.selectOption("state");
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  const resized = page.waitForResponse(async (response) => await completedObservation(response) && response.request().postDataJSON().scope?.sectorids.length < 64 && JSON.stringify(response.request().postDataJSON().scope.sectorids) === JSON.stringify((await expectedSelection()).ids));
  await page.setViewportSize({ width: 1280, height: 720 });
  const smaller = await (await resized).json();
  expect(smaller.scope.sectorids).toEqual((await expectedSelection()).ids);
  await expect(residentCells).toHaveCount(smaller.requested_count);
  const scrolled = page.waitForResponse(async (response) => await completedObservation(response) && JSON.stringify(response.request().postDataJSON().scope) !== JSON.stringify(smaller.scope) && JSON.stringify(response.request().postDataJSON().scope.sectorids) === JSON.stringify((await expectedSelection()).ids));
  await page.evaluate(() => window.scrollBy(0, 1000));
  const moved = await (await scrolled).json();
  expect(moved.scope.sectorids).toEqual((await expectedSelection()).ids);
  await expect(residentCells).toHaveCount(moved.requested_count);
  const { writeFile, mkdir } = await import("node:fs/promises");
  await mkdir(evidenceRoot, { recursive: true });
  await writeFile(`${evidenceRoot}/browser-${browserName}.json`, JSON.stringify({ browserName, renderedCells, visibleResidentCells, visibleSectors: expected.visible,
    queriedPages: first.requested_count, residentCells: 4096, captureIdentities: [first.capture.identity, second.capture.identity],
    requestBytes: Buffer.byteLength(firstResponse.request().postData()!), responseBytes: (await firstResponse.body()).length,
    selectedSectors: first.scope.sectorids, resizedSectors: smaller.scope.sectorids, scrolledSectors: moved.scope.sectorids }, null, 2) + "\n");
});

for (const malformed of ["scope", "sector-order", "slot-count", "epoch", "generation", "detail-evidence", "oversized-body"]) {
  test(`Volume refuses malformed ${malformed} response as a whole`, async ({ page }) => {
    await page.route("**/runtime/page-buffer/observe", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      if (malformed === "scope") body.scope.volid = 1;
      if (malformed === "sector-order") body.scope.sectorids.reverse();
      if (malformed === "slot-count") body.slots.push(null);
      if (malformed === "epoch") body.epoch = "99999";
      if (malformed === "generation") body.generation = "99999";
      if (malformed === "detail-evidence") body.slots.find((row: { state: string }) => row.state === "resident").evidence.dirty = false;
      if (malformed === "oversized-body") await route.fulfill({ response, body: JSON.stringify(body) + " ".repeat(1_048_576) });
      else await route.fulfill({ response, json: body });
    });
    await page.goto("http://127.0.0.1:41741/volume/0", { waitUntil: "commit" });
    await page.getByRole("button", { name: "Enable observations" }).click();
    await expect(page.getByRole("region", { name: "CUBRID page-buffer observation" })).toContainText("Incompatible observation response");
    await expect(page.locator(".runtime-resident")).toHaveCount(0);
    await page.getByRole("button", { name: /^Sector 0,/ }).click();
    await expect(page.getByRole("heading", { name: "Sector 0", exact: true })).toBeVisible();
  });
}
