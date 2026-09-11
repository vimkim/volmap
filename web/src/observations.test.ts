import { expect, test } from "vitest";
import { initialState, reduce } from "./model";

test("explicit selected-page refresh is disabled while paused and navigation revokes its epoch", () => {
  let state = initialState({ kind: "page", vol: 0, page: 7 });
  state = { ...state, visible: true, snapshot: { id: "test", revision: "1", validity: "valid", format_profile: "fixture", generation: "1", observed_at_unix_seconds: "1", input_modified_unix_seconds: "1" } };
  state = reduce(state, { kind: "toggle-observation" });
  state = reduce(state, { kind: "refresh-observation" });
  expect(state.effects.at(-1)?.kind).toBe("read-observation");
  const epoch = state.observation.epoch;
  state = reduce(state, { kind: "navigate", route: { kind: "page", vol: 0, page: 8 }, history: "push", autoEnrich: false });
  expect(state.observation.epoch).toBeGreaterThan(epoch);
  expect(state.observation.batch).toBeNull();
  state = { ...state, follow: { ...state.follow, paused: true } };
  const effects = state.effects;
  state = reduce(state, { kind: "refresh-observation" });
  expect(state.effects).toEqual(effects);
});

import type { ObservationBatch, ObservationEffect } from "./observations";

function requested() {
  let state = initialState({ kind: "page", vol: 0, page: 7 });
  state = { ...state, visible: true, follow: { ...state.follow, enabled: true }, snapshot: { id: "test", revision: "1", validity: "valid", format_profile: "fixture", generation: "1", observed_at_unix_seconds: "1", input_modified_unix_seconds: "1" } };
  state = reduce(reduce(state, { kind: "toggle-observation" }), { kind: "refresh-observation" });
  const effect = state.effects.at(-1);
  if (effect?.kind !== "read-observation") throw new Error("expected explicit observation demand");
  return { state, effect };
}

function batch(effect: ObservationEffect): ObservationBatch {
  return { rows: [], capability: "active", pages: effect.request.pages, epoch: effect.request.epoch, generation: "1", state: "resident", reason: "observed-resident", upperAgeMs: 100, incarnation: "fixture-incarnation", captureLabel: "fixture interval", evidence: { latch_mode: "read" }, requested: 1, evaluated: 1, complete: true, limitations: ["No page-image correspondence"] };
}

function loaded(effect: ObservationEffect, observation = batch(effect)) {
  return { kind: "observation-loaded" as const, scope: effect.scope, request: effect.request, batch: observation, received: 1000, wallReceived: 100000, roundTrip: 200 };
}

test("echoed epoch and ordered VPIDs independently reject incompatible responses", () => {
  const { state, effect } = requested();
  for (const response of [
    { ...batch(effect), epoch: "999" },
    { ...batch(effect), pages: [{ volid: 0, pageid: 8 }] },
    { ...batch(effect), generation: "2" },
  ]) {
    const next = reduce(state, loaded(effect, response));
    expect(next.observation.batch).toBeNull();
    expect(next.observation.message).toBe("Incompatible observation response");
  }
});

test("pause revokes late adoption independently of request cancellation", () => {
  const { state, effect } = requested();
  const paused = reduce(state, { kind: "toggle-pause" });
  expect(paused.follow.paused).toBe(true);
  expect(reduce(paused, loaded(effect)).observation.batch).toBeNull();
});

test("full round trip contributes to conservative age and paused evidence expires at 30 seconds", () => {
  const { state, effect } = requested();
  let observed = reduce(state, loaded(effect));
  expect(observed.observation.age).toBe(300);
  observed = reduce(observed, { kind: "toggle-pause" });
  observed = reduce(observed, { kind: "observation-ticked", now: 30699, wallNow: 129699 });
  expect(observed.observation.age).toBe(29999);
  expect(observed.observation.batch?.state).toBe("resident");
  observed = reduce(observed, { kind: "observation-ticked", now: 30700, wallNow: 129700 });
  expect(observed.observation.batch).toBeNull();
  expect(observed.observation.message).toBe("Observation expired");
});

