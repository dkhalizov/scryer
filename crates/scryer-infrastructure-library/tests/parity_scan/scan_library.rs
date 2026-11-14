//! Parity net, scan area, stream scan_library (notes/11-parity-net-design.md).
//!
//! Suites (corpus `parity/corpus/reference/sonarr/scan/`):
//! - `son_scan_root_guard`: SON-SCAN-0001, 0002, 0005
//!   (`MediaFiles.DiskScanServiceTests.ScanFixture.json`, `DiskScanService.Scan`
//!   cases without `files_on_disk` that keep at least one compared calls key).
//! - `son_scan_clean`: SON-SCAN-0022, 0023, 0024, 0026
//!   (`MediaFiles.MediaFileTableCleanupServiceFixture.json`).
//! - `son_scan_media_info_queue`: SON-SCAN-0027, 0028, 0029
//!   (`MediaFiles.MediaInfo.UpdateMediaInfoServiceFixture.json`, `Handle`).
//! - `son_scan_video_files`: SON-SCAN-0138, 0139, 0140
//!   (`ProviderTests.DiskScanProviderTests.GetVideoFilesFixture.json`).
//!
//! Scryer functions under test:
//! - scryer_application::parity_seams::{episodic_title_directory_present,
//!   library_root_state, LibraryRootState} (library/scan/title_scan.rs:105,
//!   :138; caller :3020-3060) and tracked_media_path_confirmed_missing
//!   (title_scan.rs:331, the only episodic retire gate, :3549-3560).
//! - MediaFileStore::list_media_files_needing_analysis (media_file_store.rs:2080,
//!   candidate query of refresh_stale_media_analysis_batch, media/refresh.rs:28-36)
//!   on in-memory SQLite.
//! - FileSystemLibraryScanner::scan_directory (recursive title inventory) and
//!   scan_directory_children (top level, media/libraries/scanner.rs:403-410).
//!
//! Translation (reference -> Scryer). Paths are rebuilt under a temp dir:
//! `/C/Test/TV/Series` -> `<tmp>/<id>/C/Test/TV/Series`.
//!
//! son_scan_root_guard (the series folder is never created by the fixture):
//! | Reference | Scryer actual |
//! |---|---|
//! | GetFiles(series.Path, true) n, GetImportDecisions(any, series, false) n | walks_series_folder = episodic_title_directory_present(series) |
//! | MediaFileTableCleanupService.Clean n | cleans_tracked_files = library_root_state(root) == Populated |
//! CreateFolder(series.Path) is ignored: both guards are read-only, so a
//! folder check made after them is constant; see SON-SCAN-0004 below.
//!
//! son_scan_clean (series folder created). NBuilder gives several rows the
//! same relative_path; Scryer keeps one tracked record per path
//! (title_scan.rs:2991-3018), so each row gets its own path
//! `<series>/<row id>-<relative_path>`, touched when `<series>/<relative_path>`
//! is in given.files_on_disk:
//! | Reference | Scryer actual |
//! |---|---|
//! | Delete(path == DELETED_PATH, MissingFromDisk) n | retired_missing_from_disk = db rows for which tracked_media_path_confirmed_missing is true |
//! | Delete(any, NoLinkedEpisodes) n | retired_without_episode_link = retired rows no episode links to |
//! | UpdateEpisode(any) n | episodes_unlinked = episodes whose episode_file_id is a retired row |
//!
//! son_scan_media_info_queue (real store, in-memory SQLite, DB path strings only):
//! | Reference | Scryer |
//! |---|---|
//! | files with MediaInfo null or SchemaRevision below the minimum | list_media_files_needing_analysis(ANALYSIS_REVISION): rows below ANALYSIS_REVISION |
//! | media_info null | never analysed row |
//! | schema_revision >= 11 (Sonarr current) | analysis saved at ANALYSIS_REVISION |
//! | schema_revision < 11 | analysis saved at ANALYSIS_REVISION - 1 (no separate minimum) |
//! | GetMediaInfo(path) n (summed) | candidate rows returned |
//!
//! son_scan_video_files (top.mkv and "Season 1/nested.mkv" supplied by the harness):
//! | Reference | Scryer |
//! |---|---|
//! | GetVideoFiles(path[, true]) | scan_directory |
//! | GetVideoFiles(path, false) | scan_directory_children |
//! | GetFiles(path, true) n | lists_nested = nested.mkv listed (expected n > 0) |
//! | GetFiles(path, false) n | top_level_only = nested.mkv not listed (expected n > 0) |
//! | harness invariant | lists_top_level = top.mkv listed (expected true, supplied by the harness) |
//! Scryer has no recursion flag; an absent flag is translated to
//! scan_directory, so 0138 and 0139 exercise the same call. top_level_only is
//! the negation of lists_nested; it is kept so each GetFiles key has a field.
//!
//! Ignored keys: expect.warnings (log line), FolderExists(series.Path) (mock
//! count), CreateFolder(series.Path) (see above), given.config.create_empty_series_folders (no Scryer setting; fixed
//! behaviour equals off), MediaFileService.Update(any) (follows a real media
//! read), given.config.enable_media_info and series.id, exact GetFiles call
//! counts (only the zero/non-zero outcome is compared).
//!
//! Unmapped (tests: []), with reasons:
//! - SON-SCAN-0004: its only compared assertion (no folder created) can be
//!   observed only through the AppUseCase title scan, which is more than a
//!   visibility seam; the read-only guards would make it a constant.
//! - SON-SCAN-0025: dangling episode links cannot exist; file_episode_map.file_id
//!   references media_files(id) ON DELETE CASCADE
//!   (crates/scryer/src/db/baselines/0341_baseline.sql:710,
//!   crates/scryer/src/db/postgres/baselines/0341_baseline.sql:3901).
//! - SON-SCAN-0030: skipping a missing file is decided inside
//!   AppUseCase::refresh_stale_media_analysis_batch (media/refresh.rs:28-200), not
//!   by the store query; needs AppUseCase with repositories and a MediaAnalyzer.
//! - SON-SCAN-0031: needs an analyzer returning nothing for one path; no
//!   deterministic reader without AppUseCase and real media.
//! - SON-SCAN-0034..0039: read-then-save-or-keep branch is AppUseCase code
//!   (media/refresh.rs:88-190) over a MediaAnalyzer on real media.

