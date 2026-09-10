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
  return { capability: "active", pages: effect.request.pages, epoch: effect.request.epoch, generation: "1", state: "resident", reason: "observed-resident", upperAgeMs: 100, incarnation: "fixture-incarnation", captureLabel: "fixture interval", evidence: { latch_mode: "read" }, requested: 1, evaluated: 1, complete: true, limitations: ["No page-image correspondence"] };
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
  expect([999, 1000, 1001, null].map(observationIsFresh)).toEqual([true, true, false, false]);
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
