# ORSX2 — Columnar Retrieval Spec (TEMPLATE)

Status: DRAFT  
Owner: (Designer)  
Applies to: `orsx2` crate

## 1) Identification

- Component: `orsx2::columnar`
- Purpose: ultra-fast **columnar** retrieval for large scans (mixed types, NULLs), suitable for **binary API transfer**.
- Non-scope:
  - ORM / query builder
  - cross-database support
  - query planning/optimization (Postgres does that)
  - general-purpose Arrow implementation (interop is allowed, but not required)

## 2) Primary use-cases and bounds (mandatory)

### 2.1 Scan sizes

- Typical scan: 10k..100k rows
- Large scan: 1M rows (must be possible; may require streaming/batching)
- Columns: 50..500 (mixed scalar + string/binary), NULLs possible

### 2.2 Constraints

- Output must be **binary-friendly** for APIs (no JSON as the primary path).
- Deterministic output ordering: rows and columns match query order.
- Safety: never panic; never read past buffers; checked arithmetic for sizes.

## 3) High-level design (mandatory)

The columnar system provides:

1. A **batch** type: fixed schema + `row_count` + per-column buffers.
2. A **reader** that can fill a batch from Postgres with minimal overhead.
3. A **binary envelope** to serialize a batch for transport or caching.

### 3.1 Retrieval paths (two-tier; fast path required)

- **Fast path (required)**: Postgres `COPY (SELECT ...) TO STDOUT (FORMAT BINARY)` and parse the binary stream into column buffers.
  - Goal: minimize per-value overhead and allocations.
  - Implementation note (v1): prefer a contiguous read buffer (`Vec<u8>` + cursor) for parsing; chunk-walking is allowed only if evidence shows a win for the target workloads.
  - Contract: the query must be syntactically wrapped into COPY; caller is responsible for stable column order.
- **Fallback path (allowed)**: row-wise decode (`sqlx::Row`) into columns.
  - Contract: correctness-first; used for portability/debugging; not the performance target.
- **Auto (recommended)**: choose COPY vs row-wise by expected shape (columns + expected rows) while always outputting the same `ColumnarBatch`.

## 4) Public API contract (mandatory)

### 4.1 Core types (indicative)

- `ColumnarBatch`
  - `row_count: usize`
  - `columns: Vec<Column>`
- `Column` (typed payload + validity)
  - Fixed-width: `I16`, `I32`, `I64`, `F32`, `F64`, `Bool`, `Uuid`, `TimestampTz`
  - Variable-width: `Bytes`, `Utf8`
  - Optional future: dictionary encoding for `Utf8`, `Bytes`
- `ValidityBitmap`
  - bit=1 means value present, bit=0 means NULL

### 4.2 Workspace + zero-copy patterns (mandatory)

For hot paths, the API must follow:

- Allocating convenience: `fn read_batch(...) -> Result<ColumnarBatch>`
- Non-allocating: `fn read_batch_into(..., out: &mut ColumnarBatch) -> Result<()>`
- Workspace variant:
  - `fn read_batch_into_with_workspace(..., out: &mut ColumnarBatch, ws: &mut ColumnarWorkspace) -> Result<()>`

`ColumnarWorkspace` must:

- provide `with_capacity(...)`,
- reuse buffers deterministically,
- avoid heap allocations inside the steady-state per-value loops.

## 5) Type mapping contract (mandatory)

### 5.1 Supported Postgres → columnar mappings (v1)

Define the initial supported mapping (reject everything else deterministically):

- `BOOL` → `Bool`
- `SMALLINT` → `I16`
- `INT` → `I32`
- `BIGINT` → `I64`
- `REAL` → `F32`
- `DOUBLE PRECISION` → `F64`
- `UUID` → `Uuid` (16 bytes)
- `TIMESTAMPTZ` → `TimestampTz` (see 5.2)
- `TEXT` / `VARCHAR` → `Utf8`
- `BYTEA` → `Bytes` (opaque bytes)

### 5.2 Timestamp encoding (mandatory)

`TIMESTAMPTZ` values are encoded as:

- `i64` microseconds since Unix epoch (UTC), lossless for Postgres `timestamptz` range commonly used.
- NULLs are represented by the validity bitmap.

This encoding is used:

- in the in-memory column buffers, and
- in the binary envelope (see section 7).

## 6) NULL model (mandatory)

All columns carry a `ValidityBitmap`:

- For fixed-width columns: values buffer is `row_count * width` bytes; NULL values may contain unspecified bytes, ignored by validity.
- For variable-width columns:
  - offsets length is `row_count + 1`
  - for NULLs: offset does not advance (same start/end), and validity bit is 0

## 7) Binary envelope (mandatory)

The transport format is an ORSX-specific, minimal columnar envelope (“orsxcol”).

### 7.1 Envelope goals

