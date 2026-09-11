import { observationInterval, observationIsFresh } from "./observations";
import { compactObservationLabel, ObservationLegend, LruSummary, ObservationMetadata, VisibleObservations, observationRows, runtimePage, sectorObservationLabel, useObservationViewport } from "./observation-view";
import { memo, useEffect, useRef, type CSSProperties, type Dispatch, type KeyboardEvent } from "react";

import type {
  AttributeName,
  AttributeValue,
  ClassName,
  InterpretedAttribute,
  Page,
  RecordInterpretation,
  Sector,
} from "./domain";
import { SlottedDistribution } from "./distribution";
import type { Action, UiState } from "./model";
import { breadcrumbItems, followLabel } from "./selectors";

export interface ViewerProps {
  readonly state: UiState;
  readonly dispatch: Dispatch<Action>;
  readonly nowUnixSeconds: number;
}

function pageClass(page: Page, extra = ""): string {
  const finding = page.diagnostic.state === "known" ? " finding" : "";
  const occupancy =
    page.allocation === "allocated"
      ? page.occupancy.state === "known"
        ? " occupancy-known"
        : " occupancy-unknown"
      : "";
  return `page ${extra} ${page.allocation}${occupancy}${finding}`.trim();
}

function pageStyle(page: Page): CSSProperties | undefined {
  return page.allocation === "allocated" && page.occupancy.state === "known"
    ? ({ "--occupied": `${page.occupancy.occupied_percent}%` } as CSSProperties)
    : undefined;
}

function pageOccupancyLabel(page: Page): string {
  return page.occupancy.state === "known"
    ? `, ${page.occupancy.occupied_percent}% occupied, ${page.occupancy.free_percent}% free`
    : ", occupancy unknown";
}

function sectorAttributionLabel(sector: Sector): string {
  const attribution = sector.attribution;
  if (attribution === undefined || attribution.state === "unclaimed") return "";
  if (attribution.state === "mixed") return "mixed";
  if (attribution.file.class_name.state === "resolved") {
    return attribution.file.class_name.value;
  }
  if (attribution.file.class_oid.state === "present") return "unresolved";
  return "internal";
}

function sectorFileTypeLabel(sector: Sector): string {
  const attribution = sector.attribution;
  return attribution?.state === "single" && attribution.file.file_type.state === "known"
    ? attribution.file.file_type.value
    : "";
}

function sectorAttributionDetail(sector: Sector): string {
  const attribution = sector.attribution;
  if (attribution === undefined || attribution.state === "unclaimed") return "";
  if (attribution.state === "mixed") {
    return `mixed: ${attribution.claims.length} conflicting file claims`;
  }
  const role =
    attribution.file.file_type.state === "known"
      ? attribution.file.file_type.value
      : "unavailable";
  return `${sectorAttributionLabel(sector)} · ${role} · ${attribution.allocated_pages}/64 allocated`;
}

function VolumeMap({ state, dispatch }: Pick<ViewerProps, "state" | "dispatch">) {
  if (state.view?.kind !== "volume") return null;
  const { view } = state;
  return (
    <section className="volume-view">
      <div className="workspace-title">
        <div>
          <h1>Volume {view.volume.vol_id} · full map</h1>
          <p>
            {view.volume.total_sectors} sectors · 64 pages per sector · revision{" "}
            {state.snapshot?.revision ?? "unknown"}
          </p>
        </div>
        <div id="legend" aria-label="Page allocation and occupancy legend" hidden={state.observation.enabled && state.observation.colorMode === "lru" && state.observation.message !== "Observation expired" && ["active", "stale"].includes(state.runtimeCapability ?? "")}>
          <span><i className="swatch unreserved" />Unreserved</span>
          <span><i className="swatch reserved-unallocated" />Reserved, unallocated</span>
          <span><i className="swatch allocated" />Occupied</span>
          <span><i className="swatch free" />Slotted free</span>
          <span><i className="swatch system-metadata" />System metadata</span>
          <span><i className="swatch finding" />Finding outline</span>
        </div>
      </div>
      <VolumeSectorMap state={state} dispatch={dispatch} />
      <p id="mapStatus" role="status">
        {state.collectionMessage || (view.nextCursor.state === "end"
          ? `All ${view.sectors.length} sectors shown · ${view.sectors.length * 64} pages`
          : `Showing ${view.sectors.length} of ${view.volume.total_sectors} sectors · scroll to continue`)}
      </p>
      {view.nextCursor.state === "present" ? (
        <LoadMoreSectors dispatch={dispatch} />
      ) : null}
    </section>
  );
}

