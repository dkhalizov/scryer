//! Stream priv_import: import settings captured from the reference apps' live
//! config (corpus/captured/{radarr,sonarr}/config/*.json). Private data (D45):
//! nothing captured is written here; every compared value is read at run time
//! and the suites skip when the captured corpus is absent.
//!
//! Suites: `rad_import_settings` (RAD-IMPORT-5003, 5006, 5010) and
//! `son_import_settings` (SON-IMPORT-5003, 5007, 5013). IDs come from an
//! explicit check list run through `super::run_config`, so `wants` is only
//! asked for these IDs, also under PARITY_ADOPT.
//!
//! Scryer functions under test (crates/scryer-application/src/):
//! - `parity_seams::evaluate_seeding_gate` (import/seeding_gate.rs:437) +
//!   `SeedingGateDecision::import_mode` (:421).
//! - `AppUseCase::recycle_bin_config_from_values` (settings/runtime/recycle_bin.rs:316,
//!   parity net seam added by this stream) + `recycle_bin::recycle_file`
//!   (library/recycle_bin.rs:535). Replaced files go through the same
//!   `recycle_file_inner` branch.
//! - `parse_download_client_remote_path_mappings` (download_client_path_mappings.rs:34) +
//!   `apply_remote_path_mappings_to_completed_download` (:151), the variant the
//!   import path uses on a completed download's `dest_dir`
//!   (import/workflow/paths.rs:27, completed_download/lookup.rs:737).
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | mediamanagement.json `copyUsingHardlinks` true / false | expected mode `hardlink_or_copy` / `copy`. Actual: the mode the seeding gate gives a configured Move when the torrent reports can_move_files = !copyUsingHardlinks (hardlinks on = the client must keep the file; the Stage 5 Auto->Move translation), so the value read from the corpus drives the production call. Scryer has no plain-copy mode, so false would be red (D29). Self-check: can_move_files true keeps `move` |
//! | mediamanagement.json `recycleBin` empty / non-empty | `enabled=false, custom_path=None` (permanent delete) / `enabled=true`, custom path = `rebase(tmp/bin, value)`. `recycleBinCleanupDays` -> retention_days (not compared). Expected `{ok, source_exists:false, recycled, in_bin}` with recycled and in_bin = recycleBin non-empty; actual from `recycle_file` on a probe file, `in_bin` read from the filesystem (bin folder has an entry). Self-check: enabled with no custom path is not a silent permanent delete |
//! | remotepathmapping.json rows `host`, `remotePath`, `localPath` | rows grouped by `host`; per host `remote_path_mappings` = `remote => local` lines of a download client config (Scryer keeps them per client, the reference per host). Per host: probe `<remotePath>/parity-probe` expected `<localPath>/parity-probe`, and the constant probe `/parity-probe-unmapped/Release` expected unchanged (non-matching rows leave the path as-is). With no rows at all the constant probe runs against an empty mapping list. Probes are a `CompletedDownload.dest_dir`. A host config Scryer refuses (e.g. a non-absolute local path, a duplicate remote root) yields `{refused: reason}` for its probes: red, not a crash. Self-check: a synthetic mapping remaps the constant probe |
//!
//! RAD-IMPORT-5010 and SON-IMPORT-5013 are weak greens while the captured
//! mapping lists are empty: the comparison is then identity on an empty
//! mapping list, and only the synthetic self-check shows the remap is not a
//! constant identity.
//!
//! Ignored keys: remotepathmapping `id`. Other mediamanagement keys belong to
//! other entries.
//!
//! Unmapped (tests: []), with reasons:
//! - RAD-IMPORT-5001, SON-IMPORT-5001 (completed download handling): no setting;
//!   `import::completed_download::check` and its gate are async on &AppUseCase.
//!   The only pure part (`tracked_download_is_scryer_origin`) is one field read.
//! - RAD-IMPORT-5005, SON-IMPORT-5006 (extra files): no extension setting; sidecars
//!   come from `SceneSubtitlePlan::discover_for_import` (AppUseCase), its pure
//!   part is private and its observer is cfg(test).
//! - SON-IMPORT-5004 (episode title required): no such concept; ImportCheckContext
//!   carries no episode title.
//! - RAD-IMPORT-5007, SON-IMPORT-5009 (file date): no file-date setting; placement
//!   is FsFileImporter in scryer-infrastructure-workflow, which this binary cannot call.
//! - RAD-IMPORT-5008, SON-IMPORT-5010 (permissions): `set_permissions_linux` exists
//!   but is applied only by FsFileImporter (other crate); default is read via AppUseCase.
//! - RAD-IMPORT-5009, SON-IMPORT-5011 (script import): no script performs the
//!   transfer; scripts are PostImport or Schedule triggers.
//! - SON-IMPORT-5008 (delete empty folders): no setting; deletion and scan run
//!   through AppUseCase.
//! - RAD-IMPORT-5012, SON-IMPORT-5012 (metadata): NFO and plexmatch writers exist
//!   but are gated only by AppUseCase settings reads; no image writer.
//! - RAD-IMPORT-5011 (root folders): which root a new title is imported under is
//!   resolved inside AppUseCase. The pure default-root picker
//!   `root_folder_entries_from_library_roots` (settings/runtime/library.rs:102,
//!   pub(crate)) only echoes the configured list, and the fs_safety root guard
//!   (`resolve_available_root_for_path`, fs_safety.rs:14) is a cleanup/delete
//!   guard, not root assignment. A check through that guard staged the
//!   filesystem from the same predicate as its expected value, so it could not
//!   go red; it was dropped.

