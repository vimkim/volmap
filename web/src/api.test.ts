import { expect, test } from "vitest";

import { createHttpApi, decodeResource, objectData } from "./api";

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
