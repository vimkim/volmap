Type: research
Status: resolved
Blocked by:

# Establish licensing and reverse-engineering provenance boundaries

## Question

What licensing and provenance constraints must the implementation-ready specification impose when a new inspector uses the CUBRID source tree as an on-disk-format authority and the recovered `volmap-standalone` behavior as an oracle? Examine the exact licenses and notices in the pinned CUBRID tree, relevant third-party dependency licenses for plausible Rust and Go stacks, and the distinction between source-derived format knowledge, behavioral compatibility evidence, and transliteration of recovered code. This is not a request for unsupported legal conclusions: record primary-source facts, uncertainties requiring owner or counsel judgment, required notices/source-distribution obligations, and a conservative clean implementation boundary.

## Comments

## Answer

Resolved in [Licensing and reverse-engineering provenance boundary](../research/licensing-provenance.md).

Use only the specifically Apache-2.0-labeled pinned CUBRID storage/OOS sources as the normative format authority, with source/commit attribution and Apache redistribution compliance. Do not link CUBRID or reuse its bundled dependencies: the tree contains conflicting legacy GPL package metadata and LGPL/GPL third-party components despite the current format files' Apache headers.

Keep the recovered executable and decompiler output quarantined and outside source, builds, packages, and default tests. The executable may be used only as an owner-authorized black-box oracle whose normalized facts are logged by hash; do not transliterate its expression or reproduce its UI. Final artifact authority, reverse-engineering permission, outbound project license, trademark wording, and bare-binary notice delivery require owner/counsel decisions. Both Rust and Go can support a permissive static dependency policy; the locked graph, runtime/libc, embedded web assets, notices, and SBOM must be audited at release.
