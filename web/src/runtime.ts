import { ObservationProtocolError, observationAgeAt, type ObservationUi } from "./observations";
import { ApiError } from "./api";
import { readRoute, type InspectorApi } from "./effects";
import type { Action, Effect, UiError, WriteHistoryEffect } from "./model";
import { parseRoute, routePath } from "./routes";

export interface HistoryPort {
  pathname(): string;
  state(): unknown;
  push(state: unknown, path: string): void;
  replace(state: unknown, path: string): void;
  back(): void;
}

interface VolmapHistoryState {
  readonly volmap: true;
  readonly previous: string | null;
  readonly parent: string | null;
}

function currentState(value: unknown): VolmapHistoryState | null {
  if (typeof value !== "object" || value === null || !("volmap" in value)) return null;
  const candidate = value as Partial<VolmapHistoryState>;
  return candidate.volmap === true
    ? {
        volmap: true,
        previous: typeof candidate.previous === "string" ? candidate.previous : null,
        parent: typeof candidate.parent === "string" ? candidate.parent : null,
      }
    : null;
}

export function applyHistory(port: HistoryPort, effect: WriteHistoryEffect): void {
  const path = routePath(effect.route);
  const parent = effect.parent === null ? null : routePath(effect.parent);
  const existing = currentState(port.state());
  if (port.pathname() === path) {
    if (existing === null) {
      port.replace({ volmap: true, previous: null, parent }, path);
    }
    return;
  }
  if (effect.mode === "replace") {
    port.replace({ volmap: true, previous: existing?.previous ?? null, parent }, path);
  } else {
    port.push({ volmap: true, previous: port.pathname(), parent }, path);
  }
}

export interface RuntimePorts {
  readonly api: InspectorApi;
  readonly history: HistoryPort;
  readonly allowRuntime?: (effect: Extract<Effect, { kind: "read-observation" | "read-runtime-capability" }>) => boolean;
  readonly schedule: (milliseconds: number, action: () => void) => void;
  readonly requestSignal?: (group: "route" | "collection" | "enrichment" | "watch" | "license" | "runtime-capability" | "observation") =>
    | AbortSignal
    | undefined;
}

export function createBrowserHistory(browser: Window): HistoryPort {
  return {
    pathname: () => browser.location.pathname,
    state: () => browser.history.state,
    push: (state, path) => browser.history.pushState(state, "", path),
    replace: (state, path) => browser.history.replaceState(state, "", path),
    back: () => browser.history.back(),
  };
}

export function createRequestSignals(): {
  readonly signal: NonNullable<RuntimePorts["requestSignal"]>;
  readonly abortAll: () => void;
  readonly abortRuntime: () => void;
} {
  const controllers = new Map<string, AbortController>();
  return {
    signal: (group) => {
      controllers.get(group)?.abort();
      const controller = new AbortController();
      controllers.set(group, controller);
      return controller.signal;
    },
    abortRuntime: () => {
      controllers.get("observation")?.abort();
      controllers.get("runtime-capability")?.abort();
    },
    abortAll: () => {
      for (const controller of controllers.values()) controller.abort();
      controllers.clear();
    },
  };
}

export function subscribeBrowserEvents(
  browser: Window,
  documentSource: Document,
  dispatch: (action: Action) => void,
): () => void {
  const navigate = () => {
    const route = parseRoute(browser.location.pathname);
    if (route !== null) dispatch({ kind: "navigate", route, history: "none", autoEnrich: false });
  };
  const visibility = () => {
    tick();
    dispatch({ kind: "visibility-changed", visible: documentSource.visibilityState === "visible" });
  };
  const tick = () => {
    dispatch({ kind: "clock-ticked", nowUnixSeconds: Math.floor(Date.now() / 1000) });
    dispatch({ kind: "observation-ticked", now: performance.now(), wallNow: Date.now() });
  };
  browser.addEventListener("popstate", navigate);
  documentSource.addEventListener("visibilitychange", visibility);
  tick();
  visibility();
  const timer = browser.setInterval(tick, 1000);
  return () => {
    browser.removeEventListener("popstate", navigate);
    documentSource.removeEventListener("visibilitychange", visibility);
    browser.clearInterval(timer);
  };
}

export function subscribeObservationExpiry(
  timers: { setTimeout(run: () => void, milliseconds: number): number; clearTimeout(id: number): void },
  observation: ObservationUi,
  dispatch: (action: Action) => void,
  clock = () => ({ now: performance.now(), wallNow: Date.now() }),
): () => void {
  if (observation.batch === null) return () => undefined;
  const reading = clock();
  const age = observationAgeAt(observation, reading.now, reading.wallNow);
  const timer = timers.setTimeout(() => dispatch({ kind: "observation-ticked", ...clock() }),
    age === null ? 0 : Math.max(0, 30_000 - age));
  return () => timers.clearTimeout(timer);
}

function uiError(error: unknown): UiError {
  if (error instanceof ApiError) {
    return { message: error.message, status: error.status, code: error.code };
  }
  return {
    message: error instanceof Error ? error.message : "unknown browser failure",
    code: "browser-error",
  };
}

function aborted(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError";
}