// Poll bookkeeping and age ticks do not rebuild thousands of unchanged cells.
// Batch, source, mode, expiry and freshness changes still update every mark.
const VolumeSectorMap = memo(function VolumeSectorMap({ state, dispatch }: Pick<ViewerProps, "state" | "dispatch">) {
  if (state.view?.kind !== "volume") return null;
  const view = state.view;
  const rows = new Map(state.observation.batch?.rows
    .filter((row) => row.volid === view.volume.vol_id)
    .map((row) => [row.pageid, row]) ?? []);
  // Unadmitted pages share the container's unknown LRU color, so a mode
  // change does not recreate their sector previews.
  const unadmitted = runtimePage(state, undefined);
  const terminal = state.observation.enabled && (state.observation.message === "Observation expired" ||
    ["refused", "incompatible"].includes(state.runtimeCapability ?? "") ||
    (state.runtimeCapability === "unavailable" && state.observation.failures > 0));
  const usable = state.observation.enabled && ["active", "stale"].includes(state.runtimeCapability ?? "") &&
    state.observation.message !== "Observation expired";
  return (
      <div id="volumeMap" aria-label="Full volume sector map" data-runtime-mode={usable ? state.observation.colorMode : undefined}>
        {view.sectors.map((sector) => {
          const marks = sector.pages.map((page) => {
            const row = rows.get(page.page_id);
            return row === undefined ? (terminal ? unadmitted : null) : runtimePage(state, row);
          });
          return <VolumeSectorCard
            key={sector.sector_id} sector={sector} dispatch={dispatch}
            observationLabel={sectorObservationLabel(state.observation.enabled, marks, unadmitted)}
            marks={marks}
          />;
        })}
      </div>
  );
}, (before, after) => {
  const a = before.state;
  const b = after.state;
  return before.dispatch === after.dispatch && a.view === b.view &&
    a.runtimeCapability === b.runtimeCapability &&
    a.observation.enabled === b.observation.enabled &&
    a.observation.batch === b.observation.batch &&
    (a.observation.failures > 0) === (b.observation.failures > 0) &&
    a.observation.colorMode === b.observation.colorMode &&
    (a.observation.message === "Observation expired") === (b.observation.message === "Observation expired") &&
    observationIsFresh(a.observation.age, observationInterval(a)) === observationIsFresh(b.observation.age, observationInterval(b));
});

type PreviewMark = ReturnType<typeof runtimePage> | null;

function samePreviewMarks(before: readonly PreviewMark[], after: readonly PreviewMark[]): boolean {
  return before === after || (before.length === after.length && before.every((mark, index) => {
    const next = after[index]!;
    return mark === next || (mark !== null && next !== null && mark.className === next.className &&
      mark.label === next.label && mark.glyph === next.glyph && mark.state === next.state);
  }));
}

// Retain unchanged sector DOM when a bounded batch rotates elsewhere.
const VolumeSectorCard = memo(function VolumeSectorCard({ sector, marks, observationLabel, dispatch }: {
  readonly sector: Sector;
  readonly marks: readonly PreviewMark[];
  readonly observationLabel: string;
  readonly dispatch: Dispatch<Action>;
}) {
  const table = sectorAttributionLabel(sector);
  const fileType = sectorFileTypeLabel(sector);
  return (
            <button
              className="sector-card"
              id={`sector-${sector.sector_id}`}
              key={sector.sector_id}
              type="button"
              aria-label={`Sector ${sector.sector_id}, ${sector.reserved ? "reserved" : "unreserved"}${table ? `, ${table}` : ""}${fileType ? `, file type ${fileType}` : ""}, 64 pages${observationLabel}`}
              onClick={() =>
                dispatch({
                  kind: "navigate",
                  route: { kind: "sector", vol: sector.vol_id, sector: sector.sector_id },
                  history: "push",
                  autoEnrich: false,
                })
              }
            >
              <span className="sector-heading">
                <strong>Sector {sector.sector_id}</strong>
                <span>{sector.reserved ? "reserved" : "unreserved"}</span>
                {table ? <em className="sector-table">{table}</em> : null}
                {fileType ? <small className="sector-file-type">{fileType}</small> : null}
              </span>
              <VolumeSectorPreview sector={sector} marks={marks} />
            </button>
  );
}, (before, after) => before.sector === after.sector && before.dispatch === after.dispatch &&
  before.observationLabel === after.observationLabel && samePreviewMarks(before.marks, after.marks));

