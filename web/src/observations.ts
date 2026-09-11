import { objectData } from "./api";
import type { UiState, RuntimeCapabilityState } from "./model";

export class ObservationProtocolError extends Error {}

export interface SectorObservationScope {
  readonly kind: "sector";
  readonly volid: number;
  readonly sectorid: number;
}

export type ViewObservationScope = SectorObservationScope | { readonly kind: "volume"; readonly volid: number; readonly sectorids: readonly number[] };

export interface ObservationRequest {
  readonly scope?: ViewObservationScope;
  readonly pages: readonly { readonly volid: number; readonly pageid: number }[];
  readonly epoch: string;
  readonly generation: string;
  readonly retry: boolean;
  readonly cadence_ms?: number;
  readonly after_request?: boolean;
}

const observationReasons = {
  resident: ["observed-resident"], "not-resident": ["observed-not-resident"],
  unknown: ["partial-omission", "duplicate-vpid", "unevaluated"],
  unavailable: ["no-usable-observation"], expired: ["observation-expired"],
} as const;

type ObservationState = {
  [State in keyof typeof observationReasons]: {
    readonly state: State;
    readonly reason: (typeof observationReasons)[State][number];
  }
}[keyof typeof observationReasons];

export type ObservationRow = ObservationState & {
  readonly volid: number;
  readonly pageid: number;
  readonly evidence: Readonly<Record<string, string>>;
};

function validObservationState(value: { state: string; reason: string }): value is ObservationState {
  const reasons: Readonly<Record<string, readonly string[]>> = observationReasons;
  return Object.hasOwn(reasons, value.state) && reasons[value.state]!.includes(value.reason);
}

export interface ObservationBatch {
  readonly scope?: ViewObservationScope;
  readonly topology?: { readonly shared: number; readonly private: number };
  readonly rows: readonly ObservationRow[];
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
  readonly colorMode: "state" | "lru";
  readonly viewport: ObservationRequest["pages"];
  readonly viewportCount: number;
  readonly rotation: number;
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
  | Readonly<{ kind: "observation-color-mode"; mode: "state" | "lru" }>
  | Readonly<{ kind: "observation-viewport"; scope: string; pages: ObservationRequest["pages"] }>
  | Readonly<{ kind: "observation-due"; epoch: number }>
  | Readonly<{ kind: "toggle-observation" }>
  | Readonly<{ kind: "refresh-observation" }>
  | Readonly<{ kind: "observation-loaded"; scope: string; request: ObservationRequest; batch: ObservationBatch | null; failure?: "protocol-incompatible" | "overloaded"; received: number; wallReceived: number; roundTrip: number }>
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
  return { colorMode: "state", viewport: [], viewportCount: 0, rotation: 0, enabled: false, epoch: 0, loading: false, batch: null, received: 0, wallReceived: 0, age: null, message: "Not observed", failures: 0, stopped: false, resumeRequired: false, newerAvailable: false };
}

