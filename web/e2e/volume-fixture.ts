import { expect, type Page } from "@playwright/test";

// The scroll sentinel can finish pagination between counting cells and locating
// its button. Activate any remaining button without waiting for a removed one.
export async function loadVolumeCells(page: Page, expected: number): Promise<void> {
  const cells = page.locator("[data-observation-page]");
  for (;;) {
    const before = await cells.count();
    if (before >= expected) break;
    await page.getByRole("button", { name: "Load more sectors" }).evaluateAll((buttons) => {
      buttons.forEach((button) => (button as HTMLButtonElement).click());
    });
    await expect.poll(() => cells.count()).toBeGreaterThan(before);
  }
  await expect(cells).toHaveCount(expected);
}
