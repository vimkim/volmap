import { expect, test } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { closeSync, openSync, readFileSync, writeSync } from "node:fs";
import { join } from "node:path";

function runtime() {
  const config = JSON.parse(readFileSync(process.env.VOLMAP_PRODUCER_RUN!, "utf8"));
  return JSON.parse(readFileSync(join(config.output_directory, "runtime.json"), "utf8"));
}

function producerCommand(action: "start" | "stop") {
  const run = runtime();
  // The daemon inherits stdout. A pipe would keep execFileSync waiting for EOF
  // even after the startup utility has completed successfully.
  const log = openSync(join(run.output_directory, "browser-lifecycle.log"), "a");
  writeSync(log, `$ cubrid server ${action} volmap07\n`);
  try {
    execFileSync(join(run.producer_install, "bin/cubrid"), ["server", action, "volmap07"], {
      cwd: run.database_root,
      env: { ...process.env, CUBRID: run.producer_install,
        CUBRID_DATABASES: run.database_root, CUBRID_TMP: run.database_root,
        CUBRID_CONF_FILE: join(run.database_root, "cubrid.conf"),
        PATH: `${run.producer_install}/bin:${process.env.PATH}`,
        LD_LIBRARY_PATH: `${run.producer_install}/lib:${run.producer_install}/cci/lib` },
      timeout: 15_000,
      stdio: ["ignore", log, log],
    });
  } finally {
    closeSync(log);
  }
}

function sanitized(value: unknown) {
  if (value === null || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    expect(["pid", "uid", "gid", "path", "socket_path", "raw_error", "pointer", "payload"]).not.toContain(key);
    sanitized(child);
  }
}

test("an installed CUBRID producer attaches through HTTP and renders selected-page evidence", async ({ page }, testInfo) => {
  await page.goto("/page/0/0", { waitUntil: "commit" });
  await expect(page.getByRole("heading", { name: "Page facts" })).toBeVisible();
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  const observed = page.waitForResponse((response) => response.url().endsWith("/runtime/page-buffer/observe"));
  await source.getByRole("button", { name: "Enable observations" }).click();
  await expect(source).toContainText("Observation source: active");
  const detail = page.getByRole("region", { name: "Selected-page buffer observation" });
  await expect(detail).toContainText("VPID 0:0");
  await expect(detail).toContainText("Observed resident");
  const capability = await page.request.get("/api/v1/runtime/capabilities");
  expect(capability.status()).toBe(200);
  expect(capability.headers()["cache-control"]).toBe("no-store");
  expect(await capability.json()).toMatchObject({
    source: "cubrid-page-buffer-observation", state: "active", verification: "verified",
  });
  const response = await observed;
  expect(response.status()).toBe(200);
  expect(response.headers()["cache-control"]).toBe("no-store");
  const batch = await response.json();
  expect(batch).toMatchObject({ requested_count: 1, evaluated_count: 1 });
  expect(batch.capture).not.toBeNull();
  sanitized(batch);
  sanitized(await capability.json());
  const run = runtime();
  for (const privateValue of [run.database_root, run.producer_install, run.socket]) {
    expect(JSON.stringify(batch)).not.toContain(privateValue);
    expect(await page.locator("body").innerText()).not.toContain(privateValue);
  }
  await testInfo.attach("observation", { body: JSON.stringify(batch, null, 2), contentType: "application/json" });
  await testInfo.attach("selected-page", { body: await page.screenshot({ fullPage: true }), contentType: "image/png" });
});

