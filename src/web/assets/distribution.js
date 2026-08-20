(() => {
  "use strict";

  function distributionLegend() {
    const root = document.createElement("div");
    root.className = "distribution-legend";
    for (const [kind, label] of [
      ["header", "Slotted header"],
      ["record", "Allocated record"],
      ["fragmented-free", "Fragmented free"],
      ["contiguous-free", "Contiguous free"],
      ["slot-directory", "Slot directory"],
    ]) {
      const item = document.createElement("span"),
        swatch = document.createElement("i");
      swatch.className = `region-${kind}`;
      item.append(swatch, label);
      root.append(item);
    }
    return root;
  }
  function distributionRegions(distribution) {
    const regions = [
      { ...distribution.header, kind: "header", label: "Slotted-page header" },
      ...distribution.record_extents.map((record) => ({
        ...record,
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
    ];
    regions.sort(
      (left, right) => left.offset - right.offset || left.length - right.length,
    );
    return regions;
  }
  function distributionMetric(value, label) {
    const node = document.createElement("div"),
      number = document.createElement("strong"),
      caption = document.createElement("span");
    node.className = "distribution-metric";
    number.textContent = String(value);
    caption.textContent = label;
    node.append(number, caption);
    return node;
  }
  function render(slots, distribution, createButton, inspectSlot) {
    const root = document.createDocumentFragment(),
      title = document.createElement("h2"),
      summary = document.createElement("div"),
      map = document.createElement("div"),
      axis = document.createElement("div"),
      regions = distributionRegions(distribution),
      slotById = new Map(slots.map((slot) => [slot.slot_id, slot])),
      notAllocated = distribution.slot_entries.filter(
        (entry) => entry.state !== "allocated",
      ).length;
    title.textContent = "Full slotted-page distribution";
    summary.className = "distribution-summary";
    summary.append(
      distributionMetric(
        distribution.record_extents.length,
        "allocated records",
      ),
      distributionMetric(notAllocated, "slots not allocated"),
      distributionMetric(distribution.free_regions.length, "free byte regions"),
      distributionMetric(
        `${distribution.unoccupied_bytes} B`,
        "unoccupied bytes",
      ),
    );
    map.className = "full-page-map";
    map.setAttribute(
      "aria-label",
      `Complete ${distribution.content_size}-byte slotted-page content map`,
    );
    for (const region of regions) {
      const node =
          region.kind === "record"
            ? createButton("", () => inspectSlot(region.slot_id))
            : document.createElement("span"),
        end = region.offset + region.length,
        label = `${region.label}: offset ${region.offset}, size ${region.length} bytes, end ${end}`;
      node.className = `page-region region-${region.kind}`;
      node.style.left = `${(region.offset / distribution.content_size) * 100}%`;
      node.style.width = `${(region.length / distribution.content_size) * 100}%`;
      node.title = label;
      node.setAttribute("aria-label", label);
      map.append(node);
    }
    axis.className = "page-map-axis";
    for (const value of [
      0,
      Math.floor(distribution.content_size / 4),
      Math.floor(distribution.content_size / 2),
      Math.floor((distribution.content_size * 3) / 4),
      distribution.content_size,
    ]) {
      const tick = document.createElement("span");
      tick.textContent = String(value);
      axis.append(tick);
    }
    root.append(
      title,
      summary,
      distributionLegend(),
      map,
      axis,
      regionList(regions, distribution.content_size, inspectSlot),
      slotDirectory(
        distribution.slot_entries,
        slotById,
        createButton,
        inspectSlot,
      ),
    );
    return root;
  }
  function sectionTitle(title, caption) {
    const root = document.createElement("div"),
      heading = document.createElement("h3"),
      detail = document.createElement("span");
    root.className = "distribution-section-title";
    heading.textContent = title;
    detail.className = "muted";
    detail.textContent = caption;
    root.append(heading, detail);
    return root;
  }
  function regionList(regions, contentSize, inspectSlot) {
    const wrapper = document.createElement("section"),
      list = document.createElement("div");
    list.className = "region-list";
    for (const region of regions) {
      const row = document.createElement("div"),
        name = document.createElement("span"),
        swatch = document.createElement("i"),
        label = document.createElement("span"),
        range = document.createElement("span"),
        size = document.createElement("span"),
        lane = document.createElement("span"),
        extent = document.createElement("i"),
        end = region.offset + region.length;
      row.className = "region-row";
      name.className = "region-name";
      swatch.className = `region-${region.kind}`;
      label.textContent = region.label;
      name.append(swatch, label);
      range.className = "region-range";
      range.textContent = `${region.offset}–${end}`;
      size.className = "region-size";
      size.textContent = `${region.length} B`;
      lane.className = "region-lane";
      extent.className = `region-${region.kind}`;
      extent.style.left = `${(region.offset / contentSize) * 100}%`;
      extent.style.width = `${(region.length / contentSize) * 100}%`;
      lane.append(extent);
      row.append(name, range, size, lane);
      if (region.kind === "record") {
        row.tabIndex = 0;
        row.setAttribute("role", "button");
        row.setAttribute("aria-label", `Inspect ${region.label}`);
        row.onclick = () => inspectSlot(region.slot_id);
        row.onkeydown = (event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            inspectSlot(region.slot_id);
          }
        };
      }
      list.append(row);
    }
    wrapper.append(
      sectionTitle(
        "Physical intervals",
        `${regions.length} exhaustive non-overlapping regions`,
      ),
      list,
    );
    return wrapper;
  }
  function slotDirectory(entries, slotById, createButton, inspectSlot) {
    const wrapper = document.createElement("section"),
      grid = document.createElement("div");
    grid.className = "slot-directory-grid";
    for (const entry of entries) {
      const slot = slotById.get(entry.slot_id),
        node = createButton(
          "",
          () => inspectSlot(entry.slot_id),
          `slot-entry ${entry.state}`,
        ),
        name = document.createElement("strong"),
        state = document.createElement("span"),
        kind = document.createElement("small"),
        directory = document.createElement("small"),
        record = document.createElement("small");
      name.textContent = `Slot ${entry.slot_id}`;
      state.className = "slot-state";
      state.textContent =
        entry.state === "allocated"
          ? "allocated"
          : entry.state === "deleted"
            ? "deleted · not allocated"
            : "not allocated";
      kind.textContent = `record type · ${entry.record_type}`;
      directory.textContent = `directory · ${entry.offset}–${entry.offset + entry.length} (${entry.length} B)`;
      record.textContent =
        slot && Number(slot.offset) > 0
          ? `record · ${slot.offset}–${Number(slot.offset) + Number(slot.length)} (${slot.length} B)`
          : `record · no live extent${slot && Number(slot.length) > 0 ? ` · retained length ${slot.length} B` : ""}`;
      node.append(name, state, kind, directory, record);
      grid.append(node);
    }
    wrapper.append(
      sectionTitle(
        "Slot directory",
        `${entries.length} entries · allocated, empty, and deleted`,
      ),
      grid,
    );
    return wrapper;
  }

  window.volmapDistribution = Object.freeze({ render });
})();
