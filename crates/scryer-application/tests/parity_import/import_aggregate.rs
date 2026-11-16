//! Parity net, import area, aggregation and augmenters (stream import_aggregate).
//!
//! Suites:
//! - `scryer-application::parity_import::son_import_aggregate`: Sonarr
//!   AggregateReleaseGroup (SON-IMPORT-0070..0074), AugmentQualityFromReleaseName
//!   with a grab (0099..0101), GetSceneName (0117..0129).
//! - `scryer-application::parity_import::rad_import_aggregate`: Radarr
//!   aggregate_release_group (RAD-IMPORT-0117..0121), augment_language_media_info
//!   (0123..0127), augment_quality_release_name with a grab (0151..0153),
//!   scene_name (0166..0175).
//!
//! Selection happens before `wants()` (by corpus file, `input.op` and, for the
//! release-name quality cases, a grab title being present), so
//! PARITY_ADOPT=1 does not adopt other streams' IDs.
//!
//! Scryer code under test:
//! - `build_augmented_episode_import_metadata_for_title`
//!   (import/workflow/series.rs:2936) and
//!   `build_augmented_movie_import_metadata_for_title` (movie.rs:38), through
//!   the parity seam. They parse the release evidence chosen by
//!   `ReleaseEvidence::release_title` (paths.rs:231): grabbed `source_title`,
//!   else the client's release name, else the file stem. The folder name is
//!   never read. `raw_title` is the media row's scene name, `release_group`
//!   and `quality`/`source` come from the same parse.
//! - `normalize_detected_audio_languages` (media/language.rs:278), the step
//!   `build_media_file_analysis` uses (post_download_gate.rs:721).
//!
//! Translation table (reference -> Scryer):
//!
//! | Reference | Scryer |
//! |---|---|
//! | path (rebased under a temp dir) | source video path; the parent folder name is replaced by `folder_title` / `folder_info.release_title` / `folder_release_title` when given, the file name by `<file_title>.<ext>` for release-group cases |
//! | `download_client_title`, `download_client_info.release_title`, `download_client_release_title` | `ReleaseEvidence::DownloaderObservation { release_name }`; absent or null = `None` |
//! | `latest_grab.source_title`, `latest_grab_title` | `ReleaseEvidence::ScryerSubmission { source_title: Some(..) }` (the grab history is `download_submissions.source_title`) |
//! | AugmentQualityFromReleaseName source video | `Path::new("")`: the case has no file; the grabbed `source_title` wins over the stem anyway (paths.rs:231). An empty `raw_title` (no parse) is compared as `result: null`, so a lost parse turns red |
//! | `source` (Sonarr) Web / WebRip / Television / Bluray / DVD / Unknown | `ReleaseSource` WebDl / WebRip / Hdtv / BluRay / Dvd / none; other sources keep `as_str()` |
//! | `source` (Radarr) WEBDL / WEBRIP / TV / BLURAY / DVD / UNKNOWN | the same sources; CAM, TELESYNC etc. keep `as_str()` |
//! | `resolution` | leading digits of the parsed quality label (`4K` = 2160, none = 0) |
//! | GetSceneName `scene_name` | `raw_title` (null when empty) |
//! | Radarr language names English / German | ISO 639-2/T `eng` / `deu`, sorted; empty result = `result: null` |
//!
//! Scene-name greens (SON-IMPORT-0117, 0120, RAD-IMPORT-0166, 0168): the value
//! compared is `raw_title`, i.e. the evidence string production chose. The only
//! Scryer decision there is which evidence wins (`ReleaseEvidence::release_title`);
//! the case inputs differ per source (client title, folder, stem), so a wrong
//! choice turns them red. They check that choice, not a parse.
//!
//! Ignored keys:
//! - `source_confidence`, `resolution_confidence`: Scryer has no confidence levels.
//! - GetSceneName `quality`, `release_group`: fixture setup the calculator does not read.
//! - `full_season` flags: Scryer reads season-pack status from the title text.
//! - Radarr scene_name `other_video_files`: the movie import takes the largest
//!   video; the movie build function has no such input.
//! - AggregateReleaseGroup: the path's own file name (replaced by `file_title`).
//! - Sonarr `NBuilder` client-item title / Radarr `download_client_item`: the
//!   title is not in the case and the reference augmenter ignores it once a
//!   grab exists.
//!
//! Unmapped (tests: []), with reasons:
//! - AggregateEpisodes (SON-IMPORT-0053..0058): asserts which parsed-info object
//!   was passed to a mock (object identity). In 0055/0056 the file and folder
//!   infos are value-identical (both `Series.Title.S01E01`) and 0058's special
//!   info is a JsonClone of the file info, so no returned value shows the
//!   source. In 0053/0054/0057 the folder differs, but Scryer never reads the
//!   folder (paths.rs:57-58: destination folders are not evidence), so its
//!   answer is always the file-stem parse and a "file" label would only echo
//!   the input. The special-episode lookup goes through the episode repository
//!   (AppUseCase).
//! - AugmentQualityFromReleaseName without a grab (SON-IMPORT-0097/0098,
//!   RAD-IMPORT-0149/0150, expect null): Scryer has no separate release-name
//!   augmenter that can return nothing. Without a grab it parses the client
//!   release name, else the file stem (`ReleaseEvidence::release_title`,
//!   paths.rs:231). The cases carry neither (0098's NBuilder client title is not
//!   in the case), so the only reachable input would be a harness-made empty
//!   path, and the green would only prove `release_title(None, "")` is None.
//! - AugmentLanguageFromMediaInfo with no audio languages (RAD-IMPORT-0122, null):
//!   `normalize_detected_audio_languages` of an empty list is empty by
//!   construction, so the comparison proves nothing.
//! - AggregateQuality (SON-IMPORT-0059..0069, RAD-IMPORT-0108..0116): inputs are
//!   confidence-ranked stand-in augmenter outputs; Scryer has no augmenter
//!   results, confidence levels, extension quality or detection-source fields.
//!   Reaching `rescore_parsed_from_analysis` would need the harness to re-implement
//!   the selection under test.
//! - AggregateLanguage (RAD-IMPORT-0101..0107): Scryer keeps no aggregated
//!   file-language list; media-info audio languages and release-name languages
//!   stay apart. 0102-0107 feed confidence-ranked stand-in augmenter outputs.
//!   0101 (no augmenter results -> the movie's original language): Scryer's
//!   original-language default (`release_audio_language_hints_for_title`,
//!   media/audio_requirements.rs:200, `infer_unlabeled_audio`) serves release
//!   scoring and is off at import (post_download_gate.rs:1216 passes false).
//!   Mapping 0101 needs a new seam on that pub(crate) fn plus
//!   `title_audio_language_context`; it would be red ([] vs English).
//! - AggregateSubtitleInfo (RAD-IMPORT-0262..0271): Scryer's sidecar parse yields
//!   only language, forced and hearing_impaired; no title, copy or tag list, so
//!   any comparison would be vacuous. Sidecar language parsing belongs to the
//!   parse_sublang stream.
//! - UpdateMediaInfoService (RAD-IMPORT-0064..0076): probe and update counts on
//!   mocks; needs AppUseCase and an analyzer, no pure re-probe predicate.
//! - VideoFileInfoReader hdr_format (RAD-IMPORT-0080..0096): Scryer derives HDR
//!   in scryer-mediainfo's private build_analysis over crate-private RawTrack
//!   types; the crate is optional and needs synthesized DV config bytes, more
//!   than a visibility seam.

