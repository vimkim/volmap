import { objectData } from "./api";
import type { UiState, RuntimeCapabilityState } from "./model";

export class ObservationProtocolError extends Error {}

export interface ObservationRequest {
  readonly pages: readonly { readonly volid: number; readonly pageid: number }[];
  readonly epoch: string;
  readonly generation: string;
  readonly retry: boolean;
  readonly cadence_ms?: number;
  readonly after_request?: boolean;
}

export interface ObservationBatch {
  readonly capability: RuntimeCapabilityState;
  readonly pages: ObservationRequest["pages"];
  readonly epoch: string;
  readonly generation: string;
  readonly state: string;
  readonly reason: string;
  readonly upperAgeMs: number | null;
  readonly incarnation: string | null;
  readonly captureLabel: string;
  readonly captureIdentity?: string;
  readonly evidence: Readonly<Record<string, string>>;
  readonly requested: number;
  readonly evaluated: number;
  readonly complete: boolean | null;
  readonly limitations: readonly string[];
}

export interface ObservationUi {
  readonly enabled: boolean;
  readonly epoch: number;
  readonly loading: boolean;
  readonly batch: ObservationBatch | null;
  readonly received: number;
  readonly wallReceived: number;
  readonly age: number | null;
  readonly message: string;
  readonly failures: number;
  readonly stopped: boolean;
  readonly resumeRequired: boolean;
  readonly newerAvailable: boolean;
}

export type ObservationAction =
  | Readonly<{ kind: "observation-due"; epoch: number }>
  | Readonly<{ kind: "toggle-observation" }>
  | Readonly<{ kind: "refresh-observation" }>
  | Readonly<{ kind: "observation-loaded"; scope: string; request: ObservationRequest; batch: ObservationBatch | null; failure?: "protocol-incompatible"; received: number; wallReceived: number; roundTrip: number }>
  | Readonly<{ kind: "observation-ticked"; now: number; wallNow: number }>;

export interface ObservationDelay {
  readonly id: number;
  readonly kind: "delay-observation";
  readonly epoch: number;
  readonly milliseconds: number;
  readonly jitter: boolean;
}

export interface ObservationEffect {
  readonly id: number;
  readonly kind: "read-observation";
  readonly scope: string;
  readonly request: ObservationRequest;
}

export function initialObservation(): ObservationUi {
  return { enabled: false, epoch: 0, loading: false, batch: null, received: 0, wallReceived: 0, age: null, message: "Not observed", failures: 0, stopped: false, resumeRequired: false, newerAvailable: false };
}

