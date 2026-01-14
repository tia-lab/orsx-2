# orsx (orsx2 rewrite, in progress)

This workspace contains a Rust library (`orsx`) and a proc-macro crate (`orsx-macros`).

The current implementation focuses on three pieces:

1. **Schema-driven Postgres migrations** (including an online rewrite path for large tables).
2. **Columnar retrieval** into typed column buffers (COPY BINARY fast path + row-wise fallback).
3. **Numeric vector compression** stored as `BYTEA` with a small self-describing envelope.

This README is written to match the repo state as of this checkout. It is not a promise of stability.

## Non-goals

- No ORM and no query builder. You write SQL.
- No cross-database support (Postgres only).
- No automatic “struct ↔ row” mapping for columnar reads (columnar reads return `ColumnarBatch`, not `Vec<MyStruct>`).

## Crates

- `orsx/`: library (public API)
- `orsx-macros/`: proc macros (`#[derive(OrsxMigrate)]`, `#[derive(OrsxColumnar)]`)

## Quick start

Add the crate and use `sqlx` for connections (orsx re-exports `sqlx`):

```toml
[dependencies]
orsx = { path = "./orsx" }
tokio = { version = "1", features = ["full"] }
```

## 1) Migrations (schema-driven, zero-loss-by-backup)

### What it does

You define a table schema in Rust via `#[derive(OrsxMigrate)]`. At runtime, `orsx::Migrations`:

- creates tables that do not exist,
- applies “safe ALTER” changes when possible (e.g. add a nullable column),
- otherwise performs an **online rewrite**:
  - creates a shadow table with the desired schema,
  - installs a trigger that records changed primary keys into a changelog table,
  - backfills data from the original table into the shadow table in chunks,
  - applies changelog catch-up rounds,
  - takes a short `ACCESS EXCLUSIVE` lock to drain remaining changes and swap tables,
  - keeps a **backup table** with the original data.

“Zero-loss” here means: when a rewrite happens, the old table is preserved as a backup table (not dropped).

### Current limitations (important)

The current online rewrite implementation has constraints that are enforced in code:

- Online rewrite requires **exactly one primary key column**.
- Introspection assumes the table is in the `public` schema.
- Constraint handling is limited (single-column PK/unique are tracked; other constraints are not fully modeled).
- Some diffs intentionally trigger rewrite (type changes, column position changes, drop column, etc.).

### Basic example

```rust
use orsx::prelude::*;

#[derive(OrsxMigrate)]
#[orsx_table("my_table")]
struct MyTable {
    #[orsx_column(primary_key)]
    id: String,
    name_: String,
    pwt: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&db_url).await?;

    let dummy = MyTable { id: "x".into(), name_: "n".into(), pwt: 0.0 };
    Migrations::init(&pool, &[(dummy, None)]).await?;

    Ok(())
}
```

### Strict schema enforcement knobs

The migration behavior is controlled by `orsx::migrations::config::MigrationConfig`:

- `enforce_column_order`: if `true`, Postgres physical column order must match the Rust spec order; mismatches become rewrite-required.
- `enforce_exact_columns`: if `true`, the live table must contain exactly the columns in the spec (no extras).
- `allow_destructive_drops`: only relevant when `enforce_exact_columns=true`; if `true`, extra DB columns are removed from the live table **via rewrite**, but the backup table retains the original columns/data.
- `allow_column_renames`: if `true`, fields annotated with `#[orsx_column(rename_from = "...")]` can be renamed via `ALTER TABLE ... RENAME COLUMN ...`.

Example:

```rust
use orsx::migrations::config::MigrationConfig;
use orsx::prelude::*;

let cfg = MigrationConfig {
    enforce_column_order: true,
    enforce_exact_columns: true,
    allow_destructive_drops: true,
    ..MigrationConfig::default()
};

Migrations::init_with_config(&pool, &[(dummy, None)], &cfg).await?;
```

## 2) Columnar retrieval (COPY BINARY + row-wise)

### What it does

You provide:

- a `SELECT ...` query,
- a `ColumnarSchema` describing the expected columns (order matters),

and you get a `ColumnarBatch` with:

- typed fixed-width buffers (`Vec<i64>`, `Vec<u64>` bits for f64, etc.),
- varlen buffers as offsets + a single `Vec<u8>` data blob (with a helper to coalesce),
- a validity bitmap per column to represent NULLs.

There are two readers:

- `CopyBinaryBatchReader`: uses `COPY (SELECT ...) TO STDOUT (FORMAT BINARY)` and parses the stream.
- `RowWiseBatchReader`: uses `sqlx::query(select_sql).fetch(...)` and `try_get` per cell, but still fills a `ColumnarBatch`.

