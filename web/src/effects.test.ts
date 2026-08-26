import { expect, test } from "vitest";

import type { Resource } from "./api";
import type {
  CollectionData,
  OosResource,
  Page,
  PageResource,
  Sector,
  SessionData,
  SlotResource,
  Volume,
} from "./domain";
import { readRoute, type InspectorApi } from "./effects";

const snapshot = {
  id: "0123456789abcdef",
  revision: "7",
  validity: "valid",
  format_profile: "fixture",
  generation: "4",
  observed_at_unix_seconds: "100",
  input_modified_unix_seconds: "90",
} as const;

function resource<T>(data: T): Resource<T> {
  return { snapshot, outcome: "success-limited", coverage: [], diagnostics: [], data };
}

const volume: Volume = { vol_id: 0, total_sectors: 64 };
const sector: Sector = { vol_id: 0, sector_id: 2, reserved: true, pages: [] };

test("the route reader resolves a direct sector from typed same-origin resources", async () => {
  const unavailable = async (): Promise<never> => {
    throw new Error("unexpected API operation");
  };
  const api: InspectorApi = {
    session: async () => resource<SessionData>({
      access: "unauthenticated-http",
      follow: { state: "disabled" },
    }),
    volumes: async () =>
      resource<CollectionData<Volume>>({ items: [volume], next_cursor: { state: "end" } }),
    sectors: unavailable,
    sector: async () => resource(sector),
    page: unavailable as (vol: number, page: number) => Promise<Resource<PageResource>>,
    slot: unavailable as (
      vol: number,
      page: number,
      slot: number,
    ) => Promise<Resource<SlotResource>>,
    oos: unavailable as (
      vol: number,
      page: number,
      slot: number,
    ) => Promise<Resource<OosResource>>,
    enrich: unavailable,
    watch: unavailable,
    licenses: unavailable,
  };

  const result = await readRoute(api, { kind: "sector", vol: 0, sector: 2 });

  expect(result).toMatchObject({
    route: { kind: "sector", vol: 0, sector: 2 },
    snapshot: { revision: "7", generation: "4" },
    follow: { state: "disabled" },
    volumes: [{ vol_id: 0 }],
    view: { kind: "sector", volume: { vol_id: 0 }, sector: { sector_id: 2 } },
  });
});
