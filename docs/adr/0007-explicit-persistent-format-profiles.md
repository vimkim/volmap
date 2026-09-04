# Select persistent format profiles explicitly

CUBRID `develop` and `feat/oos` assign different meanings to the same persistent page-type ordinals, but their volume headers do not identify the source branch. Volmap therefore applies one explicit format profile to an entire inspection, defaults its CLI to the pinned `develop` profile, and retains the pinned `feat-oos` profile behind `--format-profile feat-oos`.

Volmap never switches profiles from byte heuristics. When the selected profile rejects bounded evidence that the other profile validates, a structured diagnostic may suggest the exact retry flag without changing any interpreted fact; OOS-only operations are rejected under `develop`.