export function observationAction(state: UiState, action: ObservationAction): UiState {
  const previous = state.observation;
  if (action.kind === "toggle-observation") {
    return scheduleObservation({ ...state, observation: { ...initialObservation(), enabled: !previous.enabled, epoch: previous.epoch + 1 } }, state.follow.paused ? 5000 : 0);
  }
  if (action.kind === "refresh-observation" || action.kind === "observation-due") {
    if (action.kind === "observation-due") {
      if (action.epoch !== previous.epoch || !previous.enabled || !state.visible || previous.loading) return state;
      if (state.follow.paused) return { ...state, observation: { ...previous, loading: true }, nextEffectId: state.nextEffectId + 1,
        effects: [...state.effects, { kind: "read-runtime-capability", id: state.nextEffectId, epoch: previous.epoch }] };
      if (previous.stopped) return state;
    }
    if (!previous.enabled || state.follow.paused || !state.visible || previous.loading ||
        !("page" in state.route) || state.snapshot == null) return state;
    const epoch = previous.epoch + 1;
    const request: ObservationRequest = { pages: [{ volid: state.route.vol, pageid: state.route.page }], epoch: String(epoch), generation: state.snapshot.generation ?? "0", retry: action.kind === "refresh-observation", cadence_ms: 500, after_request: previous.resumeRequired };
    return { ...state, observation: { ...previous, epoch, loading: true, stopped: false, message: "Observing selected page" }, nextEffectId: state.nextEffectId + 1,
      effects: [...state.effects, { kind: "read-observation", id: state.nextEffectId, scope: state.scope, request }] };
  }
  if (action.kind === "observation-loaded") {
    if (!previous.enabled || state.follow.paused || !state.visible || action.scope !== state.scope ||
        action.request.epoch !== String(previous.epoch) || action.request.generation !== (state.snapshot?.generation ?? "0")) return state;
    const batch = action.batch;
    if (batch !== null && (batch.epoch !== action.request.epoch || batch.generation !== action.request.generation ||
        batch.pages.length !== action.request.pages.length || batch.pages.some((page, index) =>
          page.volid !== action.request.pages[index]?.volid || page.pageid !== action.request.pages[index]?.pageid))) {
      return stopObservation(state, "incompatible", "Incompatible observation response");
    }
    if (action.failure === "protocol-incompatible") return stopObservation(state, "incompatible", "Incompatible observation response");
    if (batch === null) return failedObservation(state, "unavailable", "Observation unavailable");
    if (batch.capability === "refused" || batch.capability === "incompatible") return stopObservation(state, batch.capability, batch.reason);
    if (batch.incarnation === null) return failedObservation(state, batch.capability, batch.reason);
    if (previous.batch !== null && previous.batch.incarnation !== batch.incarnation) return stopObservation(state, "refused", "incarnation-changed");
    if (batch.capability === "stale" && (previous.batch !== null || previous.resumeRequired)) return failedObservation(state, batch.capability, batch.reason);
    let age = batch.upperAgeMs === null || !Number.isFinite(action.roundTrip) || action.roundTrip < 0
      ? null : batch.upperAgeMs + action.roundTrip;
    if (batch.captureIdentity !== undefined && batch.captureIdentity === previous.batch?.captureIdentity) {
      const priorAge = observationAgeAt(previous, action.received, action.wallReceived);
      age = priorAge === null || age === null ? null : Math.max(priorAge, age);
    }
    if (age === null || age >= 30_000) return expireObservation(state);
    const adopted = { ...state, runtimeCapability: batch.capability, observation: { ...previous, loading: false, batch, received: action.received, wallReceived: action.wallReceived, age, failures: 0, resumeRequired: false, newerAvailable: false, message: batch.state } };
    return batch.capability === "stale" ? failedObservation(adopted, "stale", batch.reason) : scheduleObservation(adopted, 500);
  }
  if (previous.batch === null) return state;
  const age = observationAgeAt(previous, action.now, action.wallNow);
  if (age === null || age >= 30_000) return expireObservation(state);
  return { ...state, observation: { ...previous, age, received: action.now, wallReceived: action.wallNow } };
}

export function observationAgeAt(previous: ObservationUi, now: number, wallNow: number): number | null {
  const monotonic = now - previous.received;
  const wall = wallNow - previous.wallReceived;
  if (previous.age === null || monotonic < 0 || wall < 0 || !Number.isFinite(monotonic) || !Number.isFinite(wall) || Math.abs(monotonic - wall) > 1000) return null;
  return previous.age + Math.max(monotonic, wall);
}

function expireObservation(state: UiState): UiState {
  const next = { ...state, observation: { ...state.observation, batch: null, age: null, message: "Observation expired", resumeRequired: true,
    loading: false, epoch: state.observation.epoch + 1 } };
  return scheduleObservation(next, state.follow.paused ? 5000 : 500);
}

function stopObservation(state: UiState, capability: RuntimeCapabilityState, message: string): UiState {
  return { ...state, runtimeCapability: capability, observation: { ...state.observation, loading: false, batch: null, age: null, stopped: true, message, epoch: state.observation.epoch + 1 } };
}

function failedObservation(state: UiState, capability: RuntimeCapabilityState, message: string): UiState {
  const failures = state.observation.failures;
  return scheduleObservation({ ...state, runtimeCapability: capability,
    observation: { ...state.observation, loading: false, failures: Math.min(5, failures + 1), message } },
    Math.min(8000, 500 * 2 ** failures), true);
}