export function observationAction(state: UiState, action: ObservationAction): UiState {
  const previous = state.observation;
  if (action.kind === "observation-color-mode") return { ...state, observation: { ...previous, colorMode: action.mode } };
  if (action.kind === "observation-viewport") {
    if (action.scope !== state.scope) return state;
    let pages = action.pages;
    if (state.route.kind === "volume") {
      const sectors = [...new Set(pages.map((page) => Math.floor(page.pageid / 64)))].slice(0, 64);
      const selected = new Set(sectors);
      pages = pages.filter((page) => selected.has(Math.floor(page.pageid / 64))).slice().sort((a, b) => a.pageid - b.pageid);
    }
    const viewportCount = action.pages.length;
    if (samePages(previous.viewport, pages)) return viewportCount === previous.viewportCount ? state : { ...state, observation: { ...previous, viewportCount } };
    return scheduleObservation({ ...state, observation: { ...previous, viewport: pages,
      viewportCount, rotation: 0, batch: null, age: null, loading: false, epoch: previous.epoch + 1 },
      effects: state.effects.filter((effect) => effect.kind !== "read-observation" && effect.kind !== "delay-observation") }, 0);
  }

  if (action.kind === "toggle-observation") {
    return scheduleObservation({ ...state, observation: { ...initialObservation(), viewport: previous.viewport, viewportCount: previous.viewportCount, enabled: !previous.enabled, epoch: previous.epoch + 1 } }, state.follow.paused ? 5000 : 0);
  }
  if (action.kind === "refresh-observation" || action.kind === "observation-due") {
    if (action.kind === "observation-due") {
      if (action.epoch !== previous.epoch || !previous.enabled || !state.visible || previous.loading) return state;
      if (state.follow.paused) return { ...state, observation: { ...previous, loading: true }, nextEffectId: state.nextEffectId + 1,
        effects: [...state.effects, { kind: "read-runtime-capability", id: state.nextEffectId, epoch: previous.epoch }] };
      if (previous.stopped) return state;
    }
    if (!previous.enabled || state.follow.paused || !state.visible || previous.loading ||
        state.route.kind === "root" || state.snapshot == null) return state;
    const epoch = previous.epoch + 1;
    const selected = "page" in state.route ? { volid: state.route.vol, pageid: state.route.page } : null;
    const remainder = previous.viewport.filter((page) => page.volid !== selected?.volid || page.pageid !== selected?.pageid);
    const capacity = (state.route.kind === "volume" ? 4096 : 512) - (selected === null ? 0 : 1);
    const rotated = remainder.length > capacity;
    const offset = rotated ? previous.rotation % remainder.length : 0;
    let pages = selected === null ? [] : [selected];
    for (let index = 0; index < Math.min(capacity, remainder.length); index += 1) pages.push(remainder[(offset + index) % remainder.length]!);
    if (state.route.kind === "sector") {
      if (state.view?.kind !== "sector" || state.view.sector.vol_id !== state.route.vol || state.view.sector.sector_id !== state.route.sector) return scheduleObservation(state, observationInterval(state));
      pages = state.view.sector.pages.map((page) => ({ volid: page.vol_id, pageid: page.page_id })).sort((a, b) => a.pageid - b.pageid);
    }
    if (pages.length === 0) return scheduleObservation(state, observationInterval(state));
    const scope: ViewObservationScope | undefined = state.route.kind === "volume"
      ? { kind: "volume", volid: state.route.vol, sectorids: [...new Set(pages.map((page) => Math.floor(page.pageid / 64)))] }
      : state.route.kind === "sector" ? { kind: "sector", volid: state.route.vol, sectorid: state.route.sector } : undefined;
    const request: ObservationRequest = { pages, scope, epoch: String(epoch), generation: state.snapshot.generation ?? "0", retry: action.kind === "refresh-observation", cadence_ms: observationInterval(state), after_request: previous.resumeRequired };
    return { ...state, observation: { ...previous, epoch, loading: true, stopped: false, viewportCount: state.route.kind === "volume" ? previous.viewportCount : state.route.kind === "sector" ? pages.length : remainder.length + (selected === null ? 0 : 1), rotation: rotated ? (offset + capacity) % remainder.length : 0, message: previous.batch !== null ? previous.message : selected === null ? "Observing visible pages" : "Observing selected page" }, nextEffectId: state.nextEffectId + 1,
      effects: [...state.effects, { kind: "read-observation", id: state.nextEffectId, scope: state.scope, request }] };
  }
  if (action.kind === "observation-loaded") {
    if (!previous.enabled || state.follow.paused || !state.visible || action.scope !== state.scope ||
        action.request.epoch !== String(previous.epoch) || action.request.generation !== (state.snapshot?.generation ?? "0")) return state;
    const batch = action.batch;
    if (batch !== null && (batch.epoch !== action.request.epoch || batch.generation !== action.request.generation ||
        !sameObservationScope(batch.scope, action.request.scope) || batch.pages.length !== action.request.pages.length || batch.pages.some((page, index) =>
          page.volid !== action.request.pages[index]?.volid || page.pageid !== action.request.pages[index]?.pageid))) {
      return stopObservation(state, "incompatible", "Incompatible observation response");
    }
    if (action.failure === "protocol-incompatible") return stopObservation(state, "incompatible", "Incompatible observation response");
    if (batch === null) return failedObservation(state, "unavailable", action.failure === "overloaded" ? "Observation overloaded (HTTP 429); retrying" : "Observation unavailable");
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
    const adopted = { ...state, runtimeCapability: batch.capability, observation: { ...previous, loading: false, batch, received: action.received, wallReceived: action.wallReceived, age, failures: 0, resumeRequired: false, newerAvailable: false, message: "page" in state.route ? batch.state : "Visible-page observations available" } };
    return batch.capability === "stale" ? failedObservation(adopted, "stale", batch.reason) : scheduleObservation(adopted, observationInterval(state));
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
  return scheduleObservation(next, state.follow.paused ? 5000 : observationInterval(state));
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
    ...(routeChanged ? { viewport: [], viewportCount: 0, rotation: 0, batch: null, age: null, message: "Not observed" } : {}) },
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
  const envelope = objectData(value);
  let scope: ViewObservationScope | undefined;
  let data = envelope;
  if (envelope.schema === "volmap.runtime.page-buffer.scoped") {
    if (envelope.schema_version !== 1 || !Array.isArray(envelope.slots) ||
        envelope.pages !== undefined || envelope.observations !== undefined) throw new Error("invalid scoped observation envelope");
    const address = objectData(envelope.scope);
    const volid = count(address.volid, 32767);
    let sectorids: number[];
    if (address.kind === "sector" && envelope.variant === "sector-detail") {
      scope = { kind: "sector", volid, sectorid: count(address.sectorid, 33554431) };
      sectorids = [scope.sectorid];
    } else if (address.kind === "volume" && envelope.variant === "volume-residency-lru" && Array.isArray(address.sectorids) && address.sectorids.length <= 64) {
      sectorids = address.sectorids.map((id) => count(id, 33554431));
      if (sectorids.some((id, index) => index > 0 && id <= sectorids[index - 1]!)) throw new Error("noncanonical sectors");
      scope = { kind: "volume", volid, sectorids };
    } else throw new Error("invalid observation scope");
    if (envelope.slots.length !== sectorids.length * 64) throw new Error("invalid slots");
    const pages: { volid: number; pageid: number }[] = [];
    const observations: unknown[] = [];
    envelope.slots.forEach((slot, index) => {
      if (slot === null) return;
      const page = { volid, pageid: sectorids[Math.floor(index / 64)]! * 64 + index % 64 };
      pages.push(page);
      if (scope?.kind === "volume") {
        const row = objectData(slot);
        if (Object.keys(row).some((key) => !["state", "reason", "evidence"].includes(key))) throw new Error("invalid volume row");
        observations.push({ ...row, ...page });
      } else observations.push(slot);
    });
    data = { ...envelope, schema: "volmap.runtime.page-buffer", pages, observations };
  } else if (envelope.scope !== undefined || envelope.variant !== undefined || envelope.slots !== undefined) {
    throw new Error("invalid legacy observation envelope");
  }
  if (data.schema !== "volmap.runtime.page-buffer" || data.schema_version !== 1 ||
      !Array.isArray(data.pages) || data.pages.length > (scope?.kind === "volume" ? 4096 : 512) || !Array.isArray(data.observations) || data.observations.length !== data.pages.length) throw new Error("invalid observation envelope");
  const pages = data.pages.map((page) => { const entry = objectData(page); return { volid: count(entry.volid, 32767), pageid: count(entry.pageid, 2147483647) }; });
  const capability = objectData(data.capability);
  const capabilityState = decodeCapability(capability);
  const capture = data.capture === null ? null : objectData(data.capture);
  if ((capture !== null) !== ["active", "stale"].includes(capabilityState)) throw new Error("inconsistent capture");
  const topology = capture === null || (capture.shared_lru_count === undefined && capture.private_lru_count === undefined) ? undefined : { shared: count(capture.shared_lru_count, 2147483647), private: count(capture.private_lru_count, 2147483647) };
  const rows = data.observations.map((value, index) => decodeRow(value, pages[index]!, capture !== null, data.producer_complete, topology, scope?.kind === "volume"));
  const requested = count(data.requested_count, scope?.kind === "volume" ? 4096 : 512);
  const evaluated = count(data.evaluated_count, scope?.kind === "volume" ? 4096 : 512);
  const evaluatedRows = rows.filter((row) => row.reason !== "unevaluated" && row.state !== "unavailable" && row.state !== "expired").length;
  if (requested !== pages.length || evaluated !== evaluatedRows) throw new Error("inconsistent scope coverage");
  const row = rows[0];
  if (!Array.isArray(data.limitations) || data.limitations.length > 16) throw new Error("invalid limitations");
  if (data.producer_complete !== null && typeof data.producer_complete !== "boolean") throw new Error("invalid completeness");
  return { scope, capability: capabilityState, pages, epoch: text(data.epoch), generation: text(data.generation), state: row?.state ?? "unknown", reason: capabilityState !== "active" ? text(capability.reason) : row?.reason ?? "unevaluated",
    upperAgeMs: capture === null ? null : count(capture.upper_age_ms, Number.MAX_SAFE_INTEGER),
    incarnation: capture === null ? null : text(capture.incarnation_binding),
    captureIdentity: capture === null ? undefined : text(capture.identity),
    captureLabel: capture === null ? "No capture" : `Verified · database ${text(capture.database_fingerprint).slice(0, 12)} · Protocol ${count(capture.protocol_major)}.${count(capture.protocol_minor, 2147483647)} · ${text(capture.incarnation)} · scan ${text(capture.sequence)} · ${text(capture.start_time_us)}–${text(capture.end_time_us)} µs (source wall time)`,
    topology,
    rows, evidence: row?.evidence ?? {}, requested, evaluated, complete: data.producer_complete,
    limitations: data.limitations.map(text) };
}

