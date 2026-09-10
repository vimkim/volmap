import { useEffect, type Dispatch, type ReactNode } from "react";
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
  return row === undefined ? "? Not evaluated" : `? ${row.state} · ${row.reason}`;
}

export function ObservationMetadata({ state, children, detail, summary = "Capture identity and limitations" }: {
  readonly state: UiState;
  readonly children?: ReactNode;
  readonly detail?: ReactNode;
  readonly summary?: string;
}) {
  const observation = state.observation;
  const batch = observation.batch;
  if (batch === null) return null;
  const interval = observationInterval(state);
  return <>
    <p>{observation.age === null ? "Age uncertain" : `Conservative age: ${Math.ceil(observation.age / 1000)} s · ${observationIsFresh(observation.age, interval) ? `fresh (${interval} ms interval)` : `stale (${interval} ms interval)`}`}{state.follow.paused ? " · paused" : ""}</p>
    <p>Evaluated {batch.evaluated} / requested {batch.requested}; producer scan {batch.complete === null ? "unavailable" : batch.complete ? "complete" : "partial"}.</p>
    {children}
    <details><summary>{summary}</summary>
      <p>{batch.captureLabel}</p>
      {batch.limitations.map((limitation) => <p key={limitation}>{limitation}</p>)}
      {detail}
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
    {batch !== null ? <ObservationMetadata state={state} summary="Available page observations and capture limitations" detail={
        <table><thead><tr><th>VPID</th><th>Observation</th><th>Dirty</th><th>Flushing</th></tr></thead>
          <tbody>{batch.rows.map((row) => <tr key={`${row.volid}:${row.pageid}`}><td>{row.volid}:{row.pageid}</td><td>{observationLabel(row)}</td><td>{row.evidence.dirty ?? "unknown"}</td><td>{row.evidence.flushing ?? "unknown"}</td></tr>)}</tbody>
        </table>
    }>
      <p>{batch.rows.filter((row) => row.state === "resident").length} observed resident · {batch.rows.filter((row) => row.state === "not-resident").length} observed not resident · {batch.rows.filter((row) => row.state === "unknown").length} unknown.</p>
    </ObservationMetadata> : <p>No usable batch. Disk facts remain independently observed.</p>}
  </section>;
}
