import { expect, test } from "vitest";

import { initialState, reduce } from "./model";

const snapshot = {
  id: "0123456789abcdef",
  revision: "7",
  validity: "valid",
  format_profile: "fixture",
  generation: "4",
  observed_at_unix_seconds: "100",
  input_modified_unix_seconds: "90",
} as const;

const page = {
  vol_id: 0,
  page_id: 10,
  sector_id: 0,
  allocation: "reserved-unallocated",
  page_type: { state: "known", value: "heap" },
  availability: "available",
  tde_state: "not-encrypted",
  detail_support: { state: "known", value: "semantic" },
  occupancy: { state: "known", occupied_percent: 7, free_percent: 93 },
  diagnostic: { state: "unknown" },
  file_association: { state: "none" },
} as const;

test("navigation revokes the previous route's response-adoption authority", () => {
  const page = { kind: "page", vol: 0, page: 10 } as const;
  const sector = { kind: "sector", vol: 0, sector: 0 } as const;
  const booting = initialState(page);

  expect(booting.route).toEqual(page);
  expect(booting.scope).toBe("1:/page/0/10");
  expect(booting.effects).toEqual([
    {
      id: 1,
      kind: "read-route",
      scope: "1:/page/0/10",
      route: page,
      autoEnrich: false,
    },
  ]);

  const navigated = reduce(booting, {
    kind: "navigate",
    route: sector,
    history: "push",
    autoEnrich: false,
  });
  expect(navigated.scope).toBe("2:/sector/0/0");
  expect(navigated.route).toEqual(sector);
  expect(navigated.effects.at(-1)).toEqual({
    id: 2,
    kind: "read-route",
    scope: "2:/sector/0/0",
    route: sector,
    autoEnrich: false,
  });

  const afterStaleFailure = reduce(navigated, {
    kind: "route-failed",
    scope: booting.scope,
    error: { message: "late page failure", status: 404, code: "entity-not-found" },
  });
  expect(afterStaleFailure.error).toBeNull();
  expect(afterStaleFailure.route).toEqual(sector);
});

test("a current route response becomes the only displayed semantic view", () => {
  const route = { kind: "sector", vol: 0, sector: 2 } as const;
  const booting = initialState(route);
  const started = reduce(booting, { kind: "effects-started", ids: [1] });
  const loaded = reduce(started, {
    kind: "route-loaded",
    scope: booting.scope,
    result: {
      route,
      snapshot,
      outcome: "success-limited",
      follow: { state: "disabled" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "sector",
        volume: { vol_id: 0, total_sectors: 64 },
        sector: { vol_id: 0, sector_id: 2, reserved: true, pages: [] },
      },
    },
  });

  expect(loaded.snapshot).toEqual(snapshot);
  expect(loaded.outcome).toBe("success-limited");
  expect(loaded.view).toMatchObject({ kind: "sector", sector: { sector_id: 2 } });
  expect(loaded.effects).toEqual([
    {
      id: 2,
      kind: "write-history",
      mode: "replace",
      route,
      parent: { kind: "volume", vol: 0 },
    },
  ]);
});

test("progressive sector loading adopts only the cursor authorized by the volume view", () => {
  const route = { kind: "volume", vol: 0 } as const;
  let state = initialState(route);
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, {
    kind: "route-loaded",
    scope: state.scope,
    result: {
      route,
      snapshot,
      outcome: "success-limited",
      follow: { state: "disabled" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "volume",
        volume: { vol_id: 0, total_sectors: 64 },
        sectors: [{ vol_id: 0, sector_id: 0, reserved: true, pages: [] }],
        nextCursor: { state: "present", value: "cursor-a" },
      },
    },
  });
  state = reduce(state, { kind: "effects-started", ids: [2] });
  state = reduce(state, { kind: "request-more-sectors" });

  expect(state.effects).toEqual([
    {
      id: 3,
      kind: "read-sector-batch",
      scope: "1:/volume/0",
      vol: 0,
      cursor: "cursor-a",
    },
  ]);

  const loaded = reduce(state, {
    kind: "sector-batch-loaded",
    scope: state.scope,
    cursor: "cursor-a",
    resource: {
      snapshot: { ...snapshot, revision: "8" },
      outcome: "success-limited",
      coverage: [],
      diagnostics: [],
      data: {
        items: [{ vol_id: 0, sector_id: 1, reserved: false, pages: [] }],
        next_cursor: { state: "end" },
      },
    },
  });
  expect(loaded.view).toMatchObject({
    kind: "volume",
    sectors: [{ sector_id: 0 }, { sector_id: 1 }],
    nextCursor: { state: "end" },
  });
  expect(loaded.snapshot?.revision).toBe("8");
});