use std::path::{Path, PathBuf};

use scryer_application::parity_seams::{
    LibraryRootState, episodic_title_directory_present, library_root_state,
    tracked_media_path_confirmed_missing,
};
use scryer_application::{
    InsertMediaFileInput, LibraryScanner, MediaFileAnalysis, MediaFileRepository,
    TitleRepository,
};
use scryer_infrastructure_datastore::SqliteServices;
use scryer_infrastructure_library::libraries::scanner::FileSystemLibraryScanner;
use scryer_infrastructure_library::search::media_file_store::MediaFileStore;
use scryer_infrastructure_library::titles::store::TitleStore;
use scryer_parity::Suite;
use serde_json::{Map, Value, json};

const DIR: &str = "corpus/reference/sonarr/scan/";

fn cases(file: &str) -> Vec<Value> {
    scryer_parity::corpus(&format!("{DIR}{file}"))
}

/// `/C/Test/x` and `C:\Test\x` -> `<dir>/C/Test/x`.
fn rebase(dir: &Path, reference: &str) -> PathBuf {
    let normalized = reference.replace('\\', "/").replace(':', "");
    dir.join(normalized.trim_start_matches('/'))
}

fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, b"x").expect("write");
}

fn calls(case: &Value) -> &Map<String, Value> {
    case["expect"]["calls"].as_object().expect("expect.calls")
}

fn count(v: &Value) -> u64 {
    v.as_u64().expect("call count")
}

/// Mock call counts with no observable outcome through the guards.
const IGNORED_GUARD_KEYS: [&str; 2] = ["FolderExists(series.Path)", "CreateFolder(series.Path)"];

#[tokio::test]
async fn son_scan_root_guard() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-library::parity_scan::son_scan_root_guard")
    else {
        return;
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    for case in cases("MediaFiles.DiskScanServiceTests.ScanFixture.json") {
        let id = case["id"].as_str().expect("id");
        let given = &case["input"]["given"];
        if case["input"]["op"] != "DiskScanService.Scan" || given.get("files_on_disk").is_some() {
            continue;
        }
        // Skip cases with nothing left to compare (0004: CreateFolder only).
        if calls(&case).keys().all(|k| IGNORED_GUARD_KEYS.contains(&k.as_str())) {
            continue;
        }
        if !suite.wants(id) {
            continue;
        }
        let dir = tmp.path().join(id);
        let series = rebase(&dir, given["series_path"].as_str().unwrap());
        let root = rebase(&dir, given["root_folder"].as_str().unwrap());
        if given["root_folder_exists"] == true {
            std::fs::create_dir_all(&root).unwrap();
        }
        for sub in given["root_folder_subfolders"].as_array().into_iter().flatten() {
            std::fs::create_dir_all(rebase(&dir, sub.as_str().unwrap())).unwrap();
        }
        let walks = episodic_title_directory_present(&series).await.expect("present");
        let root_state = library_root_state(&root).await.expect("root state");
        let mut expected = Map::new();
        let mut actual = Map::new();
        for (key, n) in calls(&case) {
            let (name, got) = match key.as_str() {
                "GetFiles(series.Path, true)" | "GetImportDecisions(any, series, false)" => {
                    ("walks_series_folder", walks)
                }
                "MediaFileTableCleanupService.Clean" => {
                    ("cleans_tracked_files", root_state == LibraryRootState::Populated)
                }
                k if IGNORED_GUARD_KEYS.contains(&k) => continue,
                other => panic!("unknown calls key {other}"),
            };
            expected.insert(name.into(), json!(count(n) > 0));
            actual.insert(name.into(), json!(got));
        }
        suite.check(id, Value::Object(expected), Value::Object(actual));
    }
    suite.finish();
}

