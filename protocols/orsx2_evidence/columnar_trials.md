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
