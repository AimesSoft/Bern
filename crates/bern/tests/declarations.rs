//! Declarations that must compile, and declarations that must not.

#[test]
fn declarations() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass_*.rs");
    cases.compile_fail("tests/ui/fail_*.rs");
}