// Source-wide pending/unevaluated status belongs to the sector summary and
// legend. Only row evidence or terminal state changes the individual cells.
const VolumeSectorPreview = memo(function VolumeSectorPreview({ sector, marks }: {
  readonly sector: Sector;
  readonly marks: readonly PreviewMark[];
}) {
  return <span className="sector-preview-pages">
    {sector.pages.map((page, index) => {
      const runtime = marks[index];
      return <i
        aria-hidden="true"
        data-observation-page={`${page.vol_id}:${page.page_id}`}
        data-runtime-state={runtime?.state}
        title={`Page ${page.page_id}: ${runtime?.label ?? ""}`}
        className={pageClass(page, "preview-page") + (runtime?.className ?? "")}
        key={page.page_id}
        style={pageStyle(page)}
      >{runtime?.glyph && runtime.state !== "not-resident" ? <span className="runtime-glyph">{runtime.glyph}</span> : null}</i>;
    })}
  </span>;
}, (before, after) => before.sector === after.sector && samePreviewMarks(before.marks, after.marks));

function LoadMoreSectors({ dispatch }: Pick<ViewerProps, "dispatch">) {
  const sentinel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const node = sentinel.current;
    if (node === null || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        observer.disconnect();
        dispatch({ kind: "request-more-sectors" });
      }
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [dispatch]);
  return (
    <div id="mapSentinel" ref={sentinel}>
      <button type="button" className="load-more" onClick={() => dispatch({ kind: "request-more-sectors" })}>
        Load more sectors
      </button>
    </div>
  );
}

function classNameLabel(name: ClassName): string {
  if (name.state === "resolved") return name.value;
  return `${name.state === "unresolved" ? "unresolved" : "not applicable"} (${name.reason})`;
}

function FieldList({ fields }: { readonly fields: readonly (readonly [string, string | number | boolean])[] }) {
  return (
    <dl>
      {fields.map(([name, value]) => (
        <span key={name} style={{ display: "contents" }}><dt>{name}</dt><dd>{String(value)}</dd></span>
      ))}
    </dl>
  );
}

function fileRows(association: Page["file_association"]): readonly (readonly [string, string])[] {
  if (association.state === "none") {
    return [
      ["File", "none"],
      ["File role", "none"],
      ["Class OID", "none"],
      ["Class/table", "none"],
    ];
  }
  if (association.state === "mixed-claims") {
    return [
      ["File", "mixed claims"],
      ["File role", "mixed claims"],
      ["Class OID", "mixed claims"],
      ["Class/table", "mixed claims"],
    ];
  }
  const file = association.file;
  const rows: (readonly [string, string])[] = [
    [
      "File",
      `file:${file.vol_id}:${file.file_id}${association.state === "reserved-for" ? " (reserved, not allocated)" : ""}`,
    ],
    ["File role", file.file_type.state === "known" ? file.file_type.value : "unavailable"],
    [
      "Class OID",
      file.class_oid.state === "present"
        ? `oid:${file.class_oid.oid.vol_id}:${file.class_oid.oid.page_id}:${file.class_oid.oid.slot_id}`
        : "none",
    ],
    ["Class/table", classNameLabel(file.class_name)],
  ];
  return rows;
}

function ObservationDetail({ state }: Pick<ViewerProps, "state">) {
  const observation = state.observation;
  if (!observation.enabled) return null;
  const batch = observation.batch;
  const label = batch?.state === "resident" ? "◉ Observed resident"
    : batch?.state === "not-resident" ? "○ Observed not resident"
    : batch === null ? observation.message : `? ${batch.state}`;
  return (
    <section className="panel observation-detail" aria-label="Selected-page buffer observation">
      <h2>Buffer observation</h2>
      <p className={`observation-mark observation-${batch?.state ?? "unknown"}`}>{label}</p>
      {batch !== null ? <>
        <p>VPID {batch.pages[0]?.volid}:{batch.pages[0]?.pageid} · {batch.reason}</p>
        <LruSummary state={state} />
        <p>Membership is one sampled tuple. none means no list; invalid means unclassified membership; null means no applicable index. Missing fields remain unknown. Indices identify lists only within this server incarnation.</p>
        <ObservationMetadata state={state}>
        <FieldList fields={Object.entries(batch.evidence).map(([name, value]) => [name.replaceAll("_", " "), value])} />
        </ObservationMetadata>
      </> : <p>Refresh to request a separate runtime observation. Disk facts remain independently observed.</p>}
    </section>
  );
}