use std::path::{Path, PathBuf};

use scryer_application::parity_seams::{
    ReleaseEvidence, build_augmented_episode_import_metadata_for_title,
    build_augmented_movie_import_metadata_for_title,
};
use scryer_application::{DownloadSubmissionPurpose, SubmissionScope, normalize_detected_audio_languages};
use scryer_parity::Suite;
use scryer_release_parser::{ParsedReleaseMetadata, ReleaseSource};
use serde_json::{Value, json};

use super::rebase;

pub(super) fn title(name: &str, facet: &str) -> scryer_domain::Title {
    serde_json::from_value(json!({
        "id": "parity-title",
        "library_id": "parity-library",
        "name": name,
        "facet": facet,
        "monitored": true,
        "tags": [],
        "external_ids": [],
        "root_folder_id": "parity-root",
        "created_at": "2025-01-01T00:00:00Z",
        "catalog_sort_key": name.to_lowercase(),
        "canonical_tags": [],
        "aliases": [],
        "tagged_aliases": []
    }))
    .unwrap()
}

fn observation(release_name: Option<&Value>) -> ReleaseEvidence {
    ReleaseEvidence::DownloaderObservation {
        release_name: release_name.and_then(Value::as_str).map(str::to_string),
    }
}

fn submission(facet: &str, source_title: &str) -> ReleaseEvidence {
    ReleaseEvidence::ScryerSubmission {
        title_id: "parity-title".into(),
        facet: facet.into(),
        source_title: Some(source_title.into()),
        observed_release_name: None,
        release_size_bytes: None,
        release_listing_json: None,
        purpose: DownloadSubmissionPurpose::Standard,
        scope: SubmissionScope::Title,
    }
}

