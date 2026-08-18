# Licensing and reverse-engineering provenance boundary

Research date: 2026-08-18

CUBRID authority pinned to: `e1e651debf6cc100172bde96603b17424f9c135a`

> This is an engineering provenance and compliance recommendation, not legal advice. Reverse-engineering permissions depend on ownership, contract terms, purpose, and jurisdiction; the unresolved questions at the end require the artifact owner and, before public/commercial distribution, counsel.

## Decision

Use the pinned CUBRID server-engine source as the normative on-disk-format authority. Treat any parser code, constants, layouts, diagrams, or tests adapted from that source conservatively as Apache-2.0-derived material: identify the pinned source path and commit, preserve applicable attribution, ship the Apache-2.0 text, and mark modifications or translations.

Use `volmap-standalone` only as a quarantined behavioral oracle: run known fixtures, record normalized semantic observations, and compare results. Do not copy, translate, compile, link, publish, or distribute `recovered/` code or the old executable/archive as part of Volmap Inspector. Do not imitate its source structure, control flow, textual UI, symbol names, or expressive implementation details. The new product is a modern redesign, not a source reconstruction.

Call this a **clean implementation boundary**, not a formal “clean-room” process. A formal clean-room claim would require organizational controls and evidence that have not been established here.

This route is conservative and practical because the format information needed for version one is available in the Apache-licensed CUBRID tree. It avoids depending on a legal theory that decompilation of the unlicensed recovered artifact was necessary.

## Evidence inventory

### Pinned CUBRID source