function PageWorkspace({ state, dispatch }: Pick<ViewerProps, "state" | "dispatch">) {
  if (
    state.view?.kind !== "page" &&
    state.view?.kind !== "slot" &&
    state.view?.kind !== "oos"
  ) {
    return null;
  }
  const view = state.view;
  const resource = view.page;
  const page = resource.page;
  const structure = resource.deep.structure;
  const primitive =
    structure === undefined
      ? []
      : Object.entries(structure).filter(
          ([name, value]) =>
            name !== "slots" && name !== "bytes" && value !== null && typeof value !== "object",
        );
  const table =
    (page.file_association.state === "allocated" ||
      page.file_association.state === "reserved-for") &&
    page.file_association.file.class_name.state === "resolved"
      ? page.file_association.file.class_name.value
      : "";
  const inspectSlot = (slot: number) =>
    dispatch({
      kind: "navigate",
      route: { kind: "slot", vol: page.vol_id, page: page.page_id, slot },
      history: "push",
      autoEnrich: false,
    });
  return (
    <>
      <div className="workspace-title">
        <div>
          <h1>Page {page.page_id}</h1>
          <p>{page.page_type.state === "known" ? page.page_type.value : "unknown type"} · detailed structural view</p>
          {table ? <p className="muted">{table}</p> : null}
        </div>
      </div>
      <div className={`page-workspace${state.observation.enabled ? " with-observation" : ""}`}>
        <section className="panel">
          <h2>Page facts</h2>
          <FieldList
            fields={[
              ["Identity", `page:${page.vol_id}:${page.page_id}`],
              ["Sector", page.sector_id],
              ["Physical type", page.page_type.state === "known" ? page.page_type.value : "not inspected"],
              ["Allocation", page.allocation],
              ...fileRows(page.file_association),
              ["Availability", page.availability],
              ["Detail support", page.detail_support.state === "known" ? page.detail_support.value : page.detail_support.state],
              ["Deep state", resource.deep.state],
              ["TDE", page.tde_state],
            ]}
          />
          {primitive.length > 0 ? (
            <><h3>Decoded structure</h3><FieldList fields={primitive.map(([name, value]) => [name.replaceAll("_", " "), String(value)])} /></>
          ) : null}
          <p className="withheld">evidence page:{page.vol_id}:{page.page_id} · structural ranges only · bytes withheld</p>
          {view.kind === "page" && view.enriching ? (
            <p className="status-note" role="status">Enriching the selected page at a new immutable revision…</p>
          ) : null}
        </section>
        <ObservationDetail state={state} />
        <section className="panel page-distribution">
          {resource.distribution.state === "available" ? (
            <SlottedDistribution
              slots={resource.slots}
              distribution={resource.distribution}
              inspectSlot={inspectSlot}
            />
          ) : (
            <><h2>Slotted-page distribution</h2><p className="muted">
              {view.kind === "page" && view.enriching
                ? "Loading structural metadata…"
                : "No validated slot directory is available for this page."}
            </p></>
          )}
        </section>
        {view.kind === "slot" ? <SlotDetail state={state} dispatch={dispatch} /> : null}
        {view.kind === "oos" ? <OosDetail state={state} /> : null}
      </div>
    </>
  );
}

function attributeName(name: AttributeName): string {
  return name.state === "resolved" ? name.value : `unnamed (${name.reason})`;
}

function attributeValue(value: AttributeValue): string {
  if (value.state === "decoded") return value.value;
  if (value.state === "null") return "NULL";
  if (value.state === "out-of-row") return `out of row · ${value.total_length} bytes`;
  return `withheld (${value.reason})`;
}

const REGION_LABELS: Readonly<Record<string, string>> = {
  "object-header": "Object header",
  "offset-table": "Offset table",
  "fixed-region": "Fixed attributes",
  "bound-bits": "Bound bits",
  "variable-region": "Variable attributes",
};

