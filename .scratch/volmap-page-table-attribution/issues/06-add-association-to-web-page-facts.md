Type: task
Status: resolved
Blocked by: 05

# Add association to the web Page facts

## Question

Can the existing web Page workspace show the shared production association clearly without changing its visual design or inventing adapter-specific semantics?

Add `Class/table`, `Class OID`, `File`, and `File role` rows to the existing Page facts panel, consuming only the shared Page projection. Render resolved, non-applicable/internal/deferred, and unresolved states truthfully; keep numeric identities visible when names fail; safely escape identifier text; and preserve current layout, navigation, page detail enrichment, JSON API, and standalone HTML behavior. Do not add sector labels/cards or redesign the workspace. Add focused web API/DOM-source/HTML-export tests for resolved and every fail-closed state, long/non-ASCII names, escaping, and unchanged existing facts. Format, run the focused web tests, and commit the ticket's production change on `feat/page-table-attribution`.

## Answer

Yes. The production React Page facts panel now always renders `File`, `File role`, `Class OID`, and `Class/table` from the shared `PageProjection.file_association`. Resolved identifiers render as text; unresolved names keep their exact numeric OID visible; internal, no-single-class, null-OID, deferred-OOS, incomplete-inventory, mixed-claim, reserved-for, and unowned states remain explicit without adapter inference. Long Unicode and hostile identifier text are escaped by React and remain wrap-safe.

The separate frozen HTML renderer consumes the same projection and renders the same four rows through `textContent`; its inline-script CSP hash is locked by a digest regression. No Page route, navigation, enrichment, JSON schema/API, Sector rendering, or TUI behavior changed. The superseded hand-written browser asset remains untouched, while the committed React bundle was regenerated deterministically.

Verification covered 38 frontend unit tests, TypeScript checking, deterministic bundle and supply-chain checks, Chromium and Firefox against the real Rust server, a freshly generated frozen report opened under its CSP in pinned Chromium, strict locked all-feature/all-target Clippy, the full locked all-feature/all-target Rust suite, focused resolved/fail-closed/escaping/export tests, and the read-only `1/1000 -> dba.poc_table` oracle. Independent Standards and Spec reviews found no remaining production finding.

## Comments
