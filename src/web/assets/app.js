(() => {
  "use strict";
  let session = null,
    currentVolume = null,
    currentSector = null,
    currentPage = null,
    selectedPage = null,
    selectedSlot = null,
    currentLevel = "volume",
    volumeView = null,
    sectorCursor = "end",
    loadedSectors = 0,
    loadingEpoch = null,
    loadEpoch = 0,
    routeEpoch = 0,
    followEnabled = false,
    followPaused = false,
    watchedGeneration = null,
    pendingFollowPayload = null,
    refreshingFollow = false;
  const sectorCache = new Map();
  const $ = (id) => document.getElementById(id);
  const {
    parse: parseBrowserRoute,
    path: browserRoutePath,
    parentPath: routeParentPath,
  } = window.volmapRoutes;
  const { render: renderSlottedDistribution } = window.volmapDistribution;
  function button(label, action, className = "") {
    const node = document.createElement("button");
    node.textContent = label;
    node.className = className;
    node.onclick = action;
    return node;
  }
  function fieldList(fields) {
    const list = document.createElement("dl");
    for (const [name, value] of fields) {
      const term = document.createElement("dt"),
        detail = document.createElement("dd");
      term.textContent = name;
      detail.textContent = String(value);
      list.append(term, detail);
    }
    return list;
  }
  function browserRoute(kind) {
    const route = {
      kind,
      vol: currentVolume.vol_id,
    };
    if (kind === "sector") route.sector = currentSector.sector_id;
    if (kind === "page") route.page = selectedPage;
    if (kind === "slot" || kind === "oos") {
      route.page = selectedPage;
      route.slot = selectedSlot;
    }
    return route;
  }
  function browserParentPath(route) {
    const sectorId = route.kind === "page" ? currentSector.sector_id : null;
    return routeParentPath(route, sectorId);
  }
  function syncBrowserRoute(kind, mode = "push") {
    if (mode === "none") return;
    const route = browserRoute(kind),
      path = browserRoutePath(route),
      parent = browserParentPath(route);
    if (location.pathname === path) {
      if (!history.state?.volmap)
        history.replaceState(
          { volmap: true, previous: null, parent },
          "",
          path,
        );
      return;
    }
    if (mode === "replace")
      history.replaceState(
        { volmap: true, previous: history.state?.previous || null, parent },
        "",
        path,
      );
    else
      history.pushState(
        { volmap: true, previous: location.pathname, parent },
        "",
        path,
      );
  }
  function installBrowserRouteState(route) {
    const current = browserRoute(route.kind);
    history.replaceState(
      { volmap: true, previous: null, parent: browserParentPath(current) },
      "",
      browserRoutePath(current),
    );
  }
  async function api(path, options = {}) {
    const response = await fetch(path, {
      ...options,
      cache: "no-store",
      credentials: "same-origin",
    });
    if (!response.ok) {
      let payload = null;
      try {
        payload = await response.json();
      } catch {}
      const detail = payload && payload.error,
        reason =
          detail && typeof payload.error.message === "string"
            ? payload.error.message
            : `The server rejected this request (HTTP ${response.status}).`,
        error = new Error(reason);
      error.status = response.status;
      error.code =
        detail && typeof detail.code === "string" ? detail.code : "http-error";
      throw error;
    }
    return response.json();
  }
  // Entity paths, resolved by the server against whatever reading is current.
  const API_BASE = "/api/v1";
  function updateSession(payload) {
    session.snapshot = payload.snapshot;
    session.outcome = payload.outcome;
    $("outcome").textContent = payload.outcome;
    $("crumb").textContent =
      `snapshot ${payload.snapshot.id.slice(0, 12)} · revision ${payload.snapshot.revision}`;
    renderFollowChip();
  }
  function snapshotGeneration(payload = session) {
    if (payload?.snapshot?.generation === null) return null;
    const generation = Number(payload?.snapshot?.generation);
    return Number.isSafeInteger(generation) ? generation : null;
  }
  function secondsAgo(unixSeconds) {
    if (unixSeconds === null || unixSeconds === undefined)
      return "read time unknown";
    const observed = Number(unixSeconds);
    if (!Number.isFinite(observed)) return "read time unknown";
    return `${Math.max(0, Math.floor(Date.now() / 1000 - observed))}s ago`;
  }
  function diskTime(unixSeconds) {
    if (unixSeconds === null || unixSeconds === undefined)
      return "disk time unknown";
    const modified = Number(unixSeconds);
    if (!Number.isFinite(modified)) return "disk time unknown";
    const date = new Date(modified * 1000),
      hours = String(date.getHours()).padStart(2, "0"),
      minutes = String(date.getMinutes()).padStart(2, "0");
    return `disk ${hours}:${minutes}`;
  }
  function renderFollowChip() {
    const root = $("followControl");
    if (!root) return;
    root.hidden = !followEnabled;
    if (!followEnabled || !session?.snapshot) return;
    const viewed = snapshotGeneration(),
      newest = Math.max(viewed ?? 0, watchedGeneration ?? 0),
      status = $("followStatus"),
      toggle = $("followToggle");
    status.textContent = followPaused
      ? `paused at gen ${viewed} · newer: gen ${newest}`
      : `live · gen ${viewed} · ${secondsAgo(session.snapshot.observed_at_unix_seconds)} · ${diskTime(session.snapshot.input_modified_unix_seconds)}`;
    toggle.textContent = followPaused ? "Resume" : "Pause";
    toggle.setAttribute("aria-pressed", String(followPaused));
  }
  function delay(milliseconds) {
    return new Promise((resolve) => setTimeout(resolve, milliseconds));
  }
  function restoreScroll(left, top) {
    requestAnimationFrame(() => window.scrollTo(left, top));
  }
  async function refreshCurrentDrillLevel() {
    const route = parseBrowserRoute();
    if (!route) throw new Error("invalid inspector URL");
    const left = window.scrollX,
      top = window.scrollY;
    invalidateVolumeView();
    if (route.kind === "slot" || route.kind === "oos")
      await refreshEnrichedDrillLevel(route);
    else await loadVolumes(route, "none");
    restoreScroll(left, top);
    return true;
  }
  async function refreshEnrichedDrillLevel(route) {
    await loadVolumes(
      { kind: "page", vol: route.vol, page: route.page },
      "none",
    );
    const page = currentPage,
      selector = `${route.kind}:${route.vol}:${route.page}:${route.slot}`;
    try {
      const refreshed = await enrichAndRefreshPage(selector, page, "none"),
        resolved = refreshed
          ? route.kind === "slot"
            ? await showSlot(refreshed, route.slot, "none")
            : await showOos(refreshed, route.slot, "none")
          : null;
      if (!refreshed || !resolved) throw new Error("the entity is no longer present");
    } catch {
      document.querySelector(".error-note")?.remove();
      await fallBackFromEnrichedDrillLevel(route);
    }
  }
  async function fallBackFromEnrichedDrillLevel(route) {
    if (route.kind === "oos") {
      try {
        const page = currentPage,
          refreshed = await enrichAndRefreshPage(
            `slot:${route.vol}:${route.page}:${route.slot}`,
            page,
            "none",
          ),
          slot = refreshed
            ? await showSlot(refreshed, route.slot, "replace")
            : null;
        if (slot) return;
      } catch {}
      document.querySelector(".error-note")?.remove();
    }
    await showPage(route.page, true, "replace");
  }
  async function refreshPendingFollow() {
    if (refreshingFollow || followPaused || !pendingFollowPayload) return;
    refreshingFollow = true;
    try {
      while (!followPaused && pendingFollowPayload) {
        const payload = pendingFollowPayload,
          target = snapshotGeneration(payload),
          viewed = snapshotGeneration();
        if (target === null || (viewed !== null && target <= viewed)) {
          pendingFollowPayload = null;
          break;
        }
        if (!(await refreshCurrentDrillLevel())) break;
        if (pendingFollowPayload === payload) pendingFollowPayload = null;
      }
    } catch (error) {
      renderWorkspaceError(error);
    } finally {
      refreshingFollow = false;
      renderFollowChip();
    }
  }
  async function followLoop() {
    while (followEnabled) {
      try {
        const payload = await api(
            `${API_BASE}/live/watch?generation=${watchedGeneration}`,
          ),
          generation = snapshotGeneration(payload);
        if (generation !== null) watchedGeneration = generation;
        if (
          payload.data.advanced &&
          generation !== null &&
          generation !== snapshotGeneration()
        )
          pendingFollowPayload = payload;
        renderFollowChip();
        await refreshPendingFollow();
      } catch {
        await delay(1000);
      }
    }
  }
  function configureFollow() {
    followEnabled = session.data.follow.state === "following";
    watchedGeneration = snapshotGeneration();
    renderFollowChip();
    if (!followEnabled) return;
    setInterval(renderFollowChip, 1000);
    followLoop();
  }
  function toggleFollow() {
    if (!followEnabled) return;
    followPaused = !followPaused;
    renderFollowChip();
    if (!followPaused) refreshPendingFollow();
  }
  async function start() {
    try {
      const route = parseBrowserRoute();
      if (!route) throw new Error("invalid inspector URL");
      session = await api("/api/v1/session");
      updateSession(session);
      await loadVolumes(route);
      configureFollow();
    } catch (error) {
      renderWorkspaceError(error);
    }
  }
  async function loadVolumes(route = { kind: "root" }, historyMode = "restore") {
    const payload = await api(`${API_BASE}/volumes`),
      root = $("volumes"),
      volumes = payload.data.items;
    updateSession(payload);
    root.replaceChildren();
    for (const volume of volumes) {
      const node = button(
        `volume ${volume.vol_id} · ${volume.total_sectors} sectors`,
        () => selectVolume(volume),
        "nav",
      );
      node.dataset.volume = String(volume.vol_id);
      root.append(node);
    }
    if (!volumes.length) return;
    const volume =
      route.kind === "root"
        ? volumes[0]
        : volumes.find((value) => value.vol_id === route.vol);
    if (!volume)
      throw new Error("the URL volume does not exist in this revision");
    activateVolume(volume);
    if (route.kind === "root")
      await showVolume(historyMode === "none" ? "none" : "replace");
    else {
      await restoreBrowserRoute(route);
      if (historyMode !== "none") installBrowserRouteState(route);
    }
  }
  function invalidateVolumeView() {
    mapObserver.disconnect();
    volumeView = null;
    sectorCache.clear();
    sectorCursor = "end";
    loadedSectors = 0;
    loadEpoch++;
  }
  function activateVolume(volume) {
    currentVolume = volume;
    currentSector = null;
    currentPage = null;
    selectedPage = null;
    selectedSlot = null;
    invalidateVolumeView();
    document
      .querySelectorAll("#volumes .nav")
      .forEach((node) =>
        node.classList.toggle(
          "active",
          node.dataset.volume === String(volume.vol_id),
        ),
      );
  }
  async function selectVolume(volume, historyMode = "push") {
    activateVolume(volume);
    await showVolume(historyMode);
  }
  function hierarchyBack(parentPath, action) {
    if (history.state?.previous === parentPath) history.back();
    else action("replace");
  }
  function renderBreadcrumb(level) {
    currentLevel = level;
    const root = $("drillBreadcrumb"),
      route = browserRoute(level);
    root.replaceChildren();
    if (level !== "volume") {
      const parent = browserParentPath(route),
        actions = {
          sector: (mode) => showVolume(mode),
          page: (mode) => showSector(currentSector, mode),
          slot: (mode) => showPage(selectedPage, true, mode),
          oos: (mode) => showSlot(currentPage, selectedSlot, mode),
        };
      root.append(
        button("← Back", () => hierarchyBack(parent, actions[level]), "back"),
      );
    }
    root.append(button(`Volume ${currentVolume.vol_id}`, () => showVolume()));
    if (["sector", "page", "slot", "oos"].includes(level)) {
      root.append("›");
      root.append(
        button(`Sector ${currentSector.sector_id}`, () =>
          showSector(currentSector),
        ),
      );
    }
    if (["page", "slot", "oos"].includes(level)) {
      root.append("›");
      const page =
        level === "page"
          ? document.createElement("span")
          : button(`Page ${selectedPage}`, () => showPage(selectedPage, true));
      page.textContent = `Page ${selectedPage}`;
      root.append(page);
    }
    if (["slot", "oos"].includes(level)) {
      root.append("›");
      const slot =
        level === "slot"
          ? document.createElement("span")
          : button(`Slot ${selectedSlot}`, () =>
              showSlot(currentPage, selectedSlot),
            );
      slot.textContent = `Slot ${selectedSlot}`;
      root.append(slot);
    }
    if (level === "oos") {
      root.append("›");
      const oos = document.createElement("span");
      oos.textContent = "OOS chain";
      root.append(oos);
    }
  }
  function createVolumeView() {
    sectorCursor = null;
    loadedSectors = 0;
    const epoch = ++loadEpoch,
      root = document.createElement("section"),
      title = document.createElement("div"),
      map = document.createElement("div"),
      status = document.createElement("p");
    root.className = "volume-view";
    title.className = "workspace-title";
    title.innerHTML = `<div><h1>Volume ${currentVolume.vol_id} · full map</h1><p>${currentVolume.total_sectors} sectors · 64 pages per sector · revision ${session.snapshot.revision}</p></div><div id='legend' aria-label='Page allocation and occupancy legend'><span><i class='swatch unreserved'></i>Unreserved</span><span><i class='swatch reserved-unallocated'></i>Reserved, unallocated</span><span><i class='swatch allocated'></i>Occupied</span><span><i class='swatch free'></i>Slotted free</span><span><i class='swatch system-metadata'></i>System metadata</span><span><i class='swatch finding'></i>Finding outline</span></div>`;
    map.id = "volumeMap";
    map.setAttribute("aria-label", "Full volume sector map");
    status.id = "mapStatus";
    status.setAttribute("role", "status");
    status.textContent = "Loading sector maps…";
    root.append(title, map, status);
    volumeView = root;
    return epoch;
  }
  async function showVolume(historyMode = "push") {
    mapObserver.disconnect();
    currentSector = null;
    currentPage = null;
    selectedPage = null;
    selectedSlot = null;
    renderBreadcrumb("volume");
    let epoch = loadEpoch;
    if (!volumeView) epoch = createVolumeView();
    $("workspaceContent").replaceChildren(volumeView);
    syncBrowserRoute("volume", historyMode);
    if (sectorCursor === null && loadedSectors === 0)
      await loadSectorBatch(epoch);
    observeMapEnd();
  }
  async function loadSectorBatch(epoch = loadEpoch) {
    if (
      loadingEpoch === epoch ||
      sectorCursor === "end" ||
      epoch !== loadEpoch
    )
      return;
    loadingEpoch = epoch;
    try {
      const query =
          sectorCursor === null
            ? "?limit=24"
            : `?limit=24&cursor=${encodeURIComponent(sectorCursor)}`,
        payload = await api(
          `${API_BASE}/sectors/${currentVolume.vol_id}${query}`,
        );
      if (epoch !== loadEpoch) return;
      for (const sector of payload.data.items) appendSector(sector);
      loadedSectors += payload.data.items.length;
      sectorCursor =
        payload.data.next_cursor.state === "present"
          ? payload.data.next_cursor.value
          : "end";
      $("mapStatus").textContent =
        sectorCursor === "end"
          ? `All ${loadedSectors} sectors shown · ${loadedSectors * 64} pages`
          : `Showing ${loadedSectors} of ${currentVolume.total_sectors} sectors · scroll to continue`;
    } catch (error) {
      if (epoch === loadEpoch && error.code === "cursor-generation-changed") {
        if (followPaused) {
          sectorCursor = "end";
          $("mapStatus").textContent =
            "This paused generation is no longer retained · Resume to refresh the mosaic";
          return;
        }
        invalidateVolumeView();
        await showVolume("none");
        return;
      }
      if (epoch === loadEpoch) {
        sectorCursor = "end";
        $("mapStatus").textContent = error.message;
      }
    } finally {
      if (loadingEpoch === epoch) loadingEpoch = null;
    }
  }
  function pageOccupancyLabel(page) {
    return page.occupancy.state === "known"
      ? `, ${page.occupancy.occupied_percent}% occupied, ${page.occupancy.free_percent}% free`
      : ", occupancy unknown";
  }
  function applyPageFill(node, page) {
    if (page.allocation !== "allocated") return;
    if (page.occupancy.state === "known") {
      node.classList.add("occupancy-known");
      node.style.setProperty(
        "--occupied",
        `${page.occupancy.occupied_percent}%`,
      );
    } else node.classList.add("occupancy-unknown");
  }
  function classNameLabel(name) {
    if (name.state === "resolved") return name.value;
    if (name.state === "unresolved") return `unresolved (${name.reason})`;
    return `not applicable (${name.reason})`;
  }
  function attributeNameLabel(name) {
    if (name.state === "resolved") return name.value;
    return `unnamed (${name.reason})`;
  }
  function attributeValueLabel(value) {
    if (value.state === "decoded") return value.value;
    if (value.state === "null") return "NULL";
    if (value.state === "out-of-row")
      return `out of row · ${value.total_length} bytes`;
    return `withheld (${value.reason})`;
  }
  const RECORD_REGION_LABELS = {
    "object-header": "Object header",
    "offset-table": "Offset table",
    "fixed-region": "Fixed attributes",
    "bound-bits": "Bound bits",
    "variable-region": "Variable attributes",
  };
  // Where a record's bytes go, as proportional bands over its own length.
  // Widths come from the projection; the percentages are this view's choice.
  function renderRecordLayout(root, layout, attributes) {
    const total = Number(layout.record_length);
    if (!Number.isFinite(total) || total <= 0) return;
    const heading = document.createElement("h4");
    heading.textContent = `Record bytes (${total})`;
    const map = document.createElement("div");
    map.className = "record-map";
    map.setAttribute("aria-label", `Byte layout of a ${total}-byte record`);
    for (const region of layout.regions) {
      const length = Number(region.length);
      if (length <= 0) continue;
      const band = document.createElement("span"),
        share = (length / total) * 100,
        name = RECORD_REGION_LABELS[region.region] ?? region.region,
        label = `${name}: offset ${region.offset}, ${length} bytes, ${share.toFixed(1)}%`;
      band.className = `record-region region-${region.region}`;
      band.style.width = `${share}%`;
      band.title = label;
      band.setAttribute("aria-label", label);
      map.append(band);
    }
    const legend = document.createElement("div");
    legend.className = "record-legend";
    for (const region of layout.regions) {
      const length = Number(region.length);
      if (length <= 0) continue;
      const entry = document.createElement("span"),
        swatch = document.createElement("i"),
        text = document.createElement("span");
      swatch.className = `region-${region.region}`;
      text.textContent = `${RECORD_REGION_LABELS[region.region] ?? region.region} ${length} B (${((length / total) * 100).toFixed(1)}%)`;
      entry.append(swatch, text);
      legend.append(entry);
    }
    root.append(heading, map, legend);
    // The widest attributes are usually what a reader came to find out about.
    const ranked = attributes
      .map((attribute) => ({
        name: attributeNameLabel(attribute.name),
        length: Number(attribute.length),
      }))
      .filter((entry) => entry.length > 0)
      .sort((left, right) => right.length - left.length)
      .slice(0, 3);
    if (ranked.length) {
      const note = document.createElement("p");
      note.className = "record-largest";
      note.textContent = `Largest attributes: ${ranked
        .map(
          (entry) =>
            `${entry.name} ${entry.length} B (${((entry.length / total) * 100).toFixed(1)}%)`,
        )
        .join(" · ")}`;
      root.append(note);
    }
  }
  // A record's values, one row per attribute. Undecodable attributes state a
  // reason and their extent; no arm of this renders bytes.
  function renderInterpretation(root, page, slotId, data) {
    const schema = data.class_representation,
      interpretation = data.interpretation;
    if (!interpretation) {
      // A page that degraded as a whole says why; one merely not yet
      // interpreted offers the enrichment.
      if (data.interpretation_unavailable) {
        root.append(interpretationNote(data.interpretation_unavailable));
        return;
      }
      root.append(
        button("Interpret records", () => enrichRecords(page, slotId), "slot-action"),
      );
      return;
    }
    const heading = document.createElement("h3");
    heading.textContent = "Interpretation";
    root.append(heading);
    if (schema)
      root.append(
        fieldList([
          ["Class", classNameLabel(schema.class_name)],
          [
            "Representation",
            schema.is_current.state === "known"
              ? `${schema.representation_id} (${schema.is_current.value})`
              : schema.representation_id,
          ],
        ]),
      );
    if (interpretation.relocated_from.state === "present") {
      const origin = interpretation.relocated_from.oid;
      root.append(
        fieldList([
          [
            "Interpreted via relocation from",
            `${origin.vol_id}:${origin.page_id}:${origin.slot_id}`,
          ],
        ]),
      );
    }
    if (interpretation.diagnostic.state === "known") {
      root.append(interpretationNote(interpretation.diagnostic.value));
      return;
    }
    if (interpretation.layout)
      renderRecordLayout(root, interpretation.layout, interpretation.attributes);
    const table = document.createElement("table");
    table.className = "interpretation";
    const head = document.createElement("tr");
    for (const label of ["Attribute", "Type", "Bytes", "Value"]) {
      const cell = document.createElement("th");
      cell.textContent = label;
      if (label === "Bytes") cell.className = "record-bytes";
      head.append(cell);
    }
    table.append(head);
    for (const attribute of interpretation.attributes) {
      const row = document.createElement("tr"),
        name = document.createElement("td"),
        type = document.createElement("td"),
        size = document.createElement("td"),
        value = document.createElement("td");
      name.textContent = attributeNameLabel(attribute.name);
      type.textContent = attribute.type_name;
      size.className = "record-bytes";
      size.textContent = attribute.length;
      size.title = `${attribute.storage} region, offset ${attribute.offset}, ${attribute.length} bytes`;
      if (attribute.value.state === "out-of-row") {
        // The chain has its own view, but it only exists once validated, so
        // follow the same enrich-then-show path the OOS button uses.
        const head = attribute.value.head,
          label = `out of row · oos:${head.vol_id}:${head.page_id}:${head.slot_id} · ${attribute.value.total_length} bytes`,
          link = button(label, () => openOosChain(head), "oos-link");
        link.title = label;
        value.append(link);
      } else {
        value.textContent = attributeValueLabel(attribute.value);
        if (attribute.value.state !== "decoded") value.className = "withheld";
      }
      row.append(name, type, size, value);
      table.append(row);
    }
    root.append(table);
  }
  // Navigates to the OOS chain a stub references. A chain lives on the OOS
  // file's own page, which is usually a different volume and sector from the
  // record that points at it, so the workspace is moved there first — otherwise
  // the breadcrumb and the browser route would describe a page nobody loaded.
  async function openOosChain(head) {
    try {
      if (currentVolume?.vol_id !== head.vol_id) {
        const listing = await api(`${API_BASE}/volumes`),
          volume = listing.data.items.find(
            (candidate) => candidate.vol_id === head.vol_id,
          );
        if (volume) activateVolume(volume);
      }
      const payload = await api(`${API_BASE}/page/${head.vol_id}/${head.page_id}`);
      await enrichOos(payload.data.page, head.slot_id);
    } catch (error) {
      renderWorkspaceError(error);
    }
  }
  function interpretationNote(reason) {
    const note = document.createElement("p");
    note.className = "withheld";
    note.textContent = `not interpreted (${reason})`;
    return note;
  }
  // Only a heap page's data slots hold class instances. Slot 0 is the page's
  // own heap metadata, and other page types store their own structures, so
  // neither is something the interpreter can speak about.
  function interpretationScope(page, slot) {
    if (Number(slot.slot_id) === 0)
      return "slot 0 holds this page's own heap metadata, not a class instance — see the page's heap header facts above";
    if (page.page_type.state !== "known")
      return "this page's type is unknown, so its records cannot be attributed to a class";
    if (page.page_type.value !== "heap")
      return `records on a ${page.page_type.value} page are not class instances, so they carry no attribute values`;
    if (
      slot.record_type !== "home" &&
      slot.record_type !== "new-home" &&
      slot.record_type !== "relocation"
    )
      return `a ${slot.record_type} slot holds no interpretable record`;
    return null;
  }
  async function enrichRecords(page, slotId) {
    try {
      const refreshed = await enrichAndRefreshPage(
        `record:${page.vol_id}:${page.page_id}:${slotId}`,
        page,
        "none",
      );
      if (refreshed) await showSlot(refreshed, slotId, "push");
    } catch (error) {
      renderWorkspaceError(error);
    }
  }
  function sectorAttributionLabel(sector) {
    const attribution = sector.attribution;
    if (!attribution || attribution.state === "unclaimed") return "";
    if (attribution.state === "mixed") return "mixed";
    const file = attribution.file;
    if (file.class_name.state === "resolved") return file.class_name.value;
    if (file.class_oid.state === "present") return "unresolved";
    return "internal";
  }
  function sectorFileTypeLabel(sector) {
    const attribution = sector.attribution;
    return attribution?.state === "single" &&
      attribution.file.file_type.state === "known"
      ? attribution.file.file_type.value
      : "";
  }
  function sectorAttributionDetail(sector) {
    const attribution = sector.attribution;
    if (!attribution || attribution.state === "unclaimed") return "";
    if (attribution.state === "mixed")
      return `mixed: ${attribution.claims.length} conflicting file claims`;
    const file = attribution.file,
      role =
        file.file_type.state === "known" ? file.file_type.value : "unavailable";
    return `${sectorAttributionLabel(sector)} · ${role} · ${attribution.allocated_pages}/64 allocated`;
  }
  function fileAssociationRows(fileAssociation) {
    if (fileAssociation.state === "none") return [["File", "none"]];
    if (fileAssociation.state === "mixed-claims")
      return [["File", "mixed claims"]];
    const file = fileAssociation.file,
      rows = [
        [
          "File",
          `file:${file.vol_id}:${file.file_id}${fileAssociation.state === "reserved-for" ? " (reserved, not allocated)" : ""}`,
        ],
        [
          "File role",
          file.file_type.state === "known"
            ? file.file_type.value
            : "unavailable",
        ],
      ];
    if (file.class_oid.state === "present") {
      const oid = file.class_oid.oid;
      rows.push([
        "Class OID",
        `oid:${oid.vol_id}:${oid.page_id}:${oid.slot_id}`,
      ]);
    }
    rows.push(["Class/table", classNameLabel(file.class_name)]);
    return rows;
  }
  function appendSector(sector) {
    if (sector.pages.length !== 64)
      throw new Error(`sector ${sector.sector_id} did not contain 64 pages`);
    sectorCache.set(sector.sector_id, sector);
    const card = button("", () => showSector(sector), "sector-card");
    card.id = `sector-${sector.sector_id}`;
    const tableLabel = sectorAttributionLabel(sector),
      fileTypeLabel = sectorFileTypeLabel(sector);
    card.setAttribute(
      "aria-label",
      `Sector ${sector.sector_id}, ${sector.reserved ? "reserved" : "unreserved"}${tableLabel ? `, ${tableLabel}` : ""}${fileTypeLabel ? `, file type ${fileTypeLabel}` : ""}, 64 pages`,
    );
    const heading = document.createElement("span");
    heading.className = "sector-heading";
    const title = document.createElement("strong");
    title.textContent = `Sector ${sector.sector_id}`;
    const state = document.createElement("span");
    state.textContent = sector.reserved ? "reserved" : "unreserved";
    heading.append(title, state);
    if (tableLabel) {
      const table = document.createElement("em");
      table.className = "sector-table";
      table.textContent = tableLabel;
      table.title = sectorAttributionDetail(sector);
      heading.append(table);
    }
    if (fileTypeLabel) {
      const fileType = document.createElement("small");
      fileType.className = "sector-file-type";
      fileType.textContent = fileTypeLabel;
      heading.append(fileType);
    }
    const pages = document.createElement("span");
    pages.className = "sector-preview-pages";
    for (const page of sector.pages) {
      const finding = page.diagnostic.state === "known",
        node = document.createElement("i");
      node.className = `page preview-page ${page.allocation}${finding ? " finding" : ""}`;
      applyPageFill(node, page);
      pages.append(node);
    }
    card.append(heading, pages);
    volumeView.querySelector("#volumeMap").append(card);
  }
  function moveSectorGrid(event, grid, index) {
    let next = index;
    if (event.key === "ArrowLeft") next--;
    else if (event.key === "ArrowRight") next++;
    else if (event.key === "ArrowUp") next -= 8;
    else if (event.key === "ArrowDown") next += 8;
    else return;
    if (next >= 0 && next < 64) {
      event.preventDefault();
      grid.children[next].focus();
    }
  }
  const mapObserver = new IntersectionObserver((entries) => {
    if (entries.some((entry) => entry.isIntersecting)) {
      mapObserver.disconnect();
      loadSectorBatch().then(observeMapEnd);
    }
  });
  function observeMapEnd() {
    mapObserver.disconnect();
    if (currentLevel !== "volume" || !volumeView) return;
    const old = volumeView.querySelector("#mapSentinel");
    if (old) old.remove();
    if (sectorCursor === "end") return;
    const sentinel = document.createElement("div");
    sentinel.id = "mapSentinel";
    volumeView.querySelector("#volumeMap").append(sentinel);
    mapObserver.observe(sentinel);
  }
  function showSector(sector, historyMode = "push") {
    mapObserver.disconnect();
    currentSector = sector;
    currentPage = null;
    selectedPage = null;
    selectedSlot = null;
    renderBreadcrumb("sector");
    const content = $("workspaceContent"),
      title = document.createElement("div"),
      focus = document.createElement("section"),
      grid = document.createElement("div");
    title.className = "workspace-title";
    const attributionDetail = sectorAttributionDetail(sector);
    title.innerHTML = `<div><h1>Sector ${sector.sector_id}</h1><p>64 physical pages · select a page to enlarge</p></div>`;
    if (attributionDetail) {
      const note = document.createElement("p");
      note.className = "muted";
      note.textContent = attributionDetail;
      title.firstElementChild.append(note);
    }
    focus.className = "sector-focus";
    grid.className = "sector-focus-grid";
    grid.setAttribute("role", "grid");
    grid.setAttribute(
      "aria-label",
      `Sector ${sector.sector_id}, 64 physical pages`,
    );
    sector.pages.forEach((page, index) => {
      const finding = page.diagnostic.state === "known",
        node = button(
          "",
          () => showPage(page.page_id),
          `page focus-page ${page.allocation}${finding ? " finding" : ""}${page.page_id === selectedPage ? " selected" : ""}`,
        ),
        kind = document.createElement("span"),
        identity = document.createElement("span");
      kind.className = "page-kind";
      kind.textContent =
        page.page_type.state === "known"
          ? page.page_type.value
          : "not inspected";
      identity.className = "page-id";
      identity.textContent = String(page.page_id);
      node.append(kind, identity);
      applyPageFill(node, page);
      node.setAttribute("role", "gridcell");
      node.setAttribute(
        "aria-label",
        `Page ${page.page_id}, ${page.allocation}${pageOccupancyLabel(page)}${finding ? ", finding" : ""}`,
      );
      node.onkeydown = (event) => moveSectorGrid(event, grid, index);
      grid.append(node);
    });
    focus.append(grid);
    content.replaceChildren(title, focus);
    syncBrowserRoute("sector", historyMode);
  }
  function withheld(identity) {
    const note = document.createElement("p");
    note.className = "withheld";
    note.textContent = `evidence ${identity} · structural ranges only · bytes withheld`;
    return note;
  }
  async function ensureSector(sectorId) {
    if (currentSector?.sector_id === sectorId) return;
    const payload = await api(
      `${API_BASE}/sector/${currentVolume.vol_id}/${sectorId}`,
    );
    currentSector = payload.data;
  }
  async function showPage(
    pageId,
    skipEnrichment = false,
    historyMode = "push",
  ) {
    try {
      const payload = await api(
          `${API_BASE}/page/${currentVolume.vol_id}/${pageId}`,
        ),
        page = payload.data.page,
        deep = payload.data.deep;
      await ensureSector(page.sector_id);
      currentPage = page;
      selectedPage = page.page_id;
      selectedSlot = null;
      const shouldEnrich =
        !skipEnrichment &&
        deep.state === "not-enriched" &&
        page.detail_support.state === "known";
      renderPageWorkspace(payload, shouldEnrich);
      syncBrowserRoute("page", historyMode);
      if (shouldEnrich) await enrichSelectedPage(page);
      return page;
    } catch (error) {
      renderWorkspaceError(error);
      return null;
    }
  }
  function appendPrimitiveStructure(root, deep) {
    if (!deep.structure) return;
    const fields = [];
    for (const [name, value] of Object.entries(deep.structure)) {
      if (
        name === "slots" ||
        name === "bytes" ||
        value === null ||
        typeof value === "object"
      )
        continue;
      fields.push([name.replaceAll("_", " "), value]);
    }
    if (fields.length) {
      const title = document.createElement("h3");
      title.textContent = "Decoded structure";
      root.append(title, fieldList(fields));
    }
  }
  function renderPageWorkspace(payload, enriching = false) {
    const page = payload.data.page,
      deep = payload.data.deep,
      slots = payload.data.slots,
      distribution = payload.data.distribution,
      content = $("workspaceContent"),
      title = document.createElement("div"),
      layout = document.createElement("div"),
      facts = document.createElement("section"),
      distributionPanel = document.createElement("section");
    renderBreadcrumb("page");
    title.className = "workspace-title";
    const fileAssociation = page.file_association,
      pageTable =
        (fileAssociation.state === "allocated" ||
          fileAssociation.state === "reserved-for") &&
        fileAssociation.file.class_name.state === "resolved"
          ? fileAssociation.file.class_name.value
          : "";
    title.innerHTML = `<div><h1>Page ${page.page_id}</h1><p>${page.page_type.state === "known" ? page.page_type.value : "unknown type"} · detailed structural view</p></div>`;
    if (pageTable) {
      const note = document.createElement("p");
      note.className = "muted";
      note.textContent = pageTable;
      title.firstElementChild.append(note);
    }
    layout.className = "page-workspace";
    facts.className = "panel";
    const factsTitle = document.createElement("h2");
    factsTitle.textContent = "Page facts";
    facts.append(
      factsTitle,
      fieldList([
        ["Identity", `page:${page.vol_id}:${page.page_id}`],
        ["Sector", page.sector_id],
        [
          "Physical type",
          page.page_type.state === "known"
            ? page.page_type.value
            : "not inspected",
        ],
        ["Allocation", page.allocation],
        ...fileAssociationRows(page.file_association),
        ["Availability", page.availability],
        [
          "Detail support",
          page.detail_support.state === "known"
            ? page.detail_support.value
            : page.detail_support.state,
        ],
        ["Deep state", deep.state],
        ["TDE", page.tde_state],
      ]),
    );
    appendPrimitiveStructure(facts, deep);
    facts.append(withheld(`page:${page.vol_id}:${page.page_id}`));
    distributionPanel.className = "panel page-distribution";
    if (distribution.state === "available")
      distributionPanel.append(
        renderSlottedDistribution(slots, distribution, button, (slotId) =>
          showSlot(page, slotId),
        ),
      );
    else {
      const slotsTitle = document.createElement("h2"),
        note = document.createElement("p");
      slotsTitle.textContent = "Slotted-page distribution";
      note.className = "muted";
      note.textContent = enriching
        ? "Loading structural metadata…"
        : "No validated slot directory is available for this page.";
      distributionPanel.append(slotsTitle, note);
    }
    if (enriching) {
      const note = document.createElement("p");
      note.className = "status-note";
      note.setAttribute("role", "status");
      note.textContent =
        "Enriching the selected page at a new immutable revision…";
      facts.append(note);
    }
    layout.append(facts, distributionPanel);
    content.replaceChildren(title, layout);
  }
  function slotTable(page, slots) {
    const table = document.createElement("table"),
      head = document.createElement("thead"),
      body = document.createElement("tbody"),
      header = document.createElement("tr");
    table.className = "slot-table";
    for (const label of ["Slot", "Record type", "Offset", "Size (bytes)", ""]) {
      const cell = document.createElement("th");
      cell.textContent = label;
      header.append(cell);
    }
    head.append(header);
    for (const slot of slots) {
      const row = document.createElement("tr");
      for (const value of [
        slot.slot_id,
        slot.record_type,
        slot.offset,
        slot.length,
      ]) {
        const cell = document.createElement("td");
        cell.textContent = String(value);
        row.append(cell);
      }
      const action = document.createElement("td");
      action.append(
        button("Inspect", () => showSlot(page, slot.slot_id), "slot-action"),
      );
      row.append(action);
      body.append(row);
    }
    table.append(head, body);
    return table;
  }
  async function enrichAndRefreshPage(selector, page, historyMode) {
    const receipt = await api(`${API_BASE}/enrichments`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ selector }),
    });
    updateSession(receipt);
    invalidateVolumeView();
    const sectorPayload = await api(
      `${API_BASE}/sector/${page.vol_id}/${page.sector_id}`,
    );
    currentSector = sectorPayload.data;
    return showPage(page.page_id, true, historyMode);
  }
  async function enrichSelectedPage(page) {
    try {
      await enrichAndRefreshPage(
        `page:${page.vol_id}:${page.page_id}`,
        page,
        "push",
      );
    } catch (error) {
      renderWorkspaceError(error);
    }
  }
  async function showSlot(page, slotId, historyMode = "push") {
    try {
      const payload = await api(
          `${API_BASE}/slot/${page.vol_id}/${page.page_id}/${slotId}`,
        ),
        slot = payload.data.selected_slot,
        root = document.createElement("section");
      currentPage = page;
      selectedPage = page.page_id;
      selectedSlot = slot.slot_id;
      renderBreadcrumb("slot");
      root.id = "slotDetail";
      root.className = "panel slot-detail";
      const title = document.createElement("h2");
      title.textContent = `Slot ${slot.slot_id}`;
      root.append(
        title,
        fieldList([
          ["Identity", `slot:${page.vol_id}:${page.page_id}:${slot.slot_id}`],
          ["Record type", `${slot.record_type} (${slot.record_type_ordinal})`],
          ["Offset", slot.offset],
          ["Size", slot.length],
        ]),
      );
      if (
        page.page_type.state === "known" &&
        page.page_type.value === "oos" &&
        Number(slot.offset) > 0 &&
        slot.record_type === "home"
      )
        root.append(
          button("Validate OOS chain", () => enrichOos(page, slot.slot_id)),
        );
      // A relocation's own content is its forward reference; show it whether or
      // not the target has been interpreted yet.
      if (payload.data.relocation_edge) {
        const edge = payload.data.relocation_edge,
          target =
            edge.target.state === "present"
              ? `${edge.target.oid.vol_id}:${edge.target.oid.page_id}:${edge.target.oid.slot_id}`
              : "unknown";
        root.append(
          fieldList([
            ["Relocated to", target],
            ["Edge valid", edge.valid],
          ]),
        );
      }
      const outOfScope = interpretationScope(page, slot);
      if (outOfScope) root.append(interpretationNote(outOfScope));
      else renderInterpretation(root, page, slot.slot_id, payload.data);
      root.append(
        withheld(`slot:${page.vol_id}:${page.page_id}:${slot.slot_id}`),
      );
      const old = $("slotDetail");
      if (old) old.remove();
      document.querySelector(".page-workspace").append(root);
      syncBrowserRoute("slot", historyMode);
      return slot;
    } catch (error) {
      renderWorkspaceError(error);
      return null;
    }
  }
  function renderOosChain(page, slotId, chain) {
    currentPage = page;
    selectedPage = page.page_id;
    selectedSlot = slotId;
    renderBreadcrumb("oos");
    const root = document.createElement("section"),
      title = document.createElement("h2");
    root.id = "slotDetail";
    root.className = "panel slot-detail";
    title.textContent = "OOS chain";
    root.append(
      title,
      fieldList([
        ["Identity", `oos:${page.vol_id}:${page.page_id}:${slotId}`],
        ["Complete", chain.complete],
        ["Validated bytes", chain.validated_payload_bytes],
        ["Chunks", chain.chunks.length],
        [
          "Diagnostic",
          chain.diagnostic.state === "known" ? chain.diagnostic.value : "none",
        ],
      ]),
    );
    root.append(withheld(`oos:${page.vol_id}:${page.page_id}:${slotId}`));
    const old = $("slotDetail");
    if (old) old.remove();
    document.querySelector(".page-workspace").append(root);
  }
  async function showOos(page, slotId, historyMode = "push") {
    try {
      const payload = await api(
        `${API_BASE}/oos/${page.vol_id}/${page.page_id}/${slotId}`,
      );
      renderOosChain(page, slotId, payload.data.chain);
      syncBrowserRoute("oos", historyMode);
      return payload.data.chain;
    } catch (error) {
      renderWorkspaceError(error);
      return null;
    }
  }
  async function enrichOos(page, slotId) {
    try {
      const refreshed = await enrichAndRefreshPage(
        `oos:${page.vol_id}:${page.page_id}:${slotId}`,
        page,
        "none",
      );
      if (refreshed) await showOos(refreshed, slotId, "push");
    } catch (error) {
      renderWorkspaceError(error);
    }
  }
  async function restoreBrowserRoute(route) {
    if (route.kind === "volume") {
      await showVolume("none");
      return;
    }
    if (route.kind === "sector") {
      const payload = await api(
        `${API_BASE}/sector/${route.vol}/${route.sector}`,
      );
      showSector(payload.data, "none");
      return;
    }
    const page = await showPage(route.page, true, "none");
    if (!page) return;
    if (route.kind === "slot") await showSlot(page, route.slot, "none");
    if (route.kind === "oos") await showOos(page, route.slot, "none");
  }
  async function restoreBrowserLocation() {
    const epoch = ++routeEpoch;
    try {
      const route = parseBrowserRoute();
      if (!route) throw new Error("invalid inspector URL");
      // A restored URL names an entity, so it is read at whatever reading is
      // current instead of being checked against the one it was copied from.
      if (route.kind === "root") {
        session = await api("/api/v1/session");
        updateSession(session);
      }
      if (epoch === routeEpoch) await loadVolumes(route);
    } catch (error) {
      if (epoch === routeEpoch) renderWorkspaceError(error);
    }
  }
  function renderWorkspaceError(error) {
    const old = document.querySelector(".error-note"),
      note = document.createElement("section"),
      title = document.createElement("strong"),
      message = document.createElement("span"),
      detail = document.createElement("small");
    if (old) old.remove();
    note.className = "status-note error-note";
    note.setAttribute("role", "alert");
    title.textContent = "Could not complete this view";
    message.textContent = error.message;
    detail.textContent = error.status
      ? `HTTP ${error.status} · ${error.code || "unknown-error"}`
      : `Browser error · ${error.code || "unknown-error"}`;
    note.append(title, message, detail);
    $("workspaceContent").append(note);
  }
  async function showLicenses() {
    const payload = await api("/api/v1/licenses");
    $("infoContent").textContent = payload.notice;
    $("infoDialog").showModal();
  }
  window.addEventListener("popstate", () => {
    if (session) restoreBrowserLocation();
  });
  $("closeInfo").addEventListener("click", () => $("infoDialog").close());
  $("licenses").addEventListener("click", showLicenses);
  $("followToggle").addEventListener("click", toggleFollow);
  start();
})();
