# orsx

A lightweight PostgreSQL library for Rust that provides zero-loss schema migrations and automatic data compression. Built as a thin layer over sqlx, orsx focuses on migration safety and transparent compression while letting you write raw SQL for queries.

## What is orsx

orsx is a database library that solves two specific problems:

1. **Safe schema migrations**: Automatically migrates PostgreSQL tables when your struct definitions change, with backup creation and data verification to prevent data loss.

2. **Transparent compression**: Stores large numeric vectors (prices, volumes, features) in compressed form using the cydec codec, automatically compressing on write and decompressing on read.

The library provides derive macros for schema metadata but requires you to write SQL queries directly using sqlx. It does not provide an ORM, query builders, or CRUD abstractions.

## Core Features

- Zero-loss migration algorithm with automatic backup creation
- Transparent compression for numeric vectors via `Compressed<T>` wrapper
- Dynamic table name support for multi-timeframe data
- Native jiff::Timestamp support via jiff-sqlx
- PostgreSQL index management (B-tree, GIN, GiST, Hash)
- Compile-time SQL verification when using sqlx::query! macro
- Direct sqlx access with no query abstraction layer

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
orsx = "2.0"
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio-rustls"] }
tokio = { version = "1", features = ["full"] }
```

## Basic Usage

### Define Your Schema

```rust
use orsx::prelude::*;
use jiff::Timestamp;

#[derive(OrsxMigrate, sqlx::FromRow, Debug)]
#[orsx_table("users")]
struct User {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    email: Option<String>,  // Option<T> makes column nullable
    created_at: Timestamp,
}
```

### Run Migrations

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = sqlx::PgPool::connect("postgresql://localhost/mydb").await?;

    // Create or update table schema
    let dummy = User {
        id: String::new(),
        name: String::new(),
        email: None,
        created_at: Timestamp::now(),
    };

    Migrations::init(&pool, &[(dummy, None)]).await?;

    Ok(())
}
```

### Write Queries with sqlx

orsx does not provide query methods. Use sqlx directly:

```rust
// Insert
sqlx::query!(
    "INSERT INTO users (id, name, email, created_at) VALUES ($1, $2, $3, $4)",
    "user_123",
    "Alice",
    Some("alice@example.com"),
    Timestamp::now()
)
.execute(&pool)
.await?;

// Select
let user = sqlx::query_as!(
    User,
    "SELECT * FROM users WHERE id = $1",
    "user_123"
)
.fetch_one(&pool)
.await?;

// Update
sqlx::query!(
    "UPDATE users SET name = $1 WHERE id = $2",
    "Alice Smith",
    "user_123"
)
.execute(&pool)
.await?;

// Delete
sqlx::query!(
    "DELETE FROM users WHERE id = $1",
    "user_123"
)
.execute(&pool)
.await?;
```

### Batch Insert Operations

orsx does not provide batch insert helpers. Use PostgreSQL's native batch insert syntax:

```rust
// Method 1: Single INSERT with multiple VALUES
sqlx::query!(
    "INSERT INTO users (id, name, email, created_at) VALUES
     ($1, $2, $3, NOW()),
     ($4, $5, $6, NOW()),
     ($7, $8, $9, NOW())",
    "user_1", "Alice", Some("alice@example.com"),
    "user_2", "Bob", Some("bob@example.com"),
    "user_3", "Charlie", None::<String>
)
.execute(&pool)
.await?;

// Method 2: UNNEST for larger batches
let ids = vec!["user_1", "user_2", "user_3"];
let names = vec!["Alice", "Bob", "Charlie"];
let emails: Vec<Option<&str>> = vec![Some("alice@example.com"), Some("bob@example.com"), None];

sqlx::query!(
    "INSERT INTO users (id, name, email, created_at)
     SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[]) AS t(id, name, email)
     CROSS JOIN (SELECT NOW() as created_at) c",
    &ids[..],
    &names[..],
    &emails as &[Option<&str>]
)
.execute(&pool)
.await?;

// Method 3: COPY for maximum performance (requires raw CSV data)
use sqlx::postgres::PgCopyIn;

let mut copy = pool.copy_in_raw(
    "COPY users (id, name, email, created_at) FROM STDIN WITH (FORMAT CSV)"
).await?;

for i in 0..1000 {
    let csv_row = format!("user_{},Name{},email{}@example.com,2025-01-15T00:00:00Z\n", i, i, i);
    copy.send(csv_row.as_bytes()).await?;
}

copy.finish().await?;
```

## Compressed Fields

Store large numeric vectors in compressed form to save database space:

