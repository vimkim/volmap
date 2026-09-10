import { useEffect, useState, type Dispatch, type ReactNode } from "react";
import type { Action, UiState } from "./model";
import { observationInterval, observationIsFresh, type ObservationRow } from "./observations";

// DOM visibility selects inspection projections; runtime adapters never parse
// disk bytes. Stable ordering is nearest sector to viewport centre, then VPID.
export function useObservationViewport(state: UiState, dispatch: Dispatch<Action>): void {
  const { view, scope, visible } = state;
  const enabled = state.observation.enabled;
  useEffect(() => {
    if (!enabled || !visible || view === null) return;
    if (view.kind === "sector") {
      dispatch({ kind: "observation-viewport", scope, pages: view.sector.pages.map((page) => ({ volid: page.vol_id, pageid: page.page_id })) });
      return;
    }
    if (view.kind !== "volume") return;
    let pending = 0;
    const measure = () => {
      pending = 0;
      const sectors = view.sectors.flatMap((sector) => {
        const rect = document.getElementById(`sector-${sector.sector_id}`)?.getBoundingClientRect();
        if (!rect || rect.bottom <= 0 || rect.top >= window.innerHeight || rect.right <= 0 || rect.left >= window.innerWidth) return [];
        return [{ sector, distance: Math.abs((rect.top + rect.bottom) / 2 - window.innerHeight / 2) }];
      });
      sectors.sort((a, b) => a.distance - b.distance || a.sector.sector_id - b.sector.sector_id);
      dispatch({ kind: "observation-viewport", scope, pages: sectors.flatMap(({ sector }) =>
        [...sector.pages].sort((a, b) => a.page_id - b.page_id).map((page) => ({ volid: page.vol_id, pageid: page.page_id }))) });
    };
    const schedule = () => { if (pending === 0) pending = window.requestAnimationFrame(measure); };
    measure();
    window.addEventListener("scroll", schedule, true);
    window.addEventListener("resize", schedule);
    const resize = new ResizeObserver(schedule);
    const map = document.getElementById("volumeMap");
    if (map) resize.observe(map);
    return () => {
      window.cancelAnimationFrame(pending);
      window.removeEventListener("scroll", schedule, true);
      window.removeEventListener("resize", schedule);
      resize.disconnect();
    };
  }, [view, scope, enabled, visible, dispatch]);
}

export function observationLabel(row: ObservationRow | undefined): string {
  if (row?.state === "resident") return "◉ Observed resident";
  if (row?.state === "not-resident") return "○ Observed not resident";
  if (row?.reason === "duplicate-vpid") return "≠ Duplicate ambiguity · duplicate-vpid";
  return row === undefined ? "? Not evaluated" : `? ${row.state} · ${row.reason}`;
}