test("pause retains only the newest generation offer and resume adopts it without history writes", () => {
  const route = { kind: "volume", vol: 0 } as const;
  let state = initialState(route);
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, {
    kind: "route-loaded",
    scope: state.scope,
    result: {
      route,
      snapshot,
      outcome: "success-limited",
      follow: { state: "following", poll_interval_ms: "1000", retained_generations: "2" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "volume",
        volume: { vol_id: 0, total_sectors: 64 },
        sectors: [],
        nextCursor: { state: "end" },
      },
    },
  });
  expect(state.effects).toEqual([
    {
      id: 2,
      kind: "write-history",
      mode: "replace",
      route,
      parent: null,
    },
    { id: 3, kind: "watch-generation", knownGeneration: "4" },
  ]);
  state = reduce(state, { kind: "effects-started", ids: [2, 3] });
  state = reduce(state, { kind: "toggle-pause" });
  state = reduce(state, {
    kind: "watch-loaded",
    knownGeneration: "4",
    resource: {
      snapshot: { ...snapshot, generation: "5" },
      outcome: "success-limited",
      coverage: [],
      diagnostics: [],
      data: {
        advanced: true,
        follow: { state: "following", poll_interval_ms: "1000", retained_generations: "2" },
      },
    },
  });
  expect(state.follow).toMatchObject({ paused: true, watchedGeneration: "5" });
  expect(state.follow.offered?.snapshot.generation).toBe("5");
  expect(state.effects).toEqual([
    { id: 4, kind: "watch-generation", knownGeneration: "5" },
  ]);

  state = reduce(state, { kind: "toggle-pause" });
  expect(state.history).toBe("none");
  expect(state.scope).toBe("2:/volume/0");
  expect(state.effects.at(-1)).toEqual({
    id: 5,
    kind: "read-route",
    scope: "2:/volume/0",
    route,
    autoEnrich: false,
  });
});

test("a page selected inside the app requests enrichment through a reducer effect", () => {
  let state = initialState({ kind: "volume", vol: 0 });
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, {
    kind: "navigate",
    route: { kind: "page", vol: 0, page: 10 },
    history: "push",
    autoEnrich: true,
  });
  state = reduce(state, { kind: "effects-started", ids: [2] });
  state = reduce(state, {
    kind: "route-loaded",
    scope: state.scope,
    result: {
      route: { kind: "page", vol: 0, page: 10 },
      snapshot,
      outcome: "success-limited",
      follow: { state: "disabled" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "page",
        volume: { vol_id: 0, total_sectors: 64 },
        sector: { vol_id: 0, sector_id: 0, reserved: true, pages: [page] },
        page: {
          page,
          deep: { state: "not-enriched" },
          slots: [],
          distribution: { state: "not-available" },
        },
        enriching: false,
      },
    },
  });

  expect(state.view).toMatchObject({ kind: "page", enriching: true });
  expect(state.effects).toEqual([
    {
      id: 3,
      kind: "write-history",
      mode: "push",
      route: { kind: "page", vol: 0, page: 10 },
      parent: { kind: "sector", vol: 0, sector: 0 },
    },
    {
      id: 4,
      kind: "enrich-route",
      scope: "2:/page/0/10",
      selector: "page:0:10",
      targetRoute: { kind: "page", vol: 0, page: 10 },
      history: "none",
      fallbackRoute: null,
    },
  ]);

  state = reduce(state, { kind: "effects-started", ids: [3, 4] });
  state = reduce(state, {
    kind: "enrichment-loaded",
    scope: state.scope,
    resource: {
      snapshot: { ...snapshot, revision: "8" },
      outcome: "success-limited",
      coverage: [],
      diagnostics: [],
      data: { retained: true },
    },
    targetRoute: { kind: "page", vol: 0, page: 10 },
    history: "none",
  });
  expect(state.snapshot?.revision).toBe("8");
  expect(state.scope).toBe("3:/page/0/10");
  expect(state.history).toBe("none");
  expect(state.effects).toEqual([
    {
      id: 5,
      kind: "read-route",
      scope: "3:/page/0/10",
      route: { kind: "page", vol: 0, page: 10 },
      autoEnrich: false,
    },
  ]);
});

