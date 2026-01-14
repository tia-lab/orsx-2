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
