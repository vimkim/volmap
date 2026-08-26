import type { JsonObject, Snapshot } from "./api";
import type { EntityRoute } from "./routes";

export type OptionalText =
  | Readonly<{ state: "known"; value: string }>
  | Readonly<{ state: "unknown" | "unsupported" }>;
export type OptionalCount =
  | Readonly<{ state: "known"; value: string }>
  | Readonly<{ state: "unknown" }>;
export type ClassName =
  | Readonly<{ state: "resolved"; value: string }>
  | Readonly<{ state: "unresolved" | "not-applicable"; reason: string }>;
export type OptionalOid =
  | Readonly<{ state: "present"; oid: Oid }>
  | Readonly<{ state: "absent" }>;

export interface Oid {
  readonly vol_id: number;
  readonly page_id: number;
  readonly slot_id: number;
}

export interface Volume {
  readonly vol_id: number;
  readonly total_sectors: number;
  readonly purpose?: string;
  readonly volume_type?: string;
  readonly maximum_sectors?: number;
  readonly system_last_page?: number;
  readonly reserved_sectors?: number;
}

export interface FileBody {
  readonly vol_id: number;
  readonly file_id: number;
  readonly file_type: OptionalText;
  readonly class_oid: OptionalOid;
  readonly class_name: ClassName;
}

export type FileAssociation =
  | Readonly<{ state: "none" }>
  | Readonly<{ state: "mixed-claims" }>
  | Readonly<{ state: "allocated"; file: FileBody }>
  | Readonly<{ state: "reserved-for"; file: FileBody }>;

export type PageOccupancy =
  | Readonly<{ state: "unknown" }>
  | Readonly<{ state: "known"; occupied_percent: number; free_percent: number }>;

export interface Page {
  readonly vol_id: number;
  readonly page_id: number;
  readonly sector_id: number;
  readonly allocation: string;
  readonly page_type: OptionalText;
  readonly availability: string;
  readonly tde_state: string;
  readonly detail_support: OptionalText;
  readonly occupancy: PageOccupancy;
  readonly diagnostic: OptionalText;
  readonly file_association: FileAssociation;
}

export type SectorAttribution =
  | Readonly<{ state: "unclaimed" }>
  | Readonly<{
      state: "single";
      file: FileBody;
      full: boolean;
      allocated_pages: number;
      reserved_unallocated_pages: number;
    }>
  | Readonly<{ state: "mixed"; claims: readonly JsonObject[] }>;

export interface Sector {
  readonly vol_id: number;
  readonly sector_id: number;
  readonly reserved: boolean;
  readonly attribution?: SectorAttribution;
  readonly pages: readonly Page[];
}

export interface Slot {
  readonly slot_id: number;
  readonly offset: number;
  readonly length: number;
  readonly record_type: string;
  readonly record_type_ordinal: number;
}

export interface ByteRegion {
  readonly offset: number;
  readonly length: number;
}

export interface RecordExtent extends ByteRegion {
  readonly slot_id: number;
  readonly record_type: string;
}

export interface FreeRegion extends ByteRegion {
  readonly kind: string;
}

export interface SlotEntry extends ByteRegion {
  readonly slot_id: number;
  readonly state: string;
  readonly record_type: string;
}

export type PageDistribution =
  | Readonly<{ state: "not-available" }>
  | Readonly<{
      state: "available";
      content_size: number;
      header: ByteRegion;
      record_extents: readonly RecordExtent[];
      free_regions: readonly FreeRegion[];
      slot_directory: ByteRegion;
      slot_entries: readonly SlotEntry[];
      allocated_record_bytes: number;
      unoccupied_bytes: number;
    }>;

export interface DeepPage {
  readonly state: string;
  readonly structure?: JsonObject;
  readonly rule?: string;
}

export interface PageResource {
  readonly page: Page;
  readonly deep: DeepPage;
  readonly slots: readonly Slot[];
  readonly distribution: PageDistribution;
}

export type AttributeName =
  | Readonly<{ state: "resolved"; value: string }>
  | Readonly<{ state: "unresolved"; reason: string }>;
export type AttributeValue =
  | Readonly<{ state: "decoded"; value: string }>
  | Readonly<{ state: "null" }>
  | Readonly<{ state: "out-of-row"; head: Oid; total_length: string }>
  | Readonly<{ state: "withheld"; reason: string; offset: number; length: number }>;

export interface InterpretedAttribute {
  readonly name: AttributeName;
  readonly attribute_id: number;
  readonly position: number;
  readonly type_name: string;
  readonly precision: number;
  readonly scale: number;
  readonly offset: string;
  readonly length: string;
  readonly storage: string;
  readonly value: AttributeValue;
}

export interface RecordRegion {
  readonly region: string;
  readonly offset: string;
  readonly length: string;
}

export interface RecordInterpretation {
  readonly layout: Readonly<{
    record_length: string;
    regions: readonly RecordRegion[];
  }> | null;
  readonly relocated_from: OptionalOid;
  readonly diagnostic: OptionalText;
  readonly attributes: readonly InterpretedAttribute[];
}

export interface ClassRepresentation {
  readonly representation_id: number;
  readonly class_name: ClassName;
  readonly is_current: OptionalText;
}

export interface RelocationEdge {
  readonly target: OptionalOid;
  readonly valid: boolean;
}

export interface SlotResource {
  readonly page: Page;
  readonly deep: DeepPage;
  readonly selected_slot: Slot;
  readonly relocation_edge: RelocationEdge | null;
  readonly interpretation: RecordInterpretation | null;
  readonly class_representation: ClassRepresentation | null;
  readonly interpretation_unavailable: string | null;
}

export interface OosChain {
  readonly complete: boolean;
  readonly validated_payload_bytes: string;
  readonly chunks: readonly JsonObject[];
  readonly diagnostic: OptionalText;
}

export interface OosResource {
  readonly chain: OosChain;
}

export type Follow =
  | Readonly<{ state: "disabled" }>
  | Readonly<{
      state: "following";
      poll_interval_ms: string;
      retained_generations: string;
    }>;

export interface SessionData {
  readonly access: string;
  readonly follow: Follow;
}

export interface CollectionData<T> {
  readonly items: readonly T[];
  readonly next_cursor: NextCursor;
}

export interface WatchData {
  readonly advanced: boolean;
  readonly follow: Follow;
}

export interface LicenseData {
  readonly notice: string;
}

export type NextCursor =
  | Readonly<{ state: "end" }>
  | Readonly<{ state: "present"; value: string }>;

export type ResolvedView =
  | Readonly<{
      kind: "volume";
      volume: Volume;
      sectors: readonly Sector[];
      nextCursor: NextCursor;
    }>
  | Readonly<{ kind: "sector"; volume: Volume; sector: Sector }>
  | Readonly<{
      kind: "page";
      volume: Volume;
      sector: Sector;
      page: PageResource;
      enriching: boolean;
    }>
  | Readonly<{
      kind: "slot";
      volume: Volume;
      sector: Sector;
      page: PageResource;
      slot: SlotResource;
    }>
  | Readonly<{
      kind: "oos";
      volume: Volume;
      sector: Sector;
      page: PageResource;
      chain: OosResource;
    }>;

export interface ResolvedRoute {
  readonly route: EntityRoute;
  readonly snapshot: Snapshot;
  readonly outcome: string;
  readonly follow: Follow;
  readonly volumes: readonly Volume[];
  readonly view: ResolvedView;
}
