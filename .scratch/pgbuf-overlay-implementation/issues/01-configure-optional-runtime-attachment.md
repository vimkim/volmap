# 01: Configure optional runtime attachment safely

**What to build:** An operator can explicitly configure the optional CUBRID page-buffer observation source and understand disabled or unavailable attachment without changing ordinary disk inspection. This first slice delivers configuration, normalized capability HTTP behavior and browser presentation, not a simulated production attachment.

**Blocked by:** None (can start immediately).

**Status:** complete

- [x] Runtime attachment requires both explicit enablement and an explicit socket path; no discovery or environment fallback occurs. Requested attachment with a non-loopback HTTP listener fails startup. Unattached serving retains its existing listener behavior.
- [x] Introduce the smallest private runtime broker boundary needed by this behavior, independently owned per live inspection session. Its interface exposes capability metadata and bounded observations for validated scopes; producer protocol, identity, retry, cache and resource details stay private. Any necessary ownership prefactor happens first within this slice, with disk behavior unchanged.
- [x] Use private real/simulated adapter and injected clock/scheduling seams without a new runtime dependency. Simulations are test fixtures, never a production trust bypass. Until ticket 02 provides verified attachment, no observations or active/verified capability are fabricated.
- [x] GET /api/v1/runtime/capabilities returns Cache-Control: no-store and a sanitized runtime envelope. Reserve disabled, connecting, active, stale, unavailable, refused and incompatible as capability states; pause, expiry and observation coverage are separate dimensions. Missing sockets mean unavailable, not proof of the producer's parameter setting.
- [x] Capability requests have four independent admission slots, no queue and a one-second deadline. Runtime admission and errors cannot consume the ordinary inspection resource queue or change inspection outcomes or diagnostics.
- [x] The browser identifies the source as CUBRID page-buffer observation and presents disabled/unavailable states with accessible text. Paths, UID/GID/PID, raw OS errors, pointers, private structures and application bytes are absent from HTTP and UI disclosures.
- [x] Runtime capability publication never advances snapshot generations, inspection revisions or cursors, or wakes disk-watch clients. The inspection graph, TUI, exports and standalone executable contract remain unchanged.
- [x] Tests exercise actual configuration parsing, the real HTTP boundary and rendered capability presentation, including invalid listener/configuration combinations, no-store, sanitization, independent admission below/at/above capacity and unaffected disk inspection without CUBRID installed.
- [x] Document explicit opt-in and loopback/SSH use using the accepted domain vocabulary. This ticket adds neither public authentication/TLS nor engine discovery, resident-page inspection, AOUT, event history or engine implementation.

## Comments

2026-09-08 — Implementation prepared locally under work item 73, starting from
24e0177faf8bd573116007a168cae715bff23aab. Explicit paired CLI options and
loopback validation, a private capability-only broker, independent four-slot
admission/one-second deadline, sanitized runtime HTTP metadata and browser
presentation are implemented. Configured attachment is deliberately unavailable
and unverified; no producer socket is opened. Observation acquisition and the
validated-scope observation operation remain ticket 02 work, not a fabricated
working attachment in this slice.

Verification so far: full Rust test suite passed (existing manual previews and
resource benchmark remain ignored); 48 frontend tests, typechecking, formatting
and all-target/all-feature Clippy passed. Complete frontend acceptance passed:
generated asset reproducibility and advisories, Cargo-only embedding, and five
browser tests passed with one pre-existing Chromium-only parity case skipped in
Firefox. The new capability journey passed in both browsers against separate
real disabled/configured servers. An initial Firefox navigation timeout passed
unchanged on a focused rerun and then in the full gate; it is not counted as an
initial pass. Parent spec and completed map hashes remain unchanged.

Final two-axis review and the ticket-scoped commit are pending confirmation of
the proposed starting-commit review baseline; this ticket is not yet complete.

2026-09-08 — Completed after the user-approved review against `24e0177`.
The earlier deferral of the validated-scope interface is superseded: the private
broker now accepts an owned, typed scope of at most 512 VPIDs, preserving order
and epoch, and explicitly returns no observation evidence. Actual producer
connection, identity proof and capture remain ticket 02 work; no observation
HTTP route or fabricated attachment was introduced.

Both original Spec findings are resolved. Real and simulated broker constructors
accept an injected scheduler, with deterministic capability and observation
deadline tests at 999/1000/1001 ms. Scope tests cover 0/1/511/512/513 pages and
owned-copy preservation. All four broker tests passed, including an independent
reviewer rerun.

Final `just verify` passed: Rust tests, formatting, all-target/all-feature Clippy,
static musl release checks, frontend typechecking, all 48 frontend tests, generated
asset reproducibility, advisory and supply-chain checks, and browser acceptance
(5 passed, 1 existing Firefox skip). The runtime capability journey passed in
both Chromium and Firefox. Final full-ticket re-review against `24e0177` found
0 Standards findings and 0 Spec findings (worst severity: none for both). Parent
spec and completed map hashes were unchanged. Setup and unrelated planning
changes are excluded from the ticket-scoped commit.
