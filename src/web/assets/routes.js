(() => {
  "use strict";

  const ROUTE_KINDS = Object.freeze({
    volume: Object.freeze({ fields: [], parent: null }),
    sector: Object.freeze({ fields: ["sector"], parent: "volume" }),
    page: Object.freeze({ fields: ["page"], parent: "sector" }),
    slot: Object.freeze({ fields: ["page", "slot"], parent: "page" }),
    oos: Object.freeze({ fields: ["page", "slot"], parent: "slot" }),
  });

  function canonicalNumber(value) {
    return /^(0|[1-9]\d*)$/.test(value) && Number.isSafeInteger(Number(value))
      ? Number(value)
      : null;
  }

  // A route names an entity and nothing else. No snapshot and no revision
  // appear in it, so a link keeps working as the viewer reads the input again;
  // which reading answered a request is reported in the response envelope,
  // where it cannot be mistaken for part of the address.
  function parse(pathname = location.pathname) {
    if (pathname === "/") return { kind: "root" };

    const parts = pathname.split("/");
    if (parts[0] !== "") return null;

    const kind = parts[1],
      descriptor = Object.hasOwn(ROUTE_KINDS, kind) ? ROUTE_KINDS[kind] : null,
      vol = canonicalNumber(parts[2]);
    if (
      !descriptor ||
      vol === null ||
      parts.length !== 3 + descriptor.fields.length
    )
      return null;

    const route = { kind, vol };
    for (const [index, field] of descriptor.fields.entries()) {
      const value = canonicalNumber(parts[3 + index]);
      if (value === null) return null;
      route[field] = value;
    }
    return route;
  }

  function path(route) {
    const descriptor = ROUTE_KINDS[route.kind],
      suffix = descriptor.fields.map((field) => route[field]).join("/");
    return `/${route.kind}/${route.vol}${suffix ? `/${suffix}` : ""}`;
  }

  function parentPath(route, currentSectorId) {
    const parent = ROUTE_KINDS[route.kind].parent;
    if (parent === null) return null;

    const parentRoute = { ...route, kind: parent };
    if (parent === "sector") parentRoute.sector = currentSectorId;
    return path(parentRoute);
  }

  window.volmapRoutes = Object.freeze({ parse, path, parentPath });
})();