```rust
use orsx::prelude::*;

#[derive(OrsxMigrate, sqlx::FromRow)]
#[orsx_table("market_data")]
struct MarketData {
    #[orsx_column(primary_key)]
    id: String,
    symbol: String,
    prices: Compressed<f64>,   // Compressed Vec<f64>
    volumes: Compressed<i64>,  // Compressed Vec<i64>
}

// Insert with compression (automatic)
let data = MarketData {
    id: "btc_1h".to_string(),
    symbol: "BTCUSDT".to_string(),
    prices: Compressed::new(vec![100.0, 101.5, 102.0, 103.5]),
    volumes: Compressed::new(vec![1000, 1100, 1050, 1200]),
};

sqlx::query!(
    "INSERT INTO market_data (id, symbol, prices, volumes) VALUES ($1, $2, $3, $4)",
    data.id,
    data.symbol,
    &data.prices as &Compressed<f64>,
    &data.volumes as &Compressed<i64>
)
.execute(&pool)
.await?;

// Retrieve with decompression (automatic)
let retrieved = sqlx::query_as!(
    MarketData,
    "SELECT * FROM market_data WHERE id = $1",
    "btc_1h"
)
.fetch_one(&pool)
.await?;

// Access decompressed data
let prices: &[f64] = retrieved.prices.as_slice();
let volumes: &[i64] = retrieved.volumes.as_slice();
```

Supported compressed types:

- `Compressed<i32>`, `Compressed<i64>`
- `Compressed<u32>`, `Compressed<u64>`
- `Compressed<f32>`, `Compressed<f64>`

All compressed types are stored as PostgreSQL `BYTEA` columns using the cydec compression codec.

## Dynamic Table Names

Create multiple tables from the same struct definition:

```rust
#[derive(OrsxMigrate, sqlx::FromRow)]
#[orsx_table("regime_data")]
struct RegimeData {
    #[orsx_column(primary_key)]
    id: String,
    trend: f64,
    volatility: f64,
}

// Create multiple timeframe tables
let timeframes = ["1h", "4h", "12h", "1d"];

for tf in &timeframes {
    let table_name = format!("regime_{}", tf);
    let dummy = RegimeData {
        id: String::new(),
        trend: 0.0,
        volatility: 0.0,
    };

    Migrations::init(&pool, &[(dummy, Some(&table_name))]).await?;
}

// Query specific timeframe using raw SQL
let data_1h = sqlx::query_as!(
    RegimeData,
    "SELECT * FROM regime_1h WHERE id = $1",
    "btcusdt"
)
.fetch_one(&pool)
.await?;
```

### TableQuery Trait (Optional Dynamic Operations)

If you need runtime table name selection, orsx provides a `TableQuery` trait with helper methods. Note that these methods sacrifice compile-time SQL verification:

```rust
use orsx::prelude::*;

#[derive(OrsxMigrate, sqlx::FromRow)]
#[orsx_table("regime_data")]
struct RegimeData {
    #[orsx_column(primary_key)]
    id: String,
    trend: f64,
}

// Insert into dynamically selected table
let data = RegimeData { id: "btc".to_string(), trend: 0.75 };
data.insert_into_table(&pool, "regime_1h").await?;

// Update in dynamically selected table
data.update_in_table(&pool, "regime_1h").await?;

// Delete from dynamically selected table
RegimeData::delete_from_table(&pool, "regime_1h", "btc").await?;

// Fetch all from table
let all_data = RegimeData::fetch_all_from_table(&pool, "regime_1h").await?;

// Count records
let count = RegimeData::count_in_table(&pool, "regime_1h").await?;

// Find by primary key
let found = RegimeData::find_by_id_in_table(&pool, "regime_1h", "btc").await?;
```

These methods are useful for multi-timeframe patterns where table names are computed at runtime. For static table names, use `sqlx::query!` directly for compile-time verification.

## Migration System

### How Migrations Work

When you run `Migrations::init()`, orsx:

1. Checks if the table exists. If not, creates it.
2. Reads the current table schema from PostgreSQL.
3. Compares it to your struct definition.
4. If schemas match, does nothing.
5. If schemas differ, executes a zero-loss migration:
   - Creates temporary table with new schema
   - Copies all data from original table
   - Renames original table to backup (e.g., `users_backup_1234567890`)
   - Renames temporary table to original name
   - Verifies row count matches
   - Cleans up old backups per retention policy

### Schema Changes Detected

- Added columns (with appropriate defaults)
- Removed columns (preserved in backup table)
- Type changes (with automatic conversion when possible)
- Nullability changes
- Index changes

### Migration Configuration

```rust
use orsx::migrations::MigrationConfig;

let config = MigrationConfig {
    backup_suffix: "backup".to_string(),
    max_backups_per_table: 5,           // Keep last 5 backups
    backup_retention_days: Some(30),     // Delete backups older than 30 days
};

Migrations::init_with_config(&pool, &[(dummy, None)], &config).await?;
```

### Migration Safety

- All migrations run inside PostgreSQL transactions
- Backups are created before any schema changes
- Row count verification prevents silent data loss
- Failed migrations roll back automatically
- Old backups are cleaned up per retention policy
- Running migrations multiple times is safe (idempotent)

## Index Support

Define indexes using field attributes:

```rust
#[derive(OrsxMigrate, sqlx::FromRow)]
#[orsx_table("users")]
struct User {
    #[orsx_column(primary_key)]
    id: String,

    #[orsx_column(index)]
    email: String,

    #[orsx_column(index(unique))]
    username: String,

    #[orsx_column(index(type = "gin"))]
    tags: Vec<String>,
}
```

Supported index types:

- `btree` (default)
- `hash`
- `gin` (for arrays, jsonb)
- `gist` (for geometric types, full-text search)

