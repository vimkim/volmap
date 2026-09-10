import { objectData } from "./api";
import type { UiState, RuntimeCapabilityState } from "./model";

export interface ObservationRequest {
  readonly pages: readonly { readonly volid: number; readonly pageid: number }[];
  readonly epoch: string;
  readonly generation: string;
  readonly retry: boolean;
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
}

export type ObservationAction =
  | Readonly<{ kind: "toggle-observation" }>
  | Readonly<{ kind: "refresh-observation" }>
  | Readonly<{ kind: "observation-loaded"; scope: string; request: ObservationRequest; batch: ObservationBatch | null; received: number; wallReceived: number; roundTrip: number }>
  | Readonly<{ kind: "observation-ticked"; now: number; wallNow: number }>;

export interface ObservationEffect {
  readonly id: number;
  readonly kind: "read-observation";
  readonly scope: string;
  readonly request: ObservationRequest;
}

export function initialObservation(): ObservationUi {
  return { enabled: false, epoch: 0, loading: false, batch: null, received: 0, wallReceived: 0, age: null, message: "Not observed" };
}

export function observationAction(state: UiState, action: ObservationAction): UiState {
  const previous = state.observation;
  if (action.kind === "toggle-observation") {
    return { ...state, observation: { ...initialObservation(), enabled: !previous.enabled, epoch: previous.epoch + 1 } };
  }
  if (action.kind === "refresh-observation") {
    if (!previous.enabled || state.follow.paused || !state.visible || previous.loading ||
        !("page" in state.route) || state.snapshot == null) return state;
    const epoch = previous.epoch + 1;
    const request: ObservationRequest = { pages: [{ volid: state.route.vol, pageid: state.route.page }], epoch: String(epoch), generation: state.snapshot.generation ?? "0", retry: true };
    return { ...state, observation: { ...previous, epoch, loading: true, message: "Observing selected page" }, nextEffectId: state.nextEffectId + 1,
      effects: [...state.effects, { kind: "read-observation", id: state.nextEffectId, scope: state.scope, request }] };
  }
  if (action.kind === "observation-loaded") {
    if (!previous.enabled || state.follow.paused || !state.visible || action.scope !== state.scope ||
        action.request.epoch !== String(previous.epoch) || action.request.generation !== (state.snapshot?.generation ?? "0")) return state;
    const batch = action.batch;
    if (batch !== null && (batch.epoch !== action.request.epoch || batch.generation !== action.request.generation ||
        batch.pages.length !== action.request.pages.length || batch.pages.some((page, index) =>
          page.volid !== action.request.pages[index]?.volid || page.pageid !== action.request.pages[index]?.pageid))) {
      return { ...state, observation: { ...previous, loading: false, message: "Incompatible observation response" } };
    }
    if (batch === null) return { ...state, observation: { ...previous, loading: false, message: "Observation unavailable" } };
    const age = batch.upperAgeMs === null || !Number.isFinite(action.roundTrip) || action.roundTrip < 0
      ? null : batch.upperAgeMs + action.roundTrip;
    if (age !== null && age >= 30_000) return { ...state, observation: { ...previous, loading: false, batch: null, age: null, message: "Observation expired" } };
    return { ...state, runtimeCapability: batch.capability, observation: { ...previous, loading: false, batch, received: action.received, wallReceived: action.wallReceived, age, message: batch.state } };
  }
  if (previous.batch === null) return state;
  const monotonicElapsed = action.now - previous.received;
  const wallElapsed = action.wallNow - previous.wallReceived;
  // Clock discontinuity destroys freshness authority. A forward wall step
  // only makes evidence older; backwards steps expire it conservatively.
  const elapsed = Math.max(monotonicElapsed, wallElapsed);
  if (monotonicElapsed < 0 || wallElapsed < 0 || !Number.isFinite(elapsed) ||
      previous.batch.upperAgeMs === null || elapsed + (previous.age ?? 30_000) >= 30_000) {
    return { ...state, observation: { ...previous, batch: null, age: null, message: "Observation expired" } };
  }
  return { ...state, observation: { ...previous, age: (previous.age ?? 0) + elapsed, received: action.now, wallReceived: action.wallNow } };
}

export function invalidateObservation(before: UiState, after: UiState): UiState {
  const changed = before.scope !== after.scope || before.snapshot?.generation !== after.snapshot?.generation;
  const paused = before.follow.paused !== after.follow.paused;
  const hidden = before.visible && !after.visible;
  if (!changed && !paused && !hidden) return after;
  return { ...after, observation: { ...after.observation, epoch: after.observation.epoch + 1, loading: false,
    ...(changed ? { batch: null, age: null, message: "Not observed" } : {}) } };
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
    active: ["observation-available"], stale: ["observation-available"],
    unavailable: ["no-usable-observation"],
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
  return { capability: capabilityState, pages, epoch: text(data.epoch), generation: text(data.generation), state: text(row.state), reason: ["refused", "incompatible"].includes(text(capability.state)) ? text(capability.reason) : text(row.reason),
    upperAgeMs: capture === null ? null : count(capture.upper_age_ms, Number.MAX_SAFE_INTEGER),
    incarnation: capture === null ? null : text(capture.incarnation_binding),
    captureLabel: capture === null ? "No capture" : `Verified · database ${text(capture.database_fingerprint).slice(0, 12)} · Protocol ${count(capture.protocol_major)}.${count(capture.protocol_minor, 2147483647)} · ${text(capture.incarnation)} · scan ${text(capture.sequence)} · ${text(capture.start_time_us)}–${text(capture.end_time_us)} µs (source wall time)`,
    evidence, requested: count(data.requested_count), evaluated: count(data.evaluated_count), complete: data.producer_complete,
    limitations: data.limitations.map(text) };
}

export const selectedObservationIntervalMs = 500;
export function observationIsFresh(age: number | null): boolean {
  return age !== null && age >= 0 && age <= 2 * selectedObservationIntervalMs;
}