function RecordLayout({ interpretation }: { readonly interpretation: RecordInterpretation }) {
  const layout = interpretation.layout;
  if (layout === null) return null;
  const total = Number(layout.record_length);
  if (!Number.isFinite(total) || total <= 0) return null;
  const ranked = interpretation.attributes
    .map((attribute) => ({ name: attributeName(attribute.name), length: Number(attribute.length) }))
    .filter((attribute) => attribute.length > 0)
    .sort((left, right) => right.length - left.length)
    .slice(0, 3);
  return (
    <>
      <h4>Record bytes ({total})</h4>
      <div className="record-map" aria-label={`Byte layout of a ${total}-byte record`}>
        {layout.regions.map((region) => {
          const length = Number(region.length);
          if (length <= 0) return null;
          const share = (length / total) * 100;
          const name = REGION_LABELS[region.region] ?? region.region;
          const label = `${name}: offset ${region.offset}, ${length} bytes, ${share.toFixed(1)}%`;
          return (
            <span
              aria-label={label}
              className={`record-region region-${region.region}`}
              key={`${region.region}-${region.offset}`}
              style={{ width: `${share}%` }}
              title={label}
            />
          );
        })}
      </div>
      <div className="record-legend">
        {layout.regions.map((region) => {
          const length = Number(region.length);
          return length <= 0 ? null : (
            <span key={`${region.region}-legend-${region.offset}`}>
              <i className={`region-${region.region}`} />
              <span>{REGION_LABELS[region.region] ?? region.region} {length} B ({((length / total) * 100).toFixed(1)}%)</span>
            </span>
          );
        })}
      </div>
      {ranked.length > 0 ? (
        <p className="record-largest">
          Largest attributes: {ranked.map((entry) => `${entry.name} ${entry.length} B (${((entry.length / total) * 100).toFixed(1)}%)`).join(" · ")}
        </p>
      ) : null}
    </>
  );
}

function InterpretationTable({
  attributes,
  dispatch,
}: {
  readonly attributes: readonly InterpretedAttribute[];
  readonly dispatch: Dispatch<Action>;
}) {
  return (
    <table className="interpretation">
      <thead><tr><th>Attribute</th><th>Type</th><th className="record-bytes">Bytes</th><th>Value</th></tr></thead>
      <tbody>
        {attributes.map((attribute) => {
          const value = attribute.value;
          return <tr key={`${attribute.position}-${attribute.attribute_id}`}>
            <td>{attributeName(attribute.name)}</td>
            <td>{attribute.type_name}</td>
            <td
              className="record-bytes"
              title={`${attribute.storage} region, offset ${attribute.offset}, ${attribute.length} bytes`}
            >
              {attribute.length}
            </td>
            <td className={value.state === "decoded" ? undefined : "withheld"}>
              {value.state === "out-of-row" ? (
                <button
                  className="slot-action"
                  type="button"
                  onClick={() => dispatch({
                    kind: "request-enrichment",
                    selector: `oos:${value.head.vol_id}:${value.head.page_id}:${value.head.slot_id}`,
                    targetRoute: {
                      kind: "oos",
                      vol: value.head.vol_id,
                      page: value.head.page_id,
                      slot: value.head.slot_id,
                    },
                    history: "push",
                    fallbackRoute: null,
                  })}
                >
                  {attributeValue(value)}
                </button>
              ) : attributeValue(value)}
            </td>
          </tr>;
        })}
      </tbody>
    </table>
  );
}

function interpretationScope(page: Page, slot: { readonly slot_id: number; readonly record_type: string }): string | null {
  if (slot.slot_id === 0) {
    return "slot 0 holds this page's own heap metadata, not a class instance — see the page's heap header facts above";
  }
  if (page.page_type.state !== "known") {
    return "this page's type is unknown, so its records cannot be attributed to a class";
  }
  if (page.page_type.value !== "heap") {
    return `records on a ${page.page_type.value} page are not class instances, so they carry no attribute values`;
  }
  if (!["home", "new-home", "relocation"].includes(slot.record_type)) {
    return `a ${slot.record_type} slot holds no interpretable record`;
  }
  return null;
}

