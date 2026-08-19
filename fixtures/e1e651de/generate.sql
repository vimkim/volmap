-- Synthetic acceptance corpus for the CUBRID physical format at
-- e1e651debf6cc100172bde96603b17424f9c135a.
--
-- BIT VARYING is intentional: unlike VARCHAR, its serialized size is not
-- changed by the string-compression path.  The sizes mirror the pinned OOS
-- SQL tests and exercise inline, single-chunk, and multi-chunk values.

CREATE TABLE fixture_rows (
  id INTEGER PRIMARY KEY,
  lookup_key VARCHAR(128) NOT NULL,
  first_value BIT VARYING,
  second_value BIT VARYING
);

CREATE INDEX fixture_rows_lookup ON fixture_rows (lookup_key);

INSERT INTO fixture_rows VALUES
  (1, 'inline', REPEAT(X'11', 100), NULL);

-- About 5 KiB: largest-first demotion stores the 3 KiB value in OOS and
-- leaves the 2 KiB value inline after reaching the four-record target.
INSERT INTO fixture_rows VALUES
  (2, 'single-chunk', REPEAT(X'22', 3000), REPEAT(X'33', 2000));

-- The first value spans multiple OOS chunk records and pages.
INSERT INTO fixture_rows VALUES
  (3, 'multi-chunk', REPEAT(X'44', 32768), REPEAT(X'55', 128));

-- A fixed-width value cannot be demoted.  At 17,500 bytes this is a plain,
-- supported REC_BIGONE record because it has no OOS-backed attribute.
CREATE TABLE fixture_bigone (
  id INTEGER PRIMARY KEY,
  fixed_value BIT(140000)
);

INSERT INTO fixture_bigone VALUES (1, B'1');

-- Populate additional leaf pages and make allocation/sector maps nontrivial.
CREATE TABLE fixture_dense (
  id INTEGER PRIMARY KEY,
  lookup_key VARCHAR(128),
  payload BIT VARYING
);

CREATE INDEX fixture_dense_lookup ON fixture_dense (lookup_key);

INSERT INTO fixture_dense
SELECT ROWNUM,
       'key-' || CAST(ROWNUM AS VARCHAR(20)),
       REPEAT(X'66', 256)
  FROM db_root
CONNECT BY LEVEL <= 256;

COMMIT WORK;