test("a backwards browser wall clock cannot extend observation freshness", () => {
  const { state, effect } = requested();
  const observed = reduce(state, loaded(effect));
  const stepped = reduce(observed, { kind: "observation-ticked", now: 1100, wallNow: 99999 });
  expect(stepped.observation.batch).toBeNull();
});

import { decodeObservation, observationIsFresh } from "./observations";

test("selected-page freshness uses two 500 ms caller intervals", () => {
  expect([999, 1000, 1001, null].map((age) => observationIsFresh(age))).toEqual([true, true, false, false]);
});

test("absent optional evidence remains unknown while nullable LSA is known empty", () => {
  const decoded = decodeObservation({
    schema: "volmap.runtime.page-buffer", schema_version: 1,
    capability: { schema: "volmap.runtime", schema_version: 1, source: "cubrid-page-buffer-observation", state: "active", reason: "observation-available", verification: "verified" },
    pages: [{volid: 0, pageid: 7}], epoch: "1", generation: "0", requested_count: 1, evaluated_count: 1, producer_complete: true,
    capture: { identity: "capture", incarnation_binding: "binding", incarnation: "0123456789ab", protocol_major: 1, protocol_minor: 0, database_fingerprint: "fingerprint", sequence: "1", start_time_us: "1", end_time_us: "2", upper_age_ms: 100 },
    observations: [{volid: 0, pageid: 7, state: "resident", reason: "observed-resident", evidence: {volid: 0, pageid: 7, page_lsa: null}}], limitations: [],
  });
  expect(decoded.evidence.dirty).toBe("unknown");
  expect(decoded.evidence.fix_count).toBe("unknown");
  expect(decoded.evidence.latch_mode).toBe("unknown");
  expect(decoded.evidence.page_lsa).toBe("none / not applicable");
});

test("enabled selected-page observations poll without explicit retry and never queue missed ticks", () => {
  const { state, effect } = requested();
  let next = reduce(state, loaded(effect));
  next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
  const refresh = next.effects.at(-1);
  expect(refresh?.kind).toBe("read-observation");
  if (refresh?.kind !== "read-observation") throw new Error("missing poll");
  expect(refresh.request.retry).toBe(false);
  expect(refresh.request.cadence_ms).toBe(500);
  const effects = next.effects;
  next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
  expect(next.effects).toEqual(effects);
});

test("transient failures retain original evidence age, back off, and reset only after a valid scan", () => {
  const { state, effect } = requested();
  let next = reduce(state, loaded(effect));
  for (const delay of [500, 1000, 2000, 4000, 8000, 8000]) {
    next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
    const request = next.effects.at(-1);
    if (request?.kind !== "read-observation") throw new Error("missing retry");
    next = reduce(next, { ...loaded(request), batch: null, received: 1500, wallReceived: 100500 });
    expect(next.observation.batch?.state).toBe("resident");
    expect(next.observation.received).toBe(1000);
    expect(next.effects.at(-1)).toMatchObject({ kind: "delay-observation", milliseconds: delay, jitter: true });
    expect(next.runtimeCapability).toBe("unavailable");
  }
  next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
  const request = next.effects.at(-1);
  if (request?.kind !== "read-observation") throw new Error("missing retry");
  next = reduce(next, loaded(request));
  expect(next.observation.failures).toBe(0);
  expect(next.effects.at(-1)).toMatchObject({ milliseconds: 500, jitter: false });
});

test("refusal clears evidence and stops automatic retry until explicit refresh", () => {
  const { state, effect } = requested();
  for (const capability of ["refused", "incompatible"] as const) {
    let next = reduce(state, loaded(effect, { ...batch(effect), capability, state: "unavailable", incarnation: null, upperAgeMs: null, reason: "identity-mismatch" }));
    expect(next.observation.batch).toBeNull();
    expect(next.observation.stopped).toBe(true);
    const effects = next.effects;
    next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
    expect(next.effects).toEqual(effects);
    next = reduce(next, { kind: "refresh-observation" });
    expect(next.effects.at(-1)).toMatchObject({ kind: "read-observation", request: { retry: true } });
  }
});

