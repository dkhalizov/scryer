//! In-process fakes for the end-to-end flow harness (D91 F2). The HTTP fake
//! (Torznab) lives in `scryer-mock-apis`.
// fork (D91 F2)
#![allow(dead_code, reason = "each flow test binary uses a subset")]

pub mod plex;
pub mod qbit;