function scheduleObservation(state: UiState, milliseconds: number, jitter = false): UiState {
  if (!state.observation.enabled || (state.observation.stopped && !state.follow.paused) || !state.visible) return state;
  return { ...state, nextEffectId: state.nextEffectId + 1,
    effects: [...state.effects, { kind: "delay-observation", id: state.nextEffectId, epoch: state.observation.epoch, milliseconds, jitter }] };
}

export function invalidateObservation(before: UiState, after: UiState): UiState {
  const scopeChanged = before.scope !== after.scope;
  const samePage = "page" in before.route && "page" in after.route && before.route.vol === after.route.vol && before.route.page === after.route.page;
  const routeChanged = scopeChanged && !samePage;
  const generationChanged = before.snapshot?.generation !== after.snapshot?.generation;
  const paused = before.follow.paused !== after.follow.paused;
  const visibility = before.visible !== after.visible;
  if (!scopeChanged && !generationChanged && !paused && !visibility) return after;
  const next = { ...after, observation: { ...after.observation, epoch: after.observation.epoch + 1, loading: false,
    resumeRequired: after.observation.resumeRequired || (paused && !after.follow.paused) || visibility,
    ...(routeChanged ? { batch: null, age: null, message: "Not observed" } : {}) },
    effects: after.effects.filter((effect) => effect.kind !== "read-observation" && effect.kind !== "delay-observation" && (effect.kind !== "read-runtime-capability" || (effect.epoch === undefined && after.visible))) };
  return scheduleObservation(next, after.follow.paused ? 5000 : 0);
}

export interface CapabilityMetadata {
  readonly state: RuntimeCapabilityState;
  readonly reason: string;
  readonly incarnation: string | null;
  readonly captureIdentity: string | null;
  readonly revision: bigint;
}

export function observationMetadata(state: UiState, metadata: CapabilityMetadata, epoch: number): UiState {
  const previous = state.observation;
  if (!previous.enabled || !state.visible || epoch !== previous.epoch) return state;
  let next = { ...state, observation: { ...previous, loading: false } };
  // These requests are serial and epoch-bound. A server-local revision may
  // restart at zero, so it cannot suppress connectivity or incarnation checks.
  next = { ...next, runtimeCapability: metadata.state };
  if (metadata.state === "refused" || metadata.state === "incompatible" ||
      (metadata.incarnation !== null && previous.batch !== null && metadata.incarnation !== previous.batch.incarnation)) {
    next = stopObservation(next, metadata.state, metadata.reason === "observation-available" ? "incarnation-changed" : metadata.reason);
  } else if (metadata.reason === "observation-expired") {
    next = { ...next, observation: { ...next.observation, batch: null, age: null, message: "Observation expired" } };
  } else {
    next = { ...next, observation: { ...next.observation, newerAvailable: metadata.captureIdentity !== null && metadata.captureIdentity !== previous.batch?.captureIdentity } };
  }
  return scheduleObservation(next, 5000);
}

export function decodeCapabilityMetadata(value: unknown): CapabilityMetadata {
  const data = objectData(value);
  const state = decodeCapability(value);
  const revision = data.revision === undefined ? "0" : text(data.revision);
  if (!/^(0|[1-9][0-9]{0,19})$/.test(revision)) throw new Error("invalid runtime capability revision");
  return { state, reason: text(data.reason), revision: BigInt(revision),
    incarnation: data.incarnation_binding == null ? null : text(data.incarnation_binding),
    captureIdentity: data.capture_identity == null ? null : text(data.capture_identity) };
}

function text(value: unknown): string {
  if (typeof value !== "string" || value.length > 256) throw new Error("invalid observation text");
  return value;
}

function count(value: unknown, max = 512): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value > max) throw new Error("invalid observation count");
  return value;
}

