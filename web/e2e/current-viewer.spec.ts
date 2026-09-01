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
  await expect(page.locator("#volmap-react-root")).toHaveAttribute("data-viewer", "react");
  await expect(page.getByRole("banner").getByText("VOLMAP")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Snapshot hierarchy" })).toBeVisible();
  await expect(page.locator("#outcome")).not.toHaveText("loading");
  await expect(page.locator("#drillBreadcrumb")).toContainText("Page 10");
  const pageFacts = page.getByRole("heading", { name: "Page facts" }).locator("..");
  await expect(pageFacts).toBeVisible();
  for (const label of ["File", "File role", "Class OID", "Class/table"]) {
    const value = pageFacts
      .locator("dt", { hasText: new RegExp(`^${label.replace("/", "\\/")}$`) })
      .locator("xpath=following-sibling::dd");
    await expect(value).toHaveText("none");
  }
  expect(browserErrors).toEqual([]);
});

test("the React viewer preserves drilldown, semantic history, and license reachability", async ({
  browserName,
  page,
}) => {
  test.skip(browserName !== "chromium", "Chromium owns the blocking parity corpus");

  await page.goto("/page/0/10");
  await expect(page.locator("#outcome")).not.toHaveText("loading");
  await page.getByRole("navigation", { name: "Inspection hierarchy" }).getByRole("button", { name: "Volume 0" }).click();
  await expect(page).toHaveURL("/volume/0");
  await expect(page.getByRole("heading", { name: "Volume 0 · full map" })).toBeVisible();

  await page.goBack();
  await expect(page).toHaveURL("/page/0/10");
  await expect(page.getByRole("heading", { name: "Page 10" })).toBeVisible();
  await page.goForward();
  await expect(page).toHaveURL("/volume/0");

  await page.getByRole("button", { name: /^Sector 0,/ }).click();
  await expect(page).toHaveURL("/sector/0/0");
  await page.getByRole("gridcell", { name: /^Page 10,/ }).click();
  await expect(page).toHaveURL("/page/0/10");
  await expect(page.getByText("structural ranges only · bytes withheld")).toBeVisible();

  await page.getByRole("button", { name: "About & licenses" }).click();
  await expect(page.locator("#infoDialog")).toBeVisible();
  await expect(page.locator("#infoContent")).toContainText("Volmap");
  await page.getByRole("button", { name: "Close" }).click();
  await expect(page.locator("#infoDialog")).not.toBeVisible();
});
