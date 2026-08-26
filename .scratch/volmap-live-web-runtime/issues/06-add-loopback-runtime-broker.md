# 06: Add the loopback runtime broker and HTTP boundary

**What to build:** Implement W5's private Volmap runtime broker, explicit serve configuration, bounded same-origin resources, and adapter interfaces. Use disabled/simulated adapters first; producer and kernel implementations land later.

**Blocked by:** 05: Add the runtime model and deterministic simulator.

**Status:** ready-for-agent

- [ ] `serve` gains explicit runtime enablement and explicit producer-socket configuration with no discovery or environment fallback.
- [ ] Runtime attachment with any non-loopback HTTP bind fails at startup; ordinary unattached serving keeps its existing bind and unauthenticated inspection behavior.
- [ ] The broker owns database/volume identity, incarnation, deadlines, concurrency, page/byte/time caps, cancellation, and per-source capability mapping.
- [ ] Separate no-store resources exist for capabilities, page-buffer state observation, explicit resident inspection, and kernel-cache observation.
- [ ] Requests and responses carry exact scope/epoch echoes, selected and ordered VPIDs, requested/evaluated/budget counts, rotation continuation, source capture time, method, and limitations.
- [ ] Graph resources, runtime resources, source caches, and error types remain separate; runtime failures cannot mutate or block inspection revisions/generations.
- [ ] HTTP errors distinguish disabled, socket unavailable, peer refused, identity mismatch, protocol incompatibility, deadline, and resource limit.
- [ ] CSP remains same-origin without inline/eval/WebSocket/producer access; path and structural-only disclosure tests cover HTTP bodies and logs.
- [ ] Axum tests prove loopback policy, caps, cancellation, direct routes, no-store, error mapping, and unaffected ordinary inspection.
