import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";

import type { FileAssociation, Page } from "./domain";
import { initialState, reduce } from "./model";
import { Viewer } from "./view";

const snapshot = {
  id: "0123456789abcdef",
  revision: "7",
  validity: "valid",
  format_profile: "fixture",
  generation: "4",
  observed_at_unix_seconds: "100",
  input_modified_unix_seconds: "90",
} as const;

const pages = Array.from({ length: 64 }, (_, pageId) => ({
  vol_id: 0,
  page_id: pageId,
  sector_id: 0,
  allocation: pageId < 2 ? "system-metadata" : "reserved-unallocated",
  page_type: { state: "known" as const, value: pageId === 10 ? "heap" : "unknown" },
  availability: "available",
  tde_state: "not-encrypted",
  detail_support: { state: "known" as const, value: pageId === 10 ? "semantic" : "opaque" },
  occupancy:
    pageId === 10
      ? ({ state: "known", occupied_percent: 7, free_percent: 93 } as const)
      : ({ state: "unknown" } as const),
  diagnostic: { state: "unknown" as const },
  file_association: { state: "none" as const },
}));

function renderPageWithAssociation(fileAssociation: FileAssociation): string {
  const route = { kind: "page", vol: 0, page: 10 } as const;
  const selectedPage: Page = {
    ...pages[10]!,
    allocation: fileAssociation.state === "reserved-for" ? "reserved-unallocated" : "allocated",
    file_association: fileAssociation,
  };
  const sectorPages = pages.map((page) =>
    page.page_id === selectedPage.page_id ? selectedPage : page,
  );
  let state = initialState(route);
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, {
    kind: "route-loaded",
    scope: state.scope,
    result: {
      route,
      snapshot,
      outcome: "success-limited",
      follow: { state: "disabled" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "page",
        volume: { vol_id: 0, total_sectors: 64 },
        sector: {
          vol_id: 0,
          sector_id: 0,
          reserved: true,
          attribution: { state: "unclaimed" },
          pages: sectorPages,
        },
        page: {
          page: selectedPage,
          deep: { state: "not-enriched" },
          slots: [],
          distribution: { state: "not-available" },
        },
        enriching: false,
      },
    },
  });
  return renderToStaticMarkup(
    <Viewer state={state} dispatch={() => undefined} nowUnixSeconds={106} />,
  );
}

function expectFact(html: string, label: string, value: string): void {
  expect(html).toContain(`<dt>${label}</dt><dd>${value}</dd>`);
}

test("the React compatibility view renders the existing semantic hierarchy and full sector map", () => {
  const route = { kind: "volume", vol: 0 } as const;
  let state = initialState(route);
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, {
    kind: "route-loaded",
    scope: state.scope,
    result: {
      route,
      snapshot,
      outcome: "success-limited",
      follow: { state: "disabled" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "volume",
        volume: { vol_id: 0, total_sectors: 64 },
        sectors: [
          {
            vol_id: 0,
            sector_id: 0,
            reserved: true,
            attribution: { state: "unclaimed" },
            pages,
          },
        ],
        nextCursor: { state: "end" },
      },
    },
  });

  const html = renderToStaticMarkup(
    <Viewer state={state} dispatch={() => undefined} nowUnixSeconds={106} />,
  );

  expect(html).toContain('<header role="banner">');
  expect(html).toContain("Snapshot hierarchy");
  expect(html).toContain('aria-label="Full volume sector map"');
  expect(html).toContain("Sector 0");
  expect(html).toContain("64 pages");
  expect(html).toContain("All 1 sectors shown · 64 pages");
  expect(html).not.toContain("React viewer foundation ready");
});

