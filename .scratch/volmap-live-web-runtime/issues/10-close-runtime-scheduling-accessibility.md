# 10: Close runtime scheduling, density, and accessibility

**What to build:** Complete W9's production polling schedules, bounded observation coverage, visual encoding decision, large-volume resource behavior, and accessibility contract across both runtime sources.

**Blocked by:** 04: Add attribute selection and cross-highlighting; 05: Add the runtime model and deterministic simulator; 07: Add Linux page-cache residency; 08: Consume CUBRID state-only page-buffer observations; 09: Add selected resident-page inspection and correspondence.

**Status:** ready-for-agent

- [ ] Selected-page page-buffer cadence defaults to 500 ms and visible-page/kernel cadence to 2 s under a controlled scheduler.
- [ ] Hidden documents stop new runtime work; paused displays retain only the latest offer per source; visible/resume schedules fresh coherent adoption.
- [ ] Failed requests use bounded exponential backoff with jitter and recover without hiding explicit source age/capability.
- [ ] A hard browser request cap of 512 pages prioritizes selected, then nearest visible-sector pages, then physical order; the server may enforce a lower explicit cap.
- [ ] Requested/evaluated/budget counts and rotation are visible whenever coverage is partial; subsequent batches rotate only the non-selected portion and never sample silently.
- [ ] Only latest accepted per-page/source/incarnation state is retained; work and memory stay bounded independently of volume size.
- [ ] Border+badge is the default overlay encoding unless controlled contrast/density evidence selects another accepted prototype variant; allocation/occupancy/finding semantics stay visible.
- [ ] Every state has glyph/text, accessible page names and contextual detail; no color-only distinction exists across normal, reduced-motion, and high-contrast use.
- [ ] Keyboard-only users can reach page, attribute, overlay, resident request, limitations, and coverage controls; focus survives compatible refresh/virtualization.
- [ ] Live announcements are limited to committed selection, pause, capability, restart/handshake, and correspondence changes; polling and age ticks remain quiet.
- [ ] Controlled browser/model/resource tests cover cap boundaries, rotating batches, out-of-order completion, viewport churn, backoff, visibility, pause, reduced motion, and screen-reader text.
