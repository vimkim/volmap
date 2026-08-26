import { expect, test } from "vitest";

import { breadcrumbItems, followLabel } from "./selectors";

test("breadcrumbs preserve the complete semantic hierarchy", () => {
  expect(breadcrumbItems({ kind: "oos", vol: 3, page: 448, slot: 2 }, 7)).toEqual([
    { label: "Volume 3", route: { kind: "volume", vol: 3 } },
    { label: "Sector 7", route: { kind: "sector", vol: 3, sector: 7 } },
    { label: "Page 448", route: { kind: "page", vol: 3, page: 448 } },
    { label: "Slot 2", route: { kind: "slot", vol: 3, page: 448, slot: 2 } },
    { label: "OOS chain", route: null },
  ]);
});

test("the paused follow label distinguishes the displayed and offered generations", () => {
  expect(
    followLabel(
      { generation: "4", observed_at_unix_seconds: "100", input_modified_unix_seconds: "90" },
      { enabled: true, paused: true, watchedGeneration: "5" },
      106,
    ),
  ).toBe("paused at gen 4 · newer: gen 5");
});
