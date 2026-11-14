//! Parity net, import area, stream `import_orchestration`: the pure
//! sub-decisions of the orchestration fixtures. Everything else of those
//! fixtures runs through `AppUseCase` (ten ports, no public fakes) and stays
//! unmapped. Each suite selects its own ids BEFORE `suite.wants`, so a
//! PARITY_ADOPT run adopts nothing foreign.
//!
//! Suites (all `scryer-application::parity_import::<fn>`) and ids:
//! - `son_transfer_move`: SON-IMPORT-0261, 0264, 0270, 0271, 0272.
//! - `son_root_guard`: SON-IMPORT-0237, 0238, 0257.
//! - `rad_root_guard`: RAD-IMPORT-0047, 0048, 0261.
//! - `rad_orphan_records`: RAD-IMPORT-0059, 0060, 0063.
//! - `son_unwanted_executable`: SON-IMPORT-0047; `rad_unwanted_executable`: RAD-IMPORT-0046.
//! - `son_import_mode`: SON-IMPORT-0138, 0139; `rad_import_mode`: RAD-IMPORT-0184, 0185.
//! - `rad_import_largest`: RAD-IMPORT-0183.
//!
//! Scryer code under test:
//! - `fs_safety::move_file_exclusive` (fs_safety.rs:519), the renamer's move primitive.
//! - `fs_safety::resolve_available_root_for_path` (fs_safety.rs:14), the guard
//!   of library/user_delete.rs and import/upgrade.rs:96-109.
//! - `jobs::housekeeping::next_orphan_media_file` driven as the production loop
//!   (housekeeping.rs:846-867) minus the async repository delete.
//! - `import::workflow::unwanted_executable_in_download` (paths.rs:674).
//! - `import::seeding_gate::evaluate_seeding_gate` + `SeedingGateDecision::import_mode`.
//! - `import::workflow::pick_largest_import_video_file` (paths.rs:1360).
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | TransferFile(src, tgt, Move, overwrite) | `move_file_exclusive(src, tgt, MoveOptions { overwrite, .. })` |
//! | throw (IOException, DestinationAlreadyExists) | `Err(_)`; the type is not compared |
//! | `MoveFile(_, tgt, _)` called | source name gone from its dir listing, target name listed, target bytes equal the source bytes (read_dir, so case-only rename 0261 is judged equally on APFS and ext4) |
//! | `DeleteFile(tgt)`: 0 / >= 1 / no key | `target_kept` true / false, compared only when the case has a DeleteFile key; actual: the target given in existing_files still holds its 't' bytes. No key = field not compared |
//! | existing_files {path: n} | n bytes: source 's', pre-existing target 't' |
//! | root folder missing / empty (NzbDroneClientException, RootFolderNotFoundException) | `AppError::ValidationRefused` with a reason in `error_reasons::fs_safety::ALL` |
//! | Radarr delete_movie_file (no paths in corpus) | constants root `Movies`, movie `Movie Title`, file `Movie Title.mkv` |
//! | clean_file_records: records, missing_from_disk | rows `file{i}.mkv` under root/Movie, last M not created; records_deleted = orphans returned |
//! | `{movie_updated: false}` | records_deleted 0 |
//! | ProcessPath rejection "possible malicious file" | `Some(_)` from unwanted_executable_in_download -> "Rejected", `None` -> "NoVideoFiles" |
//! | ImportMode Auto (absent) and Move | configured `ImportMode::Move` through the seeding gate, Scryer's only mode where the client's verdict decides (resolve_seeding_safe_import_mode, seeding_gate.rs:878). Scryer has no forced Move, so 0139/0185 are red |
//! | download_client_item.can_move_files | torrent observation `can_move_files` (Scryer reads it only from a torrent, seeding_gate.rs:294), with `can_remove: Some(true)` as glue: the reference has no seeding obligation, so can_move_files alone decides. Self-check: `can_move_files: Some(true)` keeps Move |
//! | `copyOnly` / `copy_only` | effective mode is HardlinkOrCopy |
//! | decision with size_mb | sparse file of size_mb MiB; imported_size_mb = size of the picked file |
//!
//! Import mode (0138/0184 green, 0139/0185 red): `copy_only` is whatever the
//! production seeding gate returns for the input `can_move_files`; the
//! self-check (`can_move_files: true` keeps Move) rules out a constant gate.
//! Root guard: expected is derived from the case's exception type.
//!
//! Ignored keys: `mounts`, `on_call`, `note`, `warnings`, `delete_reason`
//! (the sweep has no reason value), `old_record_deleted_times` and the
//! `MediaFileService.Delete: 0` call (guard runs first), `video_files`,
//! `subfolders`, `files_recursive` parents, `results` (2) and `imported` (1) of
//! 0183 (per-decision results do not exist; the pick returns one file by
//! construction), `file_placed_times`, `decisions`, `download_client_item`
//! title and output_path, 0183's `new_download` and `decisions[].movie` (the
//! reference orders by size whatever new_download is, ImportApprovedMovie.cs:73;
//! one movie), SON 0257 `files_exist` / `new_episode_file` and RAD 0261
//! `existing_file_on_disk` (no file can exist under a root that does not
//! exist on disk), RAD 0047/0048 `delete_fails` (false; the guard runs before
//! any delete). OrphanSweepRootChecks is built by struct literal
//! (equal to its private `new`, housekeeping.rs:2853).
//!
//! Unmapped (tests: []), with reasons:
//! - CompletedDownloadService (SON 0001-0020): check/import take `&AppUseCase`
//!   (completed_download/check.rs:9); AppServices::builder (bundles.rs:305)
//!   needs ten ports and only settings has a public Null fake.
//! - ExtraService (SON 0021-0023, RAD 0001-0007): Scryer's sidecar plan
//!   (import/workflow/sidecars.rs:88) has no extension-list setting, no "other"
//!   handler, no recursion mode; discovery is private and uses Instant::now.
//! - DownloadedEpisodes/MoviesImportService (SON 0028-0045 minus 0032/0039/0046/0047,
//!   RAD 0027-0044): drop-folder processing has no counterpart; lookup and
//!   cleanup are AppUseCase (series_movie.rs:797, results.rs:2907).
//! - Archive-only folder (SON 0046, RAD 0045): extraction runs first in the
//!   AppUseCase import (series_movie.rs:698); mapping would give a misleading red.
//! - Folder-created events (SON 0050-0052, RAD 0099, 0100): no such event;
//!   folders are created in FsFileImporter (another crate).
//! - ImportApprovedEpisodes/Movies others (SON 0130-0137, 0140-0150, RAD 0176-0182,
//!   0186-0189, 0192-0194): mock counts; Auto with no download client item
//!   (SON 0134, RAD 0180: the `completed: None` early return is inside the
//!   AppUseCase fn, seeding_gate.rs:888); original_file_path inside
//!   execute_upgrade; AppUseCase.
//! - ImportDecisionMaker (SON 0152-0157, RAD 0195-0202): injected specification
//!   stand-ins; Scryer's check list is a fixed array (checks.rs:347).
//! - Delete after root guard (SON 0239-0242, RAD 0049-0052): user_delete via AppUseCase.
//! - FilterExistingFiles (SON 0243-0248, RAD 0053-0058): no pre-import filter;
//!   the known-file match is an exact-string HashMap lookup inside the
//!   AppUseCase scan loop (library/scan/title_scan.rs:2987, :3163); no pure
//!   function holds it.
//! - UpgradeMediaFileService beyond the guard (SON 0249-0256, 0258, RAD 0256-0260):
//!   execute_upgrade needs &AppUseCase.
//! - DiskTransferService HardLink/Copy/faults/mounts (SON 0259, 0260, 0262, 0263,
//!   0265-0269, 0273-0280, 0292): placement is FsFileImporter
//!   (scryer-infrastructure-workflow), which depends on this crate; faults need
//!   a mock disk provider; 0260 depends on volume case sensitivity.
//! - Radarr disk_scan without file listings (RAD 0008-0010, 0014, 0015):
//!   AppUseCase library scan pipeline.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use scryer_application::fs_safety::{MoveOptions, move_file_exclusive};
use scryer_application::parity_seams::{
    ImportVideoFile, ORPHAN_SWEEP_PROBES_PER_PASS, OrphanSweepRootChecks, SeedingGateInput,
    TorrentSeedingObservation, evaluate_seeding_gate, next_orphan_media_file,
    pick_largest_import_video_file, resolve_available_root_for_path, unwanted_executable_in_download,
};
use scryer_application::stored_paths::path_to_stored_string;
use scryer_application::{AppError, HousekeepingMediaFileRootRow, error_reasons};
use scryer_domain::ImportMode;
use scryer_parity::Suite;
use serde_json::{Value, json};