/// Rebased path with the parent folder and/or file name replaced.
fn place(dir: &Path, path: &str, folder: Option<&str>, file_title: Option<&str>) -> PathBuf {
    let mut p = rebase(dir, path);
    if let Some(file_title) = file_title {
        let ext = p.extension().map(|e| e.to_string_lossy().into_owned());
        p.set_file_name(match ext {
            Some(e) => format!("{file_title}.{e}"),
            None => file_title.to_string(),
        });
    }
    if let Some(folder) = folder {
        let name = p.file_name().unwrap().to_owned();
        p.pop();
        p.set_file_name(folder);
        p.push(name);
    }
    p
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn scene_name(p: &ParsedReleaseMetadata) -> Value {
    if p.raw_title.is_empty() { Value::Null } else { json!(p.raw_title) }
}

fn resolution(p: &ParsedReleaseMetadata) -> u32 {
    match p.quality.as_deref() {
        None => 0,
        Some("4K") => 2160,
        Some(q) => q.chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0),
    }
}

/// `sonarr`: Sonarr spellings, else Radarr's.
fn source_name(p: &ParsedReleaseMetadata, sonarr: bool) -> String {
    let pick = |s: &'static str, r: &'static str| (if sonarr { s } else { r }).to_string();
    match &p.source {
        Some(ReleaseSource::WebDl) => pick("Web", "WEBDL"),
        Some(ReleaseSource::WebRip) => pick("WebRip", "WEBRIP"),
        Some(ReleaseSource::Hdtv) => pick("Television", "TV"),
        Some(ReleaseSource::BluRay) => pick("Bluray", "BLURAY"),
        Some(ReleaseSource::Dvd) => pick("DVD", "DVD"),
        None => pick("Unknown", "UNKNOWN"),
        Some(other) => other.as_str().to_string(),
    }
}

/// Expected is the case's expect minus the confidence keys; actual is
/// `result: null` when nothing was parsed.
fn quality_pair(expect: &Value, p: &ParsedReleaseMetadata, sonarr: bool) -> (Value, Value) {
    let mut e = expect.clone();
    if let Some(m) = e.as_object_mut() {
        m.remove("source_confidence");
        m.remove("resolution_confidence");
    }
    let a = if p.raw_title.is_empty() {
        json!({"result": null})
    } else {
        json!({"source": source_name(p, sonarr), "resolution": resolution(p)})
    };
    (e, a)
}

fn cases(file: &str, ops: &[&str]) -> Vec<Value> {
    let all: Vec<Value> = scryer_parity::corpus(file);
    all.into_iter().filter(|c| ops.contains(&c["input"]["op"].as_str().unwrap())).collect()
}

fn release_group_pair(dir: &Path, g: &Value, expect: &Value, facet: &str) -> (Value, Value) {
    let path = place(dir, str_of(g, "path").unwrap(), str_of(g, "folder_title"), str_of(g, "file_title"));
    let evidence = observation(g.get("download_client_title"));
    let parsed = if facet == "series" {
        build_augmented_episode_import_metadata_for_title(&path, &evidence, &title("Series Title", facet), false)
    } else {
        build_augmented_movie_import_metadata_for_title(&path, &evidence, &title("Movie Title", facet))
    };
    (json!({"release_group": expect["release_group"]}), json!({"release_group": parsed.release_group}))
}

fn scene_pair(dir: &Path, path: &str, folder: Option<&str>, client: Option<&Value>, other: bool, facet: &str) -> (Value, Value) {
    let path = place(dir, path, folder, None);
    let evidence = observation(client);
    let parsed = if facet == "series" {
        build_augmented_episode_import_metadata_for_title(&path, &evidence, &title("Series Title", facet), other)
    } else {
        build_augmented_movie_import_metadata_for_title(&path, &evidence, &title("Movie Title", facet))
    };
    (Value::Null, scene_name(&parsed))
}

