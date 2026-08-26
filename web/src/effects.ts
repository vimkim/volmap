import type { JsonObject, Resource } from "./api";
import type {
  CollectionData,
  LicenseData,
  OosResource,
  PageResource,
  ResolvedRoute,
  ResolvedView,
  Sector,
  SessionData,
  SlotResource,
  Volume,
  WatchData,
} from "./domain";
import type { EntityRoute, Route } from "./routes";

export interface InspectorApi {
  session(signal?: AbortSignal): Promise<Resource<SessionData>>;
  volumes(signal?: AbortSignal): Promise<Resource<CollectionData<Volume>>>;
  sectors(
    vol: number,
    cursor?: string,
    signal?: AbortSignal,
  ): Promise<Resource<CollectionData<Sector>>>;
  sector(vol: number, sector: number, signal?: AbortSignal): Promise<Resource<Sector>>;
  page(vol: number, page: number, signal?: AbortSignal): Promise<Resource<PageResource>>;
  slot(
    vol: number,
    page: number,
    slot: number,
    signal?: AbortSignal,
  ): Promise<Resource<SlotResource>>;
  oos(
    vol: number,
    page: number,
    slot: number,
    signal?: AbortSignal,
  ): Promise<Resource<OosResource>>;
  enrich(selector: string, signal?: AbortSignal): Promise<Resource<JsonObject>>;
  watch(generation: string | null, signal?: AbortSignal): Promise<Resource<WatchData>>;
  licenses(signal?: AbortSignal): Promise<LicenseData>;
}

function selectedVolume(volumes: readonly Volume[], vol: number): Volume {
  const volume = volumes.find((candidate) => candidate.vol_id === vol);
  if (volume === undefined) throw new Error("the URL volume does not exist in this revision");
  return volume;
}

function resolved(
  route: EntityRoute,
  source: Resource<unknown>,
  session: Resource<SessionData>,
  volumes: readonly Volume[],
  view: ResolvedView,
): ResolvedRoute {
  return {
    route,
    snapshot: source.snapshot,
    outcome: source.outcome,
    follow: session.data.follow,
    volumes,
    view,
  };
}

export async function readRoute(
  api: InspectorApi,
  requested: Route,
  signal?: AbortSignal,
): Promise<ResolvedRoute> {
  const [session, volumeResource] = await Promise.all([
    api.session(signal),
    api.volumes(signal),
  ]);
  const volumes = volumeResource.data.items;
  if (volumes.length === 0) throw new Error("this snapshot contains no data volumes");
  const route: EntityRoute =
    requested.kind === "root"
      ? { kind: "volume", vol: volumes[0]?.vol_id ?? 0 }
      : requested;
  const volume = selectedVolume(volumes, route.vol);

  if (route.kind === "volume") {
    const sectors = await api.sectors(route.vol, undefined, signal);
    return resolved(route, sectors, session, volumes, {
      kind: "volume",
      volume,
      sectors: sectors.data.items,
      nextCursor: sectors.data.next_cursor,
    });
  }
  if (route.kind === "sector") {
    const sector = await api.sector(route.vol, route.sector, signal);
    return resolved(route, sector, session, volumes, {
      kind: "sector",
      volume,
      sector: sector.data,
    });
  }

  const page = await api.page(route.vol, route.page, signal);
  const sector = await api.sector(route.vol, page.data.page.sector_id, signal);
  if (route.kind === "page") {
    return resolved(route, page, session, volumes, {
      kind: "page",
      volume,
      sector: sector.data,
      page: page.data,
      enriching: false,
    });
  }
  if (route.kind === "slot") {
    const slot = await api.slot(route.vol, route.page, route.slot, signal);
    return resolved(route, slot, session, volumes, {
      kind: "slot",
      volume,
      sector: sector.data,
      page: page.data,
      slot: slot.data,
    });
  }
  const chain = await api.oos(route.vol, route.page, route.slot, signal);
  return resolved(route, chain, session, volumes, {
    kind: "oos",
    volume,
    sector: sector.data,
    page: page.data,
    chain: chain.data,
  });
}