test("pause schedules only metadata; hidden revokes requests; resume demands a post-request scan", () => {
  const { state, effect } = requested();
  let next = reduce(state, loaded(effect));
  next = reduce(next, { kind: "toggle-pause" });
  expect(next.effects.at(-1)).toMatchObject({ kind: "delay-observation", milliseconds: 5000 });
  next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
  expect(next.effects.at(-1)).toMatchObject({ kind: "read-runtime-capability", epoch: next.observation.epoch });
  const metadataEpoch = next.observation.epoch;
  next = reduce(next, { kind: "visibility-changed", visible: false });
  const effects = next.effects;
  expect(reduce(next, { kind: "observation-due", epoch: metadataEpoch }).effects).toEqual(effects);
  next = reduce(next, { kind: "visibility-changed", visible: true });
  next = reduce(next, { kind: "toggle-pause" });
  next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
  expect(next.effects.at(-1)).toMatchObject({ kind: "read-observation", request: { after_request: true, retry: false } });
  expect(reduce(next, loaded(effect)).observation.loading).toBe(true);
});

test("paused metadata is availability only and restart clears evidence even without resume", () => {
  const { state, effect } = requested();
  const observed = reduce(state, loaded(effect, { ...batch(effect), captureIdentity: "scan-1" }));
  const paused = reduce(observed, { kind: "toggle-pause" });
  const metadata = { state: "active" as const, reason: "observation-available", incarnation: "fixture-incarnation", captureIdentity: "scan-2", revision: 2n };
  const offered = reduce(paused, { kind: "runtime-capability-loaded", state: "active", metadata, epoch: paused.observation.epoch });
  expect(offered.observation.batch).toEqual(observed.observation.batch);
  expect(offered.observation.age).toBe(300);
  expect(offered.observation.newerAvailable).toBe(true);
  const restarted = reduce(offered, { kind: "runtime-capability-loaded", state: "active", metadata: { ...metadata, incarnation: "new-incarnation", revision: 3n }, epoch: offered.observation.epoch });
  expect(restarted.observation.batch).toBeNull();
  expect(restarted.observation.epoch).toBeGreaterThan(offered.observation.epoch);
  expect(reduce(restarted, loaded(effect)).observation.batch).toBeNull();
});

test("same-page generation replacement revokes late adoption but retains state-only evidence", () => {
  const { state, effect } = requested();
  const observed = reduce(state, loaded(effect));
  const next = invalidateObservation(observed, { ...observed, scope: "new-scope", snapshot: { ...observed.snapshot!, generation: "2" } });
  expect(next.observation.batch).toEqual(observed.observation.batch);
  expect(next.observation.epoch).toBeGreaterThan(observed.observation.epoch);
  expect(reduce(next, loaded(effect)).observation.batch).toEqual(observed.observation.batch);
});

test("reusing a capture cannot lower age and suspension cannot revive it from cache", () => {
  const { state, effect } = requested();
  let next = reduce(state, loaded(effect, { ...batch(effect), captureIdentity: "same" }));
  next = reduce(next, { kind: "refresh-observation" });
  const request = next.effects.at(-1);
  if (request?.kind !== "read-observation") throw new Error("missing observation");
  next = reduce(next, { ...loaded(request, { ...batch(request), captureIdentity: "same", upperAgeMs: 1 }), received: 1500, wallReceived: 100500, roundTrip: 1 });
  expect(next.observation.age).toBe(800);
  next = reduce(next, { kind: "observation-ticked", now: 1501, wallNow: 105501 });
  expect(next.observation.batch).toBeNull();
  expect(next.observation.resumeRequired).toBe(true);
});

import { invalidateObservation } from "./observations";