The exact tree identifies the server engine as Apache License 2.0 and APIs/connectors as BSD 3-Clause in [`COPYING`](https://github.com/CUBRID/cubrid/blob/e1e651debf6cc100172bde96603b17424f9c135a/COPYING). Its storage-format authorities—including [`storage_common.h`](https://github.com/CUBRID/cubrid/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/storage_common.h), [`disk_manager.c`](https://github.com/CUBRID/cubrid/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/disk_manager.c), [`file_manager.c`](https://github.com/CUBRID/cubrid/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/file_manager.c), [`slotted_page.h`](https://github.com/CUBRID/cubrid/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/slotted_page.h), and [`oos_file.hpp`](https://github.com/CUBRID/cubrid/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/oos_file.hpp)—carry explicit CUBRID/Search Solution copyright and Apache-2.0 headers. The root [`LICENSE`](https://github.com/CUBRID/cubrid/blob/e1e651debf6cc100172bde96603b17424f9c135a/LICENSE) is the Apache-2.0 text.

At this commit, `git ls-tree -r --name-only <commit>` contains no file named `NOTICE`. That means no root CUBRID `NOTICE` artifact was found to propagate under Apache section 4(d); it does **not** cancel the license-copy, changed-file, or retained-attribution duties in sections 4(a)–(c). Apache-2.0 grants reproduction, derivative-work, and distribution rights in section 2 and sets those redistribution conditions in section 4; it separately limits trademark rights in section 6. See the [official Apache-2.0 text](https://www.apache.org/licenses/LICENSE-2.0).

The tree also contains `3rdparty/license/` texts for components under several different terms: permissive licenses, Apache-2.0, LGPL-2.1, GPL-2.0, OpenJDK exceptions, and component-dependent terms such as LZ4's BSD-2-Clause library versus GPL-2.0 tools. These are an important warning against treating the entire checkout as uniformly Apache-2.0. They are **not** dependencies of the proposed inspector merely because the CUBRID checkout contains them. Volmap Inspector must not copy or link CUBRID's third-party components unless each selected component is independently reviewed and recorded.

There is also conflicting legacy package metadata inside the exact pinned tree. [`debian/copyright`](https://github.com/CUBRID/cubrid/blob/e1e651debf6cc100172bde96603b17424f9c135a/debian/copyright) (dated 2010) says the server is GPL-2.0-or-later and the Debian packaging is GPL-3.0-or-later; [`cmake/CPackOptions.cmake.in`](https://github.com/CUBRID/cubrid/blob/e1e651debf6cc100172bde96603b17424f9c135a/cmake/CPackOptions.cmake.in) still labels an RPM “GPLv2+ and BSD” even though that CMake file itself has an Apache-2.0 header. This report cannot determine whether those entries are merely stale. Therefore it makes only the narrower evidence-backed statement that the root current license files and the specific format/OOS source files inspected are expressly Apache-2.0. Do not link or distribute CUBRID binaries/libraries, copy from a file without a clear current header, or make a repository-wide license representation without CUBRID maintainer/owner clarification and counsel review.

### Recovered artifact

The local `volmap-standalone` is a statically linked x86-64 ELF with GNU build ID `50f2e7a451bae7f0c5a889dd51d6ef1d82da0131` and SHA-256 `3bbb9fc93cbe777b3201dab7dbd69a9dceba935ae02f4f14f4fa1010e08ec861`. The local archive contains only that executable. A string scan finds glibc/GCC material but no application license or copyright statement; absence of a notice is not permission or evidence of public-domain status.

The local [recovery note](../../../recovered/README.md) says `recovered/` was reconstructed from that executable without an original source tree and that the raw decompilation loses original names, macros, comments, and structured control flow. Those files therefore establish technical provenance, not a redistribution license. The ownership, acquisition authority, applicable contract terms, and legal basis for the completed decompilation are not documented in this repository.

## Allowed evidence and implementation boundary

| Evidence/action | Version-one rule | Required record |
|---|---|---|
| Read pinned CUBRID engine source to learn fields, constants, invariants, and algorithms | Allowed under the source's stated Apache-2.0 terms; apply the conservative source-derived policy | Commit, source path, symbol/line range, fact learned, implementation/test using it |
| Translate or adapt a CUBRID declaration/algorithm into Rust or Go | Allowed only with Apache compliance; do not erase provenance just because the language changes | Source attribution in the implementation module and third-party notices inventory |
| Run the recovered executable on an authorized, non-sensitive fixture | Oracle-only, after owner confirms lawful possession/use | Executable hash/build ID, exact command/options, fixture hash, environment, exit status, normalized result |
| Compare counts, classifications, error behavior, or navigation outcomes | Allowed as behavioral evidence; record semantics rather than copying the old presentation | Test/oracle record linked to the relevant requirement |
| Copy strings, screens, layouts, symbol names, pseudocode, expressions, or control flow from `recovered/` | Prohibited by the project boundary | Review should reject or require owner/counsel exception |
| Build, link, vendor, package, or publish recovered code/binary/archive | Prohibited by the project boundary | No exception without documented owner/counsel approval |
| Use generated CUBRID fixtures | Allowed only when generated from data the project may use and distribute | Generator commit/config, database schema/data provenance, fixture hash, sensitivity classification |

The implementation repository should keep a small machine-readable provenance ledger (for example, `provenance.toml`) and require every on-disk decoder and oracle parity test to reference it. This makes later updates from the pinned format auditable.

Oracle outputs should be reduced to facts such as page counts, file identifiers, classifications, and exit conditions. Avoid golden snapshots of the recovered program's full textual/ANSI UI. Test the redesigned interfaces against the new canonical model, not pixel/text parity with the old program.

## Why reverse-engineering authority remains an owner/counsel question

The current Korean Copyright Act is directly cautionary. Article 101-3 permits a lawful user, in specified circumstances, to reproduce a program while researching or testing its functions to confirm underlying ideas and principles. Article 101-4 permits program-code reverse analysis only under stated predicates—including legitimate authority, compatibility need, information not easily obtainable, inevitability, and limitation to necessary parts—and restricts using the resulting information outside compatibility or to make substantially similar expression. See the current [Korean Copyright Act, Articles 101-3 and 101-4](https://law.go.kr/LSW/lsInfoP.do?lsiSeq=260839).

This report does not decide whether those predicates apply. In particular, it is a legal inference—not a fact—that ready availability of the relevant format information in Apache-licensed CUBRID source could undermine a claim that further decompilation is unavoidable. Other jurisdictions differ: for example, the EU software directive separately addresses observation/testing and tightly conditioned interoperability decompilation in Articles 5 and 6 ([Directive 2009/24/EC](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX%3A32009L0024)). Contractual restrictions, trade-secret duties, access-control law, and ownership can add constraints beyond copyright exceptions.

Accordingly, the specification should not authorize more recovery. If a format fact is genuinely missing from the pinned CUBRID source and generated fixtures, stop and obtain an owner/counsel decision before inspecting more decompiled material. Prefer a black-box experiment against the lawfully held executable, and limit it to the smallest unresolved interoperability fact.

## Distribution obligations for the new standalone binary

“One standalone binary” is a runtime/installation property, not an exemption from notices or source obligations.

1. **CUBRID-derived material.** Ship the Apache-2.0 text, retained CUBRID/Search Solution attributions that pertain to reused material, a pinned source URL/commit, and notices that the implementation is new/modified. Use “for CUBRID volumes” descriptively; do not use CUBRID marks or wording that implies endorsement. If source is distributed, preserve relevant source notices.
2. **Choose the Volmap Inspector license deliberately.** The lowest-friction conservative choice for code materially adapted from the engine is Apache-2.0 with explicit CUBRID attribution. Apache-2.0 permits additional/different terms for modifications or a derivative whole if its own conditions remain satisfied, so the copyright owner may choose another compatible policy—but that choice is not made by this research ticket.
3. **Embed notices without creating runtime dependencies.** Provide `volmap licenses` and a web-viewer Licenses/About surface containing exact third-party texts and attributions. Also attach `LICENSE` and `THIRD_PARTY_LICENSES` to release metadata/packages. If releases distribute a bare executable only, owner/counsel should confirm that the embedded, printable copies satisfy every selected license's delivery wording.
4. **Audit the resolved dependency graph, not package names from a design sketch.** Record exact versions, features, transitive dependencies, native build scripts, embedded JS/CSS/fonts/icons, and the C/runtime actually linked. Produce an SBOM plus exact license texts from the lockfile-resolved sources. Reject missing/unknown licenses and require explicit approval for reciprocal/copyleft terms.
5. **Static linking changes what is distributed.** It normally places runtime/library code in the executable, so its license notices and any source/relocation/reciprocity conditions must be evaluated. The absence of glibc at runtime does not mean “no libc license”; a musl-linked Rust binary still contains musl material, while a pure-Go binary contains Go runtime/standard-library material.
6. **HTML outputs/assets.** Prefer project-authored HTML/CSS/JavaScript or track every embedded asset as a distributed dependency. Include applicable notices in self-contained exported HTML as well as the executable's license surface.

## Rust and Go license considerations (not a platform decision)

No application dependency graph or project license exists yet in this workspace (`Cargo.toml`, `Cargo.lock`, `go.mod`, `go.sum`, and a tracked project `LICENSE` are absent), so these are current upstream observations, not approvals of future versions:

- The Rust project states that it is generally dual-licensed Apache-2.0 or MIT and that each release has generated copyright material covering the standard-library subset; use the exact toolchain release's `COPYRIGHT-library.html`, not only the repository's top-level summary ([Rust COPYRIGHT](https://github.com/rust-lang/rust/blob/main/COPYRIGHT)). Likely stack candidates currently use permissive terms—for example, [Ratatui is MIT](https://raw.githubusercontent.com/ratatui/ratatui/main/LICENSE), and [Axum is MIT](https://raw.githubusercontent.com/tokio-rs/axum/main/LICENSE)—but the selected versions and all transitives still require audit. A musl target adds musl's own MIT and component-level copyright/license inventory ([musl COPYRIGHT](https://git.musl-libc.org/cgit/musl/plain/COPYRIGHT)).
- The Go distribution uses a BSD-style license requiring binary redistributions to reproduce its notice, conditions, and disclaimer in documentation or other materials ([Go LICENSE](https://go.dev/LICENSE)). A standard-library-first CLI/HTTP/JSON implementation reduces external dependencies but does not remove Go runtime/standard-library notice duties. Likely optional TUI/CLI packages must be checked at the chosen version; their present repository licenses are not a substitute for a locked-graph audit.

Neither ecosystem has an automatic licensing advantage for the product. Platform selection should use the separate build, parsing-safety, performance, TUI/web, and maintainability evidence; both can satisfy a permissive-dependency policy if the resolved graph is controlled.

## Specification and release gates

The implementation-ready specification should require all of the following:

- A pinned-format provenance matrix mapping every decoder to authoritative CUBRID source and tests.
- No implementation dependency on `recovered/`; the oracle harness is a test-only, opt-in external path and never downloads or redistributes the artifact.
- A source-review check for suspicious transliteration of recovered pseudocode, old UI text/layout, or symbol names.
- An approved project license and a documented trademark/attribution statement before the first public binary.
- A locked dependency graph, SBOM, per-artifact license report, and denial policy for unknown/unapproved licenses.
- Exact notices embedded in the binary and web UI/export, plus downloadable release notices; verify `volmap licenses` in the static-binary acceptance test.
- Reproducible oracle records containing hashes and normalized facts, with sensitive paths and database values removed.
- A release check proving the old binary, archive, recovered source, CUBRID source objects, and unreviewed CUBRID third-party libraries are absent from the new executable and source package.

## Decisions requiring the owner or counsel

Before implementation uses the recovered oracle beyond already recorded behavior, establish:

1. Who owns `volmap-standalone`, how this copy was obtained, and whether the project has legitimate authority to execute/analyze it and its fixtures.
2. Which jurisdictions and contractual/confidentiality terms apply, and whether prior decompilation and future black-box experiments are authorized for this purpose.
3. Whether any existing recovered notes may be retained internally, who may access them, and whether they must be quarantined or deleted after the new evidence ledger is complete.
4. The Volmap Inspector copyright owner, outbound license, permitted dependency-license policy, and whether embedded license texts alone satisfy bare-binary distribution.
5. Whether public attribution wording and use of “CUBRID” are acceptable as descriptive compatibility statements.

Until those are answered, engineering can proceed under the proposed conservative boundary from the specifically Apache-labeled pinned CUBRID sources and project-generated fixtures. Recovered artifacts should stay local, quarantined, and outside build/test defaults and distributions; release still requires the unresolved owner/license decisions above.
