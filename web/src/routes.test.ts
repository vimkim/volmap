import { describe, expect, test } from "vitest";

import { parentRoute, parseRoute, routePath } from "./routes";

describe("canonical live-viewer entity routes", () => {
  test.each([
    ["/", { kind: "root" }],
    ["/volume/0", { kind: "volume", vol: 0 }],
    ["/sector/3/7", { kind: "sector", vol: 3, sector: 7 }],
    ["/page/3/448", { kind: "page", vol: 3, page: 448 }],
    ["/slot/3/448/2", { kind: "slot", vol: 3, page: 448, slot: 2 }],
    ["/oos/3/448/2", { kind: "oos", vol: 3, page: 448, slot: 2 }],
  ] as const)("round-trips %s without generation or revision identity", (path, route) => {
    expect(parseRoute(path)).toEqual(route);
    expect(routePath(route)).toBe(path);
  });

  test.each(["/volume/00", "/page/1/-1", "/slot/1/2", "/wat/1", "/page/1/2/3"])(
    "rejects non-canonical path %s",
    (path) => expect(parseRoute(path)).toBeNull(),
  );

  test("derives the semantic parent using the loaded page's sector", () => {
    expect(parentRoute({ kind: "page", vol: 3, page: 448 }, 7)).toEqual({
      kind: "sector",
      vol: 3,
      sector: 7,
    });
    expect(parentRoute({ kind: "oos", vol: 3, page: 448, slot: 2 }, 7)).toEqual({
      kind: "slot",
      vol: 3,
      page: 448,
      slot: 2,
    });
  });
});
