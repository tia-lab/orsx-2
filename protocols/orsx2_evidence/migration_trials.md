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

