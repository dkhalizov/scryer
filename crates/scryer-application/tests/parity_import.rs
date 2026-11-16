//! Parity net, import area (notes/11-parity-net-design.md, parity/README.md).
//!
//! Suites:
//! - `scryer-application::parity_import::son_import`: corpus/reference/sonarr/import
//! - `scryer-application::parity_import::rad_import`: corpus/reference/radarr/import
//! - `scryer-application::parity_import::son_import_specs`,
//!   `rad_import_specs`: specification fixtures, in parity_import/import_specs.rs.
//! - `scryer-application::parity_import::son_import_config`,
//!   `rad_import_config`: config entries, checked from the captured JSON they
//!   reference (corpus/captured/<app>/config/*.json).
//! - `son_transfer_move`, `son_root_guard`, `rad_root_guard`,
//!   `rad_orphan_records`, `son_unwanted_executable`, `rad_unwanted_executable`,
//!   `son_import_mode`, `rad_import_mode`, `rad_import_largest`: orchestration
//!   sub-decisions, in parity_import/import_orchestration.rs.
//! - `rad_import_settings`, `son_import_settings`: captured import settings
//!   (hardlinks, recycle bin, remote path mappings), in
//!   parity_import/priv_import.rs.
//!
//! Scryer code under test:
//! - Import checks (crates/scryer-application/src/import/checks.rs):
//!   `check_not_sample` (:162), `check_not_unpacking` (:193) and
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
//! | DetectSample result `Sample` | `ImportVerdict::Reject` with code `SampleFile` or `SampleDirectory`, or a case with a runtime: `parity_seams::automatic_sample_rejection_code` returns any code but `sample_runtime_indeterminate` (too_short, zero, out_of_band) |
//! | DetectSample result `NotSample` | `ImportVerdict::Accept` |
//! | DetectSample result `Indeterminate` | `automatic_sample_rejection_code` returns `sample_runtime_indeterminate` (a null runtime key); the expected runtime comes from `expected_runtime_seconds_for_episode_import` (series) or movie minutes * 60 (Radarr) |
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
//! - DetectSample `calls.GetRunTime: 0`, `runtime_probed: false`, `media_info.runtime_seconds` (duplicates `file_runtime_seconds`): probe counts
//!   are not observable (the check has no probe seam); the result next to them
//!   is compared. Cases whose only assertion is a probe count are unmapped.
//! - Log counts `warnings`, `errors_logged`: log output, not behaviour; the
//!   accept/reject or result next to them is compared.
//! - DetectSample `call` (`series` / `local_episode`) and `quality`: not read
//!   (the runtimes, `series` and `episodes` do reach the rule through the seams).
//! - NotUnpacking `last_write_offset_hours` / `last_write_ago_minutes`, `os`:
//!   on posix the reference rejects regardless of age (corpus note).
//! - FreeSpace `root_folder`, `series_path`, `movie_path`: which directory is
//!   measured; Scryer measures the destination volume, not observable here.
//!
//! Unmapped (tests: []), with reasons. Sonarr (corpus/reference/sonarr/import):
//! - Orchestration fixtures (CompletedDownloadService, DownloadedEpisodesImportService,
//!   ImportApprovedEpisodes, ImportDecisionMaker, DiskTransferService,
//!   EpisodeFileMovingService, MediaFileDeletionService, UpgradeMediaFileService,
//!   FilterExistingFiles, ExtraService): stream import_orchestration
//!   (parity_import/import_orchestration.rs) maps SON 0047, 0138, 0139, 0237, 0238,
//!   0257, 0261, 0264, 0270-0272; its header lists the rest with reasons.
//! - Aggregation and augmenters: AggregateReleaseGroup (5),
//!   AugmentQualityFromReleaseName with a grab (3) and GetSceneName (13) are
//!   mapped in parity_import/import_aggregate.rs; AggregateEpisodes (6),
//!   AggregateQuality (11) and AugmentQualityFromReleaseName without a grab (2)
//!   stay unmapped, reasons in that header.
//! - Specifications: Upgrade (20), MatchesGrab 0206-0209 and MatchesFolder
//!   0192/0193/0195 are mapped in parity_import/import_specs.rs. The other
//!   groups (AlreadyImported: download-history rule, Scryer's
//!   check_not_already_imported, checks.rs:227, is a different content-identity
//!   rule; EpisodeTitle, FullSeason, HasAudioTrack, the remaining MatchesGrab
//!   and MatchesFolder cases) are listed there with reasons.
//! - SON-IMPORT-0210 (NotSample), SON-IMPORT-0179 (FreeSpace): the spec is
//!   skipped for a file already in the series folder. Scryer's library scan
//!   never runs import checks, so there is no skip to observe.
//! - SON-IMPORT-0176 (FreeSpace): asserts the space is read for the root
//!   folder; measured_disk_space (checks.rs:264) is private and reads the real
//!   volume.
//!
//! Radarr (corpus/reference/radarr/import):
//! - Orchestration (disk_scan without listings, decision_maker, downloaded_import,
//!   import_approved, file_ops, media_file_service, extras): stream
//!   import_orchestration (parity_import/import_orchestration.rs) maps RAD 0046,
//!   0047, 0048, 0059, 0060, 0063, 0183, 0184, 0185, 0261; its header lists the rest
//!   with reasons.
//! - aggregation and scene_name: aggregate_release_group, augment_language_media_info,
//!   augment_quality_release_name with a grab and scene_name are mapped in
//!   parity_import/import_aggregate.rs; aggregate_language, aggregate_quality,
//!   augment_quality_release_name without a grab (2), subtitle_info,
//!   update_media_info (13) and hdr_format (17) stay unmapped, reasons in that
//!   header.
//! - specifications: Upgrade (10) and FreeSpace RAD-IMPORT-0216 (red: no skip
//!   setting) are mapped in parity_import/import_specs.rs. Unmapped, with
//!   reasons listed there: AlreadyImported (5), GrabbedReleaseQuality (6),
//!   HasAudioTrack (3), MatchesGrab (5), NotMultiPart (10), NotSample
//!   RAD-IMPORT-0241 and FreeSpace RAD-IMPORT-0214 (existing file), 0211 (which
//!   folder is measured).
//! - detect_sample RAD-IMPORT-0159: asserts only that runtime is probed once.
//!
//! Config entries mapped: SON-IMPORT-5002 / RAD-IMPORT-5002 (working folders)
//! and SON-IMPORT-5005 / RAD-IMPORT-5004 (minimum free space). The other
//! import config entries: hardlinks, recycle bin and remote path mappings are
//! mapped in parity_import/priv_import.rs (suites rad_import_settings,
//! son_import_settings); completed download handling, episode title required,
//! extra files, empty folders, file dates, permissions, script import,
//! metadata writers and root folder assignment stay unmapped, reasons in that
//! header.

