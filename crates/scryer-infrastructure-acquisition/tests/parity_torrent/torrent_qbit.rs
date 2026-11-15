//! Parity net, torrent area, stream torrent_qbit (Sonarr QBittorrentFixture).
//!
//! Suite: `scryer-infrastructure-acquisition::parity_torrent::son_torrent_magnet_hash`
//! IDs: SON-TORRENT-0024 (op `qbittorrent.download`, a magnet
//! `release.download_url` and a string `expect.download_id`; selected by
//! that, then `wants()`. 0025/0026 are magnets too but expect an error or a
//! call count and stay unmapped).
//!
//! What the reference tests: the download id of a magnet grab is the hash the
//! host computes from the link, `MagnetLink.Parse(url).InfoHash.ToHex()`
//! (Sonarr TorrentClientBase.cs:224, generic, not qBittorrent code);
//! qBittorrent's AddFromMagnetLink (QBittorrent.cs:71) only echoes that hash
//! back. A base32 btih comes out as 40-char hex.
//!
//! Scryer code under test: `accepted_download_submission_identity`
//! (scryer-application download_identity.rs:839, pub), called the way
//! acquisition/submission.rs:103 calls it after a grab. Its expected hash for
//! a magnet comes from `extract_magnet_info_hash` (types.rs:2559), and it is
//! the download id when the client reply carries no hash of its own.
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | `release.download_url` (magnet) | `source_kind: MagnetUri`, `source_hint: <url>` |
//! | qBittorrent add echoes the host's hash (no hash of the client's own) | `accepted_info_hash: None`, `client_item_id: None` (the plugin is out of tree; the host's own value is what is compared) |
//! | no indexer-published hash | `info_hash_hint: None` |
//! | `QBittorrent.Download` returned id | `DownloadSubmissionIdentity.download_id` |
//! | upper-case hex id | Scryer's lower-case info hash (contracts.rs:288); compared case-insensitively |
//!
//! `initial_download_id` is None: Scryer's identity prefers the magnet hash
//! over it for a torrent client (download_identity.rs:864), so a wire id
//! would only matter when the magnet yields no hash at all.
//!
//! Echo note: `extract_magnet_info_hash` copies a 40-hex btih (lower-cased)
//! and decodes a 32-char base32 btih to hex (fork P21). For a hex magnet the
//! reference does the same (hex in, hex out), so a green there is a real match,
//! not an echo of the test. The only case today is base32, so it exercises the
//! decode.
//! `torrent_file_hash` equals the expected value but is never read here.
//!
//! Ignored input keys: `settings` (host, port, category: transport),
//! `preferences.dht_enabled` (the link has a tracker, so DHT does not gate
//! the add), `http_get` and `torrent_file_hash` (the fixture's shared mocks
//! for torrent-file grabs; a magnet grab never reads them), `release.title`,
//! `release.protocol` (always Torrent).
//!
//! Unmapped (tests: []), with reasons: every other QBittorrentFixture case
//! (SON-TORRENT-0001..0023, 0025..0075); see the root header of
//! parity_torrent.rs.

use scryer_application::{
    AcceptedDownloadIdentityInput, DownloadSourceKind, accepted_download_submission_identity,
};
use scryer_parity::Suite;
use serde_json::Value;

#[test]
fn son_torrent_magnet_hash() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-acquisition::parity_torrent::son_torrent_magnet_hash") else { return };
    let cases: Vec<Value> = scryer_parity::corpus("corpus/reference/sonarr/torrent/QBittorrentFixture.json");
    for case in &cases {
        let id = case["id"].as_str().expect("case id");
        let url = case["input"]["given"]["release"]["download_url"].as_str().unwrap_or("");
        let Some(expected) = case["expect"]["download_id"].as_str() else { continue };
        if case["input"]["op"] != "qbittorrent.download" || !url.starts_with("magnet:") || !suite.wants(id) {
            continue;
        }
        let expected = expected.to_ascii_lowercase();
        let actual = accepted_download_submission_identity(AcceptedDownloadIdentityInput {
            initial_download_id: None,
            source_kind: Some(DownloadSourceKind::MagnetUri),
            source_hint: Some(url),
            info_hash_hint: None,
            client_type: Some("qbittorrent"),
            client_item_id: None,
            accepted_info_hash: None,
        })
        .download_id
        .map(|s| s.to_ascii_lowercase());
        suite.check(id, Some(expected), actual);
    }
    suite.finish();
}