export function ObservationMetadata({ state, children, detail, summary = "Capture identity and limitations" }: {
  readonly state: UiState;
  readonly children?: ReactNode;
  readonly detail?: () => ReactNode;
  readonly summary?: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const observation = state.observation;
  const batch = observation.batch;
  if (batch === null) return null;
  return <>
    <ObservationAge state={state} />
    <p>Evaluated {batch.evaluated} / requested {batch.requested}; producer scan {batch.complete === null ? "unavailable" : batch.complete ? "complete" : "partial"}.</p>
    {children}
    <details onToggle={(event) => setExpanded(event.currentTarget.open)}><summary>{summary}</summary>
      <p>{batch.captureLabel}</p>
      {batch.limitations.map((limitation) => <p key={limitation}>{limitation}</p>)}
      {expanded ? detail?.() : null}
    </details>
  </>;
}

export function VisibleObservations({ state }: { readonly state: UiState }) {
  const observation = state.observation;
  if (!observation.enabled || (state.route.kind !== "volume" && state.route.kind !== "sector")) return null;
  const batch = observation.batch;
  return <section className="panel observation-detail" aria-label="Visible-page buffer observations">
    <h2>Visible-page buffer observations</h2>
    <p>{observation.message}</p>
    <p>{batch?.requested ?? 0} admitted / {observation.viewportCount} visible pages.
      {observation.viewportCount > 512 ? " Reduced admission: non-selected pages rotate at the 512-VPID limit. Other pages are not evaluated in this batch." : ""}</p>
    <LruSummary state={state} />
    {batch !== null ? <ObservationMetadata state={state} summary="Available page observations and capture limitations" detail={() =>
        <table><thead><tr><th>VPID</th><th>Observation</th><th>Dirty</th><th>Flushing</th><th>LRU zone</th><th>List kind</th></tr></thead>
          <tbody>{batch.rows.map((row) => <tr key={`${row.volid}:${row.pageid}`}><td>{row.volid}:{row.pageid}</td><td>{observationLabel(row)}</td><td>{row.evidence.dirty ?? "unknown"}</td><td>{row.evidence.flushing ?? "unknown"}</td><td>{row.evidence.lru_zone ?? "unknown"}</td><td>{row.evidence.lru_list_kind ?? "unknown"}</td></tr>)}</tbody>
        </table>
    }>
      <p>{batch.rows.filter((row) => row.state === "resident").length} observed resident · {batch.rows.filter((row) => row.state === "not-resident").length} observed not resident · {batch.rows.filter((row) => row.state === "unknown").length} unknown.</p>
    </ObservationMetadata> : <p>No usable batch. Disk facts remain independently observed.</p>}
  </section>;
}

// One current batch only. Never union scopes or captures into pool counters.
export function observationRows(state: UiState): ReadonlyMap<string, ObservationRow> {
  return new Map(state.observation.batch?.rows.map((row) => [`${row.volid}:${row.pageid}`, row]) ?? []);
}

// These projections contain no capture evidence. Reuse them across the many
// cells outside a bounded batch instead of allocating per-cell mark objects.
const noRuntimeMark = { className: "", label: "", glyph: "", state: undefined } as const;
const unevaluatedMark = { className: "", label: "? Not evaluated", glyph: "", state: "unknown" } as const;
const unevaluatedLruMark = { ...unevaluatedMark, className: " runtime-lru zone-unknown" } as const;
const nonresidentMark = { className: "", label: "○ Observed not resident", glyph: "○", state: "not-resident" } as const;
const nonresidentLruMark = { ...nonresidentMark, className: " runtime-lru zone-unknown" } as const;

export function runtimePage(state: UiState, row: ObservationRow | undefined) {
  const observation = state.observation;
  if (!observation.enabled) return noRuntimeMark;
  const available = state.runtimeCapability === "active" || state.runtimeCapability === "stale";
  if (available && observation.batch === null && observation.message !== "Observation expired") {
    // A revoked viewport batch means these pages are not evaluated; it does
    // not make the verified source unavailable. Keep unknown cells stable
    // while the next bounded scope is requested.
    return observation.colorMode === "lru" ? unevaluatedLruMark : unevaluatedMark;
  }
  if (!available || observation.batch === null) {
    const expired = observation.message === "Observation expired";
    return { className: "", label: expired ? "Observation expired" : `No usable observation · source ${state.runtimeCapability ?? "connecting"}`, glyph: "", state: expired ? "expired" : "unavailable" };
  }
  if (row === undefined) return observation.colorMode === "lru" ? unevaluatedLruMark : unevaluatedMark;
  if (row.state === "not-resident") return observation.colorMode === "lru" ? nonresidentLruMark : nonresidentMark;
  const label = observationLabel(row);
  if (row?.state !== "resident") return { className: observation.colorMode === "lru" ? " runtime-lru zone-unknown" : "", label, glyph: row.reason === "duplicate-vpid" ? "≠" : "?", state: row?.state ?? "unknown" };
  const evidence = row.evidence;
  const dirty = evidence.dirty === "true";
  const flushing = evidence.flushing === "true";
  const zone = evidence.lru_zone;
  const kind = evidence.lru_list_kind;
  const topology = observation.colorMode === "lru";
  const knownZone = ["lru1", "lru2", "lru3", "void", "invalid"].includes(zone ?? "");
  const stale = !observationIsFresh(observation.age, observationInterval(state));
  return {
    className: ` runtime-resident${dirty ? " runtime-dirty" : ""}${flushing ? " runtime-flushing" : ""}${stale ? " runtime-stale" : ""}${topology ? ` runtime-lru zone-${knownZone ? zone : "unknown"}${kind === "private" ? " lru-private" : ""}` : ""}`,
    label: `${label} · dirty ${evidence.dirty} · flushing ${evidence.flushing}${topology ? ` · ${zone} · ${kind} membership` : ""}${stale ? " · stale" : ""}`,
    glyph: `${topology ? ({ lru1: "1", lru2: "2", lru3: "3", void: "V", invalid: "!" }[zone ?? ""] ?? "?") : "◉"}${dirty ? "D" : ""}${flushing ? "F" : ""}`,
    state: "resident",
  };
}

export function ObservationLegend({ state }: { readonly state: UiState }) {
  if (!state.observation.enabled) return null;
  return <section className="runtime-legend" aria-label="Runtime overlay legend">
    <p>{state.observation.colorMode === "lru" ? "LRU topology colors replace storage colors: 1 lru1 · 2 lru2 · 3 lru3 · V void · ! invalid · ? unknown. Private membership has a dashed edge." : "Storage colors retained. ◉ cyan inset: observed resident · D amber corner: dirty · F static magenta edge: flushing."}</p>
    <p>○ observed not resident · ? unknown / not evaluated · ≠ duplicate ambiguity. Pages outside this batch have no runtime glyph. No usable source or expired evidence: storage colors only, no runtime marks. Sampled states, not events or durability evidence.</p>
    <ObservationAge state={state} />
    <p>Source: {state.runtimeCapability ?? "connecting"} · {state.follow.paused ? "Paused adoption" : "Adopting observations"} · {state.observation.message}</p>
  </section>;
}

export function LruSummary({ state }: { readonly state: UiState }) {
  const batch = state.observation.batch;
  if (batch === null) return null;
  const counts = new Map<string, number>();
  for (const row of batch.rows) {
    const { lru_list_kind: kind, lru_list_index: index } = row.evidence;
    if (row.state !== "resident" || !["shared", "private"].includes(kind ?? "") || !/^\d+$/.test(index ?? "")) continue;
    const key = `${kind} list ${index}`;
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return <div className="lru-summary">
    <p>{batch.topology ? `Shared lists: ${batch.topology.shared} · Private lists: ${batch.topology.private}` : "Topology counts unknown"}. Indices are incarnation-local.</p>
    <details><summary>Observed per-list counts in this batch only</summary>
      <p>{batch.complete ? "Complete producer scan; summary limited to evaluated requested pages." : "Partial producer scan; partial summary of evaluated requested pages."} Not pool totals, native counters or quotas. Captures are never combined.</p>
      {counts.size === 0 ? <p>No exact list membership observed in this batch.</p> : <ul>{[...counts].sort(([a], [b]) => a.localeCompare(b, "en", { numeric: true })).map(([list, count]) => <li key={list}>{list}: {count} observed resident</li>)}</ul>}
    </details>
  </div>;
}

export function sectorObservationLabel(enabled: boolean, marks: readonly (ReturnType<typeof runtimePage> | null)[], fallback: ReturnType<typeof runtimePage>): string {
  if (!enabled) return "";
  const counts = new Map<string, number>();
  for (const mark of marks) {
    const runtime = mark ?? fallback;
    // Keep the sector button concise; exact per-page observations remain in
    // the named disclosure table and the keyboard-accessible Sector grid.
    const label = runtime.state === "resident" ? "observed resident" : runtime.label;
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  return `, buffer observations: ${[...counts].map(([label, count]) => `${count} ${label}`).join("; ")}`;
}

export function compactObservationLabel(row: ObservationRow | undefined): string {
  if (row?.state === "resident" || row?.state === "not-resident") return observationLabel(row);
  if (row?.reason === "duplicate-vpid") return "≠ Duplicate ambiguity";
  if (row?.reason === "partial-omission") return "? Partial gap";
  return "? Not evaluated";
}

function ObservationAge({ state }: { readonly state: UiState }) {
  const { observation } = state;
  const interval = observationInterval(state);
  return <p>{observation.batch === null ? "No retained capture" : observation.age === null ? "Age uncertain" : `Conservative age: ${Math.ceil(observation.age / 1000)} s · ${observationIsFresh(observation.age, interval) ? "fresh" : "stale"} (${interval} ms interval)`}{state.follow.paused ? " · paused" : ""}</p>;
}
