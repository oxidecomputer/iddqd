//! UI tests.

#[test]
fn ui() {
    #[cfg(not(all(feature = "std", feature = "default-hasher")))]
    {
        return;
    }

    #[cfg(all(feature = "std", feature = "default-hasher"))]
    {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/ui/invalid/*.rs");
    }
}
