import { expect, test } from "vitest";

import { applyHistory, executeEffect, type HistoryPort } from "./runtime";

test("history writes keep entity paths generation-neutral and remember semantic parents", () => {
  const writes: unknown[] = [];
  const port: HistoryPort = {
    pathname: () => "/volume/0",
    state: () => ({ volmap: true, previous: null, parent: null }),
    push: (state, path) => writes.push(["push", state, path]),
    replace: (state, path) => writes.push(["replace", state, path]),
    back: () => writes.push(["back"]),
  };

  applyHistory(port, {
    id: 3,
    kind: "write-history",
    mode: "push",
    route: { kind: "page", vol: 0, page: 10 },
    parent: { kind: "sector", vol: 0, sector: 0 },
  });

  expect(writes).toEqual([
    [
      "push",
      { volmap: true, previous: "/volume/0", parent: "/sector/0/0" },
      "/page/0/10",
    ],
  ]);
});

test("watch backoff is executed by the timer adapter and returns a semantic action", async () => {
  const scheduled: Array<readonly [number, () => void]> = [];
  const actions: unknown[] = [];
  await executeEffect(
    { id: 9, kind: "delay-watch", knownGeneration: "4", milliseconds: 1000 },
    {
      api: null as never,
      history: null as never,
      schedule: (milliseconds, action) => scheduled.push([milliseconds, action]),
    },
    (action) => actions.push(action),
  );
  expect(scheduled).toHaveLength(1);
  expect(scheduled[0]?.[0]).toBe(1000);
  scheduled[0]?.[1]();
  expect(actions).toEqual([{ kind: "retry-watch", knownGeneration: "4" }]);
});

test("semantic back uses browser history only when the previous entry is the parent", async () => {
  const calls: unknown[] = [];
  const history: HistoryPort = {
    pathname: () => "/page/0/10",
    state: () => ({ volmap: true, previous: "/sector/0/0", parent: "/sector/0/0" }),
    push: () => undefined,
    replace: () => undefined,
    back: () => calls.push("back"),
  };
  await executeEffect(
    { id: 5, kind: "history-back", parent: { kind: "sector", vol: 0, sector: 0 } },
    { api: null as never, history, schedule: () => undefined },
    (action) => calls.push(action),
  );
  expect(calls).toEqual(["back"]);
});