function SlotDetail({ state, dispatch }: Pick<ViewerProps, "state" | "dispatch">) {
  if (state.view?.kind !== "slot") return null;
  const page = state.view.page.page;
  const data = state.view.slot;
  const slot = data.selected_slot;
  const outOfScope = interpretationScope(page, slot);
  const target =
    data.relocation_edge?.target.state === "present"
      ? `${data.relocation_edge.target.oid.vol_id}:${data.relocation_edge.target.oid.page_id}:${data.relocation_edge.target.oid.slot_id}`
      : "unknown";
  return (
    <section id="slotDetail" className="panel slot-detail">
      <h2>Slot {slot.slot_id}</h2>
      <FieldList
        fields={[
          ["Identity", `slot:${page.vol_id}:${page.page_id}:${slot.slot_id}`],
          ["Record type", `${slot.record_type} (${slot.record_type_ordinal})`],
          ["Offset", slot.offset],
          ["Size", slot.length],
        ]}
      />
      {page.page_type.state === "known" &&
      page.page_type.value === "oos" &&
      slot.offset > 0 &&
      slot.record_type === "home" ? (
        <button
          className="slot-action"
          type="button"
          onClick={() => dispatch({
            kind: "request-enrichment",
            selector: `oos:${page.vol_id}:${page.page_id}:${slot.slot_id}`,
            targetRoute: { kind: "oos", vol: page.vol_id, page: page.page_id, slot: slot.slot_id },
            history: "push",
            fallbackRoute: null,
          })}
        >
          Validate OOS chain
        </button>
      ) : null}
      {data.relocation_edge === null ? null : (
        <FieldList fields={[["Relocated to", target], ["Edge valid", data.relocation_edge.valid]]} />
      )}
      {outOfScope === null ? (
        data.interpretation === null ? (
          data.interpretation_unavailable === null ? (
            <button
              className="slot-action"
              type="button"
              onClick={() => dispatch({
                kind: "request-enrichment",
                selector: `record:${page.vol_id}:${page.page_id}:${slot.slot_id}`,
                targetRoute: state.route,
                history: "none",
                fallbackRoute: null,
              })}
            >Interpret records</button>
          ) : (
            <p className="withheld">not interpreted ({data.interpretation_unavailable})</p>
          )
        ) : (
          <>
            <h3>Interpretation</h3>
            {data.class_representation === null ? null : (
              <FieldList fields={[
                ["Class", classNameLabel(data.class_representation.class_name)],
                [
                  "Representation",
                  data.class_representation.is_current.state === "known"
                    ? `${data.class_representation.representation_id} (${data.class_representation.is_current.value})`
                    : data.class_representation.representation_id,
                ],
              ]} />
            )}
            {data.interpretation.relocated_from.state === "present" ? (
              <FieldList fields={[[
                "Interpreted via relocation from",
                `${data.interpretation.relocated_from.oid.vol_id}:${data.interpretation.relocated_from.oid.page_id}:${data.interpretation.relocated_from.oid.slot_id}`,
              ]]} />
            ) : null}
            {data.interpretation.diagnostic.state === "known" ? (
              <p className="withheld">not interpreted ({data.interpretation.diagnostic.value})</p>
            ) : (
              <><RecordLayout interpretation={data.interpretation} /><InterpretationTable attributes={data.interpretation.attributes} dispatch={dispatch} /></>
            )}
          </>
        )
      ) : (
        <p className="withheld">not interpreted ({outOfScope})</p>
      )}
      <p className="withheld">evidence slot:{page.vol_id}:{page.page_id}:{slot.slot_id} · structural ranges only · bytes withheld</p>
    </section>
  );
}

function OosDetail({ state }: Pick<ViewerProps, "state">) {
  if (state.view?.kind !== "oos") return null;
  const page = state.view.page.page;
  const chain = state.view.chain.chain;
  const slot = state.route.kind === "oos" ? state.route.slot : 0;
  return (
    <section id="slotDetail" className="panel slot-detail">
      <h2>OOS chain</h2>
      <FieldList fields={[
        ["Identity", `oos:${page.vol_id}:${page.page_id}:${slot}`],
        ["Complete", chain.complete],
        ["Validated bytes", chain.validated_payload_bytes],
        ["Chunks", chain.chunks.length],
        ["Diagnostic", chain.diagnostic.state === "known" ? chain.diagnostic.value : "none"],
      ]} />
      <p className="withheld">evidence oos:{page.vol_id}:{page.page_id}:{slot} · structural ranges only · bytes withheld</p>
    </section>
  );
}

function movePageFocus(event: KeyboardEvent<HTMLButtonElement>, index: number): void {
  const offset =
    event.key === "ArrowLeft"
      ? -1
      : event.key === "ArrowRight"
        ? 1
        : event.key === "ArrowUp"
          ? -8
          : event.key === "ArrowDown"
            ? 8
            : 0;
  if (offset === 0) return;
  const next = index + offset;
  const target = event.currentTarget.closest('[role="grid"]')?.querySelectorAll('[role="gridcell"]').item(next);
  if (next >= 0 && next < 64 && target instanceof HTMLElement) {
    event.preventDefault();
    target.focus();
  }
}