There is also:

- `ColumnarBatchReader` + `ColumnarReaderMode::Auto(...)`: chooses COPY vs row-wise based on expected query shape (still returns `ColumnarBatch` either way).

### Supported column types (current)

`ColumnarType` supports:

- `Bool`, `I16`, `I32`, `I64`
- `F32`, `F64` (stored as IEEE754 bits)
- `Uuid` (16 bytes)
- `TimestampTzMicros` (i64 microseconds since Unix epoch)
- `Utf8` (raw bytes + optional UTF-8 validation)
- `Bytes` (raw bytes)

Unsupported types should be treated as “not implemented yet” rather than “silently coerced”.

### Example: schema from a struct (no rewrite of your struct)

`#[derive(orsx::OrsxColumnar)]` generates:

- `MyTable::columnar_schema() -> Result<ColumnarSchema>`
- `MyTable::COL_<FIELD>` index constants

```rust
use orsx::columnar::{ColumnarBatch, ColumnarBatchReader, ColumnarReaderMode, OrsxColumnar};

#[derive(orsx::OrsxColumnar)]
struct MyTable {
    name_: String,
    pwt: f64,
}

async fn read_batch(conn: &mut sqlx::PgConnection) -> orsx::Result<ColumnarBatch> {
    let schema = MyTable::columnar_schema()?;
    let mut batch = ColumnarBatch::new(schema.clone(), 100_000)?;

    let sql = "SELECT name_, pwt FROM my_table ORDER BY name_";
    let mut reader = ColumnarBatchReader::new_select_unchecked(
        conn,
        sql,
        schema,
        ColumnarReaderMode::Auto(Default::default()),
    )
    .await?;

    let _rows = reader.next_batch_into(&mut batch).await?;
    Ok(batch)
}
```

Notes:

- The `*_unchecked` name is intentional: ORSX does not parse or sanitize your SQL. Use parameter binding for values.
- For the row-wise reader, `select_sql` must outlive the reader (pass a long-lived `&str`, not a temporary `String`).

### Accessing columns

```rust
let name_offsets = batch.var_chunks(MyTable::COL_NAME_).unwrap().0;
let mut name_data = Vec::new();
batch.coalesce_var_into(MyTable::COL_NAME_, &mut name_data)?;

let pwt_bits = batch.fixed_f64_bits(MyTable::COL_PWT).unwrap();
let pwt0 = f64::from_bits(pwt_bits[0]);

// For row i:
let i = 123usize;
let start = name_offsets[i] as usize;
let end = name_offsets[i + 1] as usize;
let name_i = std::str::from_utf8(&name_data[start..end]).unwrap();
```

### ORSXCOL envelope (binary transport)

`orsx::columnar::encode_orsxcol_v1_into` encodes a `ColumnarBatch` into a versioned byte buffer.
`decode_orsxcol_v1(_into)` decodes and validates it.

This is intended for “send batch over the wire / store in cache” use cases.

## 3) Compressed vectors (`Compressed<T>`)

`Compressed<T>` stores a numeric vector as `BYTEA` with an envelope:

- magic/version
- codec id + element type id
- element count + uncompressed byte length
- CRC32 of the compressed payload
- payload bytes

This is not generic “data compression”; it is a narrow mechanism for numeric vectors.

Example (insert + select):

```rust
use orsx::{Compressed, CompressedWorkspace};
use sqlx::Row;

let v = Compressed(vec![1.0_f64, 2.0, 3.0]);
let mut ws = CompressedWorkspace::default();
let mut bytes = Vec::new();
v.encode_envelope_into(&mut bytes, &mut ws)?;

sqlx::query("INSERT INTO my_vecs (id, payload) VALUES ($1, $2)")
    .bind("row1")
    .bind(bytes)
    .execute(&pool)
    .await?;

let raw: Vec<u8> = sqlx::query("SELECT payload FROM my_vecs WHERE id = $1")
    .bind("row1")
    .fetch_one(&pool)
    .await?
    .try_get(0)?;

let decoded = Compressed::<f64>::decode_envelope(&raw)?;
assert_eq!(decoded.as_slice(), &[1.0, 2.0, 3.0]);
```

## Full workflow example (migrate → write → columnar read → encode for API)

This example shows one possible “end-to-end” flow. It is intentionally explicit about SQL and schema.

