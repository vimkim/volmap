export type RootRoute = Readonly<{ kind: "root" }>;
export type VolumeRoute = Readonly<{ kind: "volume"; vol: number }>;
export type SectorRoute = Readonly<{ kind: "sector"; vol: number; sector: number }>;
export type PageRoute = Readonly<{ kind: "page"; vol: number; page: number }>;
export type SlotRoute = Readonly<{
  kind: "slot";
  vol: number;
  page: number;
  slot: number;
}>;
export type OosRoute = Readonly<{
  kind: "oos";
  vol: number;
  page: number;
  slot: number;
}>;
export type Route = RootRoute | VolumeRoute | SectorRoute | PageRoute | SlotRoute | OosRoute;
export type EntityRoute = Exclude<Route, RootRoute>;

function canonicalNumber(value: string | undefined): number | null {
  if (value === undefined || !/^(0|[1-9]\d*)$/.test(value)) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

export function parseRoute(pathname: string): Route | null {
  if (pathname === "/") return { kind: "root" };
  const parts = pathname.split("/");
  if (parts[0] !== "") return null;
  const vol = canonicalNumber(parts[2]);
  if (vol === null) return null;
  if (parts[1] === "volume" && parts.length === 3) return { kind: "volume", vol };
  const entity = canonicalNumber(parts[3]);
  if (entity === null) return null;
  if (parts[1] === "sector" && parts.length === 4) {
    return { kind: "sector", vol, sector: entity };
  }
  if (parts[1] === "page" && parts.length === 4) {
    return { kind: "page", vol, page: entity };
  }
  const slot = canonicalNumber(parts[4]);
  if (slot === null || parts.length !== 5) return null;
  if (parts[1] === "slot") return { kind: "slot", vol, page: entity, slot };
  if (parts[1] === "oos") return { kind: "oos", vol, page: entity, slot };
  return null;
}

export function routePath(route: Route): string {
  switch (route.kind) {
    case "root":
      return "/";
    case "volume":
      return `/volume/${route.vol}`;
    case "sector":
      return `/sector/${route.vol}/${route.sector}`;
    case "page":
      return `/page/${route.vol}/${route.page}`;
    case "slot":
      return `/slot/${route.vol}/${route.page}/${route.slot}`;
    case "oos":
      return `/oos/${route.vol}/${route.page}/${route.slot}`;
  }
}

export function parentRoute(route: Route, currentSectorId: number | null): EntityRoute | null {
  switch (route.kind) {
    case "root":
    case "volume":
      return null;
    case "sector":
      return { kind: "volume", vol: route.vol };
    case "page":
      return currentSectorId === null
        ? null
        : { kind: "sector", vol: route.vol, sector: currentSectorId };
    case "slot":
      return { kind: "page", vol: route.vol, page: route.page };
    case "oos":
      return { kind: "slot", vol: route.vol, page: route.page, slot: route.slot };
  }
}