function SectorWorkspace({ state, dispatch }: Pick<ViewerProps, "state" | "dispatch">) {
  if (state.view?.kind !== "sector") return null;
  const sector = state.view.sector;
  const detail = sectorAttributionDetail(sector);
  const rows = observationRows(state);
  return (
    <>
      <div className="workspace-title">
        <div>
          <h1>Sector {sector.sector_id}</h1>
          <p>64 physical pages · select a page to enlarge</p>
          {detail ? <p className="muted">{detail}</p> : null}
        </div>
      </div>
      <ObservationLegend state={state} />
      <section className="sector-focus">
        <div className="sector-focus-grid" role="grid" aria-label={`Sector ${sector.sector_id}, 64 physical pages`}>
          {Array.from({ length: 8 }, (_, row) => <div role="row" className="sector-focus-row" key={row}>
          {sector.pages.slice(row * 8, row * 8 + 8).map((page, column) => {
            const index = row * 8 + column;
            const runtime = runtimePage(state, rows.get(`${page.vol_id}:${page.page_id}`));
            return <button
              data-observation-page={`${page.vol_id}:${page.page_id}`}
              data-runtime-state={runtime.state}
              title={runtime.label}
              aria-label={`Page ${page.page_id}, ${page.allocation}${pageOccupancyLabel(page)}${page.diagnostic.state === "known" ? ", finding" : ""}${runtime.label ? `, ${runtime.label}` : ""}`}
              className={pageClass(page, "focus-page") + runtime.className}
              key={page.page_id}
              role="gridcell"
              style={pageStyle(page)}
              type="button"
              onClick={() =>
                dispatch({
                  kind: "navigate",
                  route: { kind: "page", vol: page.vol_id, page: page.page_id },
                  history: "push",
                  autoEnrich: true,
                })
              }
              onKeyDown={(event) => movePageFocus(event, index)}
            >
              <span className="page-kind">
                {page.page_type.state === "known" ? page.page_type.value : "not inspected"}
              </span>
              <span className="page-id">{page.page_id}</span>
              {state.observation.enabled ? <small className="runtime-cell-label">{runtime.state === "unavailable" ? "Source unavailable" : runtime.state === "expired" ? "Expired" : compactObservationLabel(rows.get(`${page.vol_id}:${page.page_id}`))}</small> : null}
              <span className="runtime-glyph" aria-hidden="true">{runtime.glyph}</span>
            </button>;
          })}
          </div>)}
        </div>
      </section>
    </>
  );
}

function Breadcrumb({ state, dispatch }: Pick<ViewerProps, "state" | "dispatch">) {
  const sectorId =
    state.view?.kind === "sector" ||
    state.view?.kind === "page" ||
    state.view?.kind === "slot" ||
    state.view?.kind === "oos"
      ? state.view.sector.sector_id
      : null;
  const items = breadcrumbItems(state.route, sectorId);
  return (
    <nav id="drillBreadcrumb" aria-label="Inspection hierarchy">
      {items.length > 1 ? (
        <button className="back" type="button" onClick={() => dispatch({ kind: "hierarchy-back" })}>
          ← Back
        </button>
      ) : null}
      {items.map((item, index) => (
        <span key={`${item.label}-${index}`}>
          {index > 0 ? "›" : null}
          {item.route === null ? (
            <span>{item.label}</span>
          ) : (
            <button
              type="button"
              onClick={() =>
                dispatch({
                  kind: "navigate",
                  route: item.route!,
                  history: "push",
                  autoEnrich: false,
                })
              }
            >
              {item.label}
            </button>
          )}
        </span>
      ))}
    </nav>
  );
}

