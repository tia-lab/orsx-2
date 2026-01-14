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

---

## Perf compare (after backfill optimization): default vs strict (1M rows, release)

Date (UTC): 2026-01-14T11:53:05Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Setup:
- Same as prior compare: base table has wrong physical order and missing `new_nullable`

Command(s):
- `ORSX_BIG_ROWS=1000000 cargo test -p orsx --release --test migrations_big_strict_compare -- --ignored --nocapture`

Outcome:
- Default: seed ~13.36s, migrate ~34.8ms
- Strict: seed ~12.85s, migrate ~7.70s

---

## Perf compare (after range catch-up): default vs strict (1M rows, release)

Date (UTC): 2026-01-14T12:08:37Z
Operator: Codex CLI (GPT-5.2)

Command(s):
- `ORSX_BIG_ROWS=1000000 cargo test -p orsx --release --test migrations_big_strict_compare -- --ignored --nocapture`

Outcome:
- Default: seed ~12.31s, migrate ~26.8ms
- Strict: seed ~13.43s, migrate ~8.13s

Notes:
- This run includes range-based changelog catch-up (no `ORDER BY pk::text`, no `ANY($1::uuid[])` lists).

---

## Online rewrite: worst-case writer (after range catch-up, UUID PK, 1M rows, release)

Date (UTC): 2026-01-14T12:08:37Z
Operator: Codex CLI (GPT-5.2)

Command(s):
- `ORSX_BIG_ROWS=1000000 ORSX_BIG_WRITER_ROWS=100000 ORSX_BIG_UPDATE_ROWS=500000 ORSX_BIG_UPDATE_BATCH=50000 ORSX_BIG_DELETE_ROWS=50000 ORSX_BIG_DELETE_BATCH=10000 cargo test -p orsx --release --test migrations_online_big_uuid -- --ignored --nocapture`

Outcome:
- Success/failure: success
- Total online rewrite duration (inside `online_rewrite_table`): ~23.968s
- Backfill: ~10.197s (backfill_rows reported: 1,090,000)
- Catchup: ~12.837s (drained_pk reported: 590,000)
- Cutover lock duration: ~205ms (budget 5000ms)

---

## Perf compare: default vs strict enforcement (1M rows, release, adaptive catch-up A/B)

Date (UTC): 2026-01-14T12:20:00Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Setup:
- Same as prior compares: base table created with wrong physical order and missing `new_nullable`.
- Note: this workload has no concurrent writers, so catch-up work is minimal; adaptive is expected to be neutral.

Command(s):
- `ORSX_BIG_ROWS=1000000 ORSX_ADAPTIVE_CHUNK=0 cargo test -p orsx --release --test migrations_big_strict_compare -- --ignored --nocapture`
- `ORSX_BIG_ROWS=1000000 ORSX_ADAPTIVE_CHUNK=1 cargo test -p orsx --release --test migrations_big_strict_compare -- --ignored --nocapture`

Outcome:
- Adaptive off:
  - Default: seed ~12.78s, migrate ~37.2ms
  - Strict:  seed ~12.88s, migrate ~7.50s
- Adaptive on:
  - Default: seed ~13.22s, migrate ~39.4ms
  - Strict:  seed ~12.89s, migrate ~7.60s

---

## Online rewrite: 200k UUID PK with inserts+updates+deletes (release, adaptive catch-up A/B)

Date (UTC): 2026-01-14T12:19:00Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Table characteristics:
- Rows: 200,000 seeded
- Columns: `id UUID` PK + 49 `INTEGER NOT NULL` + new `new_col INTEGER NOT NULL DEFAULT 0`
- Write load during migration:
  - inserts: 50,000 (batch 5,000)
  - updates: 50,000 (batch 5,000)
  - deletes: 50,000 (batch 5,000)

Command(s):
- `RUST_LOG=info ORSX_BIG_ROWS=200000 ORSX_BIG_WRITER_ROWS=50000 ORSX_BIG_WRITER_BATCH=5000 ORSX_BIG_UPDATE_ROWS=50000 ORSX_BIG_UPDATE_BATCH=5000 ORSX_BIG_DELETE_ROWS=50000 ORSX_BIG_DELETE_BATCH=5000 ORSX_ADAPTIVE_CHUNK=0 cargo test -p orsx --release --test migrations_online_big_uuid -- --ignored --nocapture`
- `RUST_LOG=info ORSX_BIG_ROWS=200000 ORSX_BIG_WRITER_ROWS=50000 ORSX_BIG_WRITER_BATCH=5000 ORSX_BIG_UPDATE_ROWS=50000 ORSX_BIG_UPDATE_BATCH=5000 ORSX_BIG_DELETE_ROWS=50000 ORSX_BIG_DELETE_BATCH=5000 ORSX_ADAPTIVE_CHUNK=1 cargo test -p orsx --release --test migrations_online_big_uuid -- --ignored --nocapture`

Outcome:
- Adaptive off:
  - online rewrite telemetry: total_ms ~3206, backfill_ms ~1910 (backfill_rows ~245000), catchup_ms ~1205 (drained_pk ~95000), cutover_lock_ms ~19
  - migrations wrapper total: ~3.29s; writer summary inserted=50000 updated=50000 deleted=50000; final rowcount=200000
- Adaptive on:
  - online rewrite telemetry: total_ms ~3130, backfill_ms ~1948 (backfill_rows ~245000), catchup_ms ~1087 (drained_pk ~95000), cutover_lock_ms ~20
  - migrations wrapper total: ~3.21s; writer summary inserted=50000 updated=50000 deleted=50000; final rowcount=200000

---

## Online rewrite: 200k UUID PK with inserts+updates+deletes (release, after backfill keyset CTE)

Date (UTC): 2026-01-14T12:31:20Z
Operator: Codex CLI (GPT-5.2)

DB:
- Postgres version: 16.11 (Debian 16.11-1.pgdg13+1)
- Storage: local Docker volume (dev)

Command(s):
- `RUST_LOG=info ORSX_BIG_ROWS=200000 ORSX_BIG_WRITER_ROWS=50000 ORSX_BIG_WRITER_BATCH=5000 ORSX_BIG_UPDATE_ROWS=50000 ORSX_BIG_UPDATE_BATCH=5000 ORSX_BIG_DELETE_ROWS=50000 ORSX_BIG_DELETE_BATCH=5000 ORSX_ADAPTIVE_CHUNK=0 cargo test -p orsx --release --test migrations_online_big_uuid -- --ignored --nocapture`

Outcome:
- Success/failure: success
- online rewrite telemetry: total_ms ~3406, backfill_ms ~1996 (backfill_rows ~245000), catchup_ms ~1307 (drained_pk ~95000), cutover_lock_ms ~38

Notes:
- A “single-statement catch-up” attempt using `WITH moved ... DELETE ... RETURNING` regressed catch-up time and was reverted; range-based catch-up remains the fast path.
