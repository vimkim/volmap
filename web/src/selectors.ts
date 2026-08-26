import type { Snapshot } from "./api";
import type { FollowUiState } from "./model";
import type { Route } from "./routes";

export interface BreadcrumbItem {
  readonly label: string;
  readonly route: Route | null;
}

export function breadcrumbItems(route: Route, sectorId: number | null): readonly BreadcrumbItem[] {
  if (route.kind === "root") return [];
  const items: BreadcrumbItem[] = [
    {
      label: `Volume ${route.vol}`,
      route: route.kind === "volume" ? null : { kind: "volume", vol: route.vol },
    },
  ];
  if (route.kind === "volume") return items;
  const sector = route.kind === "sector" ? route.sector : sectorId;
  if (sector !== null) {
    items.push({
      label: `Sector ${sector}`,
      route:
        route.kind === "sector" ? null : { kind: "sector", vol: route.vol, sector },
    });
  }
  if (route.kind === "sector") return items;
  items.push({
    label: `Page ${route.page}`,
    route: route.kind === "page" ? null : { kind: "page", vol: route.vol, page: route.page },
  });
  if (route.kind === "page") return items;
  items.push({
    label: `Slot ${route.slot}`,
    route:
      route.kind === "slot"
        ? null
        : { kind: "slot", vol: route.vol, page: route.page, slot: route.slot },
  });
  if (route.kind === "oos") items.push({ label: "OOS chain", route: null });
  return items;
}

function numericGeneration(value: string | null): number {
  const parsed = value === null ? 0 : Number(value);
  return Number.isSafeInteger(parsed) ? parsed : 0;
}

function secondsAgo(value: string | null, nowUnixSeconds: number): string {
  const observed = value === null ? Number.NaN : Number(value);
  return Number.isFinite(observed)
    ? `${Math.max(0, Math.floor(nowUnixSeconds - observed))}s ago`
    : "read time unknown";
}

function diskTime(value: string | null): string {
  const modified = value === null ? Number.NaN : Number(value);
  if (!Number.isFinite(modified)) return "disk time unknown";
  const date = new Date(modified * 1000);
  return `disk ${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}

export function followLabel(
  snapshot: Pick<
    Snapshot,
    "generation" | "observed_at_unix_seconds" | "input_modified_unix_seconds"
  >,
  follow: Pick<FollowUiState, "enabled" | "paused" | "watchedGeneration">,
  nowUnixSeconds: number,
): string {
  if (!follow.enabled) return "";
  const viewed = snapshot.generation;
  const newest = String(
    Math.max(numericGeneration(viewed), numericGeneration(follow.watchedGeneration)),
  );
  return follow.paused
    ? `paused at gen ${viewed} · newer: gen ${newest}`
    : `live · gen ${viewed} · ${secondsAgo(snapshot.observed_at_unix_seconds, nowUnixSeconds)} · ${diskTime(snapshot.input_modified_unix_seconds)}`;
}
