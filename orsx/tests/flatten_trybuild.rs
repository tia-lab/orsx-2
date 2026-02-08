#[test]
fn orsx_flatten_trybuild_ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/flatten_ok.rs");
    t.compile_fail("tests/ui/flatten_fail_*.rs");
}

