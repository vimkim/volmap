# Ticket 05 two-axis review

Engine base: e5f3cdf86908b8d23d4e09d28d22265bf240f7ad.
Reviewed staged engine tree: 94b0d8e3f987592f3539865803d0273db486e869.
External base: a8d61f27443e3c8b56bcaf94b23a8b88dff9069a.
Reviewed staged external tree: 5b1f7792534f65b85b09fd4de21afe9a9badd5ea.
Each reviewer independently reviewed both staged diffs against those bases.

## Standards

Standards: no documented breaches or material judgment-call smells found in the staged engine and external testcase changes.

The native fixture preserves engine page-access conventions, uses explicit fix/unfix handling and normal temporary-file cleanup, and keeps `memory_wrapper.hpp` last. Changes to existing code have semantic purpose. Public reproduction instructions use CMake, CTest and CTP commands.

The separate native workload, socket controller and CTP wrapper have distinct responsibilities; their separation is justified. Synchronization and workload bounds fail explicitly when preconditions cannot be established. No production instrumentation or unnecessary abstraction was added.

Standards axis: 0 findings. Final test and gate results remain with the parent.

## Spec

No Spec-axis implementation findings in either staged diff.

- Native allocation, flush/set-dirty operations, retained WRITE fix, and acknowledgements satisfy “establish residency and dirty-state preconditions independently of inspector responses.”
- Capacity-driven replacement proves target removal before explicit cleanup. Cleanup excludes the still-allocated target; private ownership and native absence checks maintain the required no-refix condition through complete and partial observations.
- Socket assertions distinguish complete omission from partial unknown. The observer performs no native page operations.
- The actual external testcase supplies standard CTP invocation and separately exercises unmodified server attachment, parameter-off behavior, and restart.
- The slow-drain adjustment preserves timing requirements while accommodating kernel allocation boundaries; it introduces no scope creep.

Release/final CTP execution, full-suite completion, and exact producer/testcase revisions remain pending gate work, not code defects.

Standards: 0 findings; Spec: 0 findings. Neither axis has an unresolved implementation issue.

## Cleanup correction review

Final reviewed tree: 890d5c4c6e505ef93f55abb86d66bfe72e136420.
Both axes independently reviewed the cleanup correction from the initial tree.

Standards axis: 0 findings.

The retry loop retains the shared five-second deadline, excludes the target before attempting cleanup, and requires a native cache miss before advancing. Logging remains bounded to once per page. The added `<thread>` include preserves `memory_wrapper.hpp` as the final include, and documentation accurately describes the correction. No material baseline smells found.

Spec: No spec concerns. Cleanup still excludes the target, follows independent eviction proof, and requires an actual native cache-only miss. Retries share the existing aggregate five-second deadline; `yield()` supplies no precondition oracle.

The correction strengthens evidence without relaxing producer limits. Final Debug/Release and CTP reruns must cover the revised tree.
