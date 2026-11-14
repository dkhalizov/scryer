//! Parity harness root for the scan area of scryer-mediainfo. Streams add
//! `#[path = "parity_scan/<stream>.rs"] mod <stream>;` lines below.
//!
//! Unmapped here: see the header of each submodule.
//!
//! - VideoFileInfoReader (20, SON-SCAN-0040..0059): mapped in
//!   parity_scan/scan_reader.rs (0040..0053); SON-SCAN-0054..0059 stay
//!   unmapped there: deferred to a scryer-application suite (seam-only, D29
//!   mappable), reason there.
//! - MediaInfoFormatter (78, SON-SCAN-0060..0137): mapped in
//!   crates/scryer-application/tests/parity_scan/scan_mediainfo.rs; the display
//!   labels are scryer-application rename tokens, unreachable from here without
//!   a dev-dependency cycle.

#[path = "parity_scan/scan_reader.rs"]
mod scan_reader;
