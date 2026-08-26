# 02: Establish the React compatibility viewer

**What to build:** Replace the served hand-written live viewer with a React/TypeScript implementation that preserves every current live behavior before adding new byte or runtime features. Follow W1 in the [implementation specification](../implementation-spec.md) and ADR-0003.

**Blocked by:** 01: Freeze frontend and release evidence.

**Status:** ready-for-agent

- [ ] React serves through all existing entity routes and same-origin asset routes; direct loads and browser back/forward restore the same semantic entity.
- [ ] One pure reducer and deterministic selectors own navigation, progressive collection loading, enrichment, live-follow generation offers, pause/resume, and fetch adoption.
- [ ] HTTP, timer, visibility, abort, and History API work lives in effect adapters rather than components or the reducer.
- [ ] Volume, Sector, Page, Slot, OOS, relationship, diagnostic, coverage, license, loading, error, and invalidation states retain current facts and reachability.
- [ ] Existing generation and cursor conflict behavior remains exact; late collection and enrichment responses cannot overwrite a newer route/generation.
- [ ] The React application performs no CUBRID byte arithmetic and has no runtime-observation concepts yet.
- [ ] Current Rust web tests plus blocking Chromium parity and Firefox bootstrap/navigation smoke pass against the actual Rust server.
- [ ] The deterministic HTML export, TUI, CLI, JSON/JSONL, schemas, disclosure, CSP, notices, and release behavior remain unchanged.
