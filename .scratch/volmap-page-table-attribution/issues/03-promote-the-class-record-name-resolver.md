Type: task
Status: open
Blocked by: 02

# Promote the class-record name resolver

## Question

Can the validated `Prototype*` class-name lookup be replaced by one bounded production resolver that returns the exact stored class name or a precise typed unresolved result for the original numeric OID?

Implement exact-OID lookup through existing page/slot readers and reuse the established handling for `REC_HOME`, `REC_NEWHOME`, bounded relocation, and `REC_BIGONE` multipage-overflow records. Validate the object-representation header and variable-offset width, locate variable attribute zero, enforce the CUBRID identifier bound and terminating NUL, and decode the snapshot codeset. Packed VARCHAR handling must consume both big-endian 32-bit compressed/decompressed lengths after `0xff`; a non-zero compressed length must be safely decompressed with validated bounds or produce a typed unsupported/unresolved reason, never parsed as text. Fail closed for cycles, depth/budget stops, missing pages/slots, dead record kinds, corrupt lengths/offsets, encryption, unsupported representation/codeset, and invalid identifiers. Delete the prototype parser after production callers and tests replace it. Add focused valid and hostile-input tests, including short/long/compressed VARCHAR, all supported codesets, home/new-home, relocation, `REC_BIGONE`, and numeric-OID retention on every failure. Format, run the focused tests, and commit the ticket's production change on `feat/page-table-attribution`.

## Comments
