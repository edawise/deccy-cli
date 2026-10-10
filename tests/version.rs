//! Gate for M-version (the build-stamped version token).
//!
//! Self-contained per M-tests: pulls the module under test in with a #[path] shim and
//! carries its own fixtures — no shared test-support module. Run via `tests/version.run`,
//! which sets the cargo-provided stamp plain `rustc` does not define.

#[path = "../src/version.rs"]
mod version;

#[test]
fn stamp_is_the_build_version() {
    assert_eq!(version::VERSION, env!("CARGO_PKG_VERSION"));
}

#[test]
fn line_is_the_version_and_nothing_else() {
    assert_eq!(version::version_line(), version::VERSION);
}

#[test]
fn line_is_single_line() {
    assert!(!version::version_line().contains('\n'));
}
