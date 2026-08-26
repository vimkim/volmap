export type JsonObject = Record<string, unknown>;

export interface Snapshot {
  readonly id: string;
  readonly revision: string;
  readonly validity: string;
  readonly format_profile: string;
  readonly generation: string | null;
  readonly observed_at_unix_seconds: string | null;
  readonly input_modified_unix_seconds: string | null;
}

export interface Resource<T> {
  readonly snapshot: Snapshot;
  readonly outcome: string;
  readonly coverage: readonly JsonObject[];
  readonly diagnostics: readonly JsonObject[];
  readonly data: T;
}

export type DataDecoder<T> = (value: unknown, context: string) => T;

export function objectData(value: unknown, context = "resource data"): JsonObject {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${context} must be an object`);
  }
  return value as JsonObject;
}

export function stringData(value: unknown, context: string): string {
  if (typeof value !== "string") throw new Error(`${context} must be a string`);
  return value;
}

function numberData(value: unknown, context: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${context} must be a finite number`);
  }
  return value;
}

function booleanData(value: unknown, context: string): boolean {
  if (typeof value !== "boolean") throw new Error(`${context} must be a boolean`);
  return value;
}

function arrayData<T>(value: unknown, context: string, decode: DataDecoder<T>): readonly T[] {
  if (!Array.isArray(value)) throw new Error(`${context} must be an array`);
  return value.map((item, index) => decode(item, `${context}[${index}]`));
}

function nullableString(value: unknown, context: string): string | null {
  return value === null ? null : stringData(value, context);
}

function objectArray(value: unknown, context: string): readonly JsonObject[] {
  if (!Array.isArray(value)) throw new Error(`${context} must be an array`);
  return value.map((item, index) => objectData(item, `${context}[${index}]`));
}

export function decodeResource<T>(value: unknown, decodeData: DataDecoder<T>): Resource<T> {
  const root = objectData(value, "Volmap resource");
  if (
    root.schema !== "volmap.inspection" ||
    root.schema_version !== 1 ||
    root.document_type !== "resource"
  ) {
    throw new Error("invalid Volmap resource schema");
  }
  const snapshotValue = objectData(root.snapshot, "snapshot");
  const snapshot: Snapshot = {
    id: stringData(snapshotValue.id, "snapshot.id"),
    revision: stringData(snapshotValue.revision, "snapshot.revision"),
    validity: stringData(snapshotValue.validity, "snapshot.validity"),
    format_profile: stringData(snapshotValue.format_profile, "snapshot.format_profile"),
    generation: nullableString(snapshotValue.generation, "snapshot.generation"),
    observed_at_unix_seconds: nullableString(
      snapshotValue.observed_at_unix_seconds,
      "snapshot.observed_at_unix_seconds",
    ),
    input_modified_unix_seconds: nullableString(
      snapshotValue.input_modified_unix_seconds,
      "snapshot.input_modified_unix_seconds",
    ),
  };
  return {
    snapshot,
    outcome: stringData(root.outcome, "outcome"),
    coverage: objectArray(root.coverage, "coverage"),
    diagnostics: objectArray(root.diagnostics, "diagnostics"),
    data: decodeData(root.data, "resource data"),
  };
}

function optionalText(value: unknown, context: string): OptionalText {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "known") return { state, value: stringData(item.value, `${context}.value`) };
  if (state === "unknown" || state === "unsupported") return { state };
  throw new Error(`${context}.state is invalid`);
}

function className(value: unknown, context: string): ClassName {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "resolved") return { state, value: stringData(item.value, `${context}.value`) };
  if (state === "unresolved" || state === "not-applicable") {
    return { state, reason: stringData(item.reason, `${context}.reason`) };
  }
  throw new Error(`${context}.state is invalid`);
}

function optionalOid(value: unknown, context: string): OptionalOid {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "absent") return { state };
  if (state !== "present") throw new Error(`${context}.state is invalid`);
  const oid = objectData(item.oid, `${context}.oid`);
  return {
    state,
    oid: {
      vol_id: numberData(oid.vol_id, `${context}.oid.vol_id`),
      page_id: numberData(oid.page_id, `${context}.oid.page_id`),
      slot_id: numberData(oid.slot_id, `${context}.oid.slot_id`),
    },
  };
}