test("the React page workspace preserves exhaustive structural distribution and disclosure", () => {
  const route = { kind: "page", vol: 0, page: 10 } as const;
  let state = initialState(route);
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, {
    kind: "route-loaded",
    scope: state.scope,
    result: {
      route,
      snapshot,
      outcome: "success-limited",
      follow: { state: "disabled" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "page",
        volume: { vol_id: 0, total_sectors: 64 },
        sector: {
          vol_id: 0,
          sector_id: 0,
          reserved: true,
          attribution: { state: "unclaimed" },
          pages,
        },
        page: {
          page: pages[10]!,
          deep: {
            state: "slotted",
            structure: {
              anchor: "anchored",
              total_free: "16000",
              contiguous_free: "15900",
            },
          },
          slots: [
            { slot_id: 0, offset: 32, length: 128, record_type: "home", record_type_ordinal: 0 },
          ],
          distribution: {
            state: "available",
            content_size: 16384,
            header: { offset: 0, length: 32 },
            record_extents: [
              { slot_id: 0, offset: 32, length: 128, record_type: "home" },
            ],
            free_regions: [{ offset: 160, length: 16220, kind: "contiguous-free" }],
            slot_directory: { offset: 16380, length: 4 },
            slot_entries: [
              { slot_id: 0, offset: 16380, length: 4, state: "allocated", record_type: "home" },
            ],
            allocated_record_bytes: 128,
            unoccupied_bytes: 16220,
          },
        },
        enriching: false,
      },
    },
  });

  const html = renderToStaticMarkup(
    <Viewer state={state} dispatch={() => undefined} nowUnixSeconds={106} />,
  );
  expect(html).toContain("Page 10");
  expect(html).toContain("Page facts");
  expect(html).toContain("Full slotted-page distribution");
  expect(html).toContain("Slot 0 · home: offset 32, size 128 bytes, end 160");
  expect(html).toContain("evidence page:0:10 · structural ranges only · bytes withheld");
  expect(html).not.toContain("0x");
});

test("the Page facts render a resolved association and safely wrap hostile Unicode names", () => {
  const storedName = `dba.고객_&<script>\"${"길".repeat(64)}`;
  const html = renderPageWithAssociation({
    state: "allocated",
    file: {
      vol_id: 1,
      file_id: 640,
      file_type: { state: "known", value: "heap-reuse-slots" },
      class_oid: {
        state: "present",
        oid: { vol_id: 0, page_id: 209, slot_id: 2 },
      },
      class_name: { state: "resolved", value: storedName },
    },
  });

  expectFact(html, "Identity", "page:0:10");
  expectFact(html, "Physical type", "heap");
  expectFact(html, "Allocation", "allocated");
  expectFact(html, "File", "file:1:640");
  expectFact(html, "File role", "heap-reuse-slots");
  expectFact(html, "Class OID", "oid:0:209:2");
  expect(html).toContain("<dt>Class/table</dt><dd>dba.고객_&amp;&lt;script&gt;&quot;");
  expect(html).toContain("길".repeat(64));
  expect(html).not.toContain("<script>");
  expectFact(html, "Availability", "available");
  expectFact(html, "Deep state", "not-enriched");
});

test("the Page facts keep all four association rows for every fail-closed state", () => {
  const knownFile = {
    vol_id: 0,
    file_id: 18,
    file_type: { state: "known", value: "catalog" },
    class_oid: { state: "absent" },
  } as const;
  const cases: readonly Readonly<{
    association: FileAssociation;
    file: string;
    role: string;
    oid: string;
    table: string;
  }>[] = [
    { association: { state: "none" }, file: "none", role: "none", oid: "none", table: "none" },
    {
      association: { state: "mixed-claims" },
      file: "mixed claims",
      role: "mixed claims",
      oid: "mixed claims",
      table: "mixed claims",
    },
    {
      association: {
        state: "allocated",
        file: {
          ...knownFile,
          class_name: {
            state: "unresolved",
            reason_code: "class-association.inventory-incomplete",
            reason: "complete file inventory is required for class attribution",
          },
        },
      },
      file: "file:0:18",
      role: "catalog",
      oid: "none",
      table: "unresolved (complete file inventory is required for class attribution)",
    },
    {
      association: {
        state: "allocated",
        file: {
          ...knownFile,
          class_oid: { state: "present", oid: { vol_id: 0, page_id: 6, slot_id: 1 } },
          class_name: {
            state: "unresolved",
            reason_code: "class-name.page-unavailable",
            reason: "class record page could not be read",
          },
        },
      },
      file: "file:0:18",
      role: "catalog",
      oid: "oid:0:6:1",
      table: "unresolved (class record page could not be read)",
    },
    ...([
      ["class-association.null-oid", "file descriptor has a null class OID"],
      ["class-association.no-single-class", "file type has no single class association"],
      ["class-association.internal-file", "internal file is not associated with one user class"],
      ["class-association.oos-deferred", "OOS class attribution is intentionally deferred"],
    ] as const).map(([reason_code, reason]) => ({
      association: {
        state: "allocated" as const,
        file: {
          ...knownFile,
          class_name: { state: "not-applicable" as const, reason_code, reason },
        },
      },
      file: "file:0:18",
      role: "catalog",
      oid: "none",
      table: `not applicable (${reason})`,
    })),
    {
      association: {
        state: "reserved-for",
        file: {
          vol_id: 0,
          file_id: 64,
          file_type: { state: "known", value: "heap" },
          class_oid: { state: "present", oid: { vol_id: 0, page_id: 6, slot_id: 1 } },
          class_name: { state: "resolved", value: "dba.reserved" },
        },
      },
      file: "file:0:64 (reserved, not allocated)",
      role: "heap",
      oid: "oid:0:6:1",
      table: "dba.reserved",
    },
  ];

  for (const item of cases) {
    const html = renderPageWithAssociation(item.association);
    expectFact(html, "File", item.file);
    expectFact(html, "File role", item.role);
    expectFact(html, "Class OID", item.oid);
    expectFact(html, "Class/table", item.table);
  }
});

