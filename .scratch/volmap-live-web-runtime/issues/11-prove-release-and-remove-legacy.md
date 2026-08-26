# 11: Prove release and remove the legacy live viewer

**What to build:** Run W10's complete compatibility, runtime, disclosure, supply-chain, reproducibility, and distribution evidence on one exact candidate; then delete the obsolete hand-written live viewer and source-text-only acceptance tests.

**Blocked by:** 10: Close runtime scheduling, density, and accessibility and every earlier ticket's blocking evidence.

**Status:** ready-for-agent

- [ ] `WEB-PROJECTION`, `WEB-MODEL`, `WEB-HTTP`, `WEB-BROWSER`, `WEB-FIREFOX`, `WEB-KERNEL`, `WEB-PRODUCER`, `WEB-DISCLOSURE`, and `WEB-RELEASE` name and pass the same exact candidate.
- [ ] Existing Rust unit/integration, TUI, CLI, JSON/JSONL, live-follow, deterministic HTML-export, CSP, static-musl, and Debian/Rocky/Alpine gates pass unchanged outside accepted live-viewer additions.
- [ ] React browser tests cover direct routes, history, progressive loads, generation conflicts, byte selection, both runtime sources, resident comparison, pause, visibility, restart, partial coverage, and accessibility.
- [ ] The complete npm graph passes exact-lock, registry/source, lifecycle, advisory, license, notices, SBOM, and build-provenance review.
- [ ] Two clean archives regenerate byte-identical web assets and byte-identical final static executables; committed generated assets have no diff.
- [ ] Disclosure sentinels prove safe structural facts and absence of raw payloads, plaintext values outside existing explicit disclosure, TDE material, paths, producer-private state, and source-map leakage.
- [ ] Production routes serve only the generated React application; the old `app.js`, global helper scripts, obsolete CSS, and replaced source-string tests are deleted rather than retained as a second implementation.
- [ ] HTML export remains its separate frozen renderer and byte determinism is proven; no Node requirement enters ordinary Cargo builds or runtime deployment.
- [ ] No generic state/query/visualization framework, remote runtime listener, runtime graph fact, event history, or forbidden unsafe code remains.