function fileBody(value: unknown, context: string): FileBody {
  const item = objectData(value, context);
  return {
    vol_id: numberData(item.vol_id, `${context}.vol_id`),
    file_id: numberData(item.file_id, `${context}.file_id`),
    file_type: optionalText(item.file_type, `${context}.file_type`),
    class_oid: optionalOid(item.class_oid, `${context}.class_oid`),
    class_name: className(item.class_name, `${context}.class_name`),
  };
}

function fileAssociation(value: unknown, context: string): FileAssociation {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "none" || state === "mixed-claims") return { state };
  if (state === "allocated" || state === "reserved-for") {
    return { state, file: fileBody(item.file, `${context}.file`) };
  }
  throw new Error(`${context}.state is invalid`);
}

function occupancy(value: unknown, context: string): PageOccupancy {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "unknown") return { state };
  if (state === "known") {
    return {
      state,
      occupied_percent: numberData(item.occupied_percent, `${context}.occupied_percent`),
      free_percent: numberData(item.free_percent, `${context}.free_percent`),
    };
  }
  throw new Error(`${context}.state is invalid`);
}

function volume(value: unknown, context: string): Volume {
  const item = objectData(value, context);
  const result: Volume = {
    vol_id: numberData(item.vol_id, `${context}.vol_id`),
    total_sectors: numberData(item.total_sectors, `${context}.total_sectors`),
  };
  return {
    ...result,
    ...(typeof item.purpose === "string" ? { purpose: item.purpose } : {}),
    ...(typeof item.volume_type === "string" ? { volume_type: item.volume_type } : {}),
    ...(typeof item.maximum_sectors === "number"
      ? { maximum_sectors: item.maximum_sectors }
      : {}),
    ...(typeof item.system_last_page === "number"
      ? { system_last_page: item.system_last_page }
      : {}),
    ...(typeof item.reserved_sectors === "number"
      ? { reserved_sectors: item.reserved_sectors }
      : {}),
  };
}

function page(value: unknown, context: string): Page {
  const item = objectData(value, context);
  return {
    vol_id: numberData(item.vol_id, `${context}.vol_id`),
    page_id: numberData(item.page_id, `${context}.page_id`),
    sector_id: numberData(item.sector_id, `${context}.sector_id`),
    allocation: stringData(item.allocation, `${context}.allocation`),
    page_type: optionalText(item.page_type, `${context}.page_type`),
    availability: stringData(item.availability, `${context}.availability`),
    tde_state: stringData(item.tde_state, `${context}.tde_state`),
    detail_support: optionalText(item.detail_support, `${context}.detail_support`),
    occupancy: occupancy(item.occupancy, `${context}.occupancy`),
    diagnostic: optionalText(item.diagnostic, `${context}.diagnostic`),
    file_association: fileAssociation(item.file_association, `${context}.file_association`),
  };
}

function attribution(value: unknown, context: string): SectorAttribution {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "unclaimed") return { state };
  if (state === "single") {
    return {
      state,
      file: fileBody(item.file, `${context}.file`),
      full: booleanData(item.full, `${context}.full`),
      allocated_pages: numberData(item.allocated_pages, `${context}.allocated_pages`),
      reserved_unallocated_pages: numberData(
        item.reserved_unallocated_pages,
        `${context}.reserved_unallocated_pages`,
      ),
    };
  }
  if (state === "mixed") {
    return { state, claims: objectArray(item.claims, `${context}.claims`) };
  }
  throw new Error(`${context}.state is invalid`);
}

function sector(value: unknown, context: string): Sector {
  const item = objectData(value, context);
  return {
    vol_id: numberData(item.vol_id, `${context}.vol_id`),
    sector_id: numberData(item.sector_id, `${context}.sector_id`),
    reserved: booleanData(item.reserved, `${context}.reserved`),
    attribution: attribution(item.attribution, `${context}.attribution`),
    pages: arrayData(item.pages, `${context}.pages`, page),
  };
}

