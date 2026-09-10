import { loadVolumeCells } from "./volume-fixture";
import { chromium, firefox, expect, test, type Page } from "@playwright/test";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { cpus, hostname, release, totalmem } from "node:os";
import { createHash } from "node:crypto";

// A dedicated browser process tree with one tab. Summed RSS includes shared
// pages in each process; this is conservative accounting, not JS heap size.
const pageSize = Number(execFileSync("getconf", ["PAGESIZE"], { encoding: "utf8" }).trim());

async function rssTree(root: number): Promise<number> {
  const pending = [root];
  const seen = new Set<number>();
  let total = 0;
  while (pending.length > 0) {
    const pid = pending.pop()!;
    if (seen.has(pid)) continue;
    seen.add(pid);
    try {
      const resident = Number((await readFile(`/proc/${pid}/statm`, "utf8")).trim().split(/\s+/)[1]);
      if (!Number.isSafeInteger(resident) || resident < 0 || (pid === root && resident === 0)) throw new Error(`Invalid resident pages for browser process ${pid}`);
      total += resident * pageSize;
      // Children may be forked by any thread (not just the process leader).
      for (const tid of await readdir(`/proc/${pid}/task`)) {
        try {
          const children = (await readFile(`/proc/${pid}/task/${tid}/children`, "utf8")).trim();
          if (children) pending.push(...children.split(/\s+/).map(Number));
        } catch (error) {
          if (!["ENOENT", "ESRCH"].includes((error as NodeJS.ErrnoException).code ?? "")) throw error;
        }
      }
    } catch (error) {
      if (pid === root || !["ENOENT", "ESRCH"].includes((error as NodeJS.ErrnoException).code ?? "")) throw error;
    }
  }
  return total;
}

