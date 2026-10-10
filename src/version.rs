//! The build-stamped version token.
//!
//! I0 is the single source of the version; D19/D11v fix the surface (`deccy --version`
//! prints the version, one line naming the running build). The stamp is taken from the
//! build, so the running binary cannot disagree with its manifest; rendering is the
//! envelope's.

/// The running build's version, stamped at compile time from the manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The line `--version` prints: the version and nothing else.
pub fn version_line() -> &'static str {
    VERSION
}
