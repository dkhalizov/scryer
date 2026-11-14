//! Parity net, torrent area, stream priv_torrent (private config entries).
//!
//! Suites (prefix `scryer-infrastructure-acquisition::parity_torrent::`):
//! - `qbt_torrent_excluded_files`: QBT-TORRENT-5015 (ref `preferences.json`
//!   keys `excluded_file_names`, `excluded_file_names_enabled`).
//!
//! Scryer code under test:
//! - `scryer_domain::classify_no_video_download` (scryer-domain lib.rs:3206,
//!   `EXECUTABLE_EXTENSIONS` :3158); consumer import/workflow/paths.rs:705 fails
//!   an executables-only grab.
//!
//! Translation tables (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | `excluded_file_names` pattern `*.<ext>` with `excluded_file_names_enabled` | a download holding only `payload.<ext>` classifies as `NoVideoDownloadContents::UnwantedExecutables` (never imported); expected map pattern -> enabled |
//!
//! Weak-green notes:
//! - Scryer does not stop qBittorrent from downloading excluded files; we keep
//!   qBittorrent's own exclusion. Scryer's counterpart is that such files never
//!   reach the library. A pattern of another shape than `*.<ext>` is recorded as a
//!   failure of its own (no classify call), never compared as a vacuous false.
//!
//! Ignored keys: every other preferences.json key.
//!
//! Unmapped (tests: []), with reasons:
//! - RAD-TORRENT-5005, 5010 (finished-download poll interval, from
//!   `downloadclientconfig.json` and `system-task.json`): Scryer's cadence is
//!   the code constant `DownloadQueuePollerOptions::default().interval`, not
//!   computed from any input. Any pass/fail rule would not depend on the
//!   expected value (an upper bound clamped to >= 1 min was expected-insensitive,
//!   mutation check) and an exact compare would test a constant against a
//!   setting. Behaviour exists but nothing seam-reachable maps the reference
//!   setting onto it.
//! - PRW-TORRENT-5002, 5004, 5006, 5008, 5010, 5012, 5014, 5016, 5018,
//!   5020..5028 (preferMagnetUrl, redirect/supportsRedirect): Prowlarr applies
//!   both itself, the magnet preference when it writes its Torznab feed
//!   (Prowlarr NewznabController.cs:201-203), redirect when its download
//!   endpoint serves the grab (:265-307). Scryer never sees either flag:
//!   `build_managed_child_plan` (prowlarr.rs:1582) reads only seed and minimum
//!   seeder fields, and the link choice happens in the out-of-tree Torznab
//!   plugin. The captured config holds no feed or download response, so a mock
//!   would echo the expected value. D29 does not apply: Scryer has no
//!   per-indexer magnet or redirect preference. The host only orders a magnet
//!   first when both forms resolve (scryer-plugins download_client_adapter.rs:961,
//!   private, in a crate this test crate does not depend on), and a magnet
//!   exists only when Prowlarr's feed or download endpoint yields one (:1173).
//! - PRW-TORRENT-5019 (Prowlarr has no download client): `downloadClientId` is
//!   copied into managed metadata (prowlarr.rs:590, :1637) but no grab path
//!   reads it; the only comparison would copy an absent field.
//! - SON-TORRENT-5001..5003, RAD-TORRENT-5001..5003 (download client category,
//!   priorities, remove completed/failed): Scryer has the settings
//!   (`DownloadClientRoutingEntry`, scryer-application catalog/helpers.rs:5)
//!   but the values are operator configuration its arr import does not copy
//!   (scryer-interface-import lib.rs:3651). The behaviour runs through
//!   AppUseCase (catalog/workflow/titles.rs:211, delete.rs:178); a pure check
//!   would echo a translated value or compare an out-of-box default.
//! - SON-TORRENT-5004, RAD-TORRENT-5004 (failed download redownload):
//!   `failed_download::process_failed(&AppUseCase, ..)` (import/failed_download.rs:48)
//!   needs repositories, same as SON-TORRENT-0098..0099.
//! - QBT-TORRENT-5001..5006, 5008, 5011..5014, 5016..5018 (qBittorrent version,
//!   categories, management mode, save path, layout, queueing, speed limits,
//!   WebUI auth, RSS, watch folders): qBittorrent's own behaviour; Scryer does
//!   not configure it and the handling is in the out-of-tree plugin
//!   (DOWNLOAD_CLIENT_BUILTINS is empty, scryer-plugins builtins.rs:24).
//! - QBT-TORRENT-5007 (temp folder, incomplete extension): Scryer holds a file
//!   while an incomplete marker exists (import/checks.rs:200) whatever
//!   qBittorrent is set to; the preference is not an input.
//! - QBT-TORRENT-5009, 5010, 5028 (global share limit and action): the seeding
//!   gate (import/seeding_gate.rs:437) passes the plugin's `can_remove` through
//!   when Scryer has no goal of its own (:537); turning limits into
//!   `can_remove` would re-implement the client in the harness.
//! - QBT-TORRENT-5029..5031 (pod hooks): Scryer does not deploy qBittorrent.
//! - QBT-TORRENT-5021 (stall recovery): concept absent here; the warning
//!   timeout skips Scryer-origin torrents and `failed_download::check` acts
//!   only on a failed client state. Ledger P3 plans it, not implemented.
//! - QBT-TORRENT-5022 (selected vs total size): Scryer never inspects file
//!   selection; the host sends `selected_file_indices` empty.
//! - QBT-TORRENT-5023 (sidecar remove, blocklist, re-search): counterpart is
//!   `process_failed(&AppUseCase)`; needs AppUseCase, and no stall input yet.
//! - QBT-TORRENT-5024, 5025 (orphan sweep, quarantine purge): concept absent.
//!   `next_orphan_media_file` (jobs/housekeeping.rs:854) goes the other way
//!   (catalogue rows whose library file is gone) and no housekeeping step sees
//!   download-folder leftovers.
//! - QBT-TORRENT-5026 (sidecar access to qBittorrent and arr apps): cluster
//!   plumbing, no Scryer behaviour.
//! - QBT-TORRENT-5027, 5033 (CronJobs): Scryer schedules in-process; no
//!   counterpart to Kubernetes CronJobs.

