-- Synthetic develop disk corpus. No production data or OOS operations.
CREATE TABLE fixture_rows (id INTEGER PRIMARY KEY, label VARCHAR(80), payload BIT VARYING);
CREATE INDEX fixture_label ON fixture_rows(label);
INSERT INTO fixture_rows VALUES (1, 'inline', REPEAT(X'11', 128));
INSERT INTO fixture_rows VALUES (2, 'overflow', REPEAT(X'22', 32768));
CREATE TABLE fixture_dense (id INTEGER PRIMARY KEY, payload BIT VARYING);
INSERT INTO fixture_dense SELECT ROWNUM, REPEAT(X'33', 256) FROM db_root CONNECT BY LEVEL <= 256;
COMMIT WORK;
SELECT COUNT(*) FROM fixture_rows;
SELECT COUNT(*) FROM fixture_dense;
