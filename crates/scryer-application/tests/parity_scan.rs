//! Parity net, scan area (notes/11-parity-net-design.md, parity/README.md).
//!
//! Suite `scryer-application::parity_scan::son_scan` runs
//! `parity/corpus/reference/sonarr/scan/*.json` against Scryer's per-title
//! inventory walk: the walker the library scanner builds for
//! `LibraryScanner::scan_directory` and `scan_directory_for_progress_with_metrics`
//! (crates/scryer-infrastructure-library/src/media/libraries/scanner.rs:299-302),
//!
//! ```text
//! FilesystemWalker::new()
//!     .skip_episodic_scan_junk_and_trailers()
//!     .supported_video_files_only()
//!     .max_depth(LIBRARY_SCAN_MAX_RECURSIVE_DEPTH)
//! ```
//!
//! walked from the title folder (library/scan/pipeline.rs:2074-2092 and
//! library/scan/candidates.rs:797 scan each title folder this way, movies and
//! series alike). The scanner's second
//! extension filter (scanner.rs:37, `standalone_video_extensions`) is the
//! list the walker already applies (`scryer_domain::is_standalone_video_file`),
//! so it is not repeated here. If that builder chain changes in the scanner,
//! change `title_scan` to match.
//!
//! Reference paths are rebuilt under a temp dir: `/C/Test/TV/x` and
//! `C:\Test\x` become `<tmp>/<case id>/C/Test/TV/x`; every file holds one byte.
//! On a case-insensitive filesystem (macOS) `EXTRAS/` and `Extras/` are one
//! directory; every such pair in the corpus is a skipped folder, so the counts
//! do not change.
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference assertion | Scryer output compared |
//! |---|---|
//! | `GetImportDecisions(l => l.Count == N, series, false)` called once | the walk returns N files (the files handed to analysis) |
//! | `GetVideoFiles(path)` returns `count` files | the walk of `path` returns that many files |
//! | `FilterPaths(path, files)`: no file under the special subfolder, more than N kept | the walk keeps no file under `<path>/<special>` and keeps more than N |
//! | `SeriesScannedEvent` (count, possible_extra_files) | none: the walk lists video files only and no scan event carries non-video candidates; compared as `null`, so the case fails |
//!
//! Ignored keys: `GetFiles(any, any)` in SON-SCAN-0010 (how many listings the
//! reference made, video and non-video; a mock-interaction detail, Scryer's
//! walk is one listing; the file count in the same case is compared).
//!
//! Unmapped (tests: []), with reasons:
//! - DiskScanService.Scan without `files_on_disk`: SON-SCAN-0001, 0002, 0005
//!   mapped in crates/scryer-infrastructure-library/tests/parity_scan/scan_library.rs
//!   (son_scan_root_guard) through the root-state and title-folder guards.
//!   SON-SCAN-0004 stays unmapped: its only assertion (no folder created) is
//!   observable only through the AppUseCase title scan (reason there).
//! - DiskScanService.GetVideoFiles call shape (SON-SCAN-0138..0140): mapped in
//!   the same file (son_scan_video_files); recursion is observable through
//!   FileSystemLibraryScanner::scan_directory vs scan_directory_children.
//! - MediaFileTableCleanupService.Clean: SON-SCAN-0022, 0023, 0024, 0026 mapped
//!   in the same file (son_scan_clean). SON-SCAN-0025 stays unmapped: a dangling
//!   episode link cannot exist in Scryer (file_episode_map ON DELETE CASCADE).
//! - MediaInfoFormatter FormatAudioCodec / FormatVideoCodec /
//!   FormatVideoDynamicRange / FormatVideoDynamicRangeType (78): mapped in
//!   parity_scan/scan_mediainfo.rs (rename tokens via title_rename_tokens).
//!   Correction of the earlier claim: scryer-mediainfo's normalize_codec_name
//!   maps container codec ids to ffprobe names; detect_hdr_format is a
//!   #[cfg(test)] wrapper returning the probe's video_hdr_format string;
//!   neither is a display label.
//! - UpdateMediaInfoService (11): SON-SCAN-0027..0029 mapped in
//!   crates/scryer-infrastructure-library/tests/parity_scan/scan_library.rs
//!   (son_scan_media_info_queue). SON-SCAN-0030, 0031, 0034..0039 stay
//!   unmapped: AppUseCase plus media analyzer and repositories.
//!   VideoFileInfoReader (20): crates/scryer-mediainfo/tests/parity_scan/scan_reader.rs
//!   (0040-0053 mapped there; 0054-0059 unmapped, reason there).
//!
//! Submodule suites (parity_scan/scan_mediainfo.rs): son_scan_format_audio_codec,
//! son_scan_format_video_codec, son_scan_format_dynamic_range,
//! son_scan_format_dynamic_range_type.
//!
//! Config suites (son_scan_config, rad_scan_config, baz_scan_config) live in
//! parity_scan/priv_scan.rs: SON-SCAN-5002, 5006, RAD-SCAN-5002, 5013 and
//! BAZ-SCAN-5001, 5003, 5005, 5007 are mapped there. The config entries still
//! unmapped (SON-SCAN-5001, 5003, 5004, 5005, 5007, RAD-SCAN-5001, 5003, 5004,
//! 5005, BAZ-SCAN-5002, 5004, 5006) and their reasons are listed in that
//! file's header.

