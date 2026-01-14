# ORSX2 — Benchmark Results (APPEND-ONLY)

Rules:

- Append-only; never rewrite old entries.
- Each entry includes command lines and machine info.

---

## TEMPLATE ENTRY

Date (UTC):
Operator:

Machine:
- CPU:
- RAM:
- OS:

Command(s):
- `...`

Profile:
- debug/release:

Results:
- ...

---

## 2026-01-14 — planning bench (local)

Date (UTC): 2026-01-14  
Operator: codex-cli

Machine:
- CPU: Intel(R) Xeon(R) W-2295 CPU @ 3.00GHz (18c/36t)
- RAM: 503Gi
- OS: Linux 5.15.0-156-generic x86_64

Command(s):
- `cargo bench -p orsx --bench planning`

Profile:
- release (cargo bench)

Results (criterion):
- `planning/diff_schema_50_cols`: ~11.2 µs
- `planning/diff_schema_200_cols`: ~45.0 µs
- `planning/diff_schema_1000_cols`: ~220 µs
