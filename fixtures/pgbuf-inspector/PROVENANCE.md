# Producer-owned conformance corpus

Source: CUBRID, `docs/pgbuf-inspector/v1`, commit
`f8c068f771ccd141a3c2f08541fe75c84e41ce53` (format-aligned producer).
Wire contract 1.0, corpus revision 2; the aggregate SHA-256 is
`11dbecc72e4b9dd78e22080f138c801c189c7e047c0db23af8233f44a804d5ce`.

The v1 directory is copied verbatim. From `v1/corpus`, run
`sha256sum -c SHA256SUMS` and `sha256sum SHA256SUMS` offline.
The Rust integration test independently verifies every listed file and the pin.
Develop producer validation and real-engine integration are separate gates.
