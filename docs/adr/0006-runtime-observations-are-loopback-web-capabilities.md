# Treat runtime observations as loopback web capabilities

Runtime observations are volatile engine and kernel readings rather than committed inspection facts. They therefore remain a web-only optional capability outside the inspection graph, revision chain, outcomes, diagnostics, exports, and terminal-parity contract. Absence, refusal, incompatibility, or staleness changes only the runtime capability state; the ordinary inspector continues unchanged.

Because `serve` is otherwise intentionally unauthenticated and runtime attachment exposes sanitized engine-internal state, an attached viewer is loopback-only in version one. Remote use goes through SSH forwarding; a wildcard HTTP listener cannot enable runtime attachment. This trades direct LAN convenience for a clear security claim without introducing authentication or TLS as an incidental part of the frontend migration.

Only the latest bounded observations for one proven server incarnation are retained. Route, scope, generation, overlay, pause, or incarnation changes prevent late responses from being adopted; a server restart atomically clears runtime state without disturbing inspection navigation. Runtime inspection remains structural-only even when the server holds plaintext memory: it never returns application values, raw page content, private memory, or a reconstructed event history.
