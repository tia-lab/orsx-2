# ORSX2 — Columnar Retrieval Trials (APPEND-ONLY)

This file is **append-only**.

Rules:

- Never edit or reorder existing entries (only append new entries at the end).
- Every entry must include: timestamp, machine, Postgres version, command, dataset shape, results.
- If a change regresses performance, record it (do not delete “bad” results).

---

## Template (copy for each trial)

```
### YYYY-MM-DD HH:MM:SSZ — <short label>

Machine:
- CPU:
- RAM:
- OS:
- Storage:

Postgres:
- Version:
- Config deltas (if any):

Command:
- (exact command line)
- Profile: release/debug

Dataset / query:
- Rows:
- Columns:
- Types:
- NULL rate:
- Query:

Implementation:
- Reader: COPY BINARY / row-wise
- Workspace reuse: yes/no

Results:
- Total wall time:
- Throughput (rows/s):
- Peak RSS (if measured):
- Notes:
```

### 2026-01-14 13:38:58Z — COPY BINARY vs row-wise (release, mixed types)

Machine:
- CPU: Intel(R) Xeon(R) W-2295 CPU @ 3.00GHz (36 vCPU / 18 cores)
- RAM: 503GiB
- OS: Linux 5.15.0-156-generic x86_64

Postgres:
- Version: PostgreSQL 16.11 (Debian 16.11-1.pgdg13+1)

Command:
- `ORSX_COL_ROWS=100000 ORSX_COL_COLS=50 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`
- `ORSX_COL_ROWS=100000 ORSX_COL_COLS=500 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`

Dataset / query:
- Rows: 100,000
- Columns:
  - 50 cols: `id BIGINT` + 47×`DOUBLE PRECISION NULL` + `t TEXT NULL` + `by BYTEA NULL`
  - 500 cols: `id BIGINT` + 497×`DOUBLE PRECISION NULL` + `t TEXT NULL` + `by BYTEA NULL`
- NULL rate: `~10%` on numeric/text/bytea columns (`gs % 10 == 0`)
- Query: `SELECT <all columns> FROM orscol_perf ORDER BY id`

Implementation:
- Reader (COPY): `COPY (SELECT ...) TO STDOUT (FORMAT BINARY)` parsed into column buffers
- Reader (row-wise): `sqlx::query(...).fetch(...)` and `try_get` for every column in every row (checksummed)

Results:
- 100k × 50 cols:
  - COPY: `285.785824ms`
  - Row-wise: `233.005854ms`
- 100k × 500 cols:
  - COPY: `2.673047588s`
  - Row-wise: `2.20493399s`

Notes:
- Timings cover retrieval+decode only (table creation + insert excluded).
- Current COPY parser is correctness-first and still does per-field conversions/copies; further optimizations are likely required to beat row-wise on this workload.

### 2026-01-14 13:43:06Z — COPY BINARY optimization pass (release, mixed types)

Change:
- Removed per-row `finish_row` scan across all columns (var offsets are now pushed during field decode).
- Reduced per-field fixed-width churn (`extend_from_slice` instead of `resize+copy`) and reduced buffer compaction frequency.

Command:
- `ORSX_COL_ROWS=100000 ORSX_COL_COLS=50 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`
- `ORSX_COL_ROWS=100000 ORSX_COL_COLS=500 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`

Results:
- 100k × 50 cols:
  - COPY: `277.914523ms` (was `285.785824ms`)
  - Row-wise: `270.28488ms` (was `233.005854ms`; row-wise workload includes full-column decoding)
- 100k × 500 cols:
  - COPY: `2.653694687s` (was `2.673047588s`)
  - Row-wise: `2.153661318s` (was `2.20493399s`)

### 2026-01-14 13:51:25Z — PgBe fixed-width storage (release, mixed types)

Change:
- Fixed-width numeric columns produced by COPY BINARY are now stored in Postgres big-endian form (`FixedEncoding::PgBe`) and ORSXCOL emits `encoding_id=1` for those columns.
- Encoding is set once per column per batch (not per cell) to avoid O(rows*cols) stores.

Command:
- `ORSX_COL_ROWS=100000 ORSX_COL_COLS=50 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`
- `ORSX_COL_ROWS=100000 ORSX_COL_COLS=500 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`

Results (COPY time is just `next_batch_into`, row-wise is full per-cell `try_get` loop):
- 100k × 50 cols:
  - COPY: `282.384929ms`
  - Row-wise: `270.711555ms`
- 100k × 500 cols:
  - COPY: `2.57300999s`
  - Row-wise: `2.338062899s`

Notes:
- The perf test also computes identical columnar vs row-wise checksums (outside the COPY timing) to validate correctness at scale.

### 2026-01-14 14:07:01Z — Fair baseline: row-wise builds the same columnar batch (release)

Change:
- Updated perf methodology: the row-wise path now builds a `ColumnarBatch` too (same schema, same NULLs), instead of only decoding into locals.

Command:
- `ORSX_COL_ROWS=100000 ORSX_COL_COLS=50 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`
- `ORSX_COL_ROWS=100000 ORSX_COL_COLS=500 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`

Results (end-to-end retrieval + building the in-memory columnar batch):
- 100k × 50 cols:
  - COPY → `ColumnarBatch`: `289.166555ms`
  - Row-wise → `ColumnarBatch`: `246.973727ms`
- 100k × 500 cols:
  - COPY → `ColumnarBatch`: `2.566705801s`
  - Row-wise → `ColumnarBatch`: `2.780923222s`

Notes:
- Both paths compute matching checksums (id/f64/text/bytea) to validate correctness at scale.
- Interpretation: for very wide tables, COPY BINARY starts to win when you actually need to materialize a columnar batch.

### 2026-01-14 14:17:08Z — Typed fixed-width buffers (release)

Change:
- Fixed-width columns now store typed buffers instead of raw byte buffers:
  - `BIGINT -> Vec<i64>`, `INTEGER -> Vec<i32>`, `DOUBLE PRECISION -> Vec<u64>` (bits), `UUID -> Vec<[u8;16]>`, `TIMESTAMPTZ -> Vec<i64>` (unix micros).
- COPY BINARY path decodes fixed-width values into the typed buffers.

Command:
- `ORSX_COL_ROWS=100000 ORSX_COL_COLS=50 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`
- `ORSX_COL_ROWS=100000 ORSX_COL_COLS=500 cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`

Results:
- 100k × 50 cols:
  - COPY → `ColumnarBatch`: `297.726311ms`
  - Row-wise → `ColumnarBatch`: `289.757402ms`
- 100k × 500 cols:
  - COPY → `ColumnarBatch`: `2.613487862s`
  - Row-wise → `ColumnarBatch`: `2.790430798s`

Notes:
- Checksums match between both paths (correctness).
- Wide-table COPY advantage persists; narrow-table remains close.