#[tokio::test]
async fn son_scan_clean() {
    let Some(mut suite) = Suite::open("scryer-infrastructure-library::parity_scan::son_scan_clean")
    else {
        return;
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    for case in cases("MediaFiles.MediaFileTableCleanupServiceFixture.json") {
        let id = case["id"].as_str().expect("id");
        let given = &case["input"]["given"];
        if case["input"]["op"] != "MediaFileTableCleanupService.Clean" {
            continue;
        }
        let db_ids: Vec<u64> = given["db_files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["id"].as_u64().unwrap())
            .collect();
        let episodes = given["episodes"].as_array().unwrap();
        // Scryer cannot hold an episode link to a file id the DB lacks (0025).
        if episodes.iter().any(|e| {
            let f = e["episode_file_id"].as_u64().unwrap();
            f > 0 && !db_ids.contains(&f)
        }) {
            continue;
        }
        if !suite.wants(id) {
            continue;
        }
        let dir = tmp.path().join(id);
        let series_dir = rebase(&dir, given["series_path"].as_str().unwrap());
        std::fs::create_dir_all(&series_dir).unwrap();
        let on_disk: Vec<PathBuf> = given["files_on_disk"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| rebase(&dir, f.as_str().unwrap()))
            .collect();
        let mut retired = Vec::new();
        for row in given["db_files"].as_array().unwrap() {
            let (row_id, rel) = (row["id"].as_u64().unwrap(), row["relative_path"].as_str().unwrap());
            let path = series_dir.join(format!("{row_id}-{rel}"));
            if on_disk.contains(&series_dir.join(rel)) {
                touch(&path);
            }
            if tracked_media_path_confirmed_missing(&path, &series_dir).await {
                retired.push(row["id"].as_u64().unwrap());
            }
        }
        let linked = |file: u64| {
            episodes
                .iter()
                .any(|e| e["episode_file_id"].as_u64() == Some(file))
        };
        let mut expected = Map::new();
        let mut actual = Map::new();
        for (key, n) in calls(&case) {
            let (name, got) = match key.as_str() {
                "Delete(e.RelativePath == DELETED_PATH, MissingFromDisk)" => {
                    ("retired_missing_from_disk", retired.len())
                }
                "Delete(any, NoLinkedEpisodes)" => (
                    "retired_without_episode_link",
                    retired.iter().filter(|f| !linked(**f)).count(),
                ),
                "UpdateEpisode(any)" => (
                    "episodes_unlinked",
                    episodes
                        .iter()
                        .filter(|e| retired.contains(&e["episode_file_id"].as_u64().unwrap()))
                        .count(),
                ),
                other => panic!("unknown calls key {other}"),
            };
            expected.insert(name.into(), json!(count(n)));
            actual.insert(name.into(), json!(got));
        }
        suite.check(id, Value::Object(expected), Value::Object(actual));
    }
    suite.finish();
}

#[tokio::test]
async fn son_scan_media_info_queue() {
    let Some(mut suite) =
        Suite::open("scryer-infrastructure-library::parity_scan::son_scan_media_info_queue")
    else {
        return;
    };
    assert!(scryer_media_types::ANALYSIS_REVISION >= 1);
    let epoch = chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap();
    for case in cases("MediaFiles.MediaInfo.UpdateMediaInfoServiceFixture.json") {
        let id = case["id"].as_str().expect("id");
        let given = &case["input"]["given"];
        if case["input"]["op"] != "UpdateMediaInfoService.Handle"
            || given["file_exists"] != true
            || given["reader"] != "returns media info for every path"
        {
            continue;
        }
        if !suite.wants(id) {
            continue;
        }
        let services = SqliteServices::new(":memory:").await.expect("sqlite");
        let title = scryer_domain::Title {
            id: format!("title-{id}"),
            name: "Media Info Queue".to_string(),
            facet: scryer_domain::MediaFacet::Series,
            library_id: scryer_domain::default_library_id_for_facet(
                &scryer_domain::MediaFacet::Series,
            ),
            monitored: true,
            tags: vec![],
            canonical_tags: vec![],
            external_ids: vec![],
            root_folder_id: scryer_domain::root_folder_id_for_path("/data/series"),
            created_by: None,
            created_at: epoch,
            year: Some(2026),
            overview: None,
            poster_url: None,
            poster_source_url: None,
            background_url: None,
            background_source_url: None,
            sort_title: None,
            catalog_sort_key: String::new(),
            slug: None,
            imdb_id: None,
            runtime_minutes: None,
            popularity: None,
            content_status: None,
            language: None,
            first_aired: None,
            network: None,
            studio: None,
            country: None,
            aliases: vec![],
            tagged_aliases: vec![],
            metadata_language: None,
            metadata_fetched_at: None,
            min_availability: None,
            digital_release_date: None,
            movie_release_dates: None,
            folder_path: None,
        };
        TitleStore::new(services.datastore())
            .create(title.clone())
            .await
            .expect("create title");
        let files = MediaFileStore::new(services.datastore());
        let series_path = given["series"]["path"].as_str().unwrap();
        for row in given["files"].as_array().unwrap() {
            let row_id = row["id"].as_u64().unwrap();
            let file_id = files
                .insert_media_file(&InsertMediaFileInput {
                    title_id: title.id.clone(),
                    file_path: format!(
                        "{series_path}/{row_id}-{}",
                        row["relative_path"].as_str().unwrap()
                    ),
                    size_bytes: 1,
                    ..Default::default()
                })
                .await
                .expect("insert");
            if let Some(info) = row["media_info"].as_object() {
                let revision = if info["schema_revision"].as_u64().unwrap() >= 11 {
                    scryer_media_types::ANALYSIS_REVISION
                } else {
                    scryer_media_types::ANALYSIS_REVISION - 1
                };
                let saved = files.get_media_file_by_id(&file_id).await.unwrap().unwrap();
                let mut analysis = MediaFileAnalysis::default();
                analysis.details.revision = revision;
                analysis.duration_seconds = Some(1);
                assert!(
                    files
                        .update_media_file_analysis_if_unchanged(&saved, analysis)
                        .await
                        .unwrap(),
                    "fixture analysis not saved"
                );
            }
        }
        let candidates = files
            .list_media_files_needing_analysis(
                &title.library_id,
                scryer_media_types::ANALYSIS_REVISION,
                epoch,
                20,
            )
            .await
            .expect("list")
            .len();
        let read: u64 = calls(&case)
            .iter()
            .map(|(key, n)| match key.as_str() {
                k if k.starts_with("GetMediaInfo(") => count(n),
                "MediaFileService.Update(any)" => 0,
                other => panic!("unknown calls key {other}"),
            })
            .sum();
        suite.check(id, json!({"files_read": read}), json!({"files_read": candidates}));
    }
    suite.finish();
}

#[tokio::test]
async fn son_scan_video_files() {
    let Some(mut suite) =
        Suite::open("scryer-infrastructure-library::parity_scan::son_scan_video_files")
    else {
        return;
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    let scanner = FileSystemLibraryScanner::new();
    for case in cases("ProviderTests.DiskScanProviderTests.GetVideoFilesFixture.json") {
        let id = case["id"].as_str().expect("id");
        let given = &case["input"]["given"];
        if case["input"]["op"] != "DiskScanService.GetVideoFiles"
            || case["expect"].get("calls").is_none()
        {
            continue;
        }
        if !suite.wants(id) {
            continue;
        }
        let dir = tmp.path().join(id);
        let root = rebase(&dir, given["path"].as_str().unwrap());
        touch(&root.join("top.mkv"));
        touch(&root.join("Season 1").join("nested.mkv"));
        let root_str = root.to_str().unwrap();
        let listed = if given.get("all_directories").map_or(true, |v| v == true) {
            scanner.scan_directory(root_str).await
        } else {
            scanner.scan_directory_children(root_str).await
        }
        .expect("scan");
        let has = |suffix: &str| listed.iter().any(|f| f.path.ends_with(suffix));
        let mut expected = Map::new();
        let mut actual = Map::new();
        for (key, n) in calls(&case) {
            let n = count(n);
            let (name, want, got) = match key.as_str() {
                "GetFiles(path, true)" => ("lists_nested", n > 0, has("nested.mkv")),
                // a non-recursive call (n > 0) must not reach the nested file
                "GetFiles(path, false)" => ("top_level_only", n > 0, !has("nested.mkv")),
                other => panic!("unknown calls key {other}"),
            };
            expected.insert(name.into(), json!(want));
            actual.insert(name.into(), json!(got));
        }
        expected.insert("lists_top_level".into(), json!(true));
        actual.insert("lists_top_level".into(), json!(has("top.mkv")));
        suite.check(id, Value::Object(expected), Value::Object(actual));
    }
    suite.finish();
}