use super::{MIB, rebase, touch};

/// (id, input, expect) of the corpus cases in `ids`, selected before any `wants`.
fn select(file: &str, ids: &[&str]) -> Vec<(String, Value, Value)> {
    let cases: Vec<Value> = scryer_parity::corpus(file);
    cases
        .into_iter()
        .filter(|c| ids.contains(&c["id"].as_str().expect("id")))
        .map(|c| (c["id"].as_str().unwrap().to_string(), c["input"].clone(), c["expect"].clone()))
        .collect()
}

fn open(name: &str) -> Option<Suite> {
    Suite::open(&format!("scryer-application::parity_import::{name}"))
}

fn list_names(dir: &Path) -> Vec<std::ffi::OsString> {
    std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name()).collect())
        .unwrap_or_default()
}

fn write_bytes(path: &Path, byte: u8, n: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![byte; n]).unwrap();
}

#[test]
fn son_transfer_move() {
    let Some(mut suite) = open("son_transfer_move") else { return };
    let ids = ["SON-IMPORT-0261", "SON-IMPORT-0264", "SON-IMPORT-0270", "SON-IMPORT-0271", "SON-IMPORT-0272"];
    let tmp = tempfile::tempdir().unwrap();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    for (id, input, expect) in select(
        "corpus/reference/sonarr/import/Common.DiskTests.DiskTransferServiceFixture.json",
        &ids,
    ) {
        if !suite.wants(&id) {
            continue;
        }
        let given = &input["given"];
        assert_eq!(input["op"], "DiskTransferService.TransferFile");
        assert_eq!(given["mode"], "Move");
        let dir = tmp.path().join(&id);
        let (src_ref, tgt_ref) = (given["source"].as_str().unwrap(), given["target"].as_str().unwrap());
        let (src, tgt) = (rebase(&dir, src_ref), rebase(&dir, tgt_ref));
        let mut original = Vec::new();
        for (path, n) in given["existing_files"].as_object().unwrap() {
            let byte = if path == src_ref { b's' } else { b't' };
            write_bytes(&rebase(&dir, path), byte, n.as_u64().unwrap() as usize);
            if path == src_ref {
                original = vec![byte; n.as_u64().unwrap() as usize];
            }
        }
        let overwrite = given["overwrite"].as_bool().unwrap_or(false);
        let result = rt.block_on(move_file_exclusive(
            &src,
            &tgt,
            MoveOptions { overwrite, ..MoveOptions::default() },
        ));
        let name_of = |p: &Path| p.file_name().unwrap().to_os_string();
        let moved = list_names(tgt.parent().unwrap()).contains(&name_of(&tgt))
            && !list_names(src.parent().unwrap()).contains(&name_of(&src))
            && std::fs::read(&tgt).map(|b| b == original).unwrap_or(false);
        // A pre-existing target (input) still holds its own 't' bytes.
        let target_kept = given["existing_files"].get(tgt_ref).map(|n| {
            std::fs::read(&tgt).is_ok_and(|b| b == vec![b't'; n.as_u64().unwrap() as usize])
        });
        let calls = expect["calls"].as_object();
        let moved_expected = calls.is_some_and(|calls| {
            calls.iter().any(|(k, n)| {
                let inner = k.strip_prefix("MoveFile(").and_then(|s| s.strip_suffix(')'));
                let parts: Vec<&str> = inner.map(|s| s.split(", ").collect()).unwrap_or_default();
                parts.get(1) == Some(&tgt_ref) && n.as_u64().unwrap_or(0) >= 1
            })
        });
        // DeleteFile(target): 0 -> kept, >= 1 -> replaced, no key -> null.
        let kept_expected = calls
            .and_then(|calls| calls.get(&format!("DeleteFile({tgt_ref})")))
            .map(|n| n.as_u64() == Some(0));
        // target_kept is compared only when the case names DeleteFile(target); with no
        // key both sides would be null by construction (a vacuous comparison).
        let (mut e, mut a) = (
            json!({"ok": expect.get("throws").is_none(), "moved": moved_expected}),
            json!({"ok": result.is_ok(), "moved": moved}),
        );
        if let Some(k) = kept_expected {
            e["target_kept"] = json!(k);
            a["target_kept"] = json!(target_kept.expect("target_kept needs a target in existing_files"));
        }
        suite.check(&id, e, a);
    }
    suite.finish();
}

