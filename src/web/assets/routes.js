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

  function parse(pathname = location.pathname) {
    if (pathname === "/") return { kind: "root" };

    const parts = pathname.split("/");
    if (
      parts[0] !== "" ||
      parts[1] !== "s" ||
      !/^[0-9a-f]{32}$/.test(parts[2] || "") ||
      parts[3] !== "r" ||
      !/^(0|[1-9]\d*)$/.test(parts[4] || "")
    )
      return null;

    const kind = parts[5],
      descriptor = Object.hasOwn(ROUTE_KINDS, kind) ? ROUTE_KINDS[kind] : null,
      vol = canonicalNumber(parts[6]);
    if (
      !descriptor ||
      vol === null ||
      parts.length !== 7 + descriptor.fields.length
    )
      return null;

    const route = {
      snapshot: parts[2],
      revision: parts[4],
      kind,
      vol,
    };
    for (const [index, field] of descriptor.fields.entries()) {
      const value = canonicalNumber(parts[7 + index]);
      if (value === null) return null;
      route[field] = value;
    }
    return route;
  }

  function path(route) {
    const descriptor = ROUTE_KINDS[route.kind],
      prefix = `/s/${route.snapshot}/r/${route.revision}`,
      suffix = descriptor.fields.map((field) => route[field]).join("/");
    return `${prefix}/${route.kind}/${route.vol}${suffix ? `/${suffix}` : ""}`;
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
