// Integration tests for orso-postgres V2
// Run with: cargo test --test integration_tests
// Requires: TEST_DATABASE_URL environment variable
//
// To run these tests, first set up a test database:
// export TEST_DATABASE_URL="postgresql://postgres:password@localhost/orso_v2_test"
//
// Or use Docker:
// docker run --name orso-postgres-test -e POSTGRES_PASSWORD=password -e POSTGRES_DB=orso_v2_test -p 5432:5432 -d postgres:15

mod integration;

// Re-export test modules
mod test_insert {
    include!("integration/test_insert.rs");
}

mod test_compressed {
    include!("integration/test_compressed.rs");
}

mod test_table_with_name {
    include!("integration/test_table_with_name.rs");
}

mod test_crud {
    include!("integration/test_crud.rs");
}

mod test_indexes {
    include!("integration/test_indexes.rs");
}
