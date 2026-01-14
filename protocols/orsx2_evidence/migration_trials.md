# ORSX2 — Migration Trials Log (APPEND-ONLY)

Rules:

- Append-only; never rewrite old entries.
- Record the exact schema, table size characteristics, command lines, and outcomes.

---

## TEMPLATE ENTRY

Date (UTC):
Operator:

DB:
- Postgres version:
- Storage:

Table characteristics:
- Rows:
- Row width estimate:
- Indexes:
- Write load during test:

Migration case:
- Change type:
- Strategy (offline/online):

Command(s):
- `...`

Outcome:
- Success/failure:
- Cutover lock duration:
- Backfill duration:
- Verification method and result:

---

## Online rewrite: add `NOT NULL` column default under writes

Date (UTC): 2026-01-14T10:30:24Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Table characteristics:
- Rows: ~100s (test scale)
- Row width estimate: small (3 columns: `id TEXT` PK, `name TEXT`, `age INTEGER`)
- Indexes: primary key
- Write load during test: concurrent inserts during migration

Migration case:
- Change type: add `age` with `DEFAULT 0` + enforce `NOT NULL`
- Strategy (offline/online): online shadow-table rewrite with trigger mirroring + changelog drain, 5s cutover budget

Command(s):
- `cargo test -p orsx --test migrations_online_rewrite -- --nocapture`

Outcome:
- Success/failure: success
- Cutover lock duration: enforced in code (budget: 5000ms); value not printed in this trial
- Backfill duration: bounded by small test scale (sub-second end-to-end test runtime)
- Verification method and result: post-migration `SELECT COUNT(*)` + `age IS NOT NULL` assertions; OK

---

## Online rewrite: big table UUID PK (200k rows fast-pass, release)

Date (UTC): 2026-01-14T10:34:00Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Table characteristics:
- Rows: 200,000 seeded + 20,000 concurrent inserts (writer)
- Row width estimate: ~50 columns (`id UUID` PK + 49 `INTEGER NOT NULL` + new `INTEGER NOT NULL DEFAULT 0`)
- Indexes: primary key
- Write load during test: concurrent inserts during migration (batch inserts via `uuid_generate_v1mc()` + `generate_series`)

Migration case:
- Change type: add `new_col INTEGER NOT NULL DEFAULT 0` (rewrite-class change)
- Strategy (offline/online): online shadow-table rewrite with trigger mirroring + changelog drain, 5s cutover budget

Command(s):
- `ORSX_BIG_ROWS=200000 ORSX_BIG_WRITER_ROWS=20000 cargo test -p orsx --release --test migrations_online_big_uuid -- --ignored --nocapture`

Outcome:
- Success/failure: success
- Cutover lock duration: enforced in code (budget: 5000ms); value not printed in this trial
- Backfill duration: seed ~2.84s; migration total ~3.46s (release build; includes backfill + catchup + cutover)
- Verification method and result: `COUNT(*)` >= seeded and `new_col IS NULL = 0`; OK

---

## Online rewrite: big table UUID PK (1M rows, release)

Date (UTC): 2026-01-14T10:48:43Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Table characteristics:
- Rows: 1,000,000 seeded + 100,000 concurrent inserts (writer) → final 1,100,000
- Row width estimate: ~51 columns (`id UUID` PK + 49 `INTEGER NOT NULL` + new `INTEGER NOT NULL DEFAULT 0`)
- Indexes: primary key
- Write load during test: concurrent inserts during migration (batch inserts via `uuid_generate_v1mc()` + `generate_series`)

Migration case:
- Change type: add `new_col INTEGER NOT NULL DEFAULT 0` (rewrite-class change)
- Strategy (offline/online): online shadow-table rewrite with trigger mirroring + changelog drain, 5s cutover budget

Command(s):
- `ORSX_BIG_ROWS=1000000 ORSX_BIG_WRITER_ROWS=100000 cargo test -p orsx --release --test migrations_online_big_uuid -- --ignored --nocapture`

Outcome:
- Success/failure: success
- Cutover lock duration: ~1012ms (budget 5000ms)
- Backfill duration: ~21.324s (backfill_rows reported: 1,100,000)
- Catchup (pre-lock) duration: ~3.559s (drained_pk reported: 90,000)
- Total online rewrite duration (inside `online_rewrite_table`): ~26.014s (migrations wrapper logged ~26.086s)
- Verification method and result: `new_col IS NULL = 0` and final rowcount; OK

