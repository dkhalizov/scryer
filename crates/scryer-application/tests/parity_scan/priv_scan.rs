//! Parity net, scan area, config entries of the private catalogue
//! (`scan.{sonarr,radarr,bazarr}-config.yaml`; corpus under
//! `corpus/captured/<app>/config/`, gitignored, ledger D45). Every compared
//! value is read from the captured corpus at run time; this file holds none.
//!
//! Suites (`Suite::open`, so they skip when the captured corpus is absent):
//! - `son_scan_config`: SON-SCAN-5002, SON-SCAN-5006
//! - `rad_scan_config`: RAD-SCAN-5002, RAD-SCAN-5013
//! - `baz_scan_config`: BAZ-SCAN-5001, 5003, 5005, 5007
//!
//! Selection is a fixed ID list per suite, checked before `wants()`.
//!
//! Scryer functions under test: `title_media_file_matches_snapshot` (decides
//! whether a scanned file is analysed or its stored analysis reused),
//! `should_monitor_season` with `extract_monitor_type`, `JobKey::interval_seconds`
//! and `JobKey::schedule_kind`, `parse_discovered_external_subtitle`, and the
//! root harness walker `super::title_scan`.
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | mediamanagement.json `enableMediaInfo` (read media info of tracked files that have none) | `!title_media_file_matches_snapshot(row, snapshot)` for a scanned, same-size row with no stored analysis |
//! | series-summary.json `monitorNewItems` (object value -> count; one probe per sorted value) | `should_monitor_season(extract_monitor_type(&[]), 2, false, None)`; expected `value == "all"`. Season 2 stands for any regular season; specials are not probed |
//! | system-task.json element with `taskName == "RefreshMovie"`, key `interval` (minutes) | `JobKey::BackgroundLibraryRefreshMovies.interval_seconds() / 60` (cadence of the pass only; per-title due gating is not compared) |
//! | languages.json entries with `enabled` false (probe codes: fixed generic ISO 639-1 list minus enabled ones) | `parse_discovered_external_subtitle("a", "a.<code>.srt")` gives a language |
//! | settings.json `general.single_language` (uncoded sidecar gets a language) | the same parse of `a.srt` gives a language |
//! | settings.json `<sonarr\|radarr>.full_update` Manually / Daily / Weekly | `JobKey::LibraryScan*.schedule_kind().as_str()`: manual / daily_at_time / weekly_at_time (sonarr: series and anime jobs; radarr: movies job) |
//! | settings.json `<section>.use_ffprobe_cache` | `title_media_file_matches_snapshot` for an analysed, unchanged row (analysis reused) |
//! | settings.json `general.enable_strm_support` and Bazarr's size floor (constant 20480, Bazarr@v1.5.6 bazarr/constants.py:9, sonarr/sync/episodes.py:126-128, radarr/sync/movies.py:139-141: keep iff size > floor, or strm support and a .strm path) | whether `super::title_scan` of a temp dir keeps files of exactly the floor, floor+1 and a tiny .strm |
//!
//! Ignored keys: `monitorNewItems` counts and `monitored` (would only echo the
//! input; not cross-tabulated); `full_update_day` and `full_update_hour` (a
//! Manual job has no time); embedded tracks and the content-language guess for
//! uncoded sidecars (it exists only behind feature `runtime-media-analysis`,
//! so a `-p scryer-application` build runs the stub).
//!
//! Known reds, phrased conditionally: RAD-SCAN-5013 is red unless the captured
//! interval equals Scryer's fixed pass cadence; BAZ-SCAN-5003 and 5005 are red
//! unless the captured full_update is Manually (Scryer's full re-read jobs are
//! Manual); BAZ-SCAN-5007 is red because Scryer has no size floor (and keeps
//! .strm regardless of the flag); SON/RAD-SCAN-5002 go red only if the
//! captured setting is off (Scryer's analysis decision is fixed); SON-SCAN-5006
//! goes red only if the captured values include one other than "all". That
//! probe models a monitored title with no monitor-type tag (what the migration
//! leaves); Scryer's advanced monitor type and unmonitored titles create new
//! seasons unmonitored, which it does not cover.
//! The matching sub-checks inside red cases (`embedded_cache_reused` in
//! BAZ-SCAN-5003/5005, the floor+1 row in BAZ-SCAN-5007) are not guarded by the
//! ratchet while the case is red; an M4 mutation of them is not caught. The
//! true branch of `title_media_file_matches_snapshot` is therefore unguarded
//! in this file (SON/RAD-SCAN-5002 probe only the false branch).
//! Weak greens: BAZ-SCAN-5001 (the parse has no enabled-set input, so a later
//! filter in reconcile would not be caught; its uncoded-sidecar half is a
//! constant, because the filename parse of an uncoded sidecar has an empty
//! suffix and never yields a language, so it goes red only if
//! `single_language` is on; in production the uncoded language comes from the
//! content guess behind `runtime-media-analysis`, not measured here) and
//! SON/RAD-SCAN-5002 (the analysis decision is unconditional).
//!
//! Unmapped (tests: []), with reasons:
//! - SON-SCAN-5003, RAD-SCAN-5001 (rescanAfterRefresh): Scryer probes the title
//!   folder after every refresh pass unconditionally inside async AppUseCase
//!   code (library/scan/refresh.rs, title_scan.rs); no decision function, needs
//!   repositories and a metadata gateway.
//! - SON-SCAN-5004, RAD-SCAN-5004, RAD-SCAN-5005 (create/delete empty folders):
//!   no Scryer setting; fixed behaviour equals off: neither title add nor the
//!   title scan creates a title folder (import creates it when a file is
//!   placed), and the scan removes none (the only removal is the one-time
//!   leftover-rename cleanup, library/leftover_title_folders.rs). Observable
//!   only through AppUseCase (as SON-SCAN-0004).
//! - SON-SCAN-5005, RAD-SCAN-5003 (autoUnmonitorPreviouslyDownloaded*): only
//!   observable through AppUseCase scan/delete paths with repositories. D29 gap
//!   for the patch session: a file gone missing keeps its episode or movie
//!   monitored in Scryer.
//! - SON-SCAN-5001: library state (root folder occupancy), not behaviour.
//! - SON-SCAN-5007: Scryer has no auto-tagging concept.
//! - BAZ-SCAN-5002, 5004: Bazarr's sync with Sonarr/Radarr has no counterpart
//!   in a single application.
//! - BAZ-SCAN-5006: path mappings between two applications do not exist.