test("12288 rendered page cells: per-browser visible latency and paired RSS", async ({ browserName }, testInfo) => {
  const browserType = browserName === "chromium" ? chromium : firefox;
  const viewport = { width: 1920, height: 1080 };
  const provenance = {
    consumerCommit: execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim(),
    dirtyDiffSha256: createHash("sha256").update(execFileSync("git", ["diff", "HEAD"])).digest("hex"),
    corpusSha256: createHash("sha256").update(await readFile("../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl")).digest("hex"),
    host: { name: hostname(), kernel: release(), cpu: cpus()[0]?.model, logicalCpus: cpus().length, memoryBytes: totalmem() },
    build: "release", browserName, command: "playwright test --config playwright.density.config.ts",
  };
  const runs: object[] = [];
  const peaks: Record<string, number[]> = { enabled: [], disabled: [] };
  const p95PerArm: number[] = [];
  // Alternate order to expose warm-cache/order effects; retain each pair.
  for (let pair = 0; pair < 2; pair += 1) {
    const mode = pair === 0 ? "state" : "lru";
    for (const enabled of pair === 0 ? [false, true] : [true, false]) {
      const server = await browserType.launchServer({ headless: true });
      const pid = server.process().pid;
      if (pid === undefined) throw new Error("No browser PID for RSS accounting");
      const browser = await browserType.connect(server.wsEndpoint());
      const samples: { elapsedMs: number; rss: number }[] = [];
      const started = performance.now();
      let sampling = true;
      let samplingError: unknown;
      let armError: unknown;
      let page: Page | undefined;
      let interaction: number[] = [];
      let rendered: { count: number; inViewport: number } | null = null;
      const sampler = (async () => {
        while (sampling) {
          samples.push({ elapsedMs: performance.now() - started, rss: await rssTree(pid) });
          await new Promise((resolve) => setTimeout(resolve, 50));
        }
      })().catch((error: unknown) => { samplingError = error; });
      try {
        page = await browser.newPage({ viewport });
        await page.goto("http://127.0.0.1:41742/volume/0", { waitUntil: "commit" });
        const cells = page.locator("[data-observation-page]");
        // Exercise real collection pagination; never inject a synthetic model.
        await loadVolumeCells(page, 12288);
        await page.evaluate(() => window.scrollTo(0, 0));
        rendered = await cells.evaluateAll((elements) => ({
          count: elements.filter((element) => element.getClientRects().length > 0).length,
          inViewport: elements.filter((element) => {
            const box = element.getBoundingClientRect();
            return box.top >= 0 && box.bottom <= innerHeight && box.left >= 0 && box.right <= innerWidth;
          }).length,
        }));
        expect(rendered.count).toBe(12288);
        if (enabled) {
          await page.getByRole("button", { name: "Enable observations" }).click();
          await expect(page.getByRole("region", { name: "Visible-page buffer observations" })).toContainText("Evaluated 512 / requested 512");
          await page.getByRole("combobox", { name: "Runtime color mode" }).selectOption(mode);
        }
        // Same keyboard focus changes, viewport and DOM workload in both arms.
        // The second animation frame gives a conservative post-paint bound.
        await page.evaluate(() => {
          const samples: number[] = [];
          Object.assign(window, { densityInputSamples: samples });
          document.addEventListener("keydown", (event) => {
            if (event.key !== "Tab") return;
            const start = event.timeStamp;
            if (!Number.isFinite(start) || start < 0 || start > performance.now()) throw new Error("Invalid input timestamp clock");
            const before = document.activeElement;
            requestAnimationFrame(() => requestAnimationFrame(() => {
              const active = document.activeElement;
              if (active === before || !(active instanceof HTMLElement)) return;
              const rect = active.getBoundingClientRect();
              if (rect.width > 0 && rect.height > 0 && rect.bottom > 54 && rect.top < innerHeight && rect.right > 0 && rect.left < innerWidth && getComputedStyle(active).outlineStyle !== "none") samples.push(performance.now() - start);
            }));
          }, true);
        });
        await page.getByRole("button", { name: /^Sector 0,/ }).focus();
        for (let index = 0; index < 40; index += 1) {
          await page.keyboard.press("Tab");
          await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
        }
        interaction = await page.evaluate(() => Reflect.get(window, "densityInputSamples") as number[]);
        expect(interaction).toHaveLength(40);
        const sorted = [...interaction].sort((a, b) => a - b);
        const p95 = sorted[Math.ceil(sorted.length * 0.95) - 1]!;
        if (enabled) p95PerArm.push(p95);
        sampling = false;
        await sampler;
        const peak = Math.max(...samples.map((sample) => sample.rss));
        peaks[enabled ? "enabled" : "disabled"]!.push(peak);
        runs.push({ pair, mode, enabled, p95Ms: p95, browserVersion: browser.version(), rendered, viewport, interactionMs: interaction, rssSamples: samples, peakRssBytes: peak });
      } catch (error) {
        armError = error;
        throw error;
      } finally {
        sampling = false;
        await sampler;
        if (page && !page.isClosed()) {
          try { interaction = await page.evaluate(() => Reflect.get(window, "densityInputSamples") as number[] ?? []); }
          catch (error) { armError ??= error; }
        }
        const raw = testInfo.outputPath(`rss-${browserName}-${pair}-${enabled ? "enabled" : "disabled"}.json`);
        await writeFile(raw, JSON.stringify({ ...provenance, browserVersion: browser.version(), pair, mode, enabled, viewport, rendered, interaction, armError: armError === undefined ? null : String(armError), samples, samplingError: samplingError === undefined ? null : String(samplingError), completedRuns: runs }, null, 2));
        await testInfo.attach("density-arm-raw", { path: raw, contentType: "application/json" });
        await browser.close();
        await server.close();
      }
      if (samplingError !== undefined) throw samplingError;
      if (armError !== undefined) throw armError;
    }
  }
  const p95 = Math.max(...p95PerArm);
  const increments = peaks.enabled!.map((peak, index) => peak - peaks.disabled![index]!);
  const report = {
    ...provenance,
    method: "One dedicated browser tree/tab per arm; 50ms sampled summed process RSS from launch through identical 40 Tab interactions. Two order-alternated pairs (state marks, LRU); worst per-arm p95 reported, never averaged across cases. Input keydown to visible outlined focus, bounded by second animation frame. Local diagnostic; manual and dedicated-host qualification remain separate.",
    p95Ms: p95, incrementalPeakRssBytes: increments, runs,
    timingPass: p95 <= 100, memoryPass: increments.every((increment) => increment <= 32 * 1024 * 1024),
  };
  const output = testInfo.outputPath(`density-${browserName}.json`);
  await writeFile(output, JSON.stringify(report, null, 2));
  await testInfo.attach("density-raw", { path: output, contentType: "application/json" });
  expect(p95).toBeLessThanOrEqual(100);
  for (const increment of increments) expect(increment).toBeLessThanOrEqual(32 * 1024 * 1024);
});
