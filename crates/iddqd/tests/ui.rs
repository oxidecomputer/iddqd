//! UI tests.

#[cfg(all(feature = "std", feature = "default-hasher"))]
#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid/*.rs");
}