use std::path::Path;

use scryer_application::parity_seams::{
    FileSourceSnapshot, extract_monitor_type, parse_discovered_external_subtitle,
    should_monitor_season, title_media_file_matches_snapshot,
};
use scryer_application::{JobKey, TitleMediaFile};
use scryer_parity::Suite;
use serde_json::{Value, json};

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

fn config(app: &str, file: &str) -> Value {
    scryer_parity::corpus(&format!("corpus/captured/{app}/config/{file}.json"))
}

fn snapshot(size: i64) -> FileSourceSnapshot {
    FileSourceSnapshot { size_bytes: size, signature: None }
}

/// Scanned, same size, no stored analysis: is it (re)read?
fn read_without_media_info() -> bool {
    let row = TitleMediaFile { scan_status: "scanned".into(), size_bytes: 4096, ..Default::default() };
    !title_media_file_matches_snapshot(&row, &snapshot(4096))
}

/// Scanned, same size, stored analysis: is the analysis reused?
fn analysis_reused() -> bool {
    let row = TitleMediaFile {
        scan_status: "scanned".into(),
        size_bytes: 4096,
        duration_seconds: Some(1),
        ..Default::default()
    };
    title_media_file_matches_snapshot(&row, &snapshot(4096))
}

fn media_info_check(app: &str) -> (Value, Value) {
    let c = config(app, "mediamanagement");
    let on = c["enableMediaInfo"].as_bool().expect("enableMediaInfo");
    (
        json!({ "tracked_file_without_media_info_is_read": on }),
        json!({ "tracked_file_without_media_info_is_read": read_without_media_info() }),
    )
}

fn new_season_check(app: &str) -> (Value, Value) {
    let c = config(app, "series-summary");
    let mut values: Vec<&String> = c["monitorNewItems"].as_object().expect("monitorNewItems").keys().collect();
    values.sort();
    let monitored = should_monitor_season(&extract_monitor_type(&[]), 2, false, None);
    let expected: Vec<Value> =
        values.iter().map(|v| json!({ "value": v, "new_season_monitored": *v == "all" })).collect();
    let actual: Vec<Value> =
        values.iter().map(|v| json!({ "value": v, "new_season_monitored": monitored })).collect();
    (json!(expected), json!(actual))
}

fn refresh_task_check(app: &str) -> (Value, Value) {
    let tasks = config(app, "system-task");
    let task = tasks
        .as_array()
        .expect("tasks")
        .iter()
        .find(|t| t["taskName"] == "RefreshMovie")
        .expect("RefreshMovie task");
    let minutes = JobKey::BackgroundLibraryRefreshMovies.interval_seconds().map(|s| s / 60);
    (task["interval"].clone(), json!(minutes))
}

#[test]
fn son_scan_config() {
    let Some(mut suite) = Suite::open("scryer-application::parity_scan::son_scan_config") else { return };
    let checks: [(&str, ConfigCheck); 2] =
        [("SON-SCAN-5002", media_info_check), ("SON-SCAN-5006", new_season_check)];
    run_config(&mut suite, "sonarr", &checks);
    suite.finish();
}

