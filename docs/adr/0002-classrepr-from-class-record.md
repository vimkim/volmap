# Class representations resolve through the class object's heap record

Record interpretation needs, for each record's reprid, the representation's
attribute list, domains, and layout. CUBRID persists representations in two parallel
places, and we resolve **only through the class object's own heap record — never the
system catalog**. The engine itself decodes instances this way
(`docs/record-interpretation-research.md` §3.5: `heap_classrepr_get` →
`or_get_classrep` over the class record), and the branch
`prototype/record-interpretation` proved the walk offline against every demodb
table.

The catalog looks like the obvious source but is the wrong one: its
`DISK_REPR`/`DISK_ATTR` records serve the query optimizer's statistics, not
instance decoding (research §3 preamble), and the catalog's extendible hash is dead
code — created but never inserted into or searched, so an offline reader walking it
finds nothing (§3.1). The
durable reprid→representation mapping the engine trusts starts from the class
record itself; old representations are found by walking the class record's
`ORC_REPRESENTATIONS_INDEX` substructure set (§3.5).

## Consequences

- The class-lookup cache is keyed `(volid, sectid)`, not `sectid`: a sector belongs
  to exactly one file and a class has exactly one heap file, but heap data pages of
  one class may sit in several permanent volumes (research §5.3).
- Enrichment is page-granular: one click interprets all home records of that page
  as one enrichment and one revision advance, because resolving the page's class
  record once amortizes over every record on the page.
- Catalog pages (`PAGE_CATALOG`) stay out of the interpretation path entirely; any
  future catalog-statistics view is a separate feature with its own evidence.
