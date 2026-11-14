//! Parity net, import area (notes/11-parity-net-design.md, parity/README.md).
//!
//! Suites:
//! - `scryer-application::parity_import::son_import`: corpus/reference/sonarr/import
//! - `scryer-application::parity_import::rad_import`: corpus/reference/radarr/import
//! - `scryer-application::parity_import::son_import_config`,
//!   `rad_import_config`: config entries, checked from the captured JSON they
//!   reference (corpus/captured/<app>/config/*.json).
//!
//! Scryer code under test:
//! - Import checks (crates/scryer-application/src/import/checks.rs):
//!   `check_not_sample` (:159), `check_not_unpacking` (:193) and
//!   `disk_space_verdict_for_measurement` (:97), the pure parts of the
//!   pre-import pipeline `run_checks_with_measurement` (:344). Reached through
//!   the parity seam re-export in lib.rs (see the seams in the phase report).
//! - The per-title inventory walk, as in tests/parity_scan.rs (the walker
//!   built by scryer-infrastructure-library scanner.rs:299-302, walked from the
//!   title folder), for Radarr's disk_scan cases.
//! - Quality from media info: `derive_primary_quality_label` (types.rs:4212,
//!   public, re-exported at lib.rs:914) with no fallback labels, i.e. the
//!   dimension thresholds of `quality_from_video_dimensions`
//!   (media/release_labels.rs:121) that the analysis label path uses.
//!
//! Every path is rebuilt under a temp dir: `/C/Test/x` and `C:\Test\x` become
//! `<tmp>/<case id>/C/Test/x`. Sample-detection paths are not created, so the
//! check is shown to decide from the path alone.
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | DetectSample result `Sample` | `ImportVerdict::Reject` with code `SampleFile` or `SampleDirectory` |
//! | DetectSample result `NotSample` | `ImportVerdict::Accept` |
//! | DetectSample result `Indeterminate` | no counterpart: Scryer's verdict is always Accept or Reject, so these cases fail |
//! | spec `accepted: true / false` | `ImportVerdict::is_accept()` |
//! | FreeSpace: `GetAvailableSpace` returns n bytes / null / throws (`"throws"`, `"error"`) | `disk_space_verdict_for_measurement(Some(n) / None / None, size)`: measured_disk_space (checks.rs:264) turns a failed measurement into `None` |
//! | FreeSpace: `MinimumFreeSpaceWhenImporting` (MiB) | no setting: Scryer always reserves 500 MiB (checks.rs:79 DISK_SPACE_RESERVE_BYTES) |
//! | Radarr sizes in MB (`file_size_mb`, `free_space_mb`) | n * 1024 * 1024 bytes (the reference's `.Megabytes()`) |
//! | NotUnpacking: `DownloadClientWorkingFolders` (`_UNPACK_|_FAILED_`) | no setting: Scryer looks for `.!qB`, `.part`, `._unpack` suffixes and sibling marker files only |
//! | Radarr disk_scan `import_candidates` | number of files the title walk returns |
//! | Radarr disk_scan `possible_extra_files` | none (the walk lists video files only); compared as `null`, so the case fails |
//! | AugmentQualityFromMediaInfo `width`, `height` (absent = 0) | `derive_primary_quality_label(Some(w), Some(h), None, None)` |
//! | `result: null` | label `None` |
//! | `resolution` 2160 / 1080 / 720 / 576 / 480 | label `4K` / `1080p` / `720p` / `576p` / `480p`; `1440p`, `4320p` have no reference resolution and are compared as the label string, so they fail |
//! | `source` Unknown (Sonarr) / UNKNOWN (Radarr) | no source: Scryer's analysis carries no container title and derives no source from media info; compared as the app's Unknown spelling, so Web / Bluray / WEBDL cases fail |
//!
//! Ignored keys:
//! - DetectSample `calls.GetRunTime: 0`, `runtime_probed: false`: probe counts
//!   are not observable (the check has no probe seam); the result next to them
//!   is compared. Cases whose only assertion is a probe count are unmapped.
//! - Log counts `warnings`, `errors_logged`: log output, not behaviour; the
//!   accept/reject or result next to them is compared.
//! - DetectSample `call` (`series` / `local_episode`), `series`, `episodes`,
//!   `quality`, runtimes: inputs to the reference's runtime rule; Scryer's rule
//!   reads only the path, so they do not reach it.
//! - NotUnpacking `last_write_offset_hours` / `last_write_ago_minutes`, `os`:
//!   on posix the reference rejects regardless of age (corpus note).
//! - FreeSpace `root_folder`, `series_path`, `movie_path`: which directory is
//!   measured; Scryer measures the destination volume, not observable here.
//!
//! Unmapped (tests: []), with reasons. Sonarr (corpus/reference/sonarr/import):
//! - CompletedDownloadService Import/Check (20), DownloadedEpisodesImportService
//!   (18), ImportApprovedEpisodes (19), ImportDecisionMaker (5),
//!   DiskTransferService (23), EpisodeFileMovingService (3),
//!   MediaFileDeletionService (6), UpgradeMediaFileService (10),
//!   MediaFileService.FilterExistingFiles (5), ExtraService (3): orchestration
//!   through AppUseCase (import/workflow/*.rs, pub(crate)) with repositories,
//!   download clients, file transfer and events; no in-memory seam, and most
//!   assertions are mock calls or published events.
//! - Aggregation and augmenters (AggregateEpisodes 6, AggregateQuality 11,
//!   AggregateReleaseGroup 5, AugmentQualityFromReleaseName 5) and
//!   GetSceneName (13): Scryer picks
//!   release evidence inside the crate-private import workflow
//!   (import/workflow/paths.rs:247 release_evidence_for_resolution) and has no
//!   source/resolution confidence levels; parsing a title itself is covered by
//!   the parse and quality areas.
//! - Specifications AlreadyImported (5, download-history rule; Scryer's
//!   check_not_already_imported, checks.rs:227, is a different content-identity
//!   rule), EpisodeTitle (4), FullSeason (3), HasAudioTrack (3), MatchesFolder
//!   (15), MatchesGrab (7), Upgrade (20): title resolution and
//!   import/decide.rs:271 decide_import (pub(crate), needs AppUseCase, profiles
//!   and media analysis).
//! - SON-IMPORT-0210 (NotSample), SON-IMPORT-0179 (FreeSpace): the spec is
//!   skipped for a file already in the series folder. Scryer's library scan
//!   never runs import checks, so there is no skip to observe.
//! - SON-IMPORT-0176 (FreeSpace): asserts the space is read for the root
//!   folder; measured_disk_space (checks.rs:264) is private and reads the real
//!   volume.
//!
//! Radarr (corpus/reference/radarr/import):
//! - disk_scan without file listings (RAD-IMPORT-0008..0010, 0014, 0015): root
//!   folder warnings, folder creation, record cleanup; AppUseCase scan pipeline.
//! - decision_maker (7), downloaded_import (19), import_approved (17),
//!   file_ops (14), media_file_service (8), extras (7): orchestration, as for
//!   Sonarr.
//! - aggregation other than augment_quality_media_info (42), scene_name (10):
//!   as for Sonarr.
//! - media_info (30): update_media_info needs AppUseCase and an analyzer;
//!   hdr_format lives in scryer-mediainfo (crate-private codec.rs:1812).
//! - specifications: AlreadyImported (5), GrabbedReleaseQuality (6),
//!   HasAudioTrack (3), MatchesGrab (5), Upgrade (10): as for Sonarr's
//!   specifications. NotMultiPart (10): no import check rejects multi-part
//!   files; the nearest code, location/conflicts.rs:436 stacked_part_number
//!   and :467 holds_multi_part_media, is private and serves the conflict
//!   keep-one feature, not import. NotSample RAD-IMPORT-0241 and FreeSpace
//!   RAD-IMPORT-0214 (existing file), 0211 (which folder is measured), 0216
//!   (SkipFreeSpaceCheckWhenImporting: Scryer has no such option, and the case
//!   gives no free space to compare).
//! - detect_sample RAD-IMPORT-0159: asserts only that runtime is probed once.
//!
//! Config entries mapped: SON-IMPORT-5002 / RAD-IMPORT-5002 (working folders)
//! and SON-IMPORT-5005 / RAD-IMPORT-5004 (minimum free space). The other
//! import config entries (completed download handling, hardlinks, episode
//! title required, extra files, recycle bin, empty folders, file dates,
//! permissions, script import, metadata writers, remote path mappings, root
//! folders) are Scryer settings resolved through AppUseCase and its settings
//! repository (e.g. settings/runtime/media.rs:595 import mode), or library
//! state; no pure function holds their defaults.

