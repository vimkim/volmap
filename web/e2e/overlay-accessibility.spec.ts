import { loadVolumeCells } from "./volume-fixture";
import { expect, test } from "@playwright/test";

for (const scale of ["volume/0", "sector/0/0"]) {
  test(`${scale} keyboard focus survives quiet polling and age updates`, async ({ page }) => {
    await page.clock.install();
    await page.goto(`http://127.0.0.1:41741/${scale}`, { waitUntil: "commit" });
    if (scale.startsWith("volume")) {
      await loadVolumeCells(page, 4096);
      await expect(page.getByRole("status")).toContainText("All 64 sectors shown");
    }
    const enable = page.getByRole("button", { name: "Enable observations" });
    await enable.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("region", { name: "Visible-page buffer observations" })).toContainText("Evaluated");
    const target = scale.startsWith("volume")
      ? page.getByRole("button", { name: /^Sector 20,/ })
      : page.getByRole("gridcell", { name: /^Page 10,/ });
    if (scale.startsWith("sector")) {
      const grid = page.getByRole("grid", { name: "Sector 0, 64 physical pages" });
      await expect(grid.getByRole("row")).toHaveCount(8);
      for (const row of await grid.getByRole("row").all()) {
        await expect(row.getByRole("gridcell")).toHaveCount(8);
      }
    }
    await target.focus();
    // Observe live-region mutations, including implicit status/alert roles.
    await page.evaluate(() => {
      const changes: string[] = [];
      Object.assign(window, { overlayAnnouncements: changes });
      new MutationObserver((records) => {
        for (const record of records) {
          const element = record.target instanceof Element ? record.target : record.target.parentElement;
          if (element?.closest('[aria-live="polite"], [aria-live="assertive"], [role="status"], [role="alert"]')) {
            changes.push(element.textContent ?? "");
          }
        }
      }).observe(document.body, { subtree: true, childList: true, characterData: true });
    });
    await page.clock.runFor(2500);
    await expect(target).toBeFocused();
    expect(await page.evaluate(() => Reflect.get(window, "overlayAnnouncements"))).toEqual([]);
    await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
    await expect(target).toBeFocused();
    expect(await target.evaluate((element) => getComputedStyle(element).outlineStyle)).not.toBe("none");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading", { name: scale.startsWith("volume") ? "Sector 20" : "Page facts", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { level: 1 })).toBeFocused();
    await expect(page.getByRole("heading", { level: 1 })).toBeInViewport();
  });
}

for (const scale of ["volume/0", "sector/0/0"]) {
  test(`${scale} absent and refused sources retain named disk navigation without absence marks`, async ({ page }) => {
    await page.clock.install();
    for (const refused of [false, true]) {
      if (refused) {
        await page.route("**/runtime/page-buffer/observe", async (route) => {
          const response = await route.fetch();
          const body = await response.json();
          body.capability = { ...body.capability, state: "refused", reason: "peer-refused", verification: "unverified" };
          body.capture = null;
          body.producer_complete = null;
          body.evaluated_count = 0;
          body.observations = body.pages.map((vpid: { volid: number; pageid: number }) => ({ ...vpid, state: "unavailable", reason: "no-usable-observation", evidence: null }));
          await route.fulfill({ response, json: body });
        });
      }
      await page.goto(`http://127.0.0.1:${refused ? 41741 : 41740}/${scale}`, { waitUntil: "commit" });
      await page.getByRole("button", { name: "Enable observations" }).click();
      const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
      await expect(source).toContainText(`Observation source: ${refused ? "refused" : "unavailable"}`);
      const cell = page.locator('[data-observation-page="0:10"]');
      await expect(cell).toHaveAttribute("title", /No usable observation/);
      await expect(cell).not.toHaveAttribute("title", /Observed not resident/);
      await expect(page.getByRole("region", { name: "Runtime overlay legend" })).toContainText(`Source: ${refused ? "refused" : "unavailable"}`);
      if (refused) await expect(source).toContainText("Automatic retry stopped");
      await expect(page.getByRole("heading", { level: 1 })).toContainText(scale.startsWith("volume") ? "Volume 0" : "Sector 0");
    }
  });
}
