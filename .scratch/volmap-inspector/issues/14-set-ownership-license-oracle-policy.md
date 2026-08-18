Type: grilling
Status: resolved
Blocked by: 03

# Set ownership, licensing, and recovered-oracle policy

## Question

Using the licensing/provenance research, what owner-approved policy governs the new project before implementation and eventual distribution? Decide the copyright owner and outbound license; confirm authority to retain and execute `volmap-standalone`; approve or reject the proposed quarantined black-box-oracle boundary; decide access/retention for `recovered/`; set CUBRID attribution and descriptive trademark wording; and state when counsel review is mandatory. The answer must distinguish confirmed owner decisions from legal questions still delegated to counsel and must block release wherever authority remains unresolved.

## Comments

## Answer

### Confirmed owner policy

- **Owner:** CUBRID owns Volmap Inspector because Vimkim is creating it as a CUBRID engineer during CUBRID work time. Vimkim is recorded as the initial author, not the copyright owner. The exact CUBRID legal-entity name remains a mandatory pre-publication confirmation.
- **License:** Apache License 2.0 is the intended outbound license from the beginning, including during internal development, subject to CUBRID's written approval before any public release.
- **Distribution phase:** development and use are company-internal first. No public source or binary release is authorized by this decision.

### Source and recovered-artifact boundary

- The **format authority** is the pinned CUBRID source plus company-generated, provenance-recorded fixtures.
- `volmap-standalone` and `recovered/` came from another CUBRID employee. Vimkim's current authority is recorded as internal-only; it does not establish authority to publish, redistribute, or make those artifacts routine implementation inputs.
- Recovered artifacts are not required for the new tool. They must not be linked, copied, translated, packaged, published, or used by default tests. Any further execution or inspection requires explicit approval from the originating employee and the appropriate CUBRID manager.
- If such approval is granted, use is limited to an optional **behavioral oracle**: black-box comparison of normalized facts on non-sensitive company fixtures. Full textual/ANSI output, UI structure, symbols, pseudocode, control flow, or expressive implementation details are not copied.
- Before implementation scaffolding begins in this repository, the currently gitignored recovered executable and recovery directory must be relocated through a company-approved process to a restricted location outside the implementation repository. This ticket authorizes the policy, not a file move or deletion.

### Attribution, dependency, and public-release gates

- Internal notices may say `Copyright CUBRID` with Vimkim identified as initial author. Before publication, CUBRID must supply the exact legal owner name and approved descriptive compatibility/trademark language. The project must not claim to be an officially supported CUBRID product without company approval.
- Release artifacts must carry Apache-2.0 and applicable CUBRID/source attributions, exact third-party notices, an embedded `volmap licenses`/web About surface, a locked dependency graph, and an SBOM.
- Unknown licenses are rejected. GPL, AGPL, LGPL, MPL, custom, or otherwise reciprocal/restricted terms require explicit company legal review and approval before entering the release graph.
- Public release is blocked until written CUBRID approval covers the outbound license, exact owner and trademark wording, recovered-artifact exclusion, notice delivery, dependency/license report, and SBOM. Counsel review is also mandatory before any public use of recovered-oracle evidence or any additional reverse engineering.