test("copied permanent volumes refuse the real producer while disk navigation remains available", async ({ page }) => {
  const run = runtime();
  await page.goto(`${run.copy_origin}/page/0/0`, { waitUntil: "commit" });
  await expect(page.getByRole("heading", { name: "Page facts" })).toBeVisible();
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  await source.getByRole("button", { name: "Enable observations" }).click();
  await expect(source).toContainText("identity-mismatch");
  await expect(source).toContainText("Automatic retry stopped");
  const response = await page.request.get(`${run.copy_origin}/api/v1/runtime/capabilities`);
  expect(response.status()).toBe(200);
  expect(response.headers()["cache-control"]).toBe("no-store");
  const capability = await response.json();
  expect(capability).toMatchObject({ state: "refused", reason: "identity-mismatch", capture_identity: null });
  sanitized(capability);
  await expect(page.getByRole("region", { name: "Selected-page buffer observation" })).not.toContainText("Observed resident");
  await expect(page.getByRole("heading", { name: "Page facts" })).toBeVisible();
});

test("a real producer restart while paused revokes its incarnation and explicit retry recovers", async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto("/page/0/0", { waitUntil: "commit" });
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  const detail = page.getByRole("region", { name: "Selected-page buffer observation" });
  await source.getByRole("button", { name: "Enable observations" }).click();
  await expect(detail).toContainText("Observed resident");
  const before = await (await page.request.get("/api/v1/runtime/capabilities")).json();
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  producerCommand("stop");
  producerCommand("start");
  await expect(source).toContainText("incarnation-changed", { timeout: 10_000 });
  await expect(detail).not.toContainText("Observed resident");
  await expect(page.getByRole("heading", { name: "Page facts" })).toBeVisible();
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  await expect(source).toContainText("Automatic retry stopped");
  await source.getByRole("button", { name: "Refresh selected-page observation" }).click();
  await expect(detail).toContainText("Observed resident");
  const after = await (await page.request.get("/api/v1/runtime/capabilities")).json();
  expect(after.state).toBe("active");
  expect(after.incarnation_binding).not.toBe(before.incarnation_binding);
});

for (const route of ["volume/0", "sector/0/0"]) {
  test(`${route} renders actual resident marks and incarnation-local LRU topology`, async ({ page }, testInfo) => {
    await page.goto(`/${route}`, { waitUntil: "commit" });
    await page.getByRole("button", { name: "Enable observations" }).click();
    const cell = page.locator('[data-observation-page="0:0"]');
    await expect(cell).toHaveAttribute("data-runtime-state", "resident");
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    await expect(cell).toHaveClass(/runtime-resident/);
    await page.getByRole("combobox", { name: "Runtime color mode" }).selectOption("lru");
    await expect(cell).toHaveClass(/runtime-lru/);
    const coverage = page.getByRole("region", { name: "Visible-page buffer observations" });
    await expect(coverage).toContainText("incarnation-local");
    await expect(coverage).toContainText("Observed per-list counts in this batch only");
    await testInfo.attach("lru-topology", { body: await page.screenshot({ fullPage: true }), contentType: "image/png" });
    await page.getByRole("combobox", { name: "Runtime color mode" }).selectOption("state");
    await expect(cell).not.toHaveClass(/runtime-lru/);
  });
}

test("real observations in another tab do not resume a paused tab", async ({ page, context, baseURL }) => {
  await page.goto("/page/0/0", { waitUntil: "commit" });
  const source = page.getByRole("region", { name: "CUBRID page-buffer observation" });
  await source.getByRole("button", { name: "Enable observations" }).click();
  await expect(source).toContainText("Observation source: active");
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  let requests = 0;
  page.on("request", (request) => {
    if (request.url().endsWith("/runtime/page-buffer/observe")) requests += 1;
  });
  const other = await context.newPage();
  await other.evaluate((url) => { window.location.href = url; }, `${baseURL}/page/0/0`);
  await other.getByRole("button", { name: "Enable observations" }).click();
  await expect(other.getByRole("region", { name: "Selected-page buffer observation" })).toContainText("Observed resident");
  await expect(source).toContainText("Newer observation available", { timeout: 10_000 });
  expect(requests).toBe(0);
  await other.close();
  const resumed = page.waitForRequest((request) => request.url().endsWith("/runtime/page-buffer/observe"));
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  expect((await resumed).postDataJSON().after_request).toBe(true);
  await expect(source).toContainText("Observation source: active");
});
