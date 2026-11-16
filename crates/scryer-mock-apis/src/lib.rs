//! Fakes for the end-to-end flow harness (D91 F2). The `scryer-mock-apis`
//! binary keeps its own module tree in `main.rs`; this lib exposes only the
//! fakes that tests start in-process.
// fork (D91 F2): lib target so tests can spawn the Torznab fake on a random port.

pub mod torznab;
