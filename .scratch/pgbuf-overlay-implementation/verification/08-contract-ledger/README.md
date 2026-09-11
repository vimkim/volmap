# Named functional evidence for release review

This ledger maps all **50 checklist entries from tickets 01–05** to named tests
or explicit source/document evidence. It replaces the preparation manifest's
aggregate-only links. It does not mark these broad contracts or release readiness
passed: each entry separately states the supported assertions and remaining
scope limits. Tickets 06–08 retain their existing manual, integration, density
and performance gate records.

[ledger.json](ledger.json) contains 81 evidence entries and 14 source fingerprints.
Repeated references to one test do not count as new executions. Rust test names
resolve to an actual `... ok` line in the preserved preparation log. Browser
families name the instantiated Chromium/Firefox cases and log lines from that
successful suite; none is the unrelated Firefox parity skip. Source/document
reviews have zero test executions and are never presented as test results.

The two new focused frontend executions provide named assertion results that the
old aggregate Vitest output lacked:

| Execution | Passed | Failed | Skipped | Raw report |
| --- | ---: | ---: | ---: | --- |
| Observation state/reducer | 21 | 0 | 0 | [observations-results.json](observations-results.json) |
| Runtime effect execution | 8 | 0 | 0 | [runtime-results.json](runtime-results.json) |

The actual commands, source commit `71e1e3c`, raw hashes, test locations, counts,
fixture preconditions and producer boundaries are recorded in the ledger.
These tests execute frontend sources with injected state/effect fixtures; they
are neither release-browser measurements nor real CUBRID runs. In particular,
backoff is now linked both to the nominal-delay reducer test and the numerical
6400/8000/9600 ms jitter-boundary effect test.

## How to inspect one requirement

Find its ticket/checklist ID, such as `03-02`, in `contracts`. Follow each
`evidence` key to a named entry, then its `execution_context` to the exact raw
artifact and command. `source` supplies the file/line and fingerprint; Rust and
browser entries additionally identify raw output lines. Read `covered` and
`remaining_or_limit` before judging acceptance. A multi-clause requirement is
not wholly established merely because one related test passed.

All referenced test sources still match the preparation source commit `08ed947`.
The updated operator document is deliberately the only listed file that differs
from that base; its evidence is a current source/document review, not an inherited
test result. Corpus version/revision/hash is carried from the independently
pinned integration evidence. Real producer tuple coherence, traversal, security
and build/platform evidence remain in ticket 07's named completion audit and
producer ledgers, not relabelled as consumer tests.

## Decision and coverage qualifications

- `01-01` is superseded in part by accepted ADR 0006: explicit IPv4 listeners
  are allowed; wildcard listeners are refused. The old non-loopback prohibition
  is retained as historical requirement text with an explicit disposition.
- `04-01` describes the shipped 512-page rotation behavior. Accepted ADR 0008's
  planned 64-sector Volume scope does not gain implementation or measurement
  evidence from those older tests.
- Automatic focus, glyph, forced-color and reduced-motion checks remain separate
  from the missing named manual screen-reader/visual reviews.
- Native tuple coherence, no-hot-path-instrumentation and broad absence of runtime
  state from every export/TUI/graph path require their native/source/design
  evidence. A selected HTTP response or a semantic browser assertion alone does
  not establish those universal claims.
- The current density gate remains failed. Nothing here changes thresholds,
  replaces a performance failure with a unit pass, or executes the dedicated-host
  matrix. Original build/environment qualifications remain visible.

The final delivery audit remains open: this is a named, scope-limited map of
existing functional evidence, not a complete proof of every clause. It gives the
release reviewer concrete starting points and explicit gaps instead of an
undifferentiated full-suite pass. Independent review is recorded in [review.md](review.md).