export function Viewer({ state, dispatch, nowUnixSeconds }: ViewerProps) {
  useObservationViewport(state, dispatch);
  const routeIdentity = JSON.stringify(state.route);
  const focusedRoute = useRef(routeIdentity);
  const previousView = useRef(state.view);
  useEffect(() => {
    const viewChanged = previousView.current !== state.view;
    previousView.current = state.view;
    if (!viewChanged || state.view === null || focusedRoute.current === routeIdentity) return;
    const heading = document.querySelector<HTMLElement>("#workspaceContent h1");
    if (heading === null) return;
    focusedRoute.current = routeIdentity;
    heading.tabIndex = -1;
    heading.focus();
  }, [routeIdentity, state.view]);
  const follow =
    state.snapshot === null ? "" : followLabel(state.snapshot, state.follow, nowUnixSeconds);
  return (
    <>
      <header role="banner">
        <strong>VOLMAP</strong>
        <span id="crumb">
          {state.snapshot === null
            ? "loading session"
            : `snapshot ${state.snapshot.id.slice(0, 12)} · revision ${state.snapshot.revision}`}
        </span>
        <span className="spacer" />
        {state.follow.enabled ? (
          <span id="followControl" className="follow-control">
            <span id="followStatus">{follow}</span>
            <button
              id="followToggle"
              type="button"
              aria-pressed={state.follow.paused}
              onClick={() => dispatch({ kind: "toggle-pause" })}
            >
              {state.follow.paused ? "Resume" : "Pause"}
            </button>
          </span>
        ) : null}
        <button id="licenses" type="button" onClick={() => dispatch({ kind: "show-licenses" })}>About &amp; licenses</button>
        <span id="outcome">{state.outcome}</span>
      </header>
      <main id="app">
        <aside>
          <section className="observation-controls" aria-label="CUBRID page-buffer observation">
            <h2>CUBRID page-buffer observation</h2>
            <button type="button" aria-pressed={state.observation.enabled} onClick={() => dispatch({ kind: "toggle-observation" })}>
              {state.observation.enabled ? "Disable observations" : "Enable observations"}
            </button>
            {state.observation.enabled ? <>
              <label className="runtime-mode">Runtime color mode
                <select value={state.observation.colorMode} onChange={(event) => dispatch({ kind: "observation-color-mode", mode: event.target.value === "lru" ? "lru" : "state" })}>
                  <option value="state">State marks</option><option value="lru">LRU topology</option>
                </select>
              </label>
              <button type="button" aria-busy={state.observation.loading && state.observation.batch !== null && !state.follow.paused && state.visible}
                disabled={state.follow.paused || !state.visible || state.observation.loading || state.route.kind === "root"}
                onClick={() => dispatch({ kind: "refresh-observation" })}>{"page" in state.route ? "Refresh selected-page observation" : "Refresh visible-page observations"}</button>
              <p>{state.observation.loading && state.observation.batch === null ? (state.follow.paused ? "Checking observation availability…" : ("page" in state.route ? "Observing selected page…" : "Observing visible pages…")) : state.observation.message}</p>
              {state.observation.stopped ? <p>Automatic retry stopped · Refresh explicitly to retry attachment.</p> : null}
              {state.follow.paused && state.observation.newerAvailable ? <p>Newer observation available · Resume to request a new capture.</p> : null}

            </> : null}
            <p>Observation source: {state.runtimeCapability ?? "checking capability"}</p>
            {state.runtimeCapability === "disabled" ? (
              <p>Not requested. Enable explicitly when starting a loopback viewer.</p>
            ) : state.runtimeCapability === "unavailable" ? (
              <p>No verified producer observation is available. Disk inspection is unaffected.</p>
            ) : null}
          </section>
          {state.view?.kind === "volume" ? <ObservationLegend state={state} /> : null}
          <VisibleObservations state={state} />
          <h2>Snapshot hierarchy</h2>
          <div id="volumes">
            {state.volumes.map((volume) => (
              <button
                className={`nav${state.route.kind !== "root" && state.route.vol === volume.vol_id ? " active" : ""}`}
                data-volume={volume.vol_id}
                key={volume.vol_id}
                type="button"
                onClick={() =>
                  dispatch({
                    kind: "navigate",
                    route: { kind: "volume", vol: volume.vol_id },
                    history: "push",
                    autoEnrich: false,
                  })
                }
              >
                volume {volume.vol_id} · {volume.total_sectors} sectors
              </button>
            ))}
          </div>
        </aside>
        <section className="workspace">
          <Breadcrumb state={state} dispatch={dispatch} />
          <div id="workspaceContent">
            {state.view === null ? <p className="status-note">Loading inspection…</p> : null}
            <VolumeMap state={state} dispatch={dispatch} />
            <SectorWorkspace state={state} dispatch={dispatch} />
            <PageWorkspace state={state} dispatch={dispatch} />
            {state.error === null ? null : (
              <section className="status-note error-note" role="alert">
                <strong>Could not complete this view</strong>
                <span>{state.error.message}</span>
                <small>
                  {state.error.status === undefined
                    ? `Browser error · ${state.error.code}`
                    : `HTTP ${state.error.status} · ${state.error.code}`}
                </small>
              </section>
            )}
          </div>
        </section>
      </main>
      <dialog id="infoDialog" open={state.license.open}>
        <button id="closeInfo" type="button" onClick={() => dispatch({ kind: "close-licenses" })}>Close</button>
        <pre id="infoContent" className="withheld">
          {state.license.loading ? "Loading notices…" : state.license.notice}
        </pre>
      </dialog>
    </>
  );
}
