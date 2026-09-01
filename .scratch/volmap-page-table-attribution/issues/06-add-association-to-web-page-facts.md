Type: task
Status: open
Blocked by: 05

# Add association to the web Page facts

## Question

Can the existing web Page workspace show the shared production association clearly without changing its visual design or inventing adapter-specific semantics?

Add `Class/table`, `Class OID`, `File`, and `File role` rows to the existing Page facts panel, consuming only the shared Page projection. Render resolved, non-applicable/internal/deferred, and unresolved states truthfully; keep numeric identities visible when names fail; safely escape identifier text; and preserve current layout, navigation, page detail enrichment, JSON API, and standalone HTML behavior. Do not add sector labels/cards or redesign the workspace. Add focused web API/DOM-source/HTML-export tests for resolved and every fail-closed state, long/non-ASCII names, escaping, and unchanged existing facts. Format, run the focused web tests, and commit the ticket's production change on `feat/page-table-attribution`.

## Comments
