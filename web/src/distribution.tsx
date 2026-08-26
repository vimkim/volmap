import type { CSSProperties, KeyboardEvent } from "react";

import type { PageDistribution, Slot } from "./domain";

interface DistributionProps {
  readonly slots: readonly Slot[];
  readonly distribution: Extract<PageDistribution, { state: "available" }>;
  readonly inspectSlot: (slot: number) => void;
}

interface Region {
  readonly kind: string;
  readonly label: string;
  readonly offset: number;
  readonly length: number;
  readonly slotId?: number;
}

const LEGEND = [
  ["header", "Slotted header"],
  ["record", "Allocated record"],
  ["fragmented-free", "Fragmented free"],
  ["contiguous-free", "Contiguous free"],
  ["slot-directory", "Slot directory"],
] as const;

function regions(distribution: DistributionProps["distribution"]): readonly Region[] {
  return [
    { ...distribution.header, kind: "header", label: "Slotted-page header" },
    ...distribution.record_extents.map((record) => ({
      offset: record.offset,
      length: record.length,
      slotId: record.slot_id,
      kind: "record",
      label: `Slot ${record.slot_id} · ${record.record_type}`,
    })),
    ...distribution.free_regions.map((region, index) => ({
      ...region,
      label: `${region.kind === "contiguous-free" ? "Contiguous" : "Fragmented"} free region ${index + 1}`,
    })),
    {
      ...distribution.slot_directory,
      kind: "slot-directory",
      label: "Slot directory",
    },
  ].sort((left, right) => left.offset - right.offset || left.length - right.length);
}

function extentStyle(region: Region, contentSize: number): CSSProperties {
  return {
    left: `${(region.offset / contentSize) * 100}%`,
    width: `${(region.length / contentSize) * 100}%`,
  };
}

function activateOnKeyboard(event: KeyboardEvent, action: () => void): void {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    action();
  }
}

function Metric({ value, label }: { readonly value: string | number; readonly label: string }) {
  return (
    <div className="distribution-metric">
      <strong>{value}</strong><span>{label}</span>
    </div>
  );
}

export function SlottedDistribution({ slots, distribution, inspectSlot }: DistributionProps) {
  const allRegions = regions(distribution);
  const slotById = new Map(slots.map((slot) => [slot.slot_id, slot]));
  const unallocated = distribution.slot_entries.filter((entry) => entry.state !== "allocated").length;
  return (
    <>
      <h2>Full slotted-page distribution</h2>
      <div className="distribution-summary">
        <Metric value={distribution.record_extents.length} label="allocated records" />
        <Metric value={unallocated} label="slots not allocated" />
        <Metric value={distribution.free_regions.length} label="free byte regions" />
        <Metric value={`${distribution.unoccupied_bytes} B`} label="unoccupied bytes" />
      </div>
      <div className="distribution-legend">
        {LEGEND.map(([kind, label]) => (
          <span key={kind}><i className={`region-${kind}`} />{label}</span>
        ))}
      </div>
      <div
        className="full-page-map"
        aria-label={`Complete ${distribution.content_size}-byte slotted-page content map`}
      >
        {allRegions.map((region, index) => {
          const end = region.offset + region.length;
          const label = `${region.label}: offset ${region.offset}, size ${region.length} bytes, end ${end}`;
          return region.kind === "record" && region.slotId !== undefined ? (
            <button
              aria-label={label}
              className={`page-region region-${region.kind}`}
              key={`${region.kind}-${region.offset}-${index}`}
              style={extentStyle(region, distribution.content_size)}
              title={label}
              type="button"
              onClick={() => inspectSlot(region.slotId!)}
            />
          ) : (
            <span
              aria-label={label}
              className={`page-region region-${region.kind}`}
              key={`${region.kind}-${region.offset}-${index}`}
              style={extentStyle(region, distribution.content_size)}
              title={label}
            />
          );
        })}
      </div>
      <div className="page-map-axis">
        {[0, 0.25, 0.5, 0.75, 1].map((share) => (
          <span key={share}>{Math.floor(distribution.content_size * share)}</span>
        ))}
      </div>
      <section>
        <div className="distribution-section-title">
          <h3>Physical intervals</h3>
          <span className="muted">{allRegions.length} exhaustive non-overlapping regions</span>
        </div>
        <div className="region-list">
          {allRegions.map((region, index) => {
            const action = () => region.slotId !== undefined && inspectSlot(region.slotId);
            return (
              <div
                aria-label={region.kind === "record" ? `Inspect ${region.label}` : undefined}
                className="region-row"
                key={`${region.kind}-row-${region.offset}-${index}`}
                role={region.kind === "record" ? "button" : undefined}
                tabIndex={region.kind === "record" ? 0 : undefined}
                onClick={region.kind === "record" ? action : undefined}
                onKeyDown={region.kind === "record" ? (event) => activateOnKeyboard(event, action) : undefined}
              >
                <span className="region-name"><i className={`region-${region.kind}`} /><span>{region.label}</span></span>
                <span className="region-range">{region.offset}–{region.offset + region.length}</span>
                <span className="region-size">{region.length} B</span>
                <span className="region-lane"><i className={`region-${region.kind}`} style={extentStyle(region, distribution.content_size)} /></span>
              </div>
            );
          })}
        </div>
      </section>
      <section>
        <div className="distribution-section-title">
          <h3>Slot directory</h3>
          <span className="muted">{distribution.slot_entries.length} entries · allocated, empty, and deleted</span>
        </div>
        <div className="slot-directory-grid">
          {distribution.slot_entries.map((entry) => {
            const live = slotById.get(entry.slot_id);
            return (
              <button
                className={`slot-entry ${entry.state}`}
                key={entry.slot_id}
                type="button"
                onClick={() => inspectSlot(entry.slot_id)}
              >
                <strong>Slot {entry.slot_id}</strong>
                <span className="slot-state">
                  {entry.state === "allocated"
                    ? "allocated"
                    : entry.state === "deleted"
                      ? "deleted · not allocated"
                      : "not allocated"}
                </span>
                <small>record type · {entry.record_type}</small>
                <small>directory · {entry.offset}–{entry.offset + entry.length} ({entry.length} B)</small>
                <small>
                  {live !== undefined && live.offset > 0
                    ? `record · ${live.offset}–${live.offset + live.length} (${live.length} B)`
                    : `record · no live extent${live !== undefined && live.length > 0 ? ` · retained length ${live.length} B` : ""}`}
                </small>
              </button>
            );
          })}
        </div>
      </section>
    </>
  );
}
