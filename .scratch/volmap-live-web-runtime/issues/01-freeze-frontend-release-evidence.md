# 01: Freeze frontend and release evidence

**What to build:** Establish the exact JavaScript toolchain, generated-asset boundary, supply-chain evidence, and executable browser harness required by W0 in the [implementation specification](../implementation-spec.md). This ticket changes no production viewer behavior.

**Blocked by:** None.

**Status:** implemented

- [x] Exact Node, pnpm, TypeScript, Vite, React, React DOM, unit-test, Playwright Chromium, and Playwright Firefox versions are repository-pinned with an immutable lockfile and reviewed lifecycle-script policy.
- [x] A minimal TypeScript entry builds into a deterministic generated directory and manifest suitable for `include_bytes!`/`include_str!`; generated outputs are committed.
- [x] Ordinary Cargo builds embed committed assets without Node, pnpm, a package registry, or a writable source tree.
- [x] One repository-owned regeneration/check command installs immutably, builds twice from clean archives, and byte-compares generated assets.
- [x] Runtime-bundled npm components enter product notices and the artifact CycloneDX SBOM; build-only components enter a separate provenance, license, and advisory report.
- [x] A real Rust test server can be launched under pinned Playwright Chromium; a blocking Firefox smoke launcher is available.
- [x] Browser assertions have a semantic-role/accessibility convention and do not rely on screenshots or incidental CSS structure for acceptance.
- [x] CSP, no-store, offline/same-origin, static-musl, distribution, and deterministic HTML-export regression checks pass unchanged.
