import { expect, test } from "@playwright/test";

test("the embedded live viewer boots from the real server and supports a direct entity route", async ({
  page,
}) => {
  const browserErrors: string[] = [];
  page.on("pageerror", (error) => browserErrors.push(error.message));

  const response = await page.goto("/page/0/10");
  expect(response).not.toBeNull();
  expect(response?.headers()["content-security-policy"]).toContain("default-src 'none'");
  expect(response?.headers()["content-security-policy"]).toContain("script-src 'self'");
  expect(response?.headers()["cache-control"]).toBe("no-store");

  await expect(page).toHaveTitle("Volmap Inspector");
  await expect(page.getByRole("banner").getByText("VOLMAP")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Snapshot hierarchy" })).toBeVisible();
  await expect(page.locator("#outcome")).not.toHaveText("loading");
  await expect(page.locator("#drillBreadcrumb")).toContainText("Page 10");
  expect(browserErrors).toEqual([]);
});
