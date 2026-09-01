import { expect, test } from "vitest";

import { createHttpApi, decodeResource, objectData } from "./api";
import type { FileAssociation } from "./domain";

function pageDocument(fileAssociation: FileAssociation): object {
  return {
    schema: "volmap.inspection",
    schema_version: 1,
    document_type: "resource",
    snapshot: {
      id: "0123456789abcdef",
      revision: "7",
      validity: "valid",
      format_profile: "fixture",
      generation: "4",
      observed_at_unix_seconds: "100",
      input_modified_unix_seconds: "90",
    },
    outcome: "success-limited",
    coverage: [],
    diagnostics: [],
    data: {
      page: {
        vol_id: 0,
        page_id: 10,
        sector_id: 0,
        allocation: "allocated",
        page_type: { state: "known", value: "heap" },
        availability: "available",
        tde_state: "not-encrypted",
        detail_support: { state: "known", value: "semantic" },
        occupancy: { state: "unknown" },
        diagnostic: { state: "unknown" },
        file_association: fileAssociation,
      },
      deep: { state: "not-enriched" },
      slots: [],
      distribution: { state: "not-available" },
    },
  };
}

test("the HTTP boundary rejects a structurally invalid Volmap resource", () => {
  expect(() =>
    decodeResource(
      {
        schema: "not-volmap",
        schema_version: 1,
        document_type: "resource",
        snapshot: {},
        outcome: "success",
        data: {},
      },
      objectData,
    ),
  ).toThrow("invalid Volmap resource schema");
});

test("the HTTP adapter requests typed resources with no-store same-origin policy", async () => {
  let request: Readonly<{ url: string; cache?: RequestCache; credentials?: RequestCredentials }> | null =
    null;
  const fetcher: typeof fetch = async (input, init) => {
    request = {
      url: String(input),
      cache: init?.cache,
      credentials: init?.credentials,
    };
    return new Response(
      JSON.stringify({
        schema: "volmap.inspection",
        schema_version: 1,
        document_type: "resource",
        snapshot: {
          id: "0123456789abcdef",
          revision: "7",
          validity: "valid",
          format_profile: "fixture",
          generation: "4",
          observed_at_unix_seconds: "100",
          input_modified_unix_seconds: "90",
        },
        outcome: "success-limited",
        coverage: [],
        diagnostics: [],
        data: {
          items: [{ vol_id: 0, total_sectors: 64 }],
          next_cursor: { state: "end" },
        },
      }),
      { headers: { "content-type": "application/json" } },
    );
  };

  const result = await createHttpApi(fetcher).volumes();

  expect(result.data.items).toEqual([{ vol_id: 0, total_sectors: 64 }]);
  expect(request).toEqual({
    url: "/api/v1/volumes",
    cache: "no-store",
    credentials: "same-origin",
  });
});

test("the HTTP boundary retains generation and revision as server identities", () => {
  const resource = decodeResource(
    {
      schema: "volmap.inspection",
      schema_version: 1,
      document_type: "resource",
      snapshot: {
        id: "0123456789abcdef",
        revision: "7",
        validity: "valid",
        format_profile: "fixture",
        generation: "4",
        observed_at_unix_seconds: "100",
        input_modified_unix_seconds: "90",
      },
      outcome: "success-limited",
      coverage: [],
      diagnostics: [],
      data: { state: "fixture" },
    },
    objectData,
  );

  expect(resource.snapshot).toMatchObject({ revision: "7", generation: "4" });
  expect(resource.data).toEqual({ state: "fixture" });
});

