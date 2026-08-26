import type { JsonObject, Resource, Snapshot } from "./api";
import type {
  CollectionData,
  ResolvedRoute,
  ResolvedView,
  Sector,
  Volume,
  WatchData,
} from "./domain";
import { parentRoute, routePath, type Route } from "./routes";

export type HistoryMode = "none" | "push" | "replace" | "restore";

export type Effect =
  | ReadRouteEffect
  | ReadSectorBatchEffect
  | EnrichRouteEffect
  | DelayWatchEffect
  | ReadLicensesEffect
  | WatchGenerationEffect
  | HistoryBackEffect
  | WriteHistoryEffect;

export interface ReadRouteEffect {
  readonly id: number;
  readonly kind: "read-route";
  readonly scope: string;
  readonly route: Route;
  readonly autoEnrich: boolean;
}

export interface WriteHistoryEffect {
  readonly id: number;
  readonly kind: "write-history";
  readonly mode: "push" | "replace";
  readonly route: Route;
  readonly parent: Route | null;
}

export interface HistoryBackEffect {
  readonly id: number;
  readonly kind: "history-back";
  readonly parent: Route;
}

export interface ReadSectorBatchEffect {
  readonly id: number;
  readonly kind: "read-sector-batch";
  readonly scope: string;
  readonly vol: number;
  readonly cursor: string;
}

export interface WatchGenerationEffect {
  readonly id: number;
  readonly kind: "watch-generation";
  readonly knownGeneration: string | null;
}

export interface EnrichRouteEffect {
  readonly id: number;
  readonly kind: "enrich-route";
  readonly scope: string;
  readonly selector: string;
  readonly targetRoute: Route;
  readonly history: HistoryMode;
  readonly fallbackRoute: Route | null;
}

export interface ReadLicensesEffect {
  readonly id: number;
  readonly kind: "read-licenses";
}

export interface DelayWatchEffect {
  readonly id: number;
  readonly kind: "delay-watch";
  readonly knownGeneration: string | null;
  readonly milliseconds: number;
}

export interface FollowUiState {
  readonly enabled: boolean;
  readonly paused: boolean;
  readonly watchedGeneration: string | null;
  readonly offered: Resource<WatchData> | null;
  readonly pendingGeneration: string | null;
}

export interface LicenseUiState {
  readonly open: boolean;
  readonly loading: boolean;
  readonly notice: string;
}

export interface UiState {
  readonly route: Route;
  readonly scope: string;
  readonly nextEpoch: number;
  readonly nextEffectId: number;
  readonly effects: readonly Effect[];
  readonly history: HistoryMode;
  readonly error: UiError | null;
  readonly snapshot: Snapshot | null;
  readonly outcome: string;
  readonly volumes: readonly Volume[];
  readonly view: ResolvedView | null;
  readonly sectorRequestCursor: string | null;
  readonly follow: FollowUiState;
  readonly autoEnrich: boolean;
  readonly license: LicenseUiState;
  readonly visible: boolean;
  readonly nowUnixSeconds: number;
  readonly collectionMessage: string;
}

export interface UiError {
  readonly message: string;
  readonly status?: number;
  readonly code: string;
}

export type Action =
  | Readonly<{
      kind: "navigate";
      route: Route;
      history: HistoryMode;
      autoEnrich: boolean;
    }>
  | Readonly<{
      kind: "route-failed";
      scope: string;
      error: UiError;
    }>
  | Readonly<{
      kind: "effects-started";
      ids: readonly number[];
    }>
  | Readonly<{
      kind: "route-loaded";
      scope: string;
      result: ResolvedRoute;
    }>
  | Readonly<{
      kind: "request-more-sectors";
    }>
  | Readonly<{
      kind: "sector-batch-loaded";
      scope: string;
      cursor: string;
      resource: Resource<CollectionData<Sector>>;
    }>
  | Readonly<{
      kind: "sector-batch-failed";
      scope: string;
      cursor: string;
      error: UiError;
    }>
  | Readonly<{
      kind: "toggle-pause";
    }>
  | Readonly<{
      kind: "watch-loaded";
      knownGeneration: string | null;
      resource: Resource<WatchData>;
    }>
  | Readonly<{
      kind: "enrichment-loaded";
      scope: string;
      resource: Resource<JsonObject>;
      targetRoute: Route;
      history: HistoryMode;
    }>
  | Readonly<{
      kind: "enrichment-failed";
      scope: string;
      fallbackRoute: Route | null;
      error: UiError;
    }>
  | Readonly<{
      kind: "request-enrichment";
      selector: string;
      targetRoute: Route;
      history: HistoryMode;
      fallbackRoute?: Route | null;
    }>
  | Readonly<{
      kind: "route-needs-enrichment";
      scope: string;
      route: Route;
    }>
  | Readonly<{
      kind: "show-licenses";
    }>
  | Readonly<{
      kind: "close-licenses";
    }>
  | Readonly<{
      kind: "license-loaded";
      notice: string;
    }>
  | Readonly<{
      kind: "watch-failed";
      knownGeneration: string | null;
    }>
  | Readonly<{
      kind: "retry-watch";
      knownGeneration: string | null;
    }>
  | Readonly<{
      kind: "hierarchy-back";
    }>
  | Readonly<{
      kind: "visibility-changed";
      visible: boolean;
    }>
  | Readonly<{
      kind: "clock-ticked";
      nowUnixSeconds: number;
    }>;