export const selectedObservationIntervalMs = 500;
export function observationIsFresh(age: number | null, interval = selectedObservationIntervalMs): boolean {
  return age !== null && age >= 0 && age <= 2 * interval;
}

export function sameObservationScope(left: ViewObservationScope | undefined, right: ViewObservationScope | undefined): boolean {
  if (left === undefined || right === undefined) return left === right;
  if (left.volid !== right.volid) return false;
  if (left.kind === "sector" && right.kind === "sector") return left.sectorid === right.sectorid;
  return left.kind === "volume" && right.kind === "volume" && left.sectorids.length === right.sectorids.length && left.sectorids.every((id, index) => id === right.sectorids[index]);
}

function samePages(left: ObservationRequest["pages"], right: ObservationRequest["pages"]): boolean {
  return left.length === right.length && left.every((page, index) => page.volid === right[index]?.volid && page.pageid === right[index]?.pageid);
}

export function observationInterval(state: UiState): number {
  return "page" in state.route ? 500 : 2000;
}

function decodeRow(value: unknown, page: ObservationRequest["pages"][number], captured: boolean, complete: unknown, topology: ObservationBatch["topology"], slim = false): ObservationRow {
  const row = objectData(value);
  const classification = { state: text(row.state), reason: text(row.reason) };
  if (!validObservationState(classification)) throw new Error("invalid observation classification");
  const { state, reason } = classification;
  if (row.volid !== page.volid || row.pageid !== page.pageid ||
      (state === "resident" ? !captured || row.evidence == null : row.evidence !== null) ||
      (state === "not-resident" && (!captured || complete !== true)) ||
      (reason === "partial-omission" && (!captured || complete !== false)) ||
      (state === "unknown" && !captured) ||
      ((state === "unavailable" || state === "expired") && captured)) throw new Error("inconsistent observation row");
  const evidence: Record<string, string> = {};
  if (row.evidence !== null) {
    const fields = objectData(row.evidence);
    validateLru(fields, topology);
    if (slim && Object.keys(fields).some((key) => !["lru_zone", "lru_list_kind", "lru_list_index"].includes(key))) throw new Error("invalid volume evidence");
    for (const field of slim ? ["lru_zone", "lru_list_kind", "lru_list_index"] : ["page_kind", "latch_mode", "waiter_present", "fix_count", "dirty", "flushing", "async_flush_requested", "to_vacuum", "lru_zone", "lru_list_kind", "lru_list_index", "page_lsa", "oldest_unflush_lsa"]) {
      const item = fields[field];
      if (item === undefined) evidence[field] = "unknown";
      else if (item === null) evidence[field] = "none / not applicable";
      else if (typeof item === "boolean" || typeof item === "number") evidence[field] = String(item);
      else if (typeof item === "string") evidence[field] = text(item);
      else { const lsa = objectData(item); evidence[field] = `${text(lsa.pageid)}:${count(lsa.offset, 32767)}`; }
    }
  }
  return { ...page, ...classification, evidence };
}

// Validate the normalized tuple as a unit before any color or list count can
// acquire authority. Missing optional members stay unknown, never zero/none.
function validateLru(fields: Record<string, unknown>, topology: ObservationBatch["topology"]): void {
  const zone = fields.lru_zone;
  const kind = fields.lru_list_kind;
  const index = fields.lru_list_index;
  if (zone !== undefined && !["lru1", "lru2", "lru3", "void", "invalid"].includes(String(zone)) ||
      kind !== undefined && !["shared", "private", "none", "invalid"].includes(String(kind))) throw new Error("invalid LRU semantics");
  if (index !== undefined && index !== null) count(index, 2147483647);
  if (index !== undefined && (kind === "shared" || kind === "private") &&
      (index === null || topology === undefined || Number(index) >= topology[kind])) throw new Error("invalid LRU index");
  if ((kind === "none" || kind === "invalid") && index !== undefined && index !== null ||
      ["lru1", "lru2", "lru3"].includes(String(zone)) && kind === "none" ||
      (zone === "void" || zone === "invalid") && ["shared", "private", "invalid"].includes(String(kind))) throw new Error("incoherent LRU tuple");
}
