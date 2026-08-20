-- Synthetic record-interpretation corpus for the CUBRID physical format at
-- e1e651debf6cc100172bde96603b17424f9c135a.
--
-- The sibling `e1e651de` corpus pins allocation and page structure. This one
-- pins the bytes a record *interpretation* must decode: every attribute type
-- version one decodes, the typed placeholder types it refuses to decode, an
-- out-of-row stub, an OBJECT reference, and a class carrying an old
-- representation.
--
-- Column order is deliberate. CUBRID stores fixed-width attributes in the
-- record's fixed region and variable-width attributes through the offset
-- table, so a mixed declaration order proves the decoder uses each attribute's
-- own location and position from the representation rather than declaration
-- order.

-- Every type version one decodes. NUMERIC and CHAR are declared between
-- genuinely fixed types on purpose: both live in the *variable* region despite
-- having a fixed precision, which is the single easiest decoding mistake.
CREATE TABLE interp_scalars (
  id           INTEGER PRIMARY KEY,
  c_short      SHORT,
  c_char       CHAR(8),
  c_bigint     BIGINT,
  c_numeric    NUMERIC(10,2),
  c_float      FLOAT,
  c_double     DOUBLE,
  c_monetary   MONETARY,
  c_date       DATE,
  c_time       TIME,
  c_timestamp  TIMESTAMP,
  c_datetime   DATETIME,
  c_varchar    VARCHAR(4000),
  c_nchar      NCHAR(8),
  c_varnchar   NCHAR VARYING(64)
);

-- Row 1: every column bound, so the decoder must read a full fixed region, a
-- full offset table, and a bound-bit vector with no unset bits.
INSERT INTO interp_scalars VALUES (
  1,
  -32768,
  'fixed8ch',
  -9223372036854775807,
  -12345678.90,
  1.25,
  -2.5,
  1234.56,
  DATE '2026-08-21',
  TIME '13:45:59',
  TIMESTAMP '2026-08-21 13:45:59',
  DATETIME '2026-08-21 13:45:59.123',
  'plain varchar value',
  N'nchar8ch',
  N'varnchar value'
);

-- Row 2: every nullable column unbound. Fixed attributes record their NULL in
-- the bound-bit vector; variable attributes record it as a zero-length extent.
INSERT INTO interp_scalars VALUES (
  2, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
  NULL, NULL, NULL, NULL, NULL, NULL, NULL
);

-- Row 3: a highly compressible VARCHAR long enough for the engine to store it
-- LZ4-compressed and inline, which switches the string prefix to its 255
-- marker plus compressed and decompressed lengths. Everything else stays small
-- so the row is never demoted out of row.
INSERT INTO interp_scalars VALUES (
  3, 3, 'row3char', 3, 3.00, 3, 3, 3,
  DATE '2026-01-03', TIME '03:03:03', TIMESTAMP '2026-01-03 03:03:03',
  DATETIME '2026-01-03 03:03:03.003',
  REPEAT('compressible-varchar-payload-', 40),
  N'row3nch', N'row3 varnchar'
);

-- A bound OBJECT column. The referenced class must be declared
-- DONT_REUSE_OID: this engine creates classes REUSE_OID by default, and a
-- REUSE_OID class is non-referable and cannot be an attribute domain. That
-- also gives the corpus one heap that is not a reuse-slots heap.
CREATE TABLE interp_target (
  id    INTEGER PRIMARY KEY,
  label VARCHAR(16)
) DONT_REUSE_OID;

INSERT INTO interp_target VALUES (1, 'target-one');

CREATE TABLE interp_reference (
  id     INTEGER PRIMARY KEY,
  target interp_target
);

INSERT INTO interp_reference
SELECT 1, t FROM interp_target t WHERE t.id = 1;

INSERT INTO interp_reference VALUES (2, NULL);

-- Types version one refuses to decode. Each must render as a typed placeholder
-- carrying name, type, offset, and length, and must never emit value bytes.
CREATE TABLE interp_placeholders (
  id       INTEGER PRIMARY KEY,
  c_set    SET(INTEGER),
  c_seq    SEQUENCE(VARCHAR(10)),
  c_enum   ENUM('alpha', 'beta'),
  c_bit    BIT(16),
  c_varbit BIT VARYING(64),
  c_json   JSON
);

INSERT INTO interp_placeholders VALUES (
  1, {1, 2, 3}, {'a', 'b'}, 'beta', B'1010101010101010', X'abcdef',
  '{"k": 1}'
);

INSERT INTO interp_placeholders VALUES (2, NULL, NULL, NULL, NULL, NULL, NULL);

-- An out-of-row stub. BIT VARYING is intentional: unlike VARCHAR its
-- serialized size is not reduced by the string-compression path, so the value
-- reliably exceeds the demotion threshold and leaves a 16-byte stub in the
-- home record. The stub arm must win over the placeholder arm for BIT VARYING.
CREATE TABLE interp_oos (
  id        INTEGER PRIMARY KEY,
  label     VARCHAR(32),
  out_value BIT VARYING
);

INSERT INTO interp_oos VALUES (1, 'inline', REPEAT(X'11', 64));
INSERT INTO interp_oos VALUES (2, 'out-of-row', REPEAT(X'22', 32768));

-- A class carrying an old representation. The row inserted before the ALTER
-- keeps the original representation id, so interpreting it requires walking
-- the class record's old-representation set instead of its current
-- representation.
CREATE TABLE interp_altered (
  id        INTEGER PRIMARY KEY,
  pre_alter INTEGER,
  label     VARCHAR(32)
);

INSERT INTO interp_altered VALUES (1, 11, 'before-alter');

ALTER TABLE interp_altered ADD COLUMN post_alter INTEGER;

INSERT INTO interp_altered VALUES (2, 22, 'after-alter', 222);

COMMIT WORK;