function slot(value: unknown, context: string): Slot {
  const item = objectData(value, context);
  return {
    slot_id: numberData(item.slot_id, `${context}.slot_id`),
    offset: numberData(item.offset, `${context}.offset`),
    length: numberData(item.length, `${context}.length`),
    record_type: stringData(item.record_type, `${context}.record_type`),
    record_type_ordinal: numberData(item.record_type_ordinal, `${context}.record_type_ordinal`),
  };
}

function deepPage(value: unknown, context: string): DeepPage {
  const item = objectData(value, context);
  const result: DeepPage = { state: stringData(item.state, `${context}.state`) };
  return {
    ...result,
    ...(item.structure === undefined
      ? {}
      : { structure: objectData(item.structure, `${context}.structure`) }),
    ...(typeof item.rule === "string" ? { rule: item.rule } : {}),
  };
}

function byteRegion(value: unknown, context: string): Readonly<{ offset: number; length: number }> {
  const item = objectData(value, context);
  return {
    offset: numberData(item.offset, `${context}.offset`),
    length: numberData(item.length, `${context}.length`),
  };
}

function distribution(value: unknown, context: string): PageDistribution {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "not-available") return { state };
  if (state !== "available") throw new Error(`${context}.state is invalid`);
  return {
    state,
    content_size: numberData(item.content_size, `${context}.content_size`),
    header: byteRegion(item.header, `${context}.header`),
    record_extents: arrayData(item.record_extents, `${context}.record_extents`, (entry, name) => {
      const source = objectData(entry, name);
      return {
        ...byteRegion(source, name),
        slot_id: numberData(source.slot_id, `${name}.slot_id`),
        record_type: stringData(source.record_type, `${name}.record_type`),
      };
    }),
    free_regions: arrayData(item.free_regions, `${context}.free_regions`, (entry, name) => {
      const source = objectData(entry, name);
      return { ...byteRegion(source, name), kind: stringData(source.kind, `${name}.kind`) };
    }),
    slot_directory: byteRegion(item.slot_directory, `${context}.slot_directory`),
    slot_entries: arrayData(item.slot_entries, `${context}.slot_entries`, (entry, name) => {
      const source = objectData(entry, name);
      return {
        ...byteRegion(source, name),
        slot_id: numberData(source.slot_id, `${name}.slot_id`),
        state: stringData(source.state, `${name}.state`),
        record_type: stringData(source.record_type, `${name}.record_type`),
      };
    }),
    allocated_record_bytes: numberData(
      item.allocated_record_bytes,
      `${context}.allocated_record_bytes`,
    ),
    unoccupied_bytes: numberData(item.unoccupied_bytes, `${context}.unoccupied_bytes`),
  };
}

function pageResource(value: unknown, context: string): PageResource {
  const item = objectData(value, context);
  return {
    page: page(item.page, `${context}.page`),
    deep: deepPage(item.deep, `${context}.deep`),
    slots: arrayData(item.slots, `${context}.slots`, slot),
    distribution: distribution(item.distribution, `${context}.distribution`),
  };
}

function attributeName(value: unknown, context: string): AttributeName {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "resolved") return { state, value: stringData(item.value, `${context}.value`) };
  if (state === "unresolved") return { state, reason: stringData(item.reason, `${context}.reason`) };
  throw new Error(`${context}.state is invalid`);
}

function oid(value: unknown, context: string): Oid {
  const item = objectData(value, context);
  return {
    vol_id: numberData(item.vol_id, `${context}.vol_id`),
    page_id: numberData(item.page_id, `${context}.page_id`),
    slot_id: numberData(item.slot_id, `${context}.slot_id`),
  };
}

function attributeValue(value: unknown, context: string): AttributeValue {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "decoded") return { state, value: stringData(item.value, `${context}.value`) };
  if (state === "null") return { state };
  if (state === "out-of-row") {
    return {
      state,
      head: oid(item.head, `${context}.head`),
      total_length: stringData(item.total_length, `${context}.total_length`),
    };
  }
  if (state === "withheld") {
    return {
      state,
      reason: stringData(item.reason, `${context}.reason`),
      offset: numberData(item.offset, `${context}.offset`),
      length: numberData(item.length, `${context}.length`),
    };
  }
  throw new Error(`${context}.state is invalid`);
}

