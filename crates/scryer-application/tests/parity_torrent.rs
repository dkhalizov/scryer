//! Parity net, torrent area, tracked-download side (notes/11-parity-net-design.md,
//! parity/README.md).
//!
//! Suites:
//! - `scryer-application::parity_torrent::son_failed_check`: Sonarr
//!   FailedDownloadService.ProcessFixture (SON-TORRENT-0100..0102), in
//!   parity_torrent/torrent_tracked.rs.
//!
//! The seed-goal and config suites live in
//! crates/scryer-infrastructure-acquisition/tests/parity_torrent.rs.
//!
//! Unmapped (tests: []), with reasons: SON-TORRENT-0103..0124 and 0126 and
//! RAD-TORRENT-0001..0005 and 0007..0012 are listed with their reasons in the
//! header of parity_torrent/torrent_tracked.rs. SON-TORRENT-0098..0099
//! (ProcessFailedFixture; 0097 is out of scope) and QBittorrentFixture
//! 0001..0075 are not covered here; their reasons are in the infra-acquisition
//! header. 0077..0096 are waived (D21).

#[path = "parity_torrent/torrent_tracked.rs"]
mod torrent_tracked;