use std::path::Path;

use scryer_application::parity_seams::{SeedingGateInput, TorrentSeedingObservation, evaluate_seeding_gate};
use scryer_application::recycle_bin::{RecycleManifest, recycle_file};
use scryer_application::{
    AppUseCase, DownloadClientRemotePathMapping, apply_remote_path_mappings_to_completed_download,
    parse_download_client_remote_path_mappings,
};
use scryer_domain::{CompletedDownload, ImportMode};
use scryer_parity::Suite;
use serde_json::{Value, json};

use super::{ConfigCheck, rebase, run_config, touch};

fn gate_mode(can_move_files: Option<bool>) -> ImportMode {
    evaluate_seeding_gate(&SeedingGateInput {
        is_torrent: true,
        client_type: String::new(),
        present_in_client: true,
        observation: Some(TorrentSeedingObservation {
            can_remove: Some(true),
            can_move_files,
            ..Default::default()
        }),
        goals: None,
        now: chrono::DateTime::UNIX_EPOCH,
    })
    .import_mode(ImportMode::Move)
}

fn mediamanagement(app: &str) -> Value {
    scryer_parity::corpus(&format!("corpus/captured/{app}/config/mediamanagement.json"))
}

fn hardlinks_check(app: &str) -> (Value, Value) {
    assert_eq!(gate_mode(Some(true)), ImportMode::Move);
    let hardlinks = mediamanagement(app)["copyUsingHardlinks"].as_bool().expect("copyUsingHardlinks");
    let expected = json!({ "mode": if hardlinks { "hardlink_or_copy" } else { "copy" } });
    // Reference term -> Scryer term: hardlinks on = the client must keep the file (cannot move).
    let actual = json!({ "mode": gate_mode(Some(!hardlinks)).as_str() });
    (expected, actual)
}

fn manifest(file: &Path) -> RecycleManifest {
    RecycleManifest {
        schema: None,
        entry_id: None,
        source_operation_id: None,
        recycled_at: "1970-01-01T00:00:00Z".into(),
        original_path: file.to_string_lossy().to_string(),
        original_file_id: None,
        size_bytes: 1,
        title_id: None,
        media_root: None,
        reason: "file_deleted".into(),
        status: None,
        replacement_file_id: None,
        replacement_path: None,
        media_row: None,
    }
}

fn recycle_probe(
    tmp: &Path,
    name: &str,
    enabled: bool,
    custom: Option<&str>,
    days: u32,
) -> (Result<bool, ()>, bool) {
    let root = tmp.join("library");
    let file = root.join(name).join(format!("{name}.mkv"));
    touch(&file);
    let config = AppUseCase::recycle_bin_config_from_values(
        enabled,
        custom,
        days,
        root.to_str(),
        &[root.clone()],
        Ok(&[root.clone()]),
    );
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    let res = rt.block_on(recycle_file(&config, &file, manifest(&file)));
    let recycled = res.as_ref().map(|r| r.is_some()).map_err(|_| ());
    (recycled, file.exists())
}

fn recycle_bin_check(app: &str) -> (Value, Value) {
    let cfg = mediamanagement(app);
    let bin = cfg["recycleBin"].as_str().expect("recycleBin").trim().to_string();
    let days = cfg["recycleBinCleanupDays"].as_u64().expect("recycleBinCleanupDays") as u32;
    let tmp = tempfile::tempdir().expect("tempdir");
    // Self-check: an enabled bin without a path never silently deletes.
    let (res, exists) = recycle_probe(tmp.path(), "Second Probe", true, None, days);
    assert!(!(res == Ok(false) && !exists), "enabled bin without a path must not delete silently");
    let custom = (!bin.is_empty()).then(|| {
        rebase(&tmp.path().join("bin"), &bin).to_string_lossy().to_string()
    });
    let (res, exists) = recycle_probe(tmp.path(), "Probe Title", custom.is_some(), custom.as_deref(), days);
    // The bin folder content comes from the filesystem, not from the flag.
    let in_bin = std::fs::read_dir(tmp.path().join("bin")).is_ok_and(|mut d| d.next().is_some());
    let expected = json!({ "ok": true, "source_exists": false, "recycled": !bin.is_empty(), "in_bin": !bin.is_empty() });
    let actual = json!({ "ok": res.is_ok(), "source_exists": exists, "recycled": res == Ok(true), "in_bin": in_bin });
    (expected, actual)
}