use std::path::{Path, PathBuf};

use scryer_application::filesystem_walk::FilesystemWalker;
use scryer_application::{
    ImportCheckCode, ImportCheckContext, ImportVerdict, LIBRARY_SCAN_MAX_RECURSIVE_DEPTH,
    check_not_sample, check_not_unpacking, derive_primary_quality_label,
    disk_space_verdict_for_measurement, parse_release_metadata,
};
use scryer_parity::Suite;
use serde_json::{Value, json};

#[path = "parity_import/import_aggregate.rs"]
mod import_aggregate;
#[path = "parity_import/import_orchestration.rs"]
mod import_orchestration;
#[path = "parity_import/import_specs.rs"]
mod import_specs;
#[path = "parity_import/priv_import.rs"]
mod priv_import;

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
/// The path verdict comes first; a case that carries a runtime then goes through the
/// automatic-import runtime rule (fork 17a seam).
fn detect_sample(dir: &Path, given: &Value, expect: &Value) -> Option<(Value, Value)> {
    use scryer_application::parity_seams::{
        automatic_sample_rejection_code, expected_runtime_seconds_for_episode_import,
    };
    let expected = expect.get("result")?.clone();
    let path = rebase(dir, given["path"].as_str()?);
    let by_path = sample_result(&verdict(&path, 0, check_not_sample));
    if by_path != "NotSample" {
        return Some((expected, json!(by_path)));
    }
    let name = path.file_name()?.to_str()?;
    let parsed = scryer_application::parse_release_metadata(name);
    let (facet, expected_runtime, actual) = if let Some(probed) = given.get("file_runtime_seconds") {
        let series = given.get("series")?;
        let mut title = import_aggregate::title("30 Rock", "series");
        title.runtime_minutes = series.get("runtime").and_then(Value::as_i64).map(|m| m as i32);
        let episodes: Vec<scryer_domain::Episode> = given
            .get("episodes")?
            .as_array()?
            .iter()
            .map(|e| scryer_domain::Episode {
                id: "parity-episode".into(),
                title_id: "parity-title".into(),
                collection_id: None,
                episode_type: scryer_domain::EpisodeType::Standard,
                episode_number: Some("1".into()),
                season_number: e.get("season").and_then(Value::as_i64).map(|s| s.to_string()),
                episode_label: None,
                title: None,
                air_date: None,
                duration_seconds: e.get("runtime").and_then(Value::as_i64).map(|m| m * 60),
                has_multi_audio: false,
                has_subtitle: false,
                is_filler: false,
                is_recap: false,
                absolute_number: None,
                contiguous_absolute_number: None,
                overview: None,
                tvdb_id: None,
                tmdb_id: None,
                image_url: None,
                monitored: true,
                created_at: chrono::Utc::now(),
            })
            .collect();
        (
            scryer_domain::MediaFacet::Series,
            expected_runtime_seconds_for_episode_import(&title, &episodes),
            probed.as_i64().map(|v| v as i32),
        )
    } else if let Some(probed) = given.get("probed_runtime_seconds") {
        let minutes = given.get("movie_runtime_minutes").and_then(Value::as_i64).unwrap_or(0);
        (
            scryer_domain::MediaFacet::Movie,
            (minutes > 0).then(|| (minutes * 60) as i32),
            probed.as_i64().map(|v| v as i32),
        )
    } else {
        return Some((expected, json!(by_path)));
    };
    let actual = match automatic_sample_rejection_code(&facet, &parsed, 0, expected_runtime, actual) {
        None => "NotSample",
        Some("sample_runtime_indeterminate") => "Indeterminate",
        Some(_) => "Sample",
    };
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
