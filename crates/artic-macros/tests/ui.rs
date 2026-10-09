// Compile-fail cases for `#[derive(XamlType)]`. Each `tests/ui/*.rs` file must fail to build with
// exactly the error in its `.stderr` file. Regenerate them with `TRYBUILD=overwrite`.

#[test]
fn ui() {
  trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