test("failed resume retains its barrier and cannot adopt another tab's old cached capture", () => {
  const { state, effect } = requested();
  let next = reduce(state, { kind: "toggle-pause" });
  next = reduce(next, { kind: "toggle-pause" });
  next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
  const resumed = next.effects.at(-1);
  if (resumed?.kind !== "read-observation") throw new Error("missing resume demand");
  expect(resumed.request.after_request).toBe(true);
  next = reduce(next, loaded(resumed, { ...batch(resumed), capability: "stale" }));
  expect(next.observation.batch).toBeNull();
  expect(next.observation.resumeRequired).toBe(true);
  next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
  expect(next.effects.at(-1)).toMatchObject({ request: { after_request: true } });
  expect(reduce(next, loaded(effect)).observation.batch).toBeNull();
});

test("a new-incarnation stale fallback clears old evidence before preserving any failure state", () => {
  const { state, effect } = requested();
  let next = reduce(state, loaded(effect));
  next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
  const request = next.effects.at(-1);
  if (request?.kind !== "read-observation") throw new Error("missing demand");
  next = reduce(next, loaded(request, { ...batch(request), capability: "stale", incarnation: "other-tab-reattached" }));
  expect(next.observation.batch).toBeNull();
  expect(next.observation.message).toBe("incarnation-changed");
  expect(next.observation.epoch).toBeGreaterThan(Number(request.request.epoch));
});

test("paused connectivity failure remains visible and a restarted broker's lower revision cannot hide incarnation change", () => {
  const { state, effect } = requested();
  let next = reduce(reduce(state, loaded(effect)), { kind: "toggle-pause" });
  const metadata = { state: "active" as const, reason: "observation-available", incarnation: "fixture-incarnation", captureIdentity: "scan", revision: 9n };
  next = reduce(next, { kind: "runtime-capability-loaded", state: "active", metadata, epoch: next.observation.epoch });
  next = reduce(next, { kind: "runtime-capability-loaded", state: "unavailable", metadata: { ...metadata, state: "unavailable", reason: "no-usable-observation", incarnation: null, captureIdentity: null, revision: 0n }, epoch: next.observation.epoch });
  expect(next.runtimeCapability).toBe("unavailable");
  expect(next.observation.batch?.state).toBe("resident");
  next = reduce(next, { kind: "runtime-capability-loaded", state: "active", metadata: { ...metadata, incarnation: "new-producer", revision: 1n }, epoch: next.observation.epoch });
  expect(next.observation.batch).toBeNull();
  expect(next.observation.message).toBe("incarnation-changed");
});

test("viewport demand preserves selected priority, rotates overflow, and revokes late batches", () => {
  const { state, effect } = requested();
  const pages = Array.from({ length: 600 }, (_, pageid) => ({ volid: 0, pageid }));
  let next = reduce(state, { kind: "observation-viewport", scope: state.scope, pages });
  expect(next.observation.batch).toBeNull();
  expect(reduce(next, loaded(effect)).observation.batch).toBeNull();
  next = reduce(next, { kind: "refresh-observation" });
  const first = next.effects.at(-1);
  if (first?.kind !== "read-observation") throw new Error("missing viewport request");
  expect(first.request.pages).toHaveLength(512);
  expect(first.request.pages[0]).toEqual({ volid: 0, pageid: 7 });
  expect(first.request.pages[1]).toEqual({ volid: 0, pageid: 0 });
  expect(next.observation.viewportCount).toBe(600);
  next = reduce(next, loaded(first, { ...batch(first), requested: 512, evaluated: 512 }));
  next = reduce(next, { kind: "observation-due", epoch: next.observation.epoch });
  const second = next.effects.at(-1);
  if (second?.kind !== "read-observation") throw new Error("missing rotated request");
  expect(second.request.pages[0]).toEqual({ volid: 0, pageid: 7 });
  expect(second.request.pages[1]).toEqual({ volid: 0, pageid: 512 });
  expect(new Set([...first.request.pages, ...second.request.pages].map((page) => page.pageid)).size).toBe(600);
});

