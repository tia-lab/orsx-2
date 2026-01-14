use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use orsx::migrations::introspection::{ColumnInfo, TableSchema};
use orsx::migrations::planning::{diff_schema, expected_schema_from_spec, filter_ignored_diffs};
use orsx::{ColumnSpec, FieldType, IndexInfo, IndexType, TableSpec};

fn make_spec(cols: usize) -> TableSpec {
    // Build a TableSpec with a stable, predictable layout.
    // This is a benchmark; allocation here is setup-only.
    let mut v: Vec<ColumnSpec> = Vec::with_capacity(cols);
    for i in 0..cols {
        let name = Box::leak(format!("c_{i}").into_boxed_str());
        v.push(ColumnSpec {
            name,
            ty: FieldType::Text,
            nullable: true,
            primary_key: i == 0,
            unique: false,
            default_sql: None,
        });
    }
    let cols_static: &'static [ColumnSpec] = Box::leak(v.into_boxed_slice());

    TableSpec {
        table_name: "bench_table",
        columns: cols_static,
        indexes: &[
            IndexInfo {
                name: "idx_c_1",
                columns: &["c_1"],
                unique: false,
                index_type: IndexType::BTree,
            },
        ],
    }
}

fn make_current_schema(cols: usize) -> TableSchema {
    let mut v: Vec<ColumnInfo> = Vec::with_capacity(cols);
    for i in 0..cols {
        v.push(ColumnInfo {
            name: format!("c_{i}"),
            sql_type: "TEXT".to_string(),
            nullable: i != 0,
            position: i as i32,
            is_primary_key: i == 0,
            is_unique: i == 0,
        });
    }
    TableSchema {
        table_name: "bench_table".to_string(),
        columns: v,
    }
}

fn bench_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("planning");

    for cols in [50usize, 200, 1000] {
        group.bench_function(format!("diff_schema_{cols}_cols"), |b| {
            b.iter_batched(
                || {
                    let spec = make_spec(cols);
                    let expected = expected_schema_from_spec("bench_table", &spec);
                    let current = make_current_schema(cols);
                    (current, expected)
                },
                |(current, expected)| {
                    let diffs = diff_schema(&current, &expected);
                    let _ = filter_ignored_diffs(diffs);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_planning);
criterion_main!(benches);