#[test]
fn son_import_aggregate() {
    let Some(mut suite) = Suite::open("scryer-application::parity_import::son_import_aggregate") else { return };
    let tmp = tempfile::tempdir().expect("tempdir");
    let d = "corpus/reference/sonarr/import/MediaFiles.EpisodeImport.";
    for c in cases(&format!("{d}Aggregation.Aggregators.AggregateReleaseGroupFixture.json"), &["AggregateReleaseGroup"]) {
        let id = c["id"].as_str().unwrap();
        if suite.wants(id) {
            let (e, a) = release_group_pair(&tmp.path().join(id), &c["input"]["given"], &c["expect"], "series");
            suite.check(id, e, a);
        }
    }
    for c in cases(
        &format!("{d}Aggregation.Aggregators.Augmenters.Quality.AugmentQualityFromReleaseNameFixture.json"),
        &["AugmentQualityFromReleaseName"],
    ) {
        let id = c["id"].as_str().unwrap();
        // No grab: unmapped (header), filtered before wants().
        let Some(grab) = c["input"]["given"].get("latest_grab").and_then(|l| str_of(l, "source_title")) else {
            continue;
        };
        if suite.wants(id) {
            let parsed = build_augmented_episode_import_metadata_for_title(
                Path::new(""),
                &submission("series", grab),
                &title("Series Title", "series"),
                false,
            );
            let (e, a) = quality_pair(&c["expect"], &parsed, true);
            suite.check(id, e, a);
        }
    }
    for c in cases(&format!("{d}GetSceneNameFixture.json"), &["SceneNameCalculator.GetSceneName"]) {
        let id = c["id"].as_str().unwrap();
        if suite.wants(id) {
            let g = &c["input"]["given"];
            let folder = g.get("folder_info").and_then(|f| str_of(f, "release_title"));
            let client = g.get("download_client_info").and_then(|f| f.get("release_title"));
            let other = g.get("other_video_files").and_then(Value::as_bool).unwrap_or(false);
            let (_, a) = scene_pair(&tmp.path().join(id), str_of(g, "path").unwrap(), folder, client, other, "series");
            suite.check(id, c["expect"]["scene_name"].clone(), a);
        }
    }
    suite.finish();
}

fn language_code(name: &str) -> &'static str {
    match name {
        "English" => "eng",
        "German" => "deu",
        other => panic!("unmapped language name {other}"),
    }
}

#[test]
fn rad_import_aggregate() {
    let Some(mut suite) = Suite::open("scryer-application::parity_import::rad_import_aggregate") else { return };
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = "corpus/reference/radarr/import/";
    for c in cases(
        &format!("{dir}aggregation.json"),
        &["aggregate_release_group", "augment_language_media_info", "augment_quality_release_name"],
    ) {
        let id = c["id"].as_str().unwrap();
        let g = &c["input"]["given"];
        // augment_quality_release_name without a grab: unmapped (header).
        if c["input"]["op"] == "augment_quality_release_name" && str_of(g, "latest_grab_title").is_none() {
            continue;
        }
        // audio_languages null (RAD-IMPORT-0122): empty in, empty out, vacuous; unmapped (header).
        if c["input"]["op"] == "augment_language_media_info" && g["audio_languages"].is_null() {
            continue;
        }
        if !suite.wants(id) {
            continue;
        }
        match c["input"]["op"].as_str().unwrap() {
            "aggregate_release_group" => {
                let (e, a) = release_group_pair(&tmp.path().join(id), g, &c["expect"], "movie");
                suite.check(id, e, a);
            }
            "augment_quality_release_name" => {
                let parsed = build_augmented_movie_import_metadata_for_title(
                    Path::new(""),
                    &submission("movie", str_of(g, "latest_grab_title").unwrap()),
                    &title("Movie Title", "movie"),
                );
                let (e, a) = quality_pair(&c["expect"], &parsed, false);
                suite.check(id, e, a);
            }
            _ => {
                let input: Vec<String> = g["audio_languages"]
                    .as_array()
                    .map(|a| a.iter().map(|v| v.as_str().unwrap().to_string()).collect())
                    .unwrap_or_default();
                let mut got = normalize_detected_audio_languages(input.iter().map(String::as_str));
                got.sort();
                let actual = if got.is_empty() { json!({"result": null}) } else { json!({"language_set": got}) };
                let expected = match c["expect"].get("language_set").and_then(Value::as_array) {
                    Some(names) => {
                        let mut codes: Vec<&str> = names.iter().map(|n| language_code(n.as_str().unwrap())).collect();
                        codes.sort();
                        json!({"language_set": codes})
                    }
                    None => json!({"result": null}),
                };
                suite.check(id, expected, actual);
            }
        }
    }
    for c in cases(&format!("{dir}scene_name.json"), &["scene_name"]) {
        let id = c["id"].as_str().unwrap();
        if suite.wants(id) {
            let g = &c["input"]["given"];
            let (_, a) = scene_pair(
                &tmp.path().join(id),
                str_of(g, "path").unwrap(),
                str_of(g, "folder_release_title"),
                g.get("download_client_release_title"),
                false,
                "movie",
            );
            suite.check(id, c["expect"]["scene_name"].clone(), a);
        }
    }
    suite.finish();
}
