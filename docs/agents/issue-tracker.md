# Issue tracker: Local Markdown

Issues and specs live under .scratch/<feature-slug>/.

- Specification: spec.md.
- Implementation tickets: issues/NN-slug.md, one per file,
  numbered from 01 in dependency order.
- Each ticket records Status and Blocked by near the top.
  Blocked by includes external prerequisites where applicable.
- Triage roles come from triage-labels.md.
- Append discussion under a Comments heading.

Publishing means writing local Markdown, not creating GitHub or JIRA issues.
Fetching means reading the referenced local ticket and its discussion.

Work the frontier: tickets whose prerequisites are satisfied.
Ready-for-agent describes triage readiness, not completion or satisfaction
of external dependencies.

## Wayfinder maps

A decision map lives in map.md with numbered child files under issues/.
Children record Type, Status and Blocked by. Claim work with Status: claimed.
Resolve it by appending an Answer, setting Status: resolved and adding a
decision summary and context pointer to the map.

Decision tickets and implementation tickets are separate.
Preserve existing historical filenames and completed maps.
