Type: grilling
Status: resolved
Blocked by: 03, 04, 14

# Define the web service, remote-access, and HTML-export contract

## Question

What exact browser-facing contract should `volmap serve` and `volmap export html` provide? Decide lazy read-only JSON resources, navigation URLs and stable identities, server lifecycle, default loopback binding, explicit `0.0.0.0` binding, mandatory non-loopback access tokens, origin and request protections, volume-path redaction, bounded raw-hex authorization, snapshot-change behavior, and the version-one boundary between a compact self-contained HTML report and details available only from the live inspector process. Assume no built-in TLS and document the trusted-network, SSH, VPN, or reverse-proxy boundary clearly.

## Comments

### Primary-source security constraints

- Bearer credentials belong in the `Authorization` header, not a URL path/query/fragment; URL-carried tokens may leak through browser history and other handling. Bearer possession without TLS does not provide a confidential remote channel. Sources: [RFC 6750](https://www.rfc-editor.org/rfc/rfc6750.html), [RFC 9700 section 4.3.2](https://www.rfc-editor.org/rfc/rfc9700.html#section-4.3.2).
- Numeric loopback-only HTTP is a narrowly local transport boundary. A remotely reachable bearer service needs HTTPS supplied by a trusted VPN/tunnel/reverse proxy because version one has no TLS implementation. Sources: [RFC 6750 section 5.3](https://www.rfc-editor.org/rfc/rfc6750.html#section-5.3), [RFC 8252 section 8.3](https://www.rfc-editor.org/rfc/rfc8252.html#section-8.3).
- `Host` must be exactly allowlisted to resist DNS rebinding. CORS does not prevent cross-origin requests from reaching a server, so same-origin API design still requires authentication plus Origin/request validation for work-triggering methods. Sources: [MDN Host](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Host), [MDN CORS](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS#simple_requests), [OWASP CSRF guidance](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html).
- GET/HEAD are safe retrieval methods. Deep enrichment, export generation, job creation, or shutdown must not be triggered by safe methods. Source: [RFC 9110 section 9.2.1](https://www.rfc-editor.org/rfc/rfc9110.html#section-9.2.1).
- Live responses should use strict same-origin CSP, `nosniff`, no-referrer, and no-store controls. A self-contained file may use an early meta CSP, but meta CSP cannot carry `frame-ancestors`, sandbox, or reporting directives. Sources: [MDN CSP](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CSP), [MDN Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Cache-Control#preventing_storing), [CSP3 meta policy](https://www.w3.org/TR/CSP/#meta-element).

### Human decision — round 1 (2026-08-19)

The user accepted all six recommendations:

1. **Listener boundary.** `serve` completes the fast scan before binding. Its default listener is the numeric loopback address `127.0.0.1` on an ephemeral port. Binding `0.0.0.0` requires an explicit remote-HTTP acknowledgement; accidental hostname or non-loopback binding is rejected.
2. **Session access token.** Every server run, including loopback-only runs, generates a fresh cryptographically secure 256-bit bearer token. The token is never accepted through argv, environment variables, URL paths, queries, fragments, browser storage, logs, or API response bodies. It is disclosed once to the controlling terminal or, when explicitly requested, to a newly created owner-only token file. The browser accepts it through an unlock form, retains it only in memory, and sends it in `Authorization: Bearer` on every API request; refresh requires re-entry.
3. **Remote-access and no-TLS boundary.** Two ordinary remote-debugging modes are supported: keep the listener on the Linux server's `127.0.0.1` and view its plain-HTTP origin through SSH local port forwarding, or explicitly bind `0.0.0.0` and connect to it directly over HTTP with mandatory bearer authentication. HTTPS is optional and is not a prerequisite for either mode. The token provides possession-based access, not transport confidentiality or integrity; therefore all-interface startup emits a prominent plaintext-transport warning, direct HTTP is intended for a trusted internal/private network, and direct use over an untrusted network remains unsupported. A VPN or trusted HTTPS reverse proxy may provide an additional transport boundary but is not required.
4. **Stable navigation.** Browser locations are pinned to immutable revisions and typed canonical identities: `/s/<snapshot-id>/r/<revision>/volume/1`, `/sector/1/3`, `/page/1/127`, `/slot/1/127/2`, and `/oos/1/127/2`. Enrichment navigates to a new revision URL, while prior revision URLs remain valid for the server session. URLs contain neither source paths nor tokens nor presentation-only identifiers.
5. **Raw-byte boundary.** Version one exposes no raw-byte or hexadecimal endpoint. It may expose ranges, offsets, lengths, byte maps, interpreted facts, and evidence locators. Authentication never relaxes the rule against revealing application payloads, ciphertext, keys, nonces, or other prohibited source bytes.
6. **Self-contained HTML boundary.** `export html` produces one offline file containing a frozen overview, compressed maps, navigable entities, all diagnostics and coverage facts, and only deep facts already present in the selected revision. It embeds its CSS, JavaScript, and data; performs no network access; registers no service worker; contains no forms, source paths, or raw bytes; and uses an early restrictive meta CSP as defense in depth. Detail absent from the selected revision remains available only through a live inspector session.

### Human decision — round 2 (2026-08-19)

The user accepted all six recommendations after clarifying that both SSH-forwarded loopback HTTP and explicitly enabled `0.0.0.0` HTTP are required for internal read-only debugging:

7. **Listener options.** `serve` accepts `--listen IP:PORT` and defaults to `127.0.0.1:0`. A non-loopback or unspecified address, including `0.0.0.0`, is rejected unless `--allow-remote-http` is also present. `--external-origin ORIGIN` is mandatory for `0.0.0.0`, defines the one browser origin and exact request authority, and contains no path, query, fragment, or credential. An optional `--token-file PATH` creates a new owner-only file and refuses overwrite; without it, the fresh token is disclosed once through the controlling terminal, and startup fails if neither channel exists.
8. **Read-only JSON resources.** Authenticated `/api/v1` resources project the pinned canonical revision's overview, volume, sector, file, page, slot, OOS, relationship, diagnostic, and coverage facts. They use the accepted `volmap.inspection` schema version 1 meanings. `GET`/`HEAD` retrieve only already-published immutable data. Collection traversal uses bounded opaque cursors that are valid only for their snapshot and revision; a cursor cannot silently cross into a newer revision.
9. **Deep-enrichment jobs.** `POST /api/v1/s/<snapshot-id>/r/<base-revision>/enrichments` accepts `application/json` naming exactly one canonical page, slot, or OOS selector. It returns `202 Accepted` and an authenticated job resource; successful completion links to the newly published immutable revision. Equivalent work may reuse cached canonical facts without changing their meaning. Version one has no browser export-generation, job-cancellation, or server-shutdown endpoint.
10. **Browser request protection.** Every request must carry exactly one `Host` matching the configured external-origin authority. Every API request requires the bearer token. The service emits no CORS permission. Work-triggering `POST` requests additionally require `application/json`, an exact non-null configured `Origin`, and, when fetch metadata is present, a non-cross-site value. Forwarded host/protocol headers are ignored; a trusted reverse proxy must preserve the configured host authority.
11. **Server lifecycle.** The foreground server binds only after the fast scan succeeds and revision zero is published. Version one has no daemon mode or persistent session state. `SIGINT`/`SIGTERM` stops acceptance of new work, cancels enrichment, performs a bounded drain of active responses, closes the listener, and ends the session; its token, jobs, cursors, and URLs then expire.
12. **Snapshot invalidation.** If the source changes, all published revisions remain readable only as retained diagnostic facts and every browser/API projection prominently declares terminal invalidation. No revision is rewritten. New enrichment receives `409 Conflict`; there is no in-session reset or revalidation. The operator must restart `serve` to create a new snapshot and session.

### Human decision — round 3 (2026-08-19)

The user accepted all six recommendations:

13. **Live response policy.** HTML, API, asset, and error responses use exact MIME types, a strict same-origin header CSP, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Cache-Control: no-store`, and same-origin resource/opener policies. Live code uses no inline script, `eval`, remote asset, analytics, or service worker. The unauthenticated shell and bundled assets contain no inspection facts; all data resources require authentication.
14. **HTTP bounds.** Request targets are limited to 8 KiB, total request headers to 32 KiB and 64 fields, and JSON bodies to 64 KiB. Header and body read deadlines are 10 seconds and the idle timeout is 30 seconds. Collection pages default to 100 and cap at 512 records. Query/enrichment admission uses the accepted `ResourcePolicy`, never an unbounded queue, and returns `429 Too Many Requests` when work cannot be admitted.
15. **HTTP outcomes.** Redacted versioned error envelopes accompany `400` malformed request, `401` authentication failure, `403` Origin/fetch-context rejection, `404` unknown resource, `405` unsupported method, `409` invalidated snapshot or unusable base revision, `413` oversized request, `421` invalid Host, `429` refused resource admission, and `500` inspector defect. Published inspection facts, including corruption findings, use `200`; accepted enrichment uses `202`. Token comparison is constant-time and failures disclose no authentication detail or secret.
16. **Export enrichment.** `export html` accepts repeatable `--enrich SELECTOR`, restricted to page, slot, and OOS selectors. Requests are deduplicated and executed in canonical order, and the final published revision is exported. Without `--enrich`, the command freezes revision zero.
17. **Offline viewer.** The file uses fragment-based typed navigation. Embedded code performs filtering, navigation, and map rendering only. An early hash-based meta CSP permits only the exact embedded script/style and `data:` images and permits no connection. The artifact includes version, snapshot/revision, validity, outcome, coverage, diagnostics, and About/license notices.
18. **File and size safety.** Export refuses an existing destination and has no overwrite option in version one. It creates a mode-`0600` sibling temporary file and atomically installs it only after complete generation. `--max-html-bytes` defaults to `64MiB` and has a hard maximum of `512MiB`. A limit failure never truncates or omits data, leaves no destination, and directs the operator to `serve`. A terminally invalidated snapshot may still produce a prominently marked diagnostic export and then exits with the canonical nonzero status. Wall-clock timestamps and nondeterministic presentation data are omitted so equal graph and build inputs produce identical bytes.

## Answer

`serve` and `export html` are two projections of the same canonical inspection graph. The live adapter retrieves and enriches immutable revisions lazily; the file adapter freezes exactly one revision. Neither adapter opens volume sources, decodes bytes, derives facts, reclassifies diagnostics, or weakens the version-one prohibition on application payloads, ciphertext, decrypted values, keys, nonces, and other prohibited source bytes.

### Live-session startup and access

The accepted server shape is:

```text
volmap serve INPUT [SCAN_OPTIONS]
    [--listen IP:PORT]
    [--allow-remote-http]
    [--external-origin ORIGIN]
    [--token-file PATH]
```

`--listen` defaults to `127.0.0.1:0`. Version one accepts a numeric IP socket address rather than resolving a hostname. It completes `Inspection::open`, publishes revision zero, and only then binds; a failure before publication never leaves a listener behind. When `--external-origin` is omitted for a concrete address, Volmap derives `http://<listen-ip>:<selected-port>` after binding. The foreground process reports that safe origin without placing a credential in it. For an IPv4 wildcard listener it also reports a sorted, deduplicated, non-exhaustive list of copyable URLs derived from active local IPv4 interfaces, with loopback as a fallback. An explicit external origin may also describe an SSH-forwarded local port that differs from the remote listener.

A non-loopback or unspecified listener, including `0.0.0.0`, additionally requires `--allow-remote-http`. Because a wildcard listener is not a browser authority, `0.0.0.0` also requires exactly one `--external-origin`. The origin is an absolute `http` or `https` origin with a canonical host and explicit effective port and no user information, path other than `/`, query, or fragment. Direct all-interface access uses an `http://HOST:PORT` origin. An `https` origin is valid only when a separately trusted reverse proxy terminates TLS and preserves that external authority; Volmap itself still speaks HTTP and never interprets forwarded-host or forwarded-protocol headers.

The primary remote-server workflows are deliberately ordinary:

```text
# SSH-forwarded loopback: choose the same PORT at each end
remote$ volmap serve INPUT --listen 127.0.0.1:PORT
local$  ssh -N -L PORT:127.0.0.1:PORT SERVER
browser http://127.0.0.1:PORT/

# Direct internal HTTP
server$ volmap serve INPUT \
           --listen 0.0.0.0:PORT \
           --allow-remote-http \
           --external-origin http://INTERNAL-HOST:PORT
```

SSH carries plain browser HTTP through its encrypted tunnel while the remote listener stays loopback-only. Direct `0.0.0.0` HTTP is a supported internal read-only debugging mode, but startup prominently states that bearer authentication does not provide transport confidentiality or integrity. It is unsupported on an untrusted public network. A VPN or HTTPS reverse proxy is optional, not a prerequisite.

Every run generates a fresh cryptographically secure 256-bit bearer token, including loopback-only and SSH-forwarded runs. Volmap never accepts a caller-selected replacement credential from argv, environment variables, configuration, or request bodies, and never transports its generated token in a URL or browser-storage mechanism. With `--token-file`, it exclusively creates a new mode-`0600` file and refuses an existing path. Otherwise it discloses the token once through the controlling terminal and fails startup if no safe disclosure channel exists. The unlock page retains the submitted token only in JavaScript memory; a reload requires re-entry. Every `/api/v1` request sends `Authorization: Bearer ...`. The token is compared in constant time and expires with the process. It never appears in an API body, navigation location, referrer, access log, or ordinary status output.

### Browser navigation and JSON resources

The unauthenticated root document and fingerprinted bundled assets are an empty application shell containing no snapshot facts. After unlock, browser navigation uses revision-pinned canonical identities:

```text
/s/<snapshot-id>/r/<revision>/volume/<volid>
/s/<snapshot-id>/r/<revision>/sector/<volid>/<sectorid>
/s/<snapshot-id>/r/<revision>/file/<volid>/<fileid>
/s/<snapshot-id>/r/<revision>/page/<volid>/<pageid>
/s/<snapshot-id>/r/<revision>/slot/<volid>/<pageid>/<slotid>
/s/<snapshot-id>/r/<revision>/oos/<volid>/<pageid>/<slotid>
```

These are adapter locations, not graph identities. They contain no input path, token, or presentation-only ID. Enrichment navigates to its result revision; all earlier revision locations remain valid until the live session ends.

The authenticated data vocabulary is rooted at `/api/v1/s/<snapshot-id>/r/<revision>` and contains overview, volume, sector, file, page, slot, OOS, relationship, diagnostic, and coverage resources. Singular entity resources use the same typed path components as browser navigation; collection resources use canonical model ordering. Relationship queries accept only the closed typed entity vocabulary. Every JSON envelope declares `volmap.inspection`, schema version 1, snapshot ID, pinned revision, validity, outcome, and the coverage/diagnostic context required to interpret its data. Potentially 64-bit integers, discriminated availability, canonical references, redaction, and compatibility rules are exactly those fixed by the CLI/JSON contract.

`GET` and `HEAD` retrieve only an already-published immutable resource or bundled asset. A collection defaults to 100 records and accepts at most 512. Its opaque cursor binds the snapshot, revision, query, and canonical order and cannot cross into another revision. There is no generic query language, arbitrary sorting, or raw-range endpoint.

Deep work uses:

```text
POST /api/v1/s/<snapshot-id>/r/<base-revision>/enrichments
GET  /api/v1/jobs/<job-id>
```

The POST body names exactly one page, slot, or OOS selector and no decoder or byte range. A slot request enriches its containing page; an OOS request enriches its selected chain. The service returns `202 Accepted` with an authenticated job location, even when equivalent work can finish immediately from coalesced or cached canonical facts. A successful job links to the resulting immutable revision. Job identifiers, cursors, revisions, and tokens are session-only. Version one exposes no HTTP export-generation, cancellation, reset, repair, write, or shutdown operation.

### HTTP security, limits, and outcomes

Before authentication, every request must contain exactly one `Host` whose canonical authority equals `--external-origin`; a mismatch is `421 Misdirected Request`. Volmap ignores `Forwarded`, `X-Forwarded-Host`, and related headers. It grants no cross-origin access and emits no `Access-Control-Allow-Origin`. Every work-triggering POST additionally requires `Content-Type: application/json`, an exact non-null `Origin` equal to the external origin, and a value other than `cross-site` when `Sec-Fetch-Site` is present. Unsupported methods do no work and receive `405` with `Allow`.

All live HTML, API, asset, and error responses receive exact MIME types and the applicable hardening headers, including errors:

```text
Content-Security-Policy:
  default-src 'none'; script-src 'self'; style-src 'self';
  img-src 'self' data:; font-src 'self'; connect-src 'self';
  object-src 'none'; base-uri 'none'; form-action 'none';
  frame-ancestors 'none'
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Cache-Control: no-store
Cross-Origin-Resource-Policy: same-origin
Cross-Origin-Opener-Policy: same-origin
```

A deny-by-default `Permissions-Policy` disables browser capabilities the viewer does not use. Live documents contain no inline script/style, `eval`, dynamic-code construction, remote asset, analytics, form, worker, or service-worker registration. The same-origin app uses only bundled fingerprinted assets and its authenticated API.

The HTTP parser admits at most an 8 KiB request target, 32 KiB of request headers across at most 64 fields, and a 64 KiB JSON body. Header and body read deadlines are each 10 seconds; an idle connection expires after 30 seconds. Query and enrichment work must also be admitted by the inspection `ResourcePolicy`. Connection handling and job scheduling have bounded queues; Volmap never substitutes unbounded work when admission fails.

HTTP transport state remains separate from the inspection outcome. A valid published response is `200 OK` even when its canonical outcome contains corruption findings. Enrichment admission is `202 Accepted`. Versioned redacted error envelopes use `400` for malformed requests or selectors, `401` for missing/invalid bearer authentication, `403` for Origin/fetch-context rejection, `404` for an unknown resource, `405` for a disallowed method, `409` for a terminally invalidated snapshot or unusable base revision, `413` for an oversized request, `421` for an invalid Host, `429` for refused resource admission, and `500` for an internal inspector defect. Authentication errors are generic and no response or log exposes a token, host path, prohibited byte, or secret.

### Lifecycle and source changes

`serve` is foreground-only and has no daemon mode, persistent cache, reusable index, or resumable session. `SIGINT` or `SIGTERM` stops admitting requests, cancels enrichment, performs a bounded drain of active responses, closes the listener, removes private spill according to the accepted storage policy, and expires every token, job, cursor, location, and revision view belonging to that session.

Source fingerprints are checked under the inspection architecture before and after startup and enrichment. A mismatch atomically and terminally invalidates the snapshot, cancels outstanding deep work, and publishes no further revision. Existing revision resources remain readable with an unavoidable invalidation marker and describe retained facts as diagnostic-only; enrichment returns `409`. There is no in-session refresh or revalidation because a restart must establish a new `SnapshotId` and revision zero.

### Self-contained HTML export

The accepted file command is:

```text
volmap export html INPUT --output PATH [SCAN_OPTIONS]
    [--enrich SELECTOR]...
    [--max-html-bytes SIZE]
```

`--enrich` is repeatable and accepts only page, slot, or OOS selectors. Volmap deduplicates targets, executes them in canonical order under the same operational budgets, and freezes the final published revision. With no enrichment request, it exports revision zero. Deep information not already committed to that revision is never manufactured by the HTML adapter and remains live-inspector-only.

The result is one offline HTML file containing a frozen overview, compact allocation maps, the selected revision's navigable entity facts, all contributing diagnostics and coverage ledgers, validity and outcome, Volmap/schema/build identity, and About/license notices. Typed identities become fragment navigation within the file. Embedded JavaScript performs only local navigation, filtering, and rendering over embedded sanitized data. It makes no fetch, connection, form submission, worker, service-worker, storage, analytics, or external-resource request.

The document places a no-referrer meta element and a restrictive meta CSP before controlled content. The generated policy uses exact hashes for its embedded style and script/data blocks and otherwise reduces to:

```text
default-src 'none';
img-src data:;
style-src 'sha256-...';
script-src 'sha256-...';
connect-src 'none'; font-src 'none'; object-src 'none';
base-uri 'none'; form-action 'none'
```

Meta CSP is defense in depth: an offline file cannot carry the live response's header-only anti-framing guarantee. All embedded graph text is still context-escaped independently of CSP.

Export refuses an existing destination and version one has no overwrite flag. It exclusively creates a mode-`0600` temporary sibling, writes and synchronizes the complete artifact, and installs it atomically without replacement. The default `--max-html-bytes` is `64MiB`, using the already accepted IEC quantity grammar, and values above the hard `512MiB` ceiling are invalid. If exact generation would exceed the selected limit, Volmap emits no partial destination, cleans its temporary artifact, and directs the operator to `serve`; it never samples, truncates, or silently drops entities or diagnostics.

If a source change terminally invalidates the snapshot after a graph root exists, Volmap may still atomically emit the diagnostic-only revision with an unmistakable invalidation banner and then returns its canonical nonzero process status. The file contains no source-volume, `_vinf`, spill, key-file, or other host path and no forbidden raw/decrypted data. It omits wall-clock generation timestamps and nondeterministic presentation state; for equal selected graph and build inputs, generation produces identical bytes.
