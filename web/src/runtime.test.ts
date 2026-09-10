import { expect, test } from "vitest";

import { applyHistory, executeEffect, type HistoryPort } from "./runtime";
import { createHttpApi } from "./api";
import { initialState, reduce, type Action } from "./model";

test("capability effects adopt normalized metadata and sanitize failures outside inspection errors", async () => {
  for (const fails of [false, true]) {
    const fetcher: typeof fetch = async () => {
      if (fails) throw new Error("/private/socket uid=1000 raw OS failure");
      return new Response(JSON.stringify({
        schema: "volmap.runtime", schema_version: 1,
        source: "cubrid-page-buffer-observation", state: "disabled",
        verification: "unverified", reason: "not-requested",
      }));
    };
    const actions: Action[] = [];
    await executeEffect({ id: 2, kind: "read-runtime-capability" }, {
      api: createHttpApi(fetcher), history: null as never, schedule: () => undefined,
    }, (action) => actions.push(action));
    expect(actions).toMatchObject([{ kind: "runtime-capability-loaded", state: fails ? "unavailable" : "disabled" }]);
    const result = actions.reduce(reduce, initialState({ kind: "root" }));
    expect(result.error).toBeNull();
    expect(result.outcome).toBe("loading");
  }
});

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

test("observation retry timers apply bounded jitter and dispatch one epoch-bound demand", async () => {
  const { vi } = await import("vitest");
  for (const [random, expected] of [[0, 6400], [0.5, 8000], [1, 9600]]) {
    const jitter = vi.spyOn(Math, "random").mockReturnValue(random!);
    const timers: Array<readonly [number, () => void]> = [];
    const actions: Action[] = [];
    await executeEffect({ kind: "delay-observation", id: 1, epoch: 7, milliseconds: 8000, jitter: true }, {
      api: null as never, history: null as never,
      schedule: (delay, run) => timers.push([delay, run]),
    }, (action) => actions.push(action));
    expect(timers[0]?.[0]).toBeCloseTo(expected!);
    timers[0]?.[1]();
    expect(actions).toEqual([{ kind: "observation-due", epoch: 7 }]);
    jitter.mockRestore();
  }
});

test("queued runtime effects send no requests after visibility or adoption authority is revoked", async () => {
  let requests = 0;
  const api = createHttpApi(async () => { requests += 1; throw new Error("must not request"); });
  for (const effect of [
    { kind: "read-runtime-capability" as const, id: 1, epoch: 7 },
    { kind: "read-observation" as const, id: 2, scope: "old", request: { pages: [{ volid: 0, pageid: 7 }], epoch: "7", generation: "1", retry: false } },
  ]) {
    await executeEffect(effect, { api, history: null as never, schedule: () => undefined, allowRuntime: () => false }, () => undefined);
  }
  expect(requests).toBe(0);
});

test("protocol-incompatible observation responses stop retry without exposing decoder details", async () => {
  const actions: Action[] = [];
  await executeEffect({ kind: "read-observation", id: 1, scope: "page", request: { pages: [{ volid: 0, pageid: 7 }], epoch: "1", generation: "1", retry: false } }, {
    api: createHttpApi(async () => new Response(JSON.stringify({ schema: "unexpected" }))),
    history: null as never, schedule: () => undefined,
  }, (action) => actions.push(action));
  expect(actions).toMatchObject([{ kind: "observation-loaded", batch: null, failure: "protocol-incompatible" }]);
});

test("browser expiry has its own cancellable deadline instead of waiting for the periodic age tick", () => {
  const scheduled: Array<readonly [number, () => void]> = [];
  const cancelled: number[] = [];
  const actions: Action[] = [];
  const batch: ObservationBatch = { rows: [], capability: "active", pages: [{ volid: 0, pageid: 7 }], epoch: "1", generation: "1", state: "resident", reason: "observed-resident", upperAgeMs: 100, incarnation: "fixture", captureIdentity: "capture", captureLabel: "fixture", evidence: {}, requested: 1, evaluated: 1, complete: true, limitations: [] };
  const observation = { ...initialObservation(), enabled: true, batch, age: 300, received: 1000, wallReceived: 100000 };
  let reading = { now: 1100, wallNow: 100100 };
  const cleanup = subscribeObservationExpiry({ setTimeout: (run, delay) => { scheduled.push([delay, run]); return 7; }, clearTimeout: (id) => cancelled.push(id) }, observation,
    (action) => actions.push(action), () => reading);
  expect(scheduled[0]?.[0]).toBe(29600);
  reading = { now: 30700, wallNow: 129700 };
  scheduled[0]?.[1]();
  const state = { ...initialState({ kind: "page", vol: 0, page: 7 }), observation };
  expect(actions.reduce(reduce, state).observation.batch).toBeNull();
  cleanup();
  expect(cancelled).toEqual([7]);
});

import { initialObservation, type ObservationBatch } from "./observations";
import { subscribeObservationExpiry } from "./runtime";
