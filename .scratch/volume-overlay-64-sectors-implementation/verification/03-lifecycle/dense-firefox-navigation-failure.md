# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: volume-observations.spec.ts >> 12288 rendered cells adopt 4096 changing results from one capture with fixed central selection
- Location: e2e/volume-observations.spec.ts:14:1

# Error details

```
Test timeout of 30000ms exceeded.
```

```
Error: page.goto: Test timeout of 30000ms exceeded.
Call log:
  - navigating to "http://127.0.0.1:41743/volume/0", waiting until "commit"

```

# Page snapshot

```yaml
- generic [ref=e2]:
  - banner [ref=e3]:
    - strong [ref=e4]: VOLMAP
    - generic [ref=e5]: snapshot a2b04b001c0a · revision 0
    - generic [ref=e6]:
      - generic [ref=e7]: live · gen 0 · 242s ago · disk 20:23
      - button "Pause" [ref=e8] [cursor=pointer]
    - button "About & licenses" [ref=e9] [cursor=pointer]
    - generic [ref=e10]: success-limited
  - main [ref=e11]:
    - complementary [ref=e12]:
      - region "CUBRID page-buffer observation" [ref=e13]:
        - heading "CUBRID page-buffer observation" [level=2] [ref=e14]
        - button "Enable observations" [ref=e15] [cursor=pointer]
        - paragraph [ref=e16]: "Observation source: unavailable"
        - paragraph [ref=e17]: No verified producer observation is available. Disk inspection is unaffected.
      - heading "Snapshot hierarchy" [level=2] [ref=e18]
      - button "volume 0 · 192 sectors" [ref=e20] [cursor=pointer]
    - generic [ref=e21]:
      - navigation "Inspection hierarchy" [ref=e22]:
        - generic [ref=e23]: Volume 0
      - generic [ref=e25]:
        - generic [ref=e26]:
          - generic [ref=e27]:
            - heading "Volume 0 · full map" [level=1] [ref=e28]
            - paragraph [ref=e29]: 192 sectors · 64 pages per sector · revision 0
          - generic "Page allocation and occupancy legend" [ref=e30]:
            - generic [ref=e31]: Unreserved
            - generic [ref=e33]: Reserved, unallocated
            - generic [ref=e35]: Occupied
            - generic [ref=e37]: Slotted free
            - generic [ref=e39]: System metadata
            - generic [ref=e41]: Finding outline
        - generic "Full volume sector map" [ref=e43]:
          - button "Sector 0, reserved, 64 pages" [ref=e44] [cursor=pointer]:
            - generic [ref=e45]:
              - strong [ref=e46]: Sector 0
              - generic [ref=e47]: reserved
          - button "Sector 1, unreserved, 64 pages" [ref=e113] [cursor=pointer]:
            - generic [ref=e114]:
              - strong [ref=e115]: Sector 1
              - generic [ref=e116]: unreserved
          - button "Sector 2, unreserved, 64 pages" [ref=e182] [cursor=pointer]:
            - generic [ref=e183]:
              - strong [ref=e184]: Sector 2
              - generic [ref=e185]: unreserved
          - button "Sector 3, unreserved, 64 pages" [ref=e251] [cursor=pointer]:
            - generic [ref=e252]:
              - strong [ref=e253]: Sector 3
              - generic [ref=e254]: unreserved
          - button "Sector 4, unreserved, 64 pages" [ref=e320] [cursor=pointer]:
            - generic [ref=e321]:
              - strong [ref=e322]: Sector 4
              - generic [ref=e323]: unreserved
          - button "Sector 5, unreserved, 64 pages" [ref=e389] [cursor=pointer]:
            - generic [ref=e390]:
              - strong [ref=e391]: Sector 5
              - generic [ref=e392]: unreserved
          - button "Sector 6, unreserved, 64 pages" [ref=e458] [cursor=pointer]:
            - generic [ref=e459]:
              - strong [ref=e460]: Sector 6
              - generic [ref=e461]: unreserved
          - button "Sector 7, unreserved, 64 pages" [ref=e527] [cursor=pointer]:
            - generic [ref=e528]:
              - strong [ref=e529]: Sector 7
              - generic [ref=e530]: unreserved
          - button "Sector 8, unreserved, 64 pages" [ref=e596] [cursor=pointer]:
            - generic [ref=e597]:
              - strong [ref=e598]: Sector 8
              - generic [ref=e599]: unreserved
          - button "Sector 9, unreserved, 64 pages" [ref=e665] [cursor=pointer]:
            - generic [ref=e666]:
              - strong [ref=e667]: Sector 9
              - generic [ref=e668]: unreserved
          - button "Sector 10, unreserved, 64 pages" [ref=e734] [cursor=pointer]:
            - generic [ref=e735]:
              - strong [ref=e736]: Sector 10
              - generic [ref=e737]: unreserved
          - button "Sector 11, unreserved, 64 pages" [ref=e803] [cursor=pointer]:
            - generic [ref=e804]:
              - strong [ref=e805]: Sector 11
              - generic [ref=e806]: unreserved
          - button "Sector 12, unreserved, 64 pages" [ref=e872] [cursor=pointer]:
            - generic [ref=e873]:
              - strong [ref=e874]: Sector 12
              - generic [ref=e875]: unreserved
          - button "Sector 13, unreserved, 64 pages" [ref=e941] [cursor=pointer]:
            - generic [ref=e942]:
              - strong [ref=e943]: Sector 13
              - generic [ref=e944]: unreserved
          - button "Sector 14, unreserved, 64 pages" [ref=e1010] [cursor=pointer]:
            - generic [ref=e1011]:
              - strong [ref=e1012]: Sector 14
              - generic [ref=e1013]: unreserved
          - button "Sector 15, unreserved, 64 pages" [ref=e1079] [cursor=pointer]:
            - generic [ref=e1080]:
              - strong [ref=e1081]: Sector 15
              - generic [ref=e1082]: unreserved
          - button "Sector 16, unreserved, 64 pages" [ref=e1148] [cursor=pointer]:
            - generic [ref=e1149]:
              - strong [ref=e1150]: Sector 16
              - generic [ref=e1151]: unreserved
          - button "Sector 17, unreserved, 64 pages" [ref=e1217] [cursor=pointer]:
            - generic [ref=e1218]:
              - strong [ref=e1219]: Sector 17
              - generic [ref=e1220]: unreserved
          - button "Sector 18, unreserved, 64 pages" [ref=e1286] [cursor=pointer]:
            - generic [ref=e1287]:
              - strong [ref=e1288]: Sector 18
              - generic [ref=e1289]: unreserved
          - button "Sector 19, unreserved, 64 pages" [ref=e1355] [cursor=pointer]:
            - generic [ref=e1356]:
              - strong [ref=e1357]: Sector 19
              - generic [ref=e1358]: unreserved
          - button "Sector 20, unreserved, 64 pages" [ref=e1424] [cursor=pointer]:
            - generic [ref=e1425]:
              - strong [ref=e1426]: Sector 20
              - generic [ref=e1427]: unreserved
          - button "Sector 21, unreserved, 64 pages" [ref=e1493] [cursor=pointer]:
            - generic [ref=e1494]:
              - strong [ref=e1495]: Sector 21
              - generic [ref=e1496]: unreserved
          - button "Sector 22, unreserved, 64 pages" [ref=e1562] [cursor=pointer]:
            - generic [ref=e1563]:
              - strong [ref=e1564]: Sector 22
              - generic [ref=e1565]: unreserved
          - button "Sector 23, unreserved, 64 pages" [ref=e1631] [cursor=pointer]:
            - generic [ref=e1632]:
              - strong [ref=e1633]: Sector 23
              - generic [ref=e1634]: unreserved
          - button "Sector 24, unreserved, 64 pages" [ref=e1700] [cursor=pointer]:
            - generic [ref=e1701]:
              - strong [ref=e1702]: Sector 24
              - generic [ref=e1703]: unreserved
          - button "Sector 25, unreserved, 64 pages" [ref=e1769] [cursor=pointer]:
            - generic [ref=e1770]:
              - strong [ref=e1771]: Sector 25
              - generic [ref=e1772]: unreserved
          - button "Sector 26, unreserved, 64 pages" [ref=e1838] [cursor=pointer]:
            - generic [ref=e1839]:
              - strong [ref=e1840]: Sector 26
              - generic [ref=e1841]: unreserved
          - button "Sector 27, unreserved, 64 pages" [ref=e1907] [cursor=pointer]:
            - generic [ref=e1908]:
              - strong [ref=e1909]: Sector 27
              - generic [ref=e1910]: unreserved
          - button "Sector 28, unreserved, 64 pages" [ref=e1976] [cursor=pointer]:
            - generic [ref=e1977]:
              - strong [ref=e1978]: Sector 28
              - generic [ref=e1979]: unreserved
          - button "Sector 29, unreserved, 64 pages" [ref=e2045] [cursor=pointer]:
            - generic [ref=e2046]:
              - strong [ref=e2047]: Sector 29
              - generic [ref=e2048]: unreserved
          - button "Sector 30, unreserved, 64 pages" [ref=e2114] [cursor=pointer]:
            - generic [ref=e2115]:
              - strong [ref=e2116]: Sector 30
              - generic [ref=e2117]: unreserved
          - button "Sector 31, unreserved, 64 pages" [ref=e2183] [cursor=pointer]:
            - generic [ref=e2184]:
              - strong [ref=e2185]: Sector 31
              - generic [ref=e2186]: unreserved
          - button "Sector 32, unreserved, 64 pages" [ref=e2252] [cursor=pointer]:
            - generic [ref=e2253]:
              - strong [ref=e2254]: Sector 32
              - generic [ref=e2255]: unreserved
          - button "Sector 33, unreserved, 64 pages" [ref=e2321] [cursor=pointer]:
            - generic [ref=e2322]:
              - strong [ref=e2323]: Sector 33
              - generic [ref=e2324]: unreserved
          - button "Sector 34, unreserved, 64 pages" [ref=e2390] [cursor=pointer]:
            - generic [ref=e2391]:
              - strong [ref=e2392]: Sector 34
              - generic [ref=e2393]: unreserved
          - button "Sector 35, unreserved, 64 pages" [ref=e2459] [cursor=pointer]:
            - generic [ref=e2460]:
              - strong [ref=e2461]: Sector 35
              - generic [ref=e2462]: unreserved
          - button "Sector 36, unreserved, 64 pages" [ref=e2528] [cursor=pointer]:
            - generic [ref=e2529]:
              - strong [ref=e2530]: Sector 36
              - generic [ref=e2531]: unreserved
          - button "Sector 37, unreserved, 64 pages" [ref=e2597] [cursor=pointer]:
            - generic [ref=e2598]:
              - strong [ref=e2599]: Sector 37
              - generic [ref=e2600]: unreserved
          - button "Sector 38, unreserved, 64 pages" [ref=e2666] [cursor=pointer]:
            - generic [ref=e2667]:
              - strong [ref=e2668]: Sector 38
              - generic [ref=e2669]: unreserved
          - button "Sector 39, unreserved, 64 pages" [ref=e2735] [cursor=pointer]:
            - generic [ref=e2736]:
              - strong [ref=e2737]: Sector 39
              - generic [ref=e2738]: unreserved
          - button "Sector 40, unreserved, 64 pages" [ref=e2804] [cursor=pointer]:
            - generic [ref=e2805]:
              - strong [ref=e2806]: Sector 40
              - generic [ref=e2807]: unreserved
          - button "Sector 41, unreserved, 64 pages" [ref=e2873] [cursor=pointer]:
            - generic [ref=e2874]:
              - strong [ref=e2875]: Sector 41
              - generic [ref=e2876]: unreserved
          - button "Sector 42, unreserved, 64 pages" [ref=e2942] [cursor=pointer]:
            - generic [ref=e2943]:
              - strong [ref=e2944]: Sector 42
              - generic [ref=e2945]: unreserved
          - button "Sector 43, unreserved, 64 pages" [ref=e3011] [cursor=pointer]:
            - generic [ref=e3012]:
              - strong [ref=e3013]: Sector 43
              - generic [ref=e3014]: unreserved
          - button "Sector 44, unreserved, 64 pages" [ref=e3080] [cursor=pointer]:
            - generic [ref=e3081]:
              - strong [ref=e3082]: Sector 44
              - generic [ref=e3083]: unreserved
          - button "Sector 45, unreserved, 64 pages" [ref=e3149] [cursor=pointer]:
            - generic [ref=e3150]:
              - strong [ref=e3151]: Sector 45
              - generic [ref=e3152]: unreserved
          - button "Sector 46, unreserved, 64 pages" [ref=e3218] [cursor=pointer]:
            - generic [ref=e3219]:
              - strong [ref=e3220]: Sector 46
              - generic [ref=e3221]: unreserved
          - button "Sector 47, unreserved, 64 pages" [ref=e3287] [cursor=pointer]:
            - generic [ref=e3288]:
              - strong [ref=e3289]: Sector 47
              - generic [ref=e3290]: unreserved
        - status [ref=e3356]: Showing 48 of 192 sectors · scroll to continue
        - button "Load more sectors" [ref=e3358] [cursor=pointer]
```