/// Refused by the root guard (not by path-outside-roots).
fn guard_actual(file: &Path, root: &Path) -> Value {
    let refused = resolve_available_root_for_path(file, &[root.to_path_buf()]);
    let root_guard = matches!(&refused, Err(AppError::ValidationRefused { reason, .. })
        if error_reasons::fs_safety::ALL.contains(reason));
    json!({"refused": refused.is_err(), "root_guard": root_guard})
}

fn guard_self_check(tmp: &Path) {
    let root = tmp.join("selfcheck");
    touch(&root.join("x/y.mkv"));
    assert!(resolve_available_root_for_path(&root.join("x/y.mkv"), &[root.clone()]).is_ok());
}

/// Expected from the case: the reference throws (or errors) with a root-folder
/// exception type; both types it uses here are its root-folder guard.
fn guard_expected(expect: &Value) -> Value {
    let kind = expect.get("throws").or_else(|| expect.get("error")).and_then(Value::as_str);
    let root = matches!(kind, Some("NzbDroneClientException" | "RootFolderNotFoundException"));
    json!({"refused": kind.is_some(), "root_guard": root})
}

fn bool_of(v: &Value, k: &str) -> bool {
    v[k].as_bool().unwrap_or(false)
}

#[test]
fn son_root_guard() {
    let Some(mut suite) = open("son_root_guard") else { return };
    let tmp = tempfile::tempdir().unwrap();
    let mut cases = select(
        "corpus/reference/sonarr/import/MediaFiles.MediaFileDeletionService.DeleteEpisodeFileFixture.json",
        &["SON-IMPORT-0237", "SON-IMPORT-0238"],
    );
    cases.extend(select(
        "corpus/reference/sonarr/import/MediaFiles.UpgradeMediaFileServiceFixture.json",
        &["SON-IMPORT-0257"],
    ));
    for (id, input, expect) in cases {
        if !suite.wants(&id) {
            continue;
        }
        let g = &input["given"];
        let dir = tmp.path().join(&id);
        let (root, file) = if input["op"] == "MediaFileDeletionService.DeleteEpisodeFile" {
            let root = rebase(&dir, g["root_folder"].as_str().unwrap());
            let series = root.join(g["series_folder"].as_str().unwrap());
            let rel = g["episode_file"]["relative_path"].as_str().unwrap().replace('\\', "/");
            let file = series.join(rel);
            if bool_of(g, "root_folder_exists") {
                std::fs::create_dir_all(&root).unwrap();
            }
            if bool_of(g, "root_folder_has_folders") {
                let sub = if bool_of(g, "series_folder_exists") { series.clone() } else { root.join("Other") };
                std::fs::create_dir_all(sub).unwrap();
            }
            if bool_of(g, "series_folder_exists") {
                std::fs::create_dir_all(&series).unwrap();
            }
            if bool_of(g, "file_exists") {
                touch(&file);
            }
            (root, file)
        } else {
            assert_eq!(input["op"], "UpgradeMediaFileService.UpgradeEpisodeFile");
            let series = rebase(&dir, g["series_path"].as_str().unwrap());
            let root = series.parent().unwrap().to_path_buf();
            let rel = g["episodes"][0]["file"]["relative_path"].as_str().unwrap().replace('\\', "/");
            let file = series.join(rel);
            if bool_of(g, "root_folder_exists") {
                touch(&file);
            }
            (root, file)
        };
        suite.check(&id, guard_expected(&expect), guard_actual(&file, &root));
    }
    guard_self_check(tmp.path());
    suite.finish();
}

