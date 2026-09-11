# Query CUBRID page-buffer observations directly

This Linux/Python-standard-library example connects directly to the same Unix
socket used by Volmap's runtime overlay. It needs no Volmap HTTP server.
The query key is `(volid, pageid)`, not a native BCB pointer or slot index.
The v1 producer supplies a bounded scan; the client filters that scan locally.

From the Volmap repository root, with the contract worktree's `demodb` running:

```sh
just example pgbuf-demodb        # VPID 0:0
just example pgbuf-demodb 0 7    # VPID 0:7
```

The convenience recipe pins the endpoint observed on 2026-09-11 for the server
installed from `/home/vimkim/gh/cb/CBRD-27398-pgbuf-inspector-contract`:
`/tmp/pgbuf-inspector/ff9fa4791a9d59ece022093f466ef795.sock`.
That running process used the `debug_gcc` installation. Database recreation,
changed database paths or a different temporary root can change the endpoint.
Use the server's `PGBUF_INSPECTOR_READY` startup message for its explicit path:

```sh
just example pgbuf 0 7 /path/to/pgbuf-inspector/identity.sock
# Or override the demodb recipe's third argument:
just example pgbuf-demodb 0 7 /path/to/pgbuf-inspector/identity.sock
```

The server must already have started with `enable_pgbuf_inspector=yes` and run
under the same UID. The parameter is startup-only. These commands neither
change its configuration nor restart it. An unavailable socket produces an error;
there is no automatic retry or fallback server discovery.

Output is JSON: server-reported incarnation, permanent-volume identities and
LRU list counts; requested VPID; the matching semantic record, if unique; and
scan header/footer including counts and truncation. Fields include latch mode,
fix count, dirty/flushing flags, page kind, LRU kind/index/zone and LSAs.
A real `demodb` run observed VPID 0:0 as `volheader`, fix count 0 and dirty false;
values can change between invocations.

`observed` means one matching record was returned. `ambiguous` suppresses duplicate
matches. `not-observed` means the scan returned no match, including partial-scan
omission; it is not proof that the page is currently absent. Nothing is published
until the footer and record count validate. A scan is not an atomic snapshot and
this example neither loads pages nor requests page contents.

This is a diagnostic wire example, not Volmap's full production decoder or
attachment verifier. It checks endpoint ownership/modes and Linux peer UID,
bounded framing/depth, deadlines, capture identity and counts. It reports the
server's database identity without matching it against independently read volume
headers; the socket pathname alone does not prove that a database is `demodb`.
Use Volmap's runtime adapter for full persistent identity and semantic validation.

```sh
python3 -m unittest discover -s examples -p test_pgbuf_socket.py
```