- Fast to encode/decode in safe Rust (no per-value parsing in the steady state).
- Stable and versioned (forward compatibility via version + flags + type ids).
- Supports mixed types + NULLs.

### 7.2 Envelope v1 (definition)

All integer fields are little-endian.

Header:

- `magic`: 8 bytes, ASCII: `ORSXCOL1`
- `version`: `u16` (must be 1)
- `flags`: `u16` (bitset; v1 defines 0)
- `row_count`: `u32`
- `col_count`: `u16`

Then `col_count` column descriptors:

- `type_id`: `u16` (stable ids defined in code/spec)
- `encoding_id`: `u16` (`0 = plain`; others reserved)
- `name_len`: `u16` (0 allowed)
- `name_bytes`: `name_len` bytes (UTF-8; optional)
- `validity_len`: `u32`
- `validity_bytes`: `validity_len` bytes (bit-packed)
- `payload_len_1`: `u32`
- `payload_1`: bytes
- `payload_len_2`: `u32`
- `payload_2`: bytes (reserved; used by var-width columns as offsets+data, fixed-width uses payload_1 only)

Payload rules:

- Fixed-width:
  - `payload_1` = raw values buffer
  - `payload_2` = empty
- Variable-width:
  - `payload_1` = offsets buffer (`u32` * (row_count + 1))
  - `payload_2` = data buffer

Validation rules (decoder must enforce):

- `row_count` and `col_count` must be within configured caps.
- `validity_len` must match `ceil(row_count / 8)`.
- Offsets length must equal `4 * (row_count + 1)`.
- Offsets must be non-decreasing and final offset must equal `data.len()`.
- All size arithmetic uses `checked_*` and fails deterministically on overflow.

## 8) Failure contract (mandatory)

The system must reject deterministically with a structured error for:

- unsupported Postgres types (including `NUMERIC` until explicitly supported),
- invalid COPY binary stream (truncated, inconsistent lengths, invalid OIDs),
- envelope decode validation failures,
- batch size exceeding configured caps,
- any internal overflow in size computations.

No panics in library code.

## 9) Determinism contract (mandatory)

- Column order equals query select-list order.
- Row order equals query result order.
- Batch encoding is deterministic given identical inputs (no hash iteration order).

Parallel decoding (if added later) must be opt-in and must document determinism guarantees (or explicit nondeterminism).

## 10) Performance budgets (mandatory)

Budgets must be written as “target + acceptance gate”.

### 10.1 Targets (v1)

- 100k rows × 500 columns: avoid per-value allocations; no `String` creation on the hot path for `Utf8` (store raw bytes).
- Steady-state decode path must be linear in bytes received.
- Encoder/decoder must reuse buffers via workspace.

### 10.2 Evidence gates

Accept the feature only if evidence shows:

- **CPU**: columnar decode is measurably faster than row-wise decode for wide tables.
- **Allocations**: repeated reads with a workspace do not allocate in the steady state (or allocations are explicitly bounded and justified).

All claims must be recorded in `protocols/orsx2_evidence/columnar_trials.md` (append-only).

## 11) Security contract (mandatory)

- The columnar module must not concatenate untrusted strings into SQL.
- If it constructs `COPY (SELECT ...)`, identifiers must be quoted via the single audited quoting function.
- Any “raw query” API must be explicitly labeled unsafe-by-contract unless it takes only validated inputs.

## 12) Test plan mapping (mandatory)

### 12.1 Unit tests (no DB)

- Envelope encode/decode round-trip for each supported type.
- NULL semantics (bitmap correctness).
- Decoder rejects invalid envelopes (offsets, lengths, overflow).

### 12.2 Integration tests (DB)

- `COPY ... FORMAT BINARY` path:
  - retrieve fixed-width columns with NULLs,
  - retrieve variable-width columns with NULLs,
  - verify results equal row-wise `SELECT` decode.
- Batch boundary behavior (N rows, N+1 rows, empty result).

### 12.3 Perf trials (DB, release)

- 100k rows, 50 columns (mixed types)
- 100k rows, 500 columns (mixed types)
- 1M rows, 50 columns (mixed types, streaming)

Record results in `protocols/orsx2_evidence/columnar_trials.md`.

## 13) Optimization roadmap (tracked)

Potential follow-ups (only if evidence shows benefit):

1. Dictionary encoding for `Utf8`/`Bytes` (opt-in).
2. Optional compression of per-column payloads for transport (opt-in).

## Appendix — Add-ons v1.2

Optional columnar expansions (additional Postgres types, row-wise strict preflight) are grouped in:

- `protocols/orsx2_specs/ADDONS_V1_2_SPEC.md:1`
3. Parallel decode of fixed-width columns (opt-in; determinism policy required).
4. Arrow IPC export (compat layer) as a separate, optional module/feature.