#[test]
fn rad_root_guard() {
    let Some(mut suite) = open("rad_root_guard") else { return };
    let tmp = tempfile::tempdir().unwrap();
    for (id, input, expect) in select(
        "corpus/reference/radarr/import/file_ops.json",
        &["RAD-IMPORT-0047", "RAD-IMPORT-0048", "RAD-IMPORT-0261"],
    ) {
        if !suite.wants(&id) {
            continue;
        }
        let g = &input["given"];
        let dir = tmp.path().join(&id);
        let (root, file) = match input["op"].as_str().unwrap() {
            "delete_movie_file" => {
                let root = dir.join("Movies");
                let movie = root.join("Movie Title");
                let file = movie.join("Movie Title.mkv");
                if bool_of(g, "root_folder_exists") {
                    std::fs::create_dir_all(&root).unwrap();
                }
                if bool_of(g, "root_folder_has_subfolders") {
                    let sub = if bool_of(g, "movie_folder_exists") { movie.clone() } else { root.join("Other Movie") };
                    std::fs::create_dir_all(sub).unwrap();
                }
                if bool_of(g, "movie_folder_exists") {
                    std::fs::create_dir_all(&movie).unwrap();
                }
                if bool_of(g, "file_exists") {
                    touch(&file);
                }
                (root, file)
            }
            "upgrade_movie_file" => {
                let movie = rebase(&dir, g["movie_path"].as_str().unwrap());
                let root = movie.parent().unwrap().to_path_buf();
                let file = movie.join(g["existing_file"].as_str().unwrap());
                if bool_of(g, "root_folder_exists") {
                    touch(&file);
                }
                (root, file)
            }
            other => panic!("unexpected op {other}"),
        };
        suite.check(&id, guard_expected(&expect), guard_actual(&file, &root));
    }
    guard_self_check(tmp.path());
    suite.finish();
}