use std::collections::BTreeMap;
use std::path::Path;

use scryer_domain::{NoVideoDownloadContents, classify_no_video_download};
use scryer_parity::{Catalogue, Suite};

#[test]
fn qbt_torrent_excluded_files() {
    let Some(mut suite) =
        Suite::open("scryer-infrastructure-acquisition::parity_torrent::qbt_torrent_excluded_files")
    else {
        return;
    };
    for (_, entry) in Catalogue::load().entries {
        if !entry.id.starts_with("QBT-TORRENT-")
            || super::split_ref(&entry.reference) != Some(("qbittorrent", "preferences", "excluded_file_names"))
            || !suite.wants(&entry.id)
        {
            continue;
        }
        let prefs = super::captured("qbittorrent", "preferences");
        let enabled = prefs["excluded_file_names_enabled"].as_bool().unwrap_or(false);
        let patterns: Vec<&str> = prefs["excluded_file_names"]
            .as_str()
            .unwrap_or_default()
            .split('\n')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        if patterns.is_empty() {
            suite.record(&entry.id, false, "no patterns captured");
            continue;
        }
        let mut expected = BTreeMap::new();
        let mut actual = BTreeMap::new();
        let mut shape_ok = true;
        for p in patterns {
            expected.insert(p.to_string(), enabled);
            let Some(e) = p
                .strip_prefix("*.")
                .filter(|e| !e.is_empty() && !e.contains(['*', '?', '/', '\\']))
            else {
                shape_ok = false;
                continue;
            };
            let file = format!("payload.{e}");
            let blocked = matches!(
                classify_no_video_download([Path::new(&file)]),
                NoVideoDownloadContents::UnwantedExecutables(_)
            );
            actual.insert(p.to_string(), blocked);
        }
        if !shape_ok {
            suite.record(&entry.id, false, "a captured pattern is not of the form *.<ext>");
            continue;
        }
        suite.check(&entry.id, expected, actual);
    }
    suite.finish();
}