function viewportEnvelope() {
  return {
    schema: "volmap.runtime.page-buffer", schema_version: 1,
    capability: { schema: "volmap.runtime", schema_version: 1, source: "cubrid-page-buffer-observation", state: "active", reason: "observation-available", verification: "verified" },
    pages: [{ volid: 0, pageid: 7 }, { volid: 0, pageid: 8 }], epoch: "1", generation: "0", requested_count: 2, evaluated_count: 2, producer_complete: false,
    capture: { identity: "capture", incarnation_binding: "binding", incarnation: "0123456789ab", protocol_major: 1, protocol_minor: 0, database_fingerprint: "fingerprint", sequence: "1", start_time_us: "1", end_time_us: "2", upper_age_ms: 100 },
    observations: [{ volid: 0, pageid: 7, state: "resident", reason: "observed-resident", evidence: { volid: 0, pageid: 7, dirty: true } },
      { volid: 0, pageid: 8, state: "unknown", reason: "partial-omission", evidence: null }], limitations: [],
  };
}

test("every batch row and scope coverage is validated independently of producer completeness", () => {
  const envelope = viewportEnvelope();
  expect(decodeObservation(envelope).rows).toMatchObject([{ state: "resident" }, { state: "unknown", reason: "partial-omission" }]);
  for (const malformed of [
    { ...envelope, observations: [envelope.observations[0], { ...envelope.observations[1], pageid: 9 }] },
    { ...envelope, observations: [envelope.observations[0], { ...envelope.observations[1], state: "not-resident", reason: "observed-not-resident" }] },
    { ...envelope, requested_count: 1 }, { ...envelope, evaluated_count: 3 }, { ...envelope, evaluated_count: 1 },
    { ...envelope, observations: [envelope.observations[0], { ...envelope.observations[1], reason: "observed-not-resident" }] },
  ]) expect(() => decodeObservation(malformed)).toThrow();
});

test("visible-page cadence is two seconds with explicit below, at and above-cap coverage", () => {
  for (const count of [511, 512, 513]) {
    const base = requested().state;
    let state = reduce(base, { kind: "navigate", route: { kind: "volume", vol: 0 }, history: "push", autoEnrich: false });
    state = reduce(state, { kind: "observation-viewport", scope: state.scope, pages: Array.from({ length: count }, (_, pageid) => ({ volid: 0, pageid })) });
    state = reduce(state, { kind: "refresh-observation" });
    const effect = state.effects.at(-1);
    if (effect?.kind !== "read-observation") throw new Error("missing visible scope");
    expect(effect.request.cadence_ms).toBe(2000);
    expect(effect.request.pages).toHaveLength(Math.min(count, 512));
    expect(state.observation.viewportCount).toBe(count);
    expect(state.observation.rotation).toBe(count > 512 ? 512 : 0);
    state = reduce(state, loaded(effect, { ...batch(effect), requested: Math.min(count, 512), evaluated: Math.min(count, 512) }));
    expect(state.effects.at(-1)).toMatchObject({ kind: "delay-observation", milliseconds: 2000 });
    expect(state.observation.message).toBe("Visible-page observations available");
  }
});


test("scoped detail preserves fixed slot addresses across absent physical slots", () => {
  const { pages: _pages, observations: _observations, ...metadata } = viewportEnvelope();
  const slots: unknown[] = Array.from({ length: 64 }, () => null);
  slots[0] = { volid: 2, pageid: 64, state: "unknown", reason: "partial-omission", evidence: null };
  slots[62] = { volid: 2, pageid: 126, state: "unknown", reason: "duplicate-vpid", evidence: null };
  const envelope = { ...metadata, schema: "volmap.runtime.page-buffer.scoped", variant: "sector-detail",
    scope: { kind: "sector", volid: 2, sectorid: 1 }, slots };
  const decoded = decodeObservation(envelope);
  expect(decoded.pages).toEqual([{ volid: 2, pageid: 64 }, { volid: 2, pageid: 126 }]);
  expect(decoded.rows.map((row) => row.reason)).toEqual(["partial-omission", "duplicate-vpid"]);
  expect(decoded.requested).toBe(2);
  expect(decoded.evaluated).toBe(2);
  expect(decoded.complete).toBe(false);
  expect(() => decodeObservation({ ...envelope, slots: slots.filter((slot) => slot !== null) })).toThrow();
});