export async function executeEffect(
  effect: Effect,
  ports: RuntimePorts,
  dispatch: (action: Action) => void,
): Promise<void> {
  try {
    if ((effect.kind === "read-observation" || effect.kind === "read-runtime-capability") && ports.allowRuntime?.(effect) === false) return;
    switch (effect.kind) {
      case "delay-observation":
        ports.schedule(effect.milliseconds * (effect.jitter ? 0.8 + Math.random() * 0.4 : 1), () =>
          dispatch({ kind: "observation-due", epoch: effect.epoch }));
        return;
      case "read-observation": {
        const start = performance.now();
        const wallStart = Date.now();
        const batch = await ports.api.observePageBuffer(effect.request, ports.requestSignal?.("observation"));
        const received = performance.now();
        const wallReceived = Date.now();
        dispatch({ kind: "observation-loaded", scope: effect.scope, request: effect.request, batch, received, wallReceived,
          roundTrip: wallReceived < wallStart || Math.abs((received - start) - (wallReceived - wallStart)) > 1000 ? Number.NaN : Math.max(received - start, wallReceived - wallStart) });
        return;
      }
      case "read-runtime-capability": {
        const metadata = await ports.api.runtimeCapabilities(ports.requestSignal?.("runtime-capability"));
        dispatch({ kind: "runtime-capability-loaded", state: metadata.state, metadata, epoch: effect.epoch });
        return;
      }
      case "write-history":
        applyHistory(ports.history, effect);
        return;
      case "history-back": {
        const parentPath = routePath(effect.parent);
        const existing = currentState(ports.history.state());
        if (existing?.previous === parentPath) {
          ports.history.back();
        } else {
          dispatch({ kind: "navigate", route: effect.parent, history: "replace", autoEnrich: false });
        }
        return;
      }
      case "delay-watch":
        ports.schedule(effect.milliseconds, () =>
          dispatch({ kind: "retry-watch", knownGeneration: effect.knownGeneration }),
        );
        return;
      case "read-route": {
        const result = await readRoute(
          ports.api,
          effect.route,
          ports.requestSignal?.("route"),
        );
        dispatch({ kind: "route-loaded", scope: effect.scope, result });
        return;
      }
      case "read-sector-batch": {
        const resource = await ports.api.sectors(
          effect.vol,
          effect.cursor,
          ports.requestSignal?.("collection"),
        );
        dispatch({
          kind: "sector-batch-loaded",
          scope: effect.scope,
          cursor: effect.cursor,
          resource,
        });
        return;
      }
      case "enrich-route": {
        const resource = await ports.api.enrich(
          effect.selector,
          ports.requestSignal?.("enrichment"),
        );
        dispatch({
          kind: "enrichment-loaded",
          scope: effect.scope,
          resource,
          targetRoute: effect.targetRoute,
          history: effect.history,
        });
        return;
      }
      case "watch-generation": {
        const resource = await ports.api.watch(
          effect.knownGeneration,
          ports.requestSignal?.("watch"),
        );
        dispatch({ kind: "watch-loaded", knownGeneration: effect.knownGeneration, resource });
        return;
      }
      case "read-licenses": {
        const result = await ports.api.licenses(ports.requestSignal?.("license"));
        dispatch({ kind: "license-loaded", notice: result.notice });
        return;
      }
    }
  } catch (error) {
    if (effect.kind === "read-observation") {
      dispatch({ kind: "observation-loaded", scope: effect.scope, request: effect.request, batch: null, ...(error instanceof ObservationProtocolError ? { failure: "protocol-incompatible" as const } : {}), received: performance.now(), wallReceived: Date.now(), roundTrip: 0 });
      return;
    }
    if (effect.kind === "read-runtime-capability") {
      // Never surface network/decoder text through inspection errors or UI.
      const incompatible = error instanceof ObservationProtocolError;
      const state = incompatible ? "incompatible" : "unavailable";
      dispatch({ kind: "runtime-capability-loaded", state, metadata: { state, reason: incompatible ? "version-unsupported" : "no-usable-observation", incarnation: null, captureIdentity: null, revision: 0n }, epoch: effect.epoch });
      return;
    }
    if (aborted(error)) return;
    if (effect.kind === "watch-generation") {
      dispatch({ kind: "watch-failed", knownGeneration: effect.knownGeneration });
      return;
    }
    if (effect.kind === "read-route") {
      const mapped = uiError(error);
      if (mapped.status === 404 && (effect.route.kind === "slot" || effect.route.kind === "oos")) {
        dispatch({ kind: "route-needs-enrichment", scope: effect.scope, route: effect.route });
      } else {
        dispatch({ kind: "route-failed", scope: effect.scope, error: mapped });
      }
      return;
    }
    if (effect.kind === "read-sector-batch") {
      dispatch({ kind: "sector-batch-failed", scope: effect.scope, cursor: effect.cursor, error: uiError(error) });
      return;
    }
    if (effect.kind === "enrich-route") {
      dispatch({ kind: "enrichment-failed", scope: effect.scope, fallbackRoute: effect.fallbackRoute, error: uiError(error) });
      return;
    }
    if (effect.kind === "read-licenses") {
      dispatch({ kind: "license-loaded", notice: `Could not load notices: ${uiError(error).message}` });
    }
  }
}
