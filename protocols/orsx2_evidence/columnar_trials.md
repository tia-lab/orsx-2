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