function recordInterpretation(value: unknown, context: string): RecordInterpretation {
  const item = objectData(value, context);
  const layout = item.layout === null ? null : objectData(item.layout, `${context}.layout`);
  return {
    layout: layout === null ? null : {
      record_length: stringData(layout.record_length, `${context}.layout.record_length`),
      regions: arrayData(layout.regions, `${context}.layout.regions`, (entry, name) => {
        const region = objectData(entry, name);
        return {
          region: stringData(region.region, `${name}.region`),
          offset: stringData(region.offset, `${name}.offset`),
          length: stringData(region.length, `${name}.length`),
        };
      }),
    },
    relocated_from: optionalOid(item.relocated_from, `${context}.relocated_from`),
    diagnostic: optionalText(item.diagnostic, `${context}.diagnostic`),
    attributes: arrayData(item.attributes, `${context}.attributes`, (entry, name) => {
      const attribute = objectData(entry, name);
      return {
        name: attributeName(attribute.name, `${name}.name`),
        attribute_id: numberData(attribute.attribute_id, `${name}.attribute_id`),
        position: numberData(attribute.position, `${name}.position`),
        type_name: stringData(attribute.type_name, `${name}.type_name`),
        precision: numberData(attribute.precision, `${name}.precision`),
        scale: numberData(attribute.scale, `${name}.scale`),
        offset: stringData(attribute.offset, `${name}.offset`),
        length: stringData(attribute.length, `${name}.length`),
        storage: stringData(attribute.storage, `${name}.storage`),
        value: attributeValue(attribute.value, `${name}.value`),
      };
    }),
  };
}

function relocationEdge(value: unknown, context: string): RelocationEdge {
  const item = objectData(value, context);
  return {
    target: optionalOid(item.target, `${context}.target`),
    valid: booleanData(item.valid, `${context}.valid`),
  };
}

function classRepresentation(value: unknown, context: string): ClassRepresentation {
  const item = objectData(value, context);
  return {
    representation_id: numberData(item.representation_id, `${context}.representation_id`),
    class_name: className(item.class_name, `${context}.class_name`),
    is_current: optionalText(item.is_current, `${context}.is_current`),
  };
}

function slotResource(value: unknown, context: string): SlotResource {
  const item = objectData(value, context);
  const selected = slot(item.selected_slot, `${context}.selected_slot`);
  const relocation =
    item.relocation_edge === null
      ? null
      : relocationEdge(item.relocation_edge, `${context}.relocation_edge`);
  const interpretation =
    item.interpretation === null
      ? null
      : recordInterpretation(item.interpretation, `${context}.interpretation`);
  const representation =
    item.class_representation === null
      ? null
      : classRepresentation(item.class_representation, `${context}.class_representation`);
  return {
    page: page(item.page, `${context}.page`),
    deep: deepPage(item.deep, `${context}.deep`),
    selected_slot: selected,
    relocation_edge: relocation,
    interpretation,
    class_representation: representation,
    interpretation_unavailable:
      item.interpretation_unavailable === null
        ? null
        : stringData(item.interpretation_unavailable, `${context}.interpretation_unavailable`),
  };
}

function oosResource(value: unknown, context: string): OosResource {
  const item = objectData(value, context);
  const chain = objectData(item.chain, `${context}.chain`);
  return {
    chain: {
      complete: booleanData(chain.complete, `${context}.chain.complete`),
      validated_payload_bytes: stringData(
        chain.validated_payload_bytes,
        `${context}.chain.validated_payload_bytes`,
      ),
      chunks: objectArray(chain.chunks, `${context}.chain.chunks`),
      diagnostic: optionalText(chain.diagnostic, `${context}.chain.diagnostic`),
    },
  };
}

function nextCursor(value: unknown, context: string): NextCursor {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "end") return { state };
  if (state === "present") return { state, value: stringData(item.value, `${context}.value`) };
  throw new Error(`${context}.state is invalid`);
}

function collection<T>(decodeItem: DataDecoder<T>): DataDecoder<CollectionData<T>> {
  return (value, context) => {
    const item = objectData(value, context);
    return {
      items: arrayData(item.items, `${context}.items`, decodeItem),
      next_cursor: nextCursor(item.next_cursor, `${context}.next_cursor`),
    };
  };
}

