# Ticket 06 manual review record

Status: **not performed**. Complete this record from actual screen-reader and
visual use; automated browser tests and screenshots do not fill these results.
Review consumer `4f129d82274a71a19370d944939eb50069ada51d` or record the newer
consumer commit and rerun affected automated evidence.

From the Volmap checkout, start the synthetic fixture preview:

```sh
just user::serve-overlay-preview 192.168.4.2 7777
```

Open `http://192.168.4.2:7777/volume/0`. This preview is synthetic and does not
attach to a real CUBRID database. It supplies the supported observation controls
for manual use. Real-engine attachment remains separate work under ticket 07.

## Reviewer and environment

| Field | Recorded value |
| --- | --- |
| Reviewer | Not provided |
| Date | Not provided |
| Consumer commit | Not provided |
| Browser and version | Not provided |
| OS and version | Not provided |
| Screen reader and version | Not provided |
| Visual settings: zoom, contrast, reduced motion | Not provided |

## Screen-reader and keyboard cases

Record pass/fail and observations for each case. Identify the actual spoken
feedback where a label or announcement is relevant.

| Case | Result and observations |
| --- | --- |
| Enable observations by keyboard; identify source, mode, coverage and limitations | Not performed |
| Navigate Volume → Sector → Page; confirm focus and physical-page identity | Not performed |
| In Sector, operate the grid with arrow keys and open a page with Enter | Not performed |
| Switch State marks / LRU topology; distinguish storage facts, runtime evidence, exact membership and unknown values | Not performed |
| Pause, wait through age/expiry, and Resume; confirm state is discoverable and polling/age ticks do not repeatedly interrupt speech | Not performed |
| Refresh observations; confirm pending disabled control is understandable and focus remains usable | Not performed |
| Disable observations; confirm ordinary disk navigation and labels remain usable | Not performed |

Automated fault scenarios cover refused sources, restart and partial/ambiguous
captures. Record any of these actually exercised manually; do not mark unexecuted
manual scenarios passed merely because their browser tests pass.

## Visual cases

| Case | Result and observations |
| --- | --- |
| Distinguish resident, nonresident circle, unknown/ambiguity and dirty/flushing marks without relying only on color | Not performed |
| Read the Volume legend in the sidebar and the Sector legend; check zoom and scrolling | Not performed |
| Read nonresident circles in normal and forced-color modes; confirm keyboard focus remains visible | Not performed |
| Inspect known/unknown occupancy together with runtime marks and the LRU-mode replacement | Not performed |
| Confirm reduced-motion settings produce static marks and quiet updates | Not performed |

The retained `normal-sector-*.png` and `forced-sector-*.png` images illustrate
the new circles. Allocated known/unknown occupancy and fault variations also have
automated coverage in `web/e2e/observations.spec.ts`; report any additional fixture
setup needed for manual visual review rather than treating screenshots as proof.

## Outcome

Overall result: **not performed**.

Findings and required changes: Not provided.

Only actual completed reviewer evidence can close ticket 06's remaining manual
review item. Failed or inconclusive observations keep acceptance non-passing.