test("license disclosure is requested through the effect boundary and retained in model state", () => {
  let state = initialState({ kind: "volume", vol: 0 });
  state = reduce(state, { kind: "show-licenses" });
  expect(state.effects.at(-1)).toEqual({ id: 2, kind: "read-licenses" });
  state = reduce(state, { kind: "license-loaded", notice: "Volmap notices" });
  expect(state.license).toEqual({ open: true, loading: false, notice: "Volmap notices" });
  state = reduce(state, { kind: "close-licenses" });
  expect(state.license.open).toBe(false);
});

test("a failed generation watch backs off through a timer effect before retrying", () => {
  const route = { kind: "volume", vol: 0 } as const;
  let state = initialState(route);
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, {
    kind: "route-loaded",
    scope: state.scope,
    result: {
      route,
      snapshot,
      outcome: "success-limited",
      follow: { state: "following", poll_interval_ms: "1000", retained_generations: "2" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "volume",
        volume: { vol_id: 0, total_sectors: 64 },
        sectors: [],
        nextCursor: { state: "end" },
      },
    },
  });
  state = reduce(state, { kind: "effects-started", ids: [2, 3] });
  state = reduce(state, { kind: "watch-failed", knownGeneration: "4" });
  expect(state.effects).toEqual([
    { id: 4, kind: "delay-watch", knownGeneration: "4", milliseconds: 1000 },
  ]);
  state = reduce(state, { kind: "effects-started", ids: [4] });
  state = reduce(state, { kind: "retry-watch", knownGeneration: "4" });
  expect(state.effects).toEqual([
    { id: 5, kind: "watch-generation", knownGeneration: "4" },
  ]);
});

test("hierarchy back delegates browser-history choice without losing the semantic parent", () => {
  const route = { kind: "page", vol: 0, page: 10 } as const;
  let state = initialState(route);
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, {
    kind: "route-loaded",
    scope: state.scope,
    result: {
      route,
      snapshot,
      outcome: "success-limited",
      follow: { state: "disabled" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "page",
        volume: { vol_id: 0, total_sectors: 64 },
        sector: { vol_id: 0, sector_id: 0, reserved: true, pages: [page] },
        page: {
          page,
          deep: { state: "not-enriched" },
          slots: [],
          distribution: { state: "not-available" },
        },
        enriching: false,
      },
    },
  });
  state = reduce(state, { kind: "effects-started", ids: [2] });
  state = reduce(state, { kind: "hierarchy-back" });
  expect(state.effects).toEqual([
    {
      id: 3,
      kind: "history-back",
      parent: { kind: "sector", vol: 0, sector: 0 },
    },
  ]);
});

test("a direct OOS route recovers through scoped enrichment and falls back semantically", () => {
  const route = { kind: "oos", vol: 0, page: 10, slot: 2 } as const;
  let state = initialState(route);
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, { kind: "route-needs-enrichment", scope: state.scope, route });
  expect(state.effects).toEqual([{
    id: 2,
    kind: "enrich-route",
    scope: "1:/oos/0/10/2",
    selector: "oos:0:10:2",
    targetRoute: route,
    history: "replace",
    fallbackRoute: { kind: "slot", vol: 0, page: 10, slot: 2 },
  }]);

  const staleScope = state.scope;
  state = reduce(state, {
    kind: "navigate",
    route: { kind: "volume", vol: 0 },
    history: "push",
    autoEnrich: false,
  });
  const unchanged = reduce(state, {
    kind: "enrichment-failed",
    scope: staleScope,
    fallbackRoute: { kind: "slot", vol: 0, page: 10, slot: 2 },
    error: { message: "late", code: "entity-not-found", status: 404 },
  });
  expect(unchanged.route).toEqual({ kind: "volume", vol: 0 });
  expect(unchanged.error).toBeNull();
});