#[test]
fn rad_orphan_records() {
    let Some(mut suite) = open("rad_orphan_records") else { return };
    let tmp = tempfile::tempdir().unwrap();
    for (id, input, expect) in select(
        "corpus/reference/radarr/import/media_file_service.json",
        &["RAD-IMPORT-0059", "RAD-IMPORT-0060", "RAD-IMPORT-0063"],
    ) {
        if !suite.wants(&id) {
            continue;
        }
        assert_eq!(input["op"], "clean_file_records");
        let g = &input["given"];
        let (n, missing) = (g["records"].as_u64().unwrap(), g["missing_from_disk"].as_u64().unwrap());
        let root = tmp.path().join(&id).join("Movies");
        let rows: Vec<HousekeepingMediaFileRootRow> = (0..n)
            .map(|i| {
                let file = root.join("Movie").join(format!("file{i}.mkv"));
                if i < n - missing {
                    touch(&file);
                } else {
                    std::fs::create_dir_all(root.join("Movie")).unwrap();
                }
                HousekeepingMediaFileRootRow {
                    media_file_id: i.to_string(),
                    title_id: "movie".into(),
                    file_path: path_to_stored_string(&file),
                    library_id: "lib".into(),
                    root_paths: vec![path_to_stored_string(&root)],
                }
            })
            .collect();
        let mut pending = VecDeque::from(rows);
        let mut checks = OrphanSweepRootChecks { verdicts: HashMap::new(), rows_in_window: 0 };
        let mut deleted = 0u64;
        while !pending.is_empty() {
            if next_orphan_media_file(&mut pending, ORPHAN_SWEEP_PROBES_PER_PASS, &mut checks).is_some() {
                deleted += 1;
            }
        }
        let expected = if let Some(d) = expect["records_deleted"].as_u64() {
            d
        } else {
            assert_eq!(expect["movie_updated"], false);
            0
        };
        suite.check(&id, json!({"records_deleted": expected}), json!({"records_deleted": deleted}));
    }
    suite.finish();
}

fn executable_suite(name: &str, file: &str, id: &str, expected_result: fn(&Value) -> Value) {
    let Some(mut suite) = open(name) else { return };
    let tmp = tempfile::tempdir().unwrap();
    for (id, input, expect) in select(file, &[id]) {
        if !suite.wants(&id) {
            continue;
        }
        let dest = tmp.path().join(&id).join("download");
        std::fs::create_dir_all(&dest).unwrap();
        for f in input["given"]["files_recursive"].as_array().unwrap() {
            let name = f.as_str().unwrap().rsplit('/').next().unwrap();
            touch(&dest.join(name));
        }
        let result = if unwanted_executable_in_download(&dest, None).is_some() { "Rejected" } else { "NoVideoFiles" };
        // Only the result string is compared; result counts have no Scryer counterpart.
        suite.check(&id, json!({"result": expected_result(&expect)}), json!({"result": result}));
    }
    suite.finish();
}

#[test]
fn son_unwanted_executable() {
    executable_suite(
        "son_unwanted_executable",
        "corpus/reference/sonarr/import/MediaFiles.DownloadedEpisodesImportServiceFixture.json",
        "SON-IMPORT-0047",
        |e| e["results"][0]["result"].clone(),
    );
}