function routeScope(epoch: number, route: Route): string {
  return `${epoch}:${routePath(route)}`;
}

export function initialState(route: Route): UiState {
  const scope = routeScope(1, route);
  return {
    route,
    scope,
    nextEpoch: 2,
    nextEffectId: 2,
    effects: [{ id: 1, kind: "read-route", scope, route, autoEnrich: false }],
    history: "restore",
    error: null,
    snapshot: null,
    outcome: "loading",
    volumes: [],
    view: null,
    sectorRequestCursor: null,
    follow: {
      enabled: false,
      paused: false,
      watchedGeneration: null,
      offered: null,
      pendingGeneration: null,
    },
    autoEnrich: false,
    license: { open: false, loading: false, notice: "" },
    visible: true,
    nowUnixSeconds: 0,
    collectionMessage: "",
  };
}

function generationNumber(value: string | null): number | null {
  if (value === null || !/^\d+$/.test(value)) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function reloadCurrentRoute(state: UiState): UiState {
  return loadRoute(state, state.route, "none", false);
}

function loadRoute(
  state: UiState,
  route: Route,
  history: HistoryMode,
  autoEnrich: boolean,
): UiState {
  const scope = routeScope(state.nextEpoch, route);
  return {
    ...state,
    route,
    scope,
    nextEpoch: state.nextEpoch + 1,
    nextEffectId: state.nextEffectId + 1,
    history,
    autoEnrich,
    error: null,
    sectorRequestCursor: null,
    collectionMessage: "",
    effects: [
      ...state.effects,
      {
        id: state.nextEffectId,
        kind: "read-route",
        scope,
        route,
        autoEnrich,
      },
    ],
  };
}

export function reduce(state: UiState, action: Action): UiState {
  if (action.kind === "effects-started") {
    const started = new Set(action.ids);
    return { ...state, effects: state.effects.filter((effect) => !started.has(effect.id)) };
  }
  if (action.kind === "route-failed") {
    return action.scope === state.scope ? { ...state, error: action.error } : state;
  }
  if (action.kind === "route-loaded") {
    if (action.scope !== state.scope) return state;
    const currentSector =
      action.result.view.kind === "sector"
        ? action.result.view.sector.sector_id
        : action.result.view.kind === "page" ||
            action.result.view.kind === "slot" ||
            action.result.view.kind === "oos"
          ? action.result.view.sector.sector_id
          : null;
    const historyMode = state.history === "push" ? "push" : "replace";
    const historyEffect: readonly Effect[] =
      state.history === "none"
        ? []
        : [
            {
              id: state.nextEffectId,
              kind: "write-history",
              mode: historyMode,
              route: action.result.route,
              parent: parentRoute(action.result.route, currentSector),
            },
          ];
    const enabled = action.result.follow.state === "following";
    const watchedGeneration = action.result.snapshot.generation;
    const shouldWatch = enabled && state.follow.pendingGeneration === null;
    const watchEffect: readonly Effect[] = shouldWatch
      ? [
          {
            id: state.nextEffectId + historyEffect.length,
            kind: "watch-generation",
            knownGeneration: watchedGeneration,
          },
        ]
      : [];
    const shouldEnrich =
      state.autoEnrich &&
      action.result.view.kind === "page" &&
      action.result.view.page.deep.state === "not-enriched" &&
      action.result.view.page.page.detail_support.state === "known";
    const enrichmentEffect: readonly Effect[] = shouldEnrich
      ? [
          {
            id: state.nextEffectId + historyEffect.length + watchEffect.length,
            kind: "enrich-route",
            scope: state.scope,
            selector: `page:${action.result.view.page.page.vol_id}:${action.result.view.page.page.page_id}`,
            targetRoute: action.result.route,
            history: "none",
            fallbackRoute: null,
          },
        ]
      : [];
    const offeredGeneration = state.follow.offered?.snapshot.generation ?? null;
    const viewedGeneration = generationNumber(watchedGeneration);
    const offered =
      viewedGeneration !== null &&
      generationNumber(offeredGeneration) !== null &&
      (generationNumber(offeredGeneration) ?? 0) <= viewedGeneration
        ? null
        : state.follow.offered;
    return {
      ...state,
      route: action.result.route,
      snapshot: action.result.snapshot,
      outcome: action.result.outcome,
      volumes: action.result.volumes,
      view:
        shouldEnrich && action.result.view.kind === "page"
          ? { ...action.result.view, enriching: true }
          : action.result.view,
      sectorRequestCursor: null,
      collectionMessage: "",
      error: null,
      nextEffectId:
        state.nextEffectId +
        historyEffect.length +
        watchEffect.length +
        enrichmentEffect.length,
      effects: [...state.effects, ...historyEffect, ...watchEffect, ...enrichmentEffect],
      follow: {
        enabled,
        paused: state.follow.paused,
        watchedGeneration,
        offered,
        pendingGeneration: shouldWatch
          ? watchedGeneration
          : state.follow.pendingGeneration,
      },
    };
  }
  if (action.kind === "request-more-sectors") {
    if (
      state.view?.kind !== "volume" ||
      state.view.nextCursor.state !== "present" ||
      state.sectorRequestCursor !== null
    ) {
      return state;
    }
    const cursor = state.view.nextCursor.value;
    return {
      ...state,
      sectorRequestCursor: cursor,
      nextEffectId: state.nextEffectId + 1,
      effects: [
        ...state.effects,
        {
          id: state.nextEffectId,
          kind: "read-sector-batch",
          scope: state.scope,
          vol: state.view.volume.vol_id,
          cursor,
        },
      ],
    };
  }
  if (action.kind === "sector-batch-loaded") {
    if (
      action.scope !== state.scope ||
      state.view?.kind !== "volume" ||
      state.sectorRequestCursor !== action.cursor
    ) {
      return state;
    }
    const existing = new Set(state.view.sectors.map((sector) => sector.sector_id));
    return {
      ...state,
      snapshot: action.resource.snapshot,
      outcome: action.resource.outcome,
      sectorRequestCursor: null,
      view: {
        ...state.view,
        sectors: [
          ...state.view.sectors,
          ...action.resource.data.items.filter((sector) => !existing.has(sector.sector_id)),
        ],
        nextCursor: action.resource.data.next_cursor,
      },
    };
  }
  if (action.kind === "sector-batch-failed") {
    if (action.scope !== state.scope || state.sectorRequestCursor !== action.cursor) return state;
    if (action.error.code === "cursor-generation-changed" && !state.follow.paused) {
      return reloadCurrentRoute({ ...state, sectorRequestCursor: null });
    }
    return {
      ...state,
      sectorRequestCursor: null,
      collectionMessage:
        action.error.code === "cursor-generation-changed"
          ? "This paused generation is no longer retained · Resume to refresh the mosaic"
          : action.error.message,
    };
  }
  if (action.kind === "watch-loaded") {
    if (state.follow.pendingGeneration !== action.knownGeneration) return state;
    const generation = action.resource.snapshot.generation;
    const advanced =
      action.resource.data.advanced &&
      generation !== null &&
      generation !== state.snapshot?.generation;
    const enabled = action.resource.data.follow.state === "following";
    let next: UiState = {
      ...state,
      follow: {
        enabled,
        paused: state.follow.paused,
        watchedGeneration: generation,
        offered: advanced ? action.resource : state.follow.offered,
        pendingGeneration: enabled ? generation : null,
      },
      nextEffectId: state.nextEffectId + (enabled ? 1 : 0),
      effects: enabled
        ? [
            ...state.effects,
            {
              id: state.nextEffectId,
              kind: "watch-generation",
              knownGeneration: generation,
            },
          ]
        : state.effects,
    };
    if (advanced && !next.follow.paused) next = reloadCurrentRoute(next);
    return next;
  }
  if (action.kind === "watch-failed") {
    if (state.follow.pendingGeneration !== action.knownGeneration) return state;
    return {
      ...state,
      follow: { ...state.follow, pendingGeneration: null },
      nextEffectId: state.nextEffectId + 1,
      effects: [
        ...state.effects,
        {
          id: state.nextEffectId,
          kind: "delay-watch",
          knownGeneration: action.knownGeneration,
          milliseconds: 1000,
        },
      ],
    };
  }
  if (action.kind === "retry-watch") {
    if (
      !state.follow.enabled ||
      state.follow.pendingGeneration !== null ||
      state.follow.watchedGeneration !== action.knownGeneration
    ) {
      return state;
    }
    return {
      ...state,
      follow: { ...state.follow, pendingGeneration: action.knownGeneration },
      nextEffectId: state.nextEffectId + 1,
      effects: [
        ...state.effects,
        {
          id: state.nextEffectId,
          kind: "watch-generation",
          knownGeneration: action.knownGeneration,
        },
      ],
    };
  }
  if (action.kind === "toggle-pause") {
    if (!state.follow.enabled) return state;
    const next = {
      ...state,
      follow: { ...state.follow, paused: !state.follow.paused },
    };
    return state.follow.paused && state.follow.offered !== null ? reloadCurrentRoute(next) : next;
  }
  if (action.kind === "enrichment-loaded") {
    if (action.scope !== state.scope) return state;
    return loadRoute({
      ...state,
      snapshot: action.resource.snapshot,
      outcome: action.resource.outcome,
      autoEnrich: false,
      error: null,
    }, action.targetRoute, action.history, false);
  }
  if (action.kind === "enrichment-failed") {
    if (action.scope !== state.scope) return state;
    return action.fallbackRoute === null
      ? { ...state, error: action.error }
      : loadRoute(state, action.fallbackRoute, "replace", false);
  }
  if (action.kind === "request-enrichment") {
    return {
      ...state,
      nextEffectId: state.nextEffectId + 1,
      effects: [...state.effects, {
        id: state.nextEffectId,
        kind: "enrich-route",
        scope: state.scope,
        selector: action.selector,
        targetRoute: action.targetRoute,
        history: action.history,
        fallbackRoute: action.fallbackRoute ?? null,
      }],
    };
  }
  if (action.kind === "route-needs-enrichment") {
    if (action.scope !== state.scope) return state;
    if (action.route.kind === "slot") {
      return reduce(state, {
        kind: "request-enrichment",
        selector: `slot:${action.route.vol}:${action.route.page}:${action.route.slot}`,
        targetRoute: action.route,
        history: "replace",
        fallbackRoute: { kind: "page", vol: action.route.vol, page: action.route.page },
      });
    }
    if (action.route.kind === "oos") {
      return reduce(state, {
        kind: "request-enrichment",
        selector: `oos:${action.route.vol}:${action.route.page}:${action.route.slot}`,
        targetRoute: action.route,
        history: "replace",
        fallbackRoute: { kind: "slot", vol: action.route.vol, page: action.route.page, slot: action.route.slot },
      });
    }
    return state;
  }
  if (action.kind === "show-licenses") {
    if (state.license.loading) return state;
    return {
      ...state,
      license: { ...state.license, open: true, loading: true },
      nextEffectId: state.nextEffectId + 1,
      effects: [...state.effects, { id: state.nextEffectId, kind: "read-licenses" }],
    };
  }
  if (action.kind === "close-licenses") {
    return { ...state, license: { ...state.license, open: false } };
  }
  if (action.kind === "license-loaded") {
    return {
      ...state,
      license: { open: true, loading: false, notice: action.notice },
    };
  }
  if (action.kind === "hierarchy-back") {
    const currentSector =
      state.view?.kind === "sector"
        ? state.view.sector.sector_id
        : state.view?.kind === "page" || state.view?.kind === "slot" || state.view?.kind === "oos"
          ? state.view.sector.sector_id
          : null;
    const parent = parentRoute(state.route, currentSector);
    if (parent === null) return state;
    return {
      ...state,
      nextEffectId: state.nextEffectId + 1,
      effects: [...state.effects, { id: state.nextEffectId, kind: "history-back", parent }],
    };
  }
  if (action.kind === "visibility-changed") return { ...state, visible: action.visible };
  if (action.kind === "clock-ticked") return { ...state, nowUnixSeconds: action.nowUnixSeconds };
  return loadRoute(state, action.route, action.history, action.autoEnrich);
}