test("the React sector workspace exposes all 64 pages as keyboard grid cells", () => {
  const route = { kind: "sector", vol: 0, sector: 0 } as const;
  let state = initialState(route);
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, {
    kind: "route-loaded",
    scope: state.scope,
    result: {
      route,
      snapshot,
      outcome: "success-limited",
      follow: { state: "disabled" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "sector",
        volume: { vol_id: 0, total_sectors: 64 },
        sector: {
          vol_id: 0,
          sector_id: 0,
          reserved: true,
          attribution: { state: "unclaimed" },
          pages,
        },
      },
    },
  });
  const html = renderToStaticMarkup(
    <Viewer state={state} dispatch={() => undefined} nowUnixSeconds={106} />,
  );
  expect(html).toContain('role="grid" aria-label="Sector 0, 64 physical pages"');
  expect(html.match(/role="gridcell"/g)).toHaveLength(64);
  expect(html).toContain("Page 10, reserved-unallocated, 7% occupied, 93% free");
});

test("the React slot panel renders every interpretation state without raw bytes", () => {
  const route = { kind: "slot", vol: 0, page: 10, slot: 1 } as const;
  let state = initialState(route);
  state = reduce(state, { kind: "effects-started", ids: [1] });
  state = reduce(state, {
    kind: "route-loaded",
    scope: state.scope,
    result: {
      route,
      snapshot,
      outcome: "success-limited",
      follow: { state: "disabled" },
      volumes: [{ vol_id: 0, total_sectors: 64 }],
      view: {
        kind: "slot",
        volume: { vol_id: 0, total_sectors: 64 },
        sector: { vol_id: 0, sector_id: 0, reserved: true, pages },
        page: {
          page: pages[10]!,
          deep: { state: "slotted", structure: {} },
          slots: [
            { slot_id: 1, offset: 40, length: 20, record_type: "home", record_type_ordinal: 0 },
          ],
          distribution: { state: "not-available" },
        },
        slot: {
          page: pages[10]!,
          deep: { state: "slotted", structure: {} },
          selected_slot: {
            slot_id: 1,
            offset: 40,
            length: 20,
            record_type: "home",
            record_type_ordinal: 0,
          },
          relocation_edge: null,
          class_representation: {
            representation_id: 3,
            class_name: { state: "resolved", value: "orders" },
            is_current: { state: "known", value: "current" },
          },
          interpretation_unavailable: null,
          interpretation: {
            layout: {
              record_length: "20",
              regions: [{ region: "fixed-region", offset: "0", length: "20" }],
            },
            relocated_from: { state: "absent" },
            diagnostic: { state: "unknown" },
            attributes: [
              {
                name: { state: "resolved", value: "id" },
                attribute_id: 1,
                position: 0,
                type_name: "integer",
                precision: 0,
                scale: 0,
                offset: "0",
                length: "4",
                storage: "fixed",
                value: { state: "decoded", value: "42" },
              },
              {
                name: { state: "resolved", value: "note" },
                attribute_id: 2,
                position: 1,
                type_name: "varchar",
                precision: 20,
                scale: 0,
                offset: "4",
                length: "0",
                storage: "variable",
                value: { state: "null" },
              },
              {
                name: { state: "unresolved", reason: "old representation" },
                attribute_id: 3,
                position: 2,
                type_name: "varchar",
                precision: 20,
                scale: 0,
                offset: "4",
                length: "16",
                storage: "variable",
                value: { state: "withheld", reason: "unsupported-domain", offset: 4, length: 16 },
              },
            ],
          },
        },
      },
    },
  });
  const html = renderToStaticMarkup(
    <Viewer state={state} dispatch={() => undefined} nowUnixSeconds={106} />,
  );
  expect(html).toContain("Interpretation");
  expect(html).toContain("orders");
  expect(html).toContain("42");
  expect(html).toContain("NULL");
  expect(html).toContain("withheld (unsupported-domain)");
  expect(html).toContain("evidence slot:0:10:1 · structural ranges only · bytes withheld");
  expect(html).not.toContain("0x");
});