fn languages_check(_app: &str) -> (Value, Value) {
    let langs = config("bazarr", "languages");
    let enabled: Vec<&str> = langs
        .as_array()
        .expect("languages")
        .iter()
        .filter(|l| l["enabled"].as_bool().unwrap_or(false) || l["enabled"] == 1)
        .filter_map(|l| l["code2"].as_str())
        .collect();
    let probes: Vec<&str> =
        ["de", "es", "it", "ja", "nl", "pt"].into_iter().filter(|c| !enabled.contains(c)).collect();
    assert!(!probes.is_empty(), "no disabled probe language");
    let indexed = |code: &str| {
        parse_discovered_external_subtitle("a", Path::new(&format!("a.{code}.srt")))
            .is_some_and(|c| c.language.is_some())
    };
    let single = config("bazarr", "settings")["general"]["single_language"].as_bool().expect("single_language");
    let uncoded = parse_discovered_external_subtitle("a", Path::new("a.srt")).and_then(|c| c.language).is_some();
    (
        json!({
            "disabled_language_indexed": probes.iter().map(|c| json!({"code": c, "indexed": true})).collect::<Vec<_>>(),
            "uncoded_named_by_enabled_set": single,
        }),
        json!({
            "disabled_language_indexed": probes.iter().map(|c| json!({"code": c, "indexed": indexed(c)})).collect::<Vec<_>>(),
            "uncoded_named_by_enabled_set": uncoded,
        }),
    )
}

fn full_update_check(section: &str, keys: &[JobKey]) -> (Value, Value) {
    let s = config("bazarr", "settings");
    let kind = match s[section]["full_update"].as_str().expect("full_update") {
        "Manually" => "manual",
        "Daily" => "daily_at_time",
        "Weekly" => "weekly_at_time",
        other => panic!("unknown full_update {other}"),
    };
    let cache = s[section]["use_ffprobe_cache"].as_bool().expect("use_ffprobe_cache");
    (
        json!({ "full_reread_schedule": vec![kind; keys.len()], "embedded_cache_reused": cache }),
        json!({
            "full_reread_schedule": keys.iter().map(|k| k.schedule_kind().as_str()).collect::<Vec<_>>(),
            "embedded_cache_reused": analysis_reused(),
        }),
    )
}

fn sonarr_full_update(_: &str) -> (Value, Value) {
    full_update_check("sonarr", &[JobKey::LibraryScanSeries, JobKey::LibraryScanAnime])
}

fn radarr_full_update(_: &str) -> (Value, Value) {
    full_update_check("radarr", &[JobKey::LibraryScanMovies])
}

/// Bazarr@v1.5.6 bazarr/constants.py:9
const MINIMUM_VIDEO_SIZE: usize = 20480;

fn size_floor_check(_app: &str) -> (Value, Value) {
    let strm = config("bazarr", "settings")["general"]["enable_strm_support"].as_bool().expect("strm flag");
    let probes = [("a.mkv", MINIMUM_VIDEO_SIZE), ("b.mkv", MINIMUM_VIDEO_SIZE + 1), ("c.strm", 100)];
    let dir = tempfile::tempdir().expect("tempdir");
    for (name, size) in probes {
        std::fs::write(dir.path().join(name), vec![0u8; size]).expect("write");
    }
    let kept = super::title_scan(dir.path());
    let expected: Vec<Value> = probes
        .iter()
        .map(|(n, size)| json!({ "file": n, "synced": *size > MINIMUM_VIDEO_SIZE || (strm && n.ends_with(".strm")) }))
        .collect();
    let actual: Vec<Value> =
        probes.iter().map(|(n, _)| json!({ "file": n, "synced": kept.contains(&dir.path().join(n)) })).collect();
    (json!(expected), json!(actual))
}

#[test]
fn rad_scan_config() {
    let Some(mut suite) = Suite::open("scryer-application::parity_scan::rad_scan_config") else { return };
    let checks: [(&str, ConfigCheck); 2] =
        [("RAD-SCAN-5002", media_info_check), ("RAD-SCAN-5013", refresh_task_check)];
    run_config(&mut suite, "radarr", &checks);
    suite.finish();
}

#[test]
fn baz_scan_config() {
    let Some(mut suite) = Suite::open("scryer-application::parity_scan::baz_scan_config") else { return };
    let checks: [(&str, ConfigCheck); 4] = [
        ("BAZ-SCAN-5001", languages_check),
        ("BAZ-SCAN-5003", sonarr_full_update),
        ("BAZ-SCAN-5005", radarr_full_update),
        ("BAZ-SCAN-5007", size_floor_check),
    ];
    run_config(&mut suite, "bazarr", &checks);
    suite.finish();
}