type Mappings = Vec<DownloadClientRemotePathMapping>;

fn parse_mappings(lines: &str) -> Result<Mappings, String> {
    parse_download_client_remote_path_mappings(&json!({ "remote_path_mappings": lines }).to_string())
        .map_err(|e| e.to_string())
}

/// The import-path remap: a completed download's `dest_dir` through the client's mappings.
fn remap(mappings: &Mappings, dest_dir: &str) -> String {
    let mut completed = CompletedDownload {
        client_type: String::new(),
        client_id: String::new(),
        download_client_item_id: String::new(),
        download_id: None,
        name: String::new(),
        release_name: None,
        dest_dir: dest_dir.to_string(),
        category: None,
        size_bytes: None,
        completed_at: None,
        parameters: Vec::new(),
        posted_file_names: Vec::new(),
        subtitle_paths: Vec::new(),
    };
    apply_remote_path_mappings_to_completed_download(&mut completed, mappings);
    completed.dest_dir
}

const UNMAPPED_PROBE: &str = "/parity-probe-unmapped/Release";

fn remote_path_mapping_check(app: &str) -> (Value, Value) {
    let synthetic = parse_mappings("/parity-probe-unmapped => /parity-probe-local").expect("synthetic mapping");
    assert_eq!(remap(&synthetic, UNMAPPED_PROBE), "/parity-probe-local/Release");
    let rows: Vec<Value> = scryer_parity::corpus(&format!("corpus/captured/{app}/config/remotepathmapping.json"));
    let field = |r: &Value, k: &str| r[k].as_str().expect(k).to_string();
    let join = |p: &str| Path::new(p).join("parity-probe").to_string_lossy().to_string();
    assert!(rows.iter().all(|r| !field(r, "remotePath").contains("parity-probe-unmapped")));
    let mut hosts: Vec<String> = rows.iter().map(|r| field(r, "host")).collect();
    hosts.sort();
    hosts.dedup();
    // No rows: the path is used as-is.
    let (mut expected, mut actual) = (vec![json!(UNMAPPED_PROBE)], vec![json!(remap(&Vec::new(), UNMAPPED_PROBE))]);
    for host in hosts {
        let of_host: Vec<&Value> = rows.iter().filter(|r| r["host"] == host.as_str()).collect();
        let lines: Vec<String> =
            of_host.iter().map(|r| format!("{} => {}", field(r, "remotePath"), field(r, "localPath"))).collect();
        let mut probes: Vec<String> = of_host.iter().map(|r| join(&field(r, "remotePath"))).collect();
        probes.push(UNMAPPED_PROBE.to_string());
        expected.extend(of_host.iter().map(|r| json!(join(&field(r, "localPath")))));
        expected.push(json!(UNMAPPED_PROBE));
        match parse_mappings(&lines.join("\n")) {
            Ok(mappings) => actual.extend(probes.iter().map(|p| json!(remap(&mappings, p)))),
            // A row Scryer refuses is a behaviour difference (red), not a harness crash.
            Err(reason) => actual.extend(probes.iter().map(|_| json!({ "refused": reason }))),
        }
    }
    (json!(expected), json!(actual))
}

#[test]
fn rad_import_settings() {
    let Some(mut suite) = Suite::open("scryer-application::parity_import::rad_import_settings") else { return };
    let checks: [(&str, ConfigCheck); 3] = [
        ("RAD-IMPORT-5003", hardlinks_check),
        ("RAD-IMPORT-5006", recycle_bin_check),
        ("RAD-IMPORT-5010", remote_path_mapping_check),
    ];
    run_config(&mut suite, "radarr", &checks);
    suite.finish();
}

#[test]
fn son_import_settings() {
    let Some(mut suite) = Suite::open("scryer-application::parity_import::son_import_settings") else { return };
    let checks: [(&str, ConfigCheck); 3] = [
        ("SON-IMPORT-5003", hardlinks_check),
        ("SON-IMPORT-5007", recycle_bin_check),
        ("SON-IMPORT-5013", remote_path_mapping_check),
    ];
    run_config(&mut suite, "sonarr", &checks);
    suite.finish();
}
