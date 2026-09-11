# 06: Pin large boundary recipes

**What to build:** Both repositories can verify the page-cap and byte-cap boundaries against identical generated streams without vendoring megabytes. The contract defines deterministic recipes, the corpus pins the SHA-256 of each generated stream with its expected outcome, and the CUBRID serializer proves it refuses the page or byte that would exceed a cap.

**Blocked by:** 04 — Pin truncated scans, footer reservation and additive-evolution cases.

**Status:** ready-for-agent

- [ ] The contract defines the recipe format (a deterministic page identity and state sequence from a seed and a count, canonical framing, stated hello, header and footer values) precisely enough that an independent implementation produces identical bytes.
- [ ] Recipes and pinned hashes exist for: a complete scan of exactly 65,536 pages (`accepted-complete`, not truncated); a stream of 65,537 pages (`discarded`, page cap exceeded); a stream exceeding 64 MiB (`discarded`, byte cap exceeded). The contract states that the producer never emits the last two, so those recipes exist for the consumer's enforcement tests only.
- [ ] Tests generate the 65,536-page stream through the serializer and its scan budget and match the pinned hash; prove the budget refuses the 65,537th page and refuses the page that would breach 64 MiB less the footer bound while still admitting the footer; and regenerate the checksum file and manifest with the corpus revision incremented.
- [ ] Execution evidence with named cases, nonzero counts and generation runtimes is recorded in this ticket's comments with the exact commit.