use std::path::{Path, PathBuf};

use scryer_application::LIBRARY_SCAN_MAX_RECURSIVE_DEPTH;
use scryer_application::filesystem_walk::FilesystemWalker;
use scryer_parity::Suite;
use serde_json::{Value, json};

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
        if let Some((expected, actual)) = handler(&dir, &case["input"], &case["expect"]) {
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

/// Scryer's per-title inventory walk (see the header).
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

fn str_list(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect()
}

/// N from a `GetImportDecisions(l => l.Count == N, ...)` key called once.
fn import_decisions_count(calls: &Value) -> Option<u64> {
    calls.as_object()?.iter().find_map(|(key, times)| {
        let n = key
            .strip_prefix("GetImportDecisions(l => l.Count == ")?
            .split(',')
            .next()?
            .trim()
            .parse()
            .ok()?;
        (times.as_u64() == Some(1)).then_some(n)
    })
}

fn son_scan_case(dir: &Path, input: &Value, expect: &Value) -> Option<(Value, Value)> {
    let given = &input["given"];
    match input["op"].as_str()? {
        "DiskScanService.Scan" => {
            let files = given.get("files_on_disk")?;
            let count = import_decisions_count(expect.get("calls")?)?;
            let root = rebase(dir, given["series_path"].as_str()?);
            std::fs::create_dir_all(&root).expect("mkdir");
            for file in str_list(files) {
                touch(&rebase(dir, file));
            }
            let found = title_scan(&root).len() as u64;
            let mut expected = json!({ "import_candidates": count });
            let mut actual = json!({ "import_candidates": found });
            if let Some(event) = expect
                .get("events")
                .and_then(|e| e.get("SeriesScannedEvent"))
            {
                expected["scanned_event"] = event.clone();
                actual["scanned_event"] = Value::Null;
            }
            Some((expected, actual))
        }
        "DiskScanService.GetVideoFiles" => {
            let count = expect.get("count")?.as_u64()?;
            let root = rebase(dir, given["path"].as_str()?);
            std::fs::create_dir_all(&root).expect("mkdir");
            for name in str_list(given.get("file_names")?) {
                touch(&root.join(name));
            }
            Some((json!(count), json!(title_scan(&root).len() as u64)))
        }
        "DiskScanService.FilterPaths" => {
            let threshold = expect.get("kept_count_greater_than")?.as_u64()?;
            let root = rebase(dir, given["path"].as_str()?);
            let special = root.join(given["special_subfolder"].as_str()?);
            for name in str_list(&given["file_names"]) {
                touch(&root.join(name));
                touch(&special.join(name));
            }
            let kept = title_scan(&root);
            let excludes = !kept.iter().any(|p| p.starts_with(&special));
            Some((
                json!({
                    "excludes_special_subfolder_files": expect["excludes_special_subfolder_files"],
                    "kept_more_than_threshold": true,
                }),
                json!({
                    "excludes_special_subfolder_files": excludes,
                    "kept_more_than_threshold": kept.len() as u64 > threshold,
                }),
            ))
        }
        _ => None,
    }
}

#[test]
fn son_scan() {
    let Some(mut suite) = Suite::open("scryer-application::parity_scan::son_scan") else { return };
    for file in [
        "corpus/reference/sonarr/scan/MediaFiles.DiskScanServiceTests.ScanFixture.json",
        "corpus/reference/sonarr/scan/ProviderTests.DiskScanProviderTests.GetVideoFilesFixture.json",
    ] {
        run(&mut suite, file, son_scan_case);
    }
    suite.finish();
}

#[path = "parity_scan/scan_mediainfo.rs"]
mod scan_mediainfo;

#[path = "parity_scan/priv_scan.rs"]
mod priv_scan;
