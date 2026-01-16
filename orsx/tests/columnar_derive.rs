#![allow(dead_code)]

use orsx::columnar::{ColumnarType, OrsxColumnar};

#[derive(orsx::OrsxColumnar)]
struct MyTable {
    name_: String,
    pwt: f64,
}

#[test]
fn derive_generates_schema() {
    let schema = MyTable::columnar_schema().unwrap();
    let fields = schema.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name.as_deref(), Some("name_"));
    assert_eq!(fields[0].ty, ColumnarType::Utf8);
    assert_eq!(fields[1].name.as_deref(), Some("pwt"));
    assert_eq!(fields[1].ty, ColumnarType::F64);

    // Index constants exist and are stable.
    assert_eq!(MyTable::COL_NAME_, 0);
    assert_eq!(MyTable::COL_PWT, 1);
}