#[test]
fn rad_unwanted_executable() {
    executable_suite(
        "rad_unwanted_executable",
        "corpus/reference/radarr/import/downloaded_import.json",
        "RAD-IMPORT-0046",
        |e| e["result"].clone(),
    );
}

/// Whether a configured Move becomes HardlinkOrCopy for a torrent whose client
/// reports `can_move_files`. `can_remove: Some(true)` discharges the client's
/// seeding obligation (the reference has none), so `can_move_files` alone decides.
fn copy_only_for_move(can_move_files: Option<bool>) -> bool {
    let decision = evaluate_seeding_gate(&SeedingGateInput {
        is_torrent: true,
        client_type: String::new(),
        present_in_client: true,
        observation: Some(TorrentSeedingObservation { can_remove: Some(true), can_move_files, ..Default::default() }),
        goals: None,
        now: chrono::DateTime::UNIX_EPOCH,
    });
    decision.import_mode(ImportMode::Move) == ImportMode::HardlinkOrCopy
}

/// The mode cases: import_mode absent (Auto) or Move, both run as configured
/// Move; can_move_files comes from the case's download_client_item.
fn mode_input(input: &Value) -> Option<bool> {
    let mode = &input["given"]["import_mode"];
    assert!(mode.is_null() || mode == "Move", "unexpected import_mode {mode}");
    let can_move = input["given"]["download_client_item"]["can_move_files"].as_bool();
    assert!(can_move.is_some(), "mode cases need a download client item");
    can_move
}

/// A gate that always downgrades cannot pass: a movable payload keeps Move.
fn mode_self_check() {
    assert!(!copy_only_for_move(Some(true)));
}

#[test]
fn son_import_mode() {
    let Some(mut suite) = open("son_import_mode") else { return };
    for (id, input, expect) in select(
        "corpus/reference/sonarr/import/MediaFiles.EpisodeImport.ImportApprovedEpisodesFixture.json",
        &["SON-IMPORT-0138", "SON-IMPORT-0139"],
    ) {
        if !suite.wants(&id) {
            continue;
        }
        let can_move = mode_input(&input);
        let (key, _) = expect["calls"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(k, n)| k.starts_with("UpgradeEpisodeFile(copyOnly: ") && n.as_u64().unwrap_or(0) >= 1)
            .expect("copyOnly call");
        let expected = key.contains("copyOnly: true");
        suite.check(&id, json!({"copy_only": expected}), json!({"copy_only": copy_only_for_move(can_move)}));
    }
    mode_self_check();
    suite.finish();
}

#[test]
fn rad_import_mode() {
    let Some(mut suite) = open("rad_import_mode") else { return };
    for (id, input, expect) in select(
        "corpus/reference/radarr/import/import_approved.json",
        &["RAD-IMPORT-0184", "RAD-IMPORT-0185"],
    ) {
        if !suite.wants(&id) {
            continue;
        }
        let can_move = mode_input(&input);
        suite.check(
            &id,
            json!({"copy_only": expect["copy_only"]}),
            json!({"copy_only": copy_only_for_move(can_move)}),
        );
    }
    mode_self_check();
    suite.finish();
}

#[test]
fn rad_import_largest() {
    let Some(mut suite) = open("rad_import_largest") else { return };
    let tmp = tempfile::tempdir().unwrap();
    for (id, input, expect) in select("corpus/reference/radarr/import/import_approved.json", &["RAD-IMPORT-0183"]) {
        if !suite.wants(&id) {
            continue;
        }
        let dir = tmp.path().join(&id);
        let decisions = input["given"]["decisions"].as_array().unwrap();
        let mut files: Vec<(ImportVideoFile, u64)> = Vec::new();
        for d in decisions {
            let path: PathBuf = rebase(&dir, d["path"].as_str().unwrap());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mb = d["size_mb"].as_u64().unwrap();
            std::fs::File::create(&path).unwrap().set_len(mb * MIB).unwrap();
            files.push((ImportVideoFile::physical(path), mb));
        }
        let all: Vec<ImportVideoFile> = files.iter().map(|(f, _)| f.clone()).collect();
        let picked = pick_largest_import_video_file(&all).expect("pick");
        let size = files.iter().find(|(f, _)| *f == picked).map(|(_, mb)| *mb);
        suite.check(
            &id,
            json!({"imported_size_mb": expect["imported_size_mb"]}),
            json!({"imported_size_mb": size}),
        );
    }
    suite.finish();
}
