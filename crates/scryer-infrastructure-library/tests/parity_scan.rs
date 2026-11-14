//! Parity harness root for the scan area of scryer-infrastructure-library.
//! Streams add `#[path = "parity_scan/<stream>.rs"] mod <stream>;` lines below.
//! Suites here call this crate's scanner and stores plus the scryer-application
//! scan guards (D41 done as new suites). The older suite
//! scryer-application::parity_scan::son_scan stays in
//! crates/scryer-application/tests/parity_scan.rs.

#[path = "parity_scan/scan_library.rs"]
mod scan_library;
