#[test]
fn test_parse_empty() {
    let t = trybuild::TestCases::new();
    t.pass("tests/structfields_empty.rs");
    t.pass("tests/structfields_slice.rs");
    t.pass("tests/structfields_slice_ctx.rs");
    t.pass("tests/structfields_slice_context.rs");
    t.pass("tests/structfields_slice_sizet.rs");
    t.pass("tests/structfields_ndarray.rs");
}