Indexes are created during migrations if they don't exist.

## Type Mapping

### Native Types

| Rust Type          | PostgreSQL Type    |
| ------------------ | ------------------ |
| `String`           | `TEXT`             |
| `i32`              | `INTEGER`          |
| `i64`              | `BIGINT`           |
| `f32`              | `REAL`             |
| `f64`              | `DOUBLE PRECISION` |
| `bool`             | `BOOLEAN`          |
| `jiff::Timestamp`  | `TIMESTAMPTZ`      |
| `Vec<u8>`          | `BYTEA`            |
| `Option<T>`        | Nullable column    |
| `pgvector::Vector` | `vector(N)`        |

### Compressed Types

| Rust Type         | PostgreSQL Type | Storage               |
| ----------------- | --------------- | --------------------- |
| `Compressed<i32>` | `BYTEA`         | Compressed `Vec<i32>` |
| `Compressed<i64>` | `BYTEA`         | Compressed `Vec<i64>` |
| `Compressed<u32>` | `BYTEA`         | Compressed `Vec<u32>` |
| `Compressed<u64>` | `BYTEA`         | Compressed `Vec<u64>` |
| `Compressed<f32>` | `BYTEA`         | Compressed `Vec<f32>` |
| `Compressed<f64>` | `BYTEA`         | Compressed `Vec<f64>` |

## Testing

### Unit Tests (No Database Required)

```bash
cargo test --test core_functionality
```

Tests the derive macro code generation and type mapping without requiring a PostgreSQL instance.

### Integration Tests (Requires PostgreSQL)

```bash
# Start PostgreSQL (example using Docker)
docker run -d \
  --name orsx-test-db \
  -e POSTGRES_PASSWORD=password \
  -p 5432:5432 \
  postgres:15

# Set database URL
export TEST_DATABASE_URL="postgresql://postgres:password@localhost/postgres"

# Run integration tests
cargo test --test integration_tests -- --ignored
```

### Examples

Run examples to see orsx in action:

```bash
export DATABASE_URL="postgresql://postgres:password@localhost/orso_example"

# Basic CRUD operations
cargo run --example basic_crud

# Compression demonstration
cargo run --example compression

# Multi-timeframe pattern (MATHILDE use case)
cargo run --example mathilde_pattern
```

## Performance Notes

orsx achieves fast performance by:

- Zero abstraction over sqlx native types
- Direct SQL execution without query builders
- No intermediate value conversions
- Compile-time SQL verification (when using `sqlx::query!`)
- Efficient compression using cydec codec

For bulk operations, use PostgreSQL's native batch insert syntax (`UNNEST`, `COPY`) rather than loops.

## Comparison to V1 (orso-postgres)

orsx is a simplified evolution of orso-postgres V1:

**What Changed:**

- Removed ORM layer (no more `insert()`, `update()`, `delete()` methods on structs)
- Removed query builders (no more `FilterOperator`, `Filter`, `Operator`)
- Removed `Value` enum wrapper (use native types)
- Direct sqlx usage required for all queries
- Migration system preserved with same algorithm
- Compression requires explicit `Compressed<T>` wrapper

**What Stayed:**

- Zero-loss migration algorithm
- Migration backup and verification
- Dynamic table name support
- jiff::Timestamp support
- Same migration initialization API

**Migration Path:**

Old (V1):

```rust
user.insert(db).await?;
let users = User::find_where(filter, db).await?;
```

New (orsx):

```rust
sqlx::query!("INSERT INTO users (id, name) VALUES ($1, $2)", user.id, user.name)
    .execute(&pool).await?;
let users = sqlx::query_as!(User, "SELECT * FROM users WHERE active = $1", true)
    .fetch_all(&pool).await?;
```

## Error Handling

orsx defines a `Result<T>` type that wraps potential errors:

```rust
use orsx::prelude::*;

async fn example(pool: &PgPool) -> Result<()> {
    // Migration errors
    Migrations::init(&pool, &[(dummy, None)]).await?;

    // Query errors (from sqlx)
    sqlx::query!("INSERT INTO users (id) VALUES ($1)", "user_1")
        .execute(pool)
        .await?;

    Ok(())
}
```

Error types:

- `Error::Migration`: Schema migration failures with SQL context
- `Error::Database`: Wrapped sqlx errors
- `Error::Compression`: Compression/decompression failures
- `Error::Schema`: Schema validation errors

## Design Philosophy

orsx follows these principles:

1. **Thin layer over sqlx**: Minimal abstraction, maximum control
2. **Migration safety first**: Never lose data during schema changes
3. **Explicit over implicit**: Compression and table names are explicit
4. **Compile-time verification**: Use sqlx::query! for static SQL checking
5. **PostgreSQL-specific**: Optimized for PostgreSQL, not database-agnostic

orsx does not try to be an ORM. It solves two specific problems (migrations and compression) and lets you write SQL for everything else.

## Documentation

Generate full API documentation:

```bash
cargo doc --open
```

## License

MIT OR Apache-2.0

## Credits

Built by TIA Lab for the MATHILDE cryptocurrency technical analysis platform.