test("the Page decoder retains the additive association reason and accepts its omission", async () => {
  const fetcher: typeof fetch = async () =>
    new Response(
      JSON.stringify({
        schema: "volmap.inspection",
        schema_version: 1,
        document_type: "resource",
        snapshot: {
          id: "0123456789abcdef",
          revision: "7",
          validity: "valid",
          format_profile: "fixture",
          generation: "4",
          observed_at_unix_seconds: "100",
          input_modified_unix_seconds: "90",
        },
        outcome: "success-limited",
        coverage: [],
        diagnostics: [],
        data: {
          page: {
            vol_id: 0,
            page_id: 10,
            sector_id: 0,
            allocation: "allocated",
            page_type: { state: "known", value: "heap" },
            availability: "available",
            tde_state: "not-encrypted",
            detail_support: { state: "known", value: "semantic" },
            occupancy: { state: "unknown" },
            diagnostic: { state: "unknown" },
            file_association: {
              state: "allocated",
              file: {
                vol_id: 0,
                file_id: 7,
                file_type: { state: "known", value: "heap" },
                class_oid: { state: "absent" },
                class_name: {
                  state: "unresolved",
                  reason_code: "class-association.inventory-incomplete",
                  reason: "complete file inventory is required for class attribution",
                },
              },
            },
          },
          deep: { state: "not-enriched" },
          slots: [],
          distribution: { state: "not-available" },
        },
      }),
      { headers: { "content-type": "application/json" } },
    );

  const result = await createHttpApi(fetcher).page(0, 10);
  const association = result.data.page.file_association;
  expect(association.state).toBe("allocated");
  if (association.state !== "allocated") throw new Error("expected allocated association");
  expect(association.file.class_name).toEqual({
    state: "unresolved",
    reason_code: "class-association.inventory-incomplete",
    reason: "complete file inventory is required for class attribution",
  });

  const legacyDocument = objectData(await (await fetcher("")).json(), "legacy resource");
  const legacyData = objectData(legacyDocument.data, "legacy resource.data");
  const legacyPage = objectData(legacyData.page, "legacy resource.data.page");
  const legacyAssociation = objectData(
    legacyPage.file_association,
    "legacy resource.data.page.file_association",
  );
  const legacyFile = objectData(
    legacyAssociation.file,
    "legacy resource.data.page.file_association.file",
  );
  const legacyClassName = objectData(
    legacyFile.class_name,
    "legacy resource.data.page.file_association.file.class_name",
  );
  delete legacyClassName.reason_code;
  const legacyResult = await createHttpApi(
    async () => new Response(JSON.stringify(legacyDocument)),
  ).page(0, 10);
  const legacyDecoded = legacyResult.data.page.file_association;
  if (legacyDecoded.state !== "allocated") throw new Error("expected allocated association");
  expect(legacyDecoded.file.class_name).toEqual({
    state: "unresolved",
    reason: "complete file inventory is required for class attribution",
  });
});

test("the Page API retains every shared association state without adapter inference", async () => {
  const file = {
    vol_id: 0,
    file_id: 7,
    file_type: { state: "known", value: "heap" },
    class_oid: { state: "absent" },
  } as const;
  const associations: readonly FileAssociation[] = [
    { state: "none" },
    { state: "mixed-claims" },
    {
      state: "allocated",
      file: {
        ...file,
        class_oid: { state: "present", oid: { vol_id: 0, page_id: 6, slot_id: 1 } },
        class_name: { state: "resolved", value: "dba.고객<&" },
      },
    },
    {
      state: "allocated",
      file: {
        ...file,
        class_oid: { state: "present", oid: { vol_id: 0, page_id: 6, slot_id: 1 } },
        class_name: {
          state: "unresolved",
          reason_code: "class-name.page-unavailable",
          reason: "class record page could not be read",
        },
      },
    },
    ...([
      ["class-association.null-oid", "file descriptor has a null class OID"],
      ["class-association.no-single-class", "file type has no single class association"],
      ["class-association.internal-file", "internal file is not associated with one user class"],
      ["class-association.oos-deferred", "OOS class attribution is intentionally deferred"],
    ] as const).map(([reason_code, reason]) => ({
      state: "allocated" as const,
      file: {
        ...file,
        class_name: { state: "not-applicable" as const, reason_code, reason },
      },
    })),
    {
      state: "reserved-for",
      file: {
        ...file,
        class_oid: { state: "present", oid: { vol_id: 0, page_id: 6, slot_id: 1 } },
        class_name: { state: "resolved", value: "dba.reserved" },
      },
    },
  ];

  for (const association of associations) {
    const result = await createHttpApi(
      async () => new Response(JSON.stringify(pageDocument(association))),
    ).page(0, 10);
    expect(result.data.page.file_association).toEqual(association);
  }
});