export function decodeCapability(value: unknown): RuntimeCapabilityState {
  const data = objectData(value);
  const reasons: Record<RuntimeCapabilityState, readonly string[]> = {
    disabled: ["not-requested"], connecting: ["observation-in-flight"],
    active: ["observation-available"], stale: ["observation-available", "no-usable-observation", "busy", "rate-limited", "parameter-off"],
    unavailable: ["no-usable-observation", "observation-expired", "busy", "rate-limited", "parameter-off"],
    refused: ["identity-mismatch", "peer-refused", "peer-unverifiable", "incarnation-changed", "identity-oversized"],
    incompatible: ["version-unsupported"],
  };
  const state = data.state as RuntimeCapabilityState;
  if (data.schema !== "volmap.runtime" || data.schema_version !== 1 || data.source !== "cubrid-page-buffer-observation" ||
      !Object.hasOwn(reasons, state) || !reasons[state].includes(String(data.reason)) ||
      data.verification !== (["active", "stale"].includes(state) ? "verified" : "unverified")) throw new Error("invalid runtime capability");
  return state;
}

export function decodeObservation(value: unknown): ObservationBatch {
  const data = objectData(value);
  if (data.schema !== "volmap.runtime.page-buffer" || data.schema_version !== 1 ||
      !Array.isArray(data.pages) || data.pages.length > 512 || !Array.isArray(data.observations) || data.observations.length !== data.pages.length) throw new Error("invalid observation envelope");
  const pages = data.pages.map((page) => { const entry = objectData(page); return { volid: count(entry.volid, 32767), pageid: count(entry.pageid, 2147483647) }; });
  const capability = objectData(data.capability);
  const capabilityState = decodeCapability(capability);
  const row = objectData(data.observations[0]);
  if (!pages[0] || row.volid !== pages[0].volid || row.pageid !== pages[0].pageid ||
      !["resident", "not-resident", "unknown", "unavailable", "expired"].includes(text(row.state))) throw new Error("invalid observation row");
  const capture = data.capture === null ? null : objectData(data.capture);
  if ((capture !== null) !== ["active", "stale"].includes(capabilityState) ||
      (row.state === "resident" && (capture === null || row.evidence === null)) ||
      (row.state === "not-resident" && (capture === null || data.producer_complete !== true))) throw new Error("inconsistent observation evidence");
  const evidence: Record<string, string> = {};
  if (row.evidence !== null) {
    const fields = objectData(row.evidence);
    for (const field of ["page_kind", "latch_mode", "waiter_present", "fix_count", "dirty", "flushing", "async_flush_requested", "to_vacuum", "lru_zone", "lru_list_kind", "lru_list_index", "page_lsa", "oldest_unflush_lsa"]) {
      const item = fields[field];
      if (item === undefined) evidence[field] = "unknown";
      else if (item === null) evidence[field] = "none / not applicable";
      else if (typeof item === "boolean" || typeof item === "number") evidence[field] = String(item);
      else if (typeof item === "string") evidence[field] = text(item);
      else { const lsa = objectData(item); evidence[field] = `${text(lsa.pageid)}:${count(lsa.offset, 32767)}`; }
    }
  }
  if (!Array.isArray(data.limitations) || data.limitations.length > 16) throw new Error("invalid limitations");
  if (data.producer_complete !== null && typeof data.producer_complete !== "boolean") throw new Error("invalid completeness");
  return { capability: capabilityState, pages, epoch: text(data.epoch), generation: text(data.generation), state: text(row.state), reason: capabilityState !== "active" ? text(capability.reason) : text(row.reason),
    upperAgeMs: capture === null ? null : count(capture.upper_age_ms, Number.MAX_SAFE_INTEGER),
    incarnation: capture === null ? null : text(capture.incarnation_binding),
    captureIdentity: capture === null ? undefined : text(capture.identity),
    captureLabel: capture === null ? "No capture" : `Verified · database ${text(capture.database_fingerprint).slice(0, 12)} · Protocol ${count(capture.protocol_major)}.${count(capture.protocol_minor, 2147483647)} · ${text(capture.incarnation)} · scan ${text(capture.sequence)} · ${text(capture.start_time_us)}–${text(capture.end_time_us)} µs (source wall time)`,
    evidence, requested: count(data.requested_count), evaluated: count(data.evaluated_count), complete: data.producer_complete,
    limitations: data.limitations.map(text) };
}

export const selectedObservationIntervalMs = 500;
export function observationIsFresh(age: number | null): boolean {
  return age !== null && age >= 0 && age <= 2 * selectedObservationIntervalMs;
}