# Test source

```ts
  1   | import { expect, test, type Response } from "@playwright/test";
  2   | import { loadVolumeCells } from "./volume-fixture";
  3   | 
  4   | const evidenceRoot = "../.scratch/volume-overlay-64-sectors-implementation/verification/02-volume-scope";
  5   | 
  6   | // Header arrival may precede a scope cancellation. Only completed response
  7   | // bodies can be candidates for the successful adoption assertions below.
  8   | async function completedObservation(response: Response): Promise<boolean> {
  9   |   if (!response.url().endsWith("/page-buffer/observe") || response.status() !== 200) return false;
  10  |   try { await response.json(); return true; }
  11  |   catch { return false; }
  12  | }
  13  | 
  14  | test("12288 rendered cells adopt 4096 changing results from one capture with fixed central selection", async ({ page, browserName }) => {
  15  |   await page.setViewportSize({ width: 2560, height: 2160 });
> 16  |   await page.goto("http://127.0.0.1:41743/volume/0", { waitUntil: "commit" });
      |              ^ Error: page.goto: Test timeout of 30000ms exceeded.
  17  |   await loadVolumeCells(page, 12288);
  18  |   const renderedCells = await page.locator("[data-observation-page]").evaluateAll((elements) => elements.filter((element) => element.getClientRects().length > 0).length);
  19  |   expect(renderedCells).toBe(12288);
  20  |   await page.evaluate(() => window.scrollTo(0, 0));
  21  |   const expectedSelection = () => page.locator(".sector-card").evaluateAll((elements) => {
  22  |     const visible = elements.flatMap((element) => {
  23  |       const rect = element.getBoundingClientRect();
  24  |       if (rect.bottom <= 0 || rect.top >= innerHeight || rect.right <= 0 || rect.left >= innerWidth) return [];
  25  |       return [{ id: Number(element.id.replace("sector-", "")), distance: Math.hypot((rect.top + rect.bottom) / 2 - innerHeight / 2, (rect.left + rect.right) / 2 - innerWidth / 2) }];
  26  |     });
  27  |     return { visible: visible.length, ids: visible.sort((a, b) => a.distance - b.distance || a.id - b.id).slice(0, 64).map((sector) => sector.id).sort((a, b) => a - b) };
  28  |   });
  29  |   const responsePromise = page.waitForResponse(async (response) => await completedObservation(response) &&
  30  |     JSON.stringify(response.request().postDataJSON().scope.sectorids) === JSON.stringify((await expectedSelection()).ids));
  31  |   await page.getByRole("button", { name: "Enable observations" }).click();
  32  |   const firstResponse = await responsePromise;
  33  |   const first = await firstResponse.json();
  34  |   const expected = await expectedSelection();
  35  |   expect(expected.visible).toBeGreaterThan(64);
  36  |   expect(first.scope).toEqual({ kind: "volume", volid: 0, sectorids: expected.ids });
  37  |   expect(firstResponse.request().postDataJSON().pages).toBeUndefined();
  38  |   expect(first.slots).toHaveLength(4096);
  39  |   expect(first.requested_count).toBe(4096);
  40  |   expect(first.evaluated_count).toBe(4096);
  41  |   expect(first.slots.every((row: { state: string }) => row.state === "resident")).toBe(true);
  42  |   const residentCells = page.locator('[data-observation-page][data-runtime-state="resident"]');
  43  |   await expect(residentCells).toHaveCount(4096);
  44  |   const visibleResidentCells = await residentCells.evaluateAll((elements) => elements.filter((element) => {
  45  |     const rect = element.getBoundingClientRect();
  46  |     return rect.top >= 0 && rect.bottom <= innerHeight && rect.left >= 0 && rect.right <= innerWidth;
  47  |   }).length);
  48  |   expect(visibleResidentCells).toBe(4096);
  49  |   const coverage = page.getByRole("region", { name: "Visible-page buffer observations" });
  50  |   await expect(coverage).toContainText("selection does not rotate");
  51  |   const unqueried = page.locator(".sector-card").filter({ has: page.locator("[data-observation-page]:not([data-runtime-state])") }).first();
  52  |   await expect(unqueried).toHaveAccessibleName(/Not evaluated/);
  53  |   const focused = page.locator(`#sector-${expected.ids[0]}`);
  54  |   await focused.evaluate((element: HTMLElement) => element.focus({ preventScroll: true }));
  55  |   const nextResponse = await page.waitForResponse(async (response) => await completedObservation(response) && (await response.json()).capture?.identity !== first.capture.identity);
  56  |   const second = await nextResponse.json();
  57  |   expect(second.scope).toEqual(first.scope);
  58  |   expect(second.slots.every((row: { evidence: { lru_zone: string } }, index: number) => row.evidence.lru_zone !== first.slots[index].evidence.lru_zone)).toBe(true);
  59  |   await expect(coverage).toContainText(`scan ${second.capture.sequence} ·`);
  60  |   await expect(focused).toBeFocused();
  61  |   await expect(residentCells).toHaveCount(4096);
  62  |   await page.getByRole("button", { name: "Pause", exact: true }).click();
  63  |   const mode = page.getByRole("combobox", { name: "Runtime color mode" });
  64  |   await mode.selectOption("lru");
  65  |   await expect(page.locator(`.runtime-resident.runtime-lru.zone-${second.slots[0].evidence.lru_zone}`)).toHaveCount(4096);
  66  |   await expect(residentCells.first()).toHaveAttribute("title", /membership · index/);
  67  |   await expect(residentCells.first()).not.toHaveAttribute("title", /dirty|flushing|undefined/);
  68  |   await coverage.getByText("Available page observations and capture limitations", { exact: true }).click();
  69  |   await expect(coverage.getByRole("row")).toHaveCount(4097);
  70  |   await expect(coverage.getByRole("columnheader", { name: "Dirty", exact: true })).toHaveCount(0);
  71  |   await expect(coverage.getByRole("columnheader", { name: "List index", exact: true })).toBeVisible();
  72  |   await expect(coverage).toContainText("records and scans are not atomic");
  73  |   await page.screenshot({ path: `${evidenceRoot}/volume-${browserName}.png` });
  74  |   await coverage.getByText("Available page observations and capture limitations", { exact: true }).click();
  75  |   await mode.selectOption("state");
  76  |   await page.getByRole("button", { name: "Resume", exact: true }).click();
  77  |   const resized = page.waitForResponse(async (response) => await completedObservation(response) && response.request().postDataJSON().scope?.sectorids.length < 64 && JSON.stringify(response.request().postDataJSON().scope.sectorids) === JSON.stringify((await expectedSelection()).ids));
  78  |   await page.setViewportSize({ width: 1280, height: 720 });
  79  |   const smaller = await (await resized).json();
  80  |   expect(smaller.scope.sectorids).toEqual((await expectedSelection()).ids);
  81  |   await expect(residentCells).toHaveCount(smaller.requested_count);
  82  |   const scrolled = page.waitForResponse(async (response) => await completedObservation(response) && JSON.stringify(response.request().postDataJSON().scope) !== JSON.stringify(smaller.scope) && JSON.stringify(response.request().postDataJSON().scope.sectorids) === JSON.stringify((await expectedSelection()).ids));
  83  |   await page.evaluate(() => window.scrollBy(0, 1000));
  84  |   const moved = await (await scrolled).json();
  85  |   expect(moved.scope.sectorids).toEqual((await expectedSelection()).ids);
  86  |   await expect(residentCells).toHaveCount(moved.requested_count);
  87  |   const { writeFile, mkdir } = await import("node:fs/promises");
  88  |   await mkdir(evidenceRoot, { recursive: true });
  89  |   await writeFile(`${evidenceRoot}/browser-${browserName}.json`, JSON.stringify({ browserName, renderedCells, visibleResidentCells, visibleSectors: expected.visible,
  90  |     queriedPages: first.requested_count, residentCells: 4096, captureIdentities: [first.capture.identity, second.capture.identity],
  91  |     requestBytes: Buffer.byteLength(firstResponse.request().postData()!), responseBytes: (await firstResponse.body()).length,
  92  |     selectedSectors: first.scope.sectorids, resizedSectors: smaller.scope.sectorids, scrolledSectors: moved.scope.sectorids }, null, 2) + "\n");
  93  | });
  94  | 
  95  | for (const malformed of ["scope", "sector-order", "slot-count", "epoch", "generation", "detail-evidence", "oversized-body"]) {
  96  |   test(`Volume refuses malformed ${malformed} response as a whole`, async ({ page }) => {
  97  |     await page.route("**/runtime/page-buffer/observe", async (route) => {
  98  |       const response = await route.fetch();
  99  |       const body = await response.json();
  100 |       if (malformed === "scope") body.scope.volid = 1;
  101 |       if (malformed === "sector-order") body.scope.sectorids.reverse();
  102 |       if (malformed === "slot-count") body.slots.push(null);
  103 |       if (malformed === "epoch") body.epoch = "99999";
  104 |       if (malformed === "generation") body.generation = "99999";
  105 |       if (malformed === "detail-evidence") body.slots.find((row: { state: string }) => row.state === "resident").evidence.dirty = false;
  106 |       if (malformed === "oversized-body") await route.fulfill({ response, body: JSON.stringify(body) + " ".repeat(1_048_576) });
  107 |       else await route.fulfill({ response, json: body });
  108 |     });
  109 |     await page.goto("http://127.0.0.1:41741/volume/0", { waitUntil: "commit" });
  110 |     await page.getByRole("button", { name: "Enable observations" }).click();
  111 |     await expect(page.getByRole("region", { name: "CUBRID page-buffer observation" })).toContainText("Incompatible observation response");
  112 |     await expect(page.locator(".runtime-resident")).toHaveCount(0);
  113 |     await page.getByRole("button", { name: /^Sector 0,/ }).click();
  114 |     await expect(page.getByRole("heading", { name: "Sector 0", exact: true })).toBeVisible();
  115 |   });
  116 | }
```