use std::path::{Path, PathBuf};

use scryer_application::filesystem_walk::FilesystemWalker;
use scryer_application::{
    ImportCheckCode, ImportCheckContext, ImportVerdict, LIBRARY_SCAN_MAX_RECURSIVE_DEPTH,
    check_not_sample, check_not_unpacking, derive_primary_quality_label,
    disk_space_verdict_for_measurement, parse_release_metadata,
};
use scryer_parity::Suite;
use serde_json::{Value, json};

const MIB: u64 = 1024 * 1024;

type Handler = fn(&Path, &Value, &Value) -> Option<(Value, Value)>;

/// Runs every case of one corpus file the suite wants; a handler returning
/// `None` leaves that case unmapped (nothing recorded).
fn run(suite: &mut Suite, file: &str, handler: Handler) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cases: Vec<Value> = scryer_parity::corpus(file);
    for case in &cases {
        let id = case["id"].as_str().expect("case id");
        if !suite.wants(id) {
            continue;
        }
        let dir = tmp.path().join(id);
        if let Some((expected, actual)) = handler(&dir, &case["input"]["given"], &case["expect"]) {
            suite.check(id, expected, actual);
        }
    }
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

/// Runs one import check on `path` with an otherwise neutral context.
fn verdict(
    path: &Path,
    size: u64,
    check: fn(&ImportCheckContext<'_>) -> ImportVerdict,
) -> ImportVerdict {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parsed = parse_release_metadata(&name);
    let dest = path.with_file_name("imported.mkv");
    check(&ImportCheckContext {
        source_path: path,
        dest_path: &dest,
        source_size: size,
        import_mode: scryer_domain::ImportMode::HardlinkOrCopy,
        parsed: &parsed,
        existing_files: &[],
        content_qualified_video: false,
    })
}

fn sample_result(verdict: &ImportVerdict) -> String {
    match verdict {
        ImportVerdict::Accept => "NotSample".into(),
        ImportVerdict::Reject {
            code: ImportCheckCode::SampleFile | ImportCheckCode::SampleDirectory,
            ..
        } => "Sample".into(),
        ImportVerdict::Reject { code, .. } => format!("Rejected({})", code.as_str()),
    }
}

/// Sonarr DetectSampleFixture and Radarr detect_sample: compare the result.
fn detect_sample(dir: &Path, given: &Value, expect: &Value) -> Option<(Value, Value)> {
    let expected = expect.get("result")?.clone();
    let path = rebase(dir, given["path"].as_str()?);
    let actual = sample_result(&verdict(&path, 0, check_not_sample));
    Some((expected, json!(actual)))
}

/// Available space as the reference's disk provider returns it.
fn measured(value: Option<&Value>, unit: u64) -> Option<Option<u64>> {
    match value? {
        Value::Null => Some(None),
        Value::String(s) if s == "throws" || s == "error" => Some(None),
        v => Some(Some(v.as_u64()? * unit)),
    }
}

fn free_space_outcome(available: Option<u64>, size: u64, expect: &Value) -> Option<(Value, Value)> {
    let expected = expect.get("accepted")?.as_bool()?;
    let actual = disk_space_verdict_for_measurement(available, size).is_accept();
    Some((json!(expected), json!(actual)))
}

fn unpacking_outcome(dir: &Path, given: &Value, expect: &Value) -> Option<(Value, Value)> {
    let expected = expect.get("accepted")?.as_bool()?;
    let path = rebase(dir, given["path"].as_str()?);
    touch(&path);
    let size = given.get("size").and_then(Value::as_u64).unwrap_or(0);
    let actual = verdict(&path, size, check_not_unpacking).is_accept();
    Some((json!(expected), json!(actual)))
}

fn son_free_space(_dir: &Path, given: &Value, expect: &Value) -> Option<(Value, Value)> {
    if given.get("existing_file") == Some(&json!(true)) || expect.get("calls").is_some() {
        return None;
    }
    let available = measured(given.get("available_space"), 1)?;
    let size = given.get("size").and_then(Value::as_u64).unwrap_or(0);
    free_space_outcome(available, size, expect)
}

fn son_unpacking(dir: &Path, given: &Value, expect: &Value) -> Option<(Value, Value)> {
    if given.get("existing_file") == Some(&json!(true)) {
        return None;
    }
    unpacking_outcome(dir, given, expect)
}

fn rad_spec(dir: &Path, given: &Value, expect: &Value) -> Option<(Value, Value)> {
    if given.get("existing_file") == Some(&json!(true)) {
        return None;
    }
    match given["spec"].as_str()? {
        "FreeSpace" => {
            if given.get("skip_free_space_check") == Some(&json!(true))
                || expect.get("free_space_read_for").is_some()
                || expect.get("free_space_read").is_some()
            {
                return None;
            }
            let available = measured(given.get("free_space_mb"), MIB)?;
            let size = given
                .get("file_size_mb")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                * MIB;
            free_space_outcome(available, size, expect)
        }
        "NotUnpacking" => unpacking_outcome(dir, given, expect),
        _ => None,
    }
}

/// Sonarr AugmentQualityFromMediaInfo, Radarr augment_quality_media_info
/// (translation table in the header). `unknown` is the app's spelling of
/// Source.Unknown.
fn media_info_quality(given: &Value, expect: &Value, unknown: &str) -> Option<(Value, Value)> {
    let info = given.get("media_info")?;
    let dim = |k: &str| info.get(k).and_then(Value::as_i64).unwrap_or(0) as i32;
    let actual = if info.is_null() {
        None
    } else {
        derive_primary_quality_label(Some(dim("width")), Some(dim("height")), None, None)
    };
    let actual = match actual.as_deref() {
        None => json!({ "result": null }),
        Some(label) => {
            let resolution = match label {
                "4K" => json!(2160),
                "1080p" => json!(1080),
                "720p" => json!(720),
                "576p" => json!(576),
                "480p" => json!(480),
                other => json!(other),
            };
            json!({ "resolution": resolution, "source": unknown })
        }
    };
    Some((expect.clone(), actual))
}

fn son_media_info(_dir: &Path, given: &Value, expect: &Value) -> Option<(Value, Value)> {
    media_info_quality(given, expect, "Unknown")
}

fn rad_aggregation(_dir: &Path, given: &Value, expect: &Value) -> Option<(Value, Value)> {
    media_info_quality(given, expect, "UNKNOWN")
}

/// Scryer's per-title inventory walk (tests/parity_scan.rs header).
fn title_scan(root: &Path) -> Vec<PathBuf> {
    FilesystemWalker::new()
        .skip_episodic_scan_junk_and_trailers()
        .supported_video_files_only()
        .max_depth(LIBRARY_SCAN_MAX_RECURSIVE_DEPTH)
        .walk(root)
        .expect("walk")
        .into_iter()
        .flat_map(|dir| dir.files)
        .collect()
}

fn rad_disk_scan(dir: &Path, given: &Value, expect: &Value) -> Option<(Value, Value)> {
    let count = expect.get("import_candidates")?.as_u64()?;
    let root = rebase(dir, given["movie_path"].as_str()?);
    std::fs::create_dir_all(&root).expect("mkdir");
    for file in given["files"].as_array()? {
        touch(&rebase(dir, file.as_str()?));
    }
    let found = title_scan(&root).len() as u64;
    let mut expected = json!({ "import_candidates": count });
    let mut actual = json!({ "import_candidates": found });
    if let Some(extra) = expect.get("possible_extra_files") {
        expected["possible_extra_files"] = extra.clone();
        actual["possible_extra_files"] = Value::Null;
    }
    Some((expected, actual))
}

#[test]
fn son_import() {
    let Some(mut suite) = Suite::open("scryer-application::parity_import::son_import") else { return };
    let dir = "corpus/reference/sonarr/import/";
    let files: [(&str, Handler); 4] = [
        (
            "MediaFiles.EpisodeImport.Aggregation.Aggregators.Augmenters.Quality.AugmentQualityFromMediaInfoFixture.json",
            son_media_info,
        ),
        (
            "MediaFiles.EpisodeImport.DetectSampleFixture.json",
            detect_sample,
        ),
        (
            "MediaFiles.EpisodeImport.Specifications.FreeSpaceSpecificationFixture.json",
            son_free_space,
        ),
        (
            "MediaFiles.EpisodeImport.Specifications.NotUnpackingSpecificationFixture.json",
            son_unpacking,
        ),
    ];
    for (file, handler) in files {
        run(&mut suite, &format!("{dir}{file}"), handler);
    }
    suite.finish();
}

#[test]
fn rad_import() {
    let Some(mut suite) = Suite::open("scryer-application::parity_import::rad_import") else { return };
    let dir = "corpus/reference/radarr/import/";
    let files: [(&str, Handler); 4] = [
        ("aggregation.json", rad_aggregation),
        ("detect_sample.json", detect_sample),
        ("specifications.json", rad_spec),
        ("disk_scan.json", rad_disk_scan),
    ];
    for (file, handler) in files {
        run(&mut suite, &format!("{dir}{file}"), handler);
    }
    suite.finish();
}

/// Working folders (downloadclientconfig.json#downloadClientWorkingFolders):
/// on posix the reference rejects any file with a parent folder whose name
/// starts with a listed token (Sonarr NotUnpackingSpecification.cs:34-45).
/// One release per token, each expected to be rejected.
fn working_folders_check(app: &str) -> (Value, Value) {
    let config: Value = scryer_parity::corpus(&format!(
        "corpus/captured/{app}/config/downloadclientconfig.json"
    ));
    let folders = config["downloadClientWorkingFolders"]
        .as_str()
        .expect("working folders");
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut expected = Vec::new();
    let mut actual = Vec::new();
    for token in folders.split('|') {
        let path = tmp
            .path()
            .join("downloads")
            .join(format!("{token}Release.Title.S01E01"))
            .join("Release.Title.S01E01.mkv");
        touch(&path);
        expected.push(json!({ "folder": token, "accepted": false }));
        actual.push(json!({ "folder": token, "accepted": verdict(&path, 1, check_not_unpacking).is_accept() }));
    }
    (json!(expected), json!(actual))
}

/// Minimum free space (mediamanagement.json#minimumFreeSpaceWhenImporting, MB,
/// with skipFreeSpaceCheckWhenImporting): the reference rejects when
/// `free < size + minimum` (Sonarr FreeSpaceSpecification.cs:50). Probed at
/// the boundary for a 1 GiB file: exactly `size + minimum` free, and one byte
/// less.
fn minimum_free_space_check(app: &str) -> (Value, Value) {
    let config: Value = scryer_parity::corpus(&format!(
        "corpus/captured/{app}/config/mediamanagement.json"
    ));
    let minimum = config["minimumFreeSpaceWhenImporting"]
        .as_u64()
        .expect("minimum")
        * MIB;
    let skip = config["skipFreeSpaceCheckWhenImporting"]
        .as_bool()
        .expect("skip flag");
    let size = 1024 * MIB;
    let probes = [size + minimum, size + minimum - 1];
    let expected: Vec<Value> = probes
        .iter()
        .map(|&free| json!({ "free": free, "accepted": skip || free >= size + minimum }))
        .collect();
    let actual: Vec<Value> = probes
        .iter()
        .map(|&free| {
            json!({ "free": free, "accepted": disk_space_verdict_for_measurement(Some(free), size).is_accept() })
        })
        .collect();
    (json!(expected), json!(actual))
}

type ConfigCheck = fn(&str) -> (Value, Value);

fn run_config(suite: &mut Suite, app: &str, checks: &[(&str, ConfigCheck)]) {
    for (id, check) in checks {
        if !suite.wants(id) {
            continue;
        }
        let (expected, actual) = check(app);
        suite.check(id, expected, actual);
    }
}

#[test]
fn son_import_config() {
    let Some(mut suite) = Suite::open("scryer-application::parity_import::son_import_config") else { return };
    let checks: [(&str, ConfigCheck); 2] = [
        ("SON-IMPORT-5002", working_folders_check),
        ("SON-IMPORT-5005", minimum_free_space_check),
    ];
    run_config(&mut suite, "sonarr", &checks);
    suite.finish();
}

#[test]
fn rad_import_config() {
    let Some(mut suite) = Suite::open("scryer-application::parity_import::rad_import_config") else { return };
    let checks: [(&str, ConfigCheck); 2] = [
        ("RAD-IMPORT-5002", working_folders_check),
        ("RAD-IMPORT-5004", minimum_free_space_check),
    ];
    run_config(&mut suite, "radarr", &checks);
    suite.finish();
}