---

## Online rewrite: worst-case writer (UUID PK, 1M rows, release)

Date (UTC): 2026-01-14T11:04:49Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Table characteristics:
- Rows: 1,000,000 seeded
- Columns: `id UUID` PK + 49 `INTEGER NOT NULL` + new `INTEGER NOT NULL DEFAULT 0`
- Write load during test (during migration):
  - inserts: 100,000
  - updates: 500,000 (batched updates of 50,000 rows per statement)
  - deletes: 50,000 (batched deletes of 10,000 rows per statement)

Migration case:
- Change type: add `new_col INTEGER NOT NULL DEFAULT 0` (rewrite-class change)
- Strategy (offline/online): online shadow-table rewrite; trigger records PK changelog; migration process applies changelog to shadow; 5s cutover budget

Command(s):
- `ORSX_BIG_ROWS=1000000 ORSX_BIG_WRITER_ROWS=100000 ORSX_BIG_UPDATE_ROWS=500000 ORSX_BIG_UPDATE_BATCH=50000 ORSX_BIG_DELETE_ROWS=50000 ORSX_BIG_DELETE_BATCH=10000 cargo test -p orsx --release --test migrations_online_big_uuid -- --ignored --nocapture`

Outcome:
- Success/failure: success
- Cutover lock duration: ~2101ms (budget 5000ms)
- Backfill duration: ~24.132s (backfill_rows reported: 1,080,000)
- Catchup (pre-lock) duration: ~25.211s (drained_pk reported: 590,000)
- Total online rewrite duration (inside `online_rewrite_table`): ~52.080s
- Writer summary: inserted=100,000 updated=500,000 deleted=50,000; final rowcount=1,050,000
- Verification method and result: `new_col IS NULL = 0` and final rowcount matches inserts/deletes; OK

---

## Correctness: strict schema enforcement + rename

Date (UTC): 2026-01-14T11:33:59Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Cases:
- Enforced physical column order (rewrite required) + data preserved
- Enforced exact columns (fails unless `allow_destructive_drops=true`; rewrite removes extras; backup retains dropped data)
- `rename_from` safe rename (`ALTER TABLE ... RENAME COLUMN ...`) + data preserved

Command(s):
- `cargo test -p orsx --test migrations_strict_correctness`

Outcome:
- Success/failure: success (4 tests)

---

## Perf compare: default vs strict enforcement (200k rows, release)

Date (UTC): 2026-01-14T11:40:00Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Table characteristics:
- Rows: 200,000
- Columns: UUID PK + 49 `INTEGER NOT NULL` + new `new_nullable INTEGER NULL`
- Setup: base table created with wrong physical order (c02 before c01) and missing `new_nullable`

Configs compared:
- Default: `enforce_column_order=false`, `enforce_exact_columns=false` (safe alter path)
- Strict: `enforce_column_order=true`, `enforce_exact_columns=true`, `allow_destructive_drops=true` (forces online rewrite due to order mismatch)

Command(s):
- `ORSX_BIG_ROWS=200000 cargo test -p orsx --release --test migrations_big_strict_compare -- --ignored --nocapture`

Outcome:
- Default: seed ~2.45s, migrate ~30.9ms (ALTER TABLE ADD COLUMN)
- Strict: seed ~2.56s, migrate ~2.75s (online rewrite to fix order)

---

## Perf compare: default vs strict enforcement (1M rows, release)

Date (UTC): 2026-01-14T11:45:05Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Table characteristics:
- Rows: 1,000,000
- Columns: UUID PK + 49 `INTEGER NOT NULL` + new `new_nullable INTEGER NULL`
- Setup: base table created with wrong physical order (c02 before c01) and missing `new_nullable`

Configs compared:
- Default: `enforce_column_order=false`, `enforce_exact_columns=false` (safe alter path)
- Strict: `enforce_column_order=true`, `enforce_exact_columns=true`, `allow_destructive_drops=true` (forces online rewrite due to order mismatch)

Command(s):
- `ORSX_BIG_ROWS=1000000 cargo test -p orsx --release --test migrations_big_strict_compare -- --ignored --nocapture`

Outcome:
- Default: seed ~12.64s, migrate ~28.6ms (ALTER TABLE ADD COLUMN)
- Strict: seed ~12.08s, migrate ~16.97s (online rewrite to fix order)