```rust
use orsx::prelude::*;
use orsx::columnar::{ColumnarBatch, ColumnarBatchReader, ColumnarReaderMode, OrsxColumnar, encode_orsxcol_v1_into};

#[derive(OrsxMigrate, orsx::OrsxColumnar)]
#[orsx_table("wf_items")]
struct Item {
    #[orsx_column(primary_key)]
    id: String,
    name_: String,
    pwt: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&db_url).await?;

    // 1) Migrate (create or update schema)
    let dummy = Item { id: "x".into(), name_: "n".into(), pwt: 0.0 };
    Migrations::init(&pool, &[(dummy, None)]).await?;

    // 2) Write some rows (raw SQL)
    sqlx::query("INSERT INTO wf_items (id, name_, pwt) VALUES ($1,$2,$3) ON CONFLICT (id) DO UPDATE SET name_ = EXCLUDED.name_, pwt = EXCLUDED.pwt")
        .bind("1")
        .bind("alice")
        .bind(1.25_f64)
        .execute(&pool)
        .await?;

    // 3) Columnar read
    let mut conn = pool.acquire().await?;
    let schema = Item::columnar_schema()?;
    let mut batch = ColumnarBatch::new(schema.clone(), 100_000)?;

    let sql = "SELECT name_, pwt FROM wf_items ORDER BY id";
    let mut reader = ColumnarBatchReader::new_select_unchecked(
        &mut conn,
        sql,
        schema,
        ColumnarReaderMode::Auto(Default::default()),
    )
    .await?;
    let _rows = reader.next_batch_into(&mut batch).await?;

    // 4) Encode for transport
    let mut out = Vec::new();
    encode_orsxcol_v1_into(&batch, &mut out)?;
    // `out` can be returned from an API or written to disk.

    Ok(())
}
```

## Performance (from real logs in this repo)

### Columnar retrieval

All of these numbers are from `protocols/orsx2_evidence/columnar_trials.md` and are “release” builds.

From `protocols/orsx2_evidence/columnar_trials.md`:

- 2026-01-14 14:40:38Z:
  - 100k × 50 cols: COPY → `ColumnarBatch` `262.405752ms`, row-wise → `ColumnarBatch` `299.598514ms`
  - 100k × 500 cols: COPY → `ColumnarBatch` `2.430023982s`, row-wise → `ColumnarBatch` `2.892875905s`
- 2026-01-14 14:41:55Z:
  - 1M × 50 cols: COPY → `ColumnarBatch` `2.75208442s`, row-wise → `ColumnarBatch` `2.528227132s` (row-wise is faster here)

Exact commands used for these trials are recorded in the log entries.

### Migrations (online rewrite)

All of these numbers are from `protocols/orsx2_evidence/migration_trials.md` and are “release” builds.

From `protocols/orsx2_evidence/migration_trials.md`:

- 2026-01-14T10:48:43Z (UUID PK, 1,000,000 seeded + 100,000 writer inserts):
  - cutover lock: ~`1012ms` (budget `5000ms`)
  - backfill: ~`21.324s` (rows reported: `1,100,000`)
  - total online rewrite: ~`26.014s`
- 2026-01-14T11:53:05Z (strict order/exact enforced on 1M rows, forces rewrite):
  - strict migration: ~`7.70s` vs default alter ~`34.8ms`

These are workload- and hardware-dependent; they are meant as evidence of current behavior, not a guarantee.

## Running tests locally

Most integration tests require a running Postgres.

Environment variable used by tests:

- `ORSX_TEST_DATABASE_URL` (defaults to `postgresql://orsx:orsx@localhost:15432/orsx2_test`)

Useful commands:

- Unit tests (no DB): `cargo test -p orsx --lib`
- DB correctness tests (require Postgres):
  - `cargo test -p orsx --test columnar_copy_binary --release`
  - `cargo test -p orsx --test migrations_strict_correctness`
- Perf / large-table tests (ignored by default; require Postgres and time):
  - `cargo test -p orsx --test columnar_perf_trials --release -- --ignored --nocapture`
  - `cargo test -p orsx --test migrations_online_big_uuid --release -- --ignored --nocapture`

Some large-table tests create the `uuid-ossp` extension (`CREATE EXTENSION IF NOT EXISTS "uuid-ossp"`).

## Repo protocols and evidence logs

The rewrite work in this repo tracks decisions and results in `protocols/`:

- `protocols/orsx2_rewrite_protocol.md`
- Specs: `protocols/orsx2_specs/`
- Append-only evidence logs:
  - `protocols/orsx2_evidence/migration_trials.md`
  - `protocols/orsx2_evidence/columnar_trials.md`