function follow(value: unknown, context: string): Follow {
  const item = objectData(value, context);
  const state = stringData(item.state, `${context}.state`);
  if (state === "disabled") return { state };
  if (state === "following") {
    return {
      state,
      poll_interval_ms: stringData(item.poll_interval_ms, `${context}.poll_interval_ms`),
      retained_generations: stringData(
        item.retained_generations,
        `${context}.retained_generations`,
      ),
    };
  }
  throw new Error(`${context}.state is invalid`);
}

function sessionData(value: unknown, context: string): SessionData {
  const item = objectData(value, context);
  return {
    access: stringData(item.access, `${context}.access`),
    follow: follow(item.follow, `${context}.follow`),
  };
}

function watchData(value: unknown, context: string): WatchData {
  const item = objectData(value, context);
  return {
    advanced: booleanData(item.advanced, `${context}.advanced`),
    follow: follow(item.follow, `${context}.follow`),
  };
}

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(message: string, status: number, code: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

export function createHttpApi(fetcher: typeof fetch = globalThis.fetch.bind(globalThis)): InspectorApi {
  async function json(path: string, init: RequestInit = {}): Promise<unknown> {
    const response = await fetcher(path, {
      ...init,
      cache: "no-store",
      credentials: "same-origin",
    });
    const value: unknown = await response.json().catch(() => null);
    if (!response.ok) {
      const root = value === null ? null : objectData(value, "error response");
      const detail = root?.error === undefined ? null : objectData(root.error, "error");
      throw new ApiError(
        typeof detail?.message === "string"
          ? detail.message
          : `The server rejected this request (HTTP ${response.status}).`,
        response.status,
        typeof detail?.code === "string" ? detail.code : "http-error",
      );
    }
    return value;
  }

  async function resource<T>(
    path: string,
    decoder: DataDecoder<T>,
    init: RequestInit = {},
  ): Promise<Resource<T>> {
    return decodeResource(await json(path, init), decoder);
  }

  return {
    session: (signal) => resource("/api/v1/session", sessionData, { signal }),
    volumes: (signal) => resource("/api/v1/volumes", collection(volume), { signal }),
    sectors: (vol, cursor, signal) =>
      resource(
        `/api/v1/sectors/${vol}${cursor === undefined ? "?limit=24" : `?limit=24&cursor=${encodeURIComponent(cursor)}`}`,
        collection(sector),
        { signal },
      ),
    sector: (vol, sectorId, signal) =>
      resource(`/api/v1/sector/${vol}/${sectorId}`, sector, { signal }),
    page: (vol, pageId, signal) =>
      resource(`/api/v1/page/${vol}/${pageId}`, pageResource, { signal }),
    slot: (vol, pageId, slotId, signal) =>
      resource(`/api/v1/slot/${vol}/${pageId}/${slotId}`, slotResource, { signal }),
    oos: (vol, pageId, slotId, signal) =>
      resource(`/api/v1/oos/${vol}/${pageId}/${slotId}`, oosResource, { signal }),
    enrich: (selector, signal) =>
      resource("/api/v1/enrichments", objectData, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ selector }),
        signal,
      }),
    watch: (generation, signal) =>
      resource(
        `/api/v1/live/watch${generation === null ? "" : `?generation=${encodeURIComponent(generation)}`}`,
        watchData,
        { signal },
      ),
    licenses: async (signal): Promise<LicenseData> => {
      const value = objectData(await json("/api/v1/licenses", { signal }), "license response");
      return { notice: stringData(value.notice, "license response.notice") };
    },
  };
}
import type {
  AttributeName,
  AttributeValue,
  ClassName,
  ClassRepresentation,
  CollectionData,
  DeepPage,
  FileAssociation,
  FileBody,
  Follow,
  LicenseData,
  NextCursor,
  OosResource,
  Oid,
  OptionalOid,
  OptionalText,
  Page,
  PageDistribution,
  PageOccupancy,
  PageResource,
  RecordInterpretation,
  RelocationEdge,
  Sector,
  SectorAttribution,
  SessionData,
  Slot,
  SlotResource,
  Volume,
  WatchData,
} from "./domain";
import type { InspectorApi } from "./effects";
