//! Parity net, area `subtitles` (notes/11-parity-net-design.md, parity/README.md).
//!
//! Scryer under test: `subtitles::scoring` (weights, `percent_to_raw_threshold`,
//! `compute_verified_score`), `subtitles::wanted::compute_missing_subtitles`
//! and `subtitles::language::normalize_subtitle_language_code`.
//!
//! Translation table, Bazarr -> Scryer:
//!
//! | Bazarr | Scryer |
//! |---|---|
//! | score weight `series` (episodes) | `ScoreWeights::title` (`compute_score_bits` scores Title or Series with it) |
//! | `title` (movies), `hash`, `year`, `season`, `episode`, `release_group`, `source`, `audio_codec`, `resolution`, `video_codec`, `hearing_impaired` | the `ScoreWeights` field of the same name |
//! | `edition`, `streaming_service` | no Scryer factor: compared as missing (fails) |
//! | item_type `episode` / `movie`, video kind `Episode` / `Movie` | `SubtitleScoreKind::Episode` / `Movie` |
//! | language profile item `{language, hi, forced}` | `SubtitleLanguagePref { code: normalize_subtitle_language_code(language), hearing_impaired: hi, forced }` (hi/forced as bool or "True"/"False") |
//! | a kept subtitle `{language, hi, forced}` | one `SubtitleDownload` with those values |
//! | "profile is satisfied" | `compute_missing_subtitles(prefs, [download], [])` is empty |
//! | score "94.17%" | raw = round(percent x Bazarr max / 100), Bazarr max = hash + hearing_impaired from config/scores.json |
//! | minimum score percent (settings general/minimum_score[_movie]) | `percent_to_raw_threshold(kind, percent)` |
//! | `video_fromname(file name)` | `parse_release_metadata(name without its container extension)`, read through the fields `build_subtitle_query` takes from it (`subtitles/orchestration.rs:2113-2190`; Scryer parses scene and grabbed release names, which carry no extension) |
//! | video `series` (episode) / `title` (movie) | `normalized_title` (the query's first title candidate) |
//! | video `title` of an episode (the episode title), `streaming_service`, `other` | no field in the subtitle query: compared as missing (fails) |
//! | video `year`, `release_group`, `resolution` | `year`, `release_group`, `quality` |
//! | video `season` / `episode` | `episode.season` / first of `episode.episode_numbers` |
//! | video `source` "Web" / "Blu-ray", "Ultra HD Blu-ray" | `ReleaseSource` "WEB-DL" / "BluRay" (Scryer has no UHD source; 2160p is the resolution) |
//! | video `video_codec` "H.264", "H.265"; `audio_codec` "FLAC" | `VideoCodec::as_str`, `release_audio_codec` (`audio`, else the first of `audio_codecs`) of the same name |
//! | the video is an episode (Scryer: the title's facet) | the case asserts `series` |
//!
//! Suites and mapped IDs:
//! - `baz_subtitles`: BAZ-SUBTITLES-0055, 0056 (score.json `compute_score`,
//!   `expect.no_error`: scoring the case's matches does not panic), 0057
//!   (`compute_score_weights` with no custom weights: the default table),
//!   0066, 0067 (video.json `video_fromname`).
//! - `baz_subtitles_config`: BAZ-SUBTITLES-5001 (profile: a plain and an HI
//!   subtitle of each listed language both satisfy an item with hi False,
//!   per Bazarr's rule in BAZ-SUBTITLES-0008), 5006 / 5008 (episode / movie
//!   weights), 5007 / 5009 (max score and minimum-score threshold).
//! - `baz_subtitles_captured`: BAZ-SUBTITLES-6001..6116 (history: `accepted` =
//!   the score clears our minimum and the subtitle satisfies the profile) and
//!   6117 (wanted: the missing languages of a video with no subtitle).
//!   Ignored keys: `provider` (provider choice is not a Scryer decision here),
//!   `action` (always 1 = kept; `accepted` already encodes it),
//!   `expect.language/hi/forced` (the kept subtitle's own properties; used as
//!   the download under test).
//!
//! - `priv_subtitles::baz_subtitles_cfg_wanted` (5010, 5012, 5013),
//!   `::baz_subtitles_cfg_files` (5015, 5018), `::baz_subtitles_cfg_scene_name`
//!   (5017): `parity_subtitles/priv_subtitles.rs`.
//! - `subtitles::baz_subtitles_custom_weights` (0058, 0059) and
//!   `subtitles::baz_subtitles_absent` (0060..0065): mapped red (D29) in
//!   `parity_subtitles/subtitles.rs`.
//!
//! Unmapped (tests: []), with reasons:
//! - BAZ-SUBTITLES-0001..0018: no corpus case (the expectation exists only in
//!   catalogue prose); a corpus case has to be added before a test can read it.
//! - BAZ-SUBTITLES-0023, 0045, 0049, 0050, 5022 (language equivalences): Scryer
//!   has no language-equivalence setting (re-checked:
//!   `normalize_subtitle_language_code` (`subtitles/language.rs`) and
//!   `normalize_detected_subtitle_language_code` (`media/language.rs`, which
//!   falls back to the primary subtag) are fixed normalisers; es-MX, pt-BR and
//!   zh-TW stay distinct codes; there is no user equivalence list and nothing
//!   widens a language set).
//! - BAZ-SUBTITLES-0037, 0038 (scan_video kind from a file name): Scryer takes
//!   the kind from the title's facet and never guesses it from a name.
//! - BAZ-SUBTITLES-0039..0042 (provider pool update/discard), 5004, 5005
//!   (enabled providers, OpenSubtitles options): provider configs and the
//!   pool live behind async `AppUseCase` settings with no pure seam; 5004 and
//!   5005 are listed in `parity_subtitles/priv_subtitles.rs`.
//! - BAZ-SUBTITLES-0058..0065 are mapped red in `parity_subtitles/subtitles.rs`.
//! - BAZ-SUBTITLES-5010, 5012, 5013, 5015, 5017, 5018 are mapped in
//!   `parity_subtitles/priv_subtitles.rs`; the reasons for 5002, 5003, 5011,
//!   5014, 5016, 5019..5021, 5024..5031 are in its header.

use std::collections::{BTreeMap, HashSet};

use scryer_application::parse_release_metadata;
use scryer_application::subtitles::language::normalize_subtitle_language_code;
use scryer_application::subtitles::scoring::{
    ScoreWeights, SubtitleScoreKind, compute_verified_score, percent_to_raw_threshold,
};
use scryer_application::subtitles::wanted::{SubtitleLanguagePref, compute_missing_subtitles};
use scryer_domain::{ExternalSubtitleSourceKind, SubtitleDownload};
use scryer_parity::{Catalogue, Suite, corpus};
use serde_json::{Value, json};

#[path = "parity_subtitles/priv_subtitles.rs"]
mod priv_subtitles;
#[path = "parity_subtitles/subtitles.rs"]
mod subtitles;

fn kind(name: &str) -> SubtitleScoreKind {
    match name {
        "episode" | "Episode" => SubtitleScoreKind::Episode,
        "movie" | "Movie" => SubtitleScoreKind::Movie,
        other => panic!("unknown Bazarr item kind {other}"),
    }
}

/// Scryer's weight for a Bazarr score name (table above); `None` = no factor.
fn scryer_weight(w: &ScoreWeights, kind: SubtitleScoreKind, name: &str) -> Option<i32> {
    let episode = kind == SubtitleScoreKind::Episode;
    Some(match name {
        "hash" => w.hash,
        "title" if !episode => w.title,
        "series" if episode => w.title,
        "year" => w.year,
        "season" if episode => w.season,
        "episode" if episode => w.episode,
        "release_group" => w.release_group,
        "source" => w.source,
        "audio_codec" => w.audio_codec,
        "resolution" => w.resolution,
        "video_codec" => w.video_codec,
        "hearing_impaired" => w.hearing_impaired,
        _ => return None,
    })
}

/// Bazarr weight table (name -> score) vs Scryer's weights for the same names.
fn weights_pair(
    kind: SubtitleScoreKind,
    bazarr: impl IntoIterator<Item = (String, i64)>,
) -> (BTreeMap<String, Option<i64>>, BTreeMap<String, Option<i64>>) {
    let scryer = kind.weights();
    let mut expected = BTreeMap::new();
    let mut actual = BTreeMap::new();
    for (name, score) in bazarr {
        actual.insert(
            name.clone(),
            scryer_weight(&scryer, kind, &name).map(i64::from),
        );
        expected.insert(name, Some(score));
    }
    (expected, actual)
}

/// `scores.json` list `[{name, score}]` -> pairs.
fn score_list(list: &Value) -> Vec<(String, i64)> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["name"].as_str().unwrap().to_string(),
                e["score"].as_i64().unwrap(),
            )
        })
        .collect()
}

/// Bazarr max score = hash + hearing_impaired (score.py:71; profiles empty).
fn bazarr_max(scores: &[(String, i64)]) -> i64 {
    let get = |n: &str| {
        scores
            .iter()
            .find(|(name, _)| name == n)
            .map(|(_, s)| *s)
            .unwrap()
    };
    get("hash") + get("hearing_impaired")
}

fn flag(value: &Value) -> bool {
    value
        .as_bool()
        .unwrap_or_else(|| value.as_str() == Some("True"))
}

fn prefs(profile_items: &Value) -> Vec<SubtitleLanguagePref> {
    profile_items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            let language = item["language"].as_str().unwrap();
            SubtitleLanguagePref {
                code: normalize_subtitle_language_code(language)
                    .unwrap_or_else(|| language.to_string()),
                hearing_impaired: flag(&item["hi"]),
                forced: flag(&item["forced"]),
            }
        })
        .collect()
}

fn download(language: &str, hearing_impaired: bool, forced: bool) -> SubtitleDownload {
    SubtitleDownload {
        id: "parity-download".into(),
        media_file_id: "parity-file".into(),
        title_id: "parity-title".into(),
        episode_id: None,
        source_kind: ExternalSubtitleSourceKind::Downloaded,
        language: normalize_subtitle_language_code(language)
            .unwrap_or_else(|| language.to_string()),
        provider: None,
        provider_file_id: None,
        file_path: "/parity/video.srt".into(),
        score: None,
        hearing_impaired,
        forced,
        ai_translated: false,
        machine_translated: false,
        uploader: None,
        release_info: None,
        synced: false,
        downloaded_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn satisfies(prefs: &[SubtitleLanguagePref], language: &str, hi: bool, forced: bool) -> bool {
    compute_missing_subtitles(prefs, &[download(language, hi, forced)], &[]).is_empty()
}

fn catalogue_ref(catalogue: &Catalogue, id: &str) -> String {
    catalogue
        .entries
        .iter()
        .find(|(_, e)| e.id == id)
        .unwrap()
        .1
        .reference
        .clone()
}

#[test]
fn baz_subtitles() {
    let Some(mut suite) = Suite::open("scryer-application::parity_subtitles::baz_subtitles") else { return };
    let cases: Vec<Value> = corpus("corpus/reference/bazarr/subtitles/score.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let given = &case["input"]["given"];
        match (id, case["input"]["op"].as_str().unwrap()) {
            ("BAZ-SUBTITLES-0055" | "BAZ-SUBTITLES-0056", "compute_score") => {
                let kind = kind(given["video"]["kind"].as_str().unwrap());
                let matches: HashSet<String> = given["matches"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|m| m.as_str().unwrap().to_string())
                    .collect();
                let no_error = std::panic::catch_unwind(|| {
                    compute_verified_score(&kind.weights(), kind, &matches, false)
                })
                .is_ok();
                suite.check(id, case["expect"]["no_error"].as_bool().unwrap(), no_error);
            }
            ("BAZ-SUBTITLES-0057", "compute_score_weights") => {
                let mut expected = BTreeMap::new();
                let mut actual = BTreeMap::new();
                for (section, k) in [
                    ("episode", SubtitleScoreKind::Episode),
                    ("movie", SubtitleScoreKind::Movie),
                ] {
                    let table = case["expect"]["weights"][section].as_object().unwrap();
                    let (e, a) = weights_pair(
                        k,
                        table.iter().map(|(n, s)| (n.clone(), s.as_i64().unwrap())),
                    );
                    expected.insert(section, e);
                    actual.insert(section, a);
                }
                suite.check(id, expected, actual);
            }
            _ => {}
        }
    }

    let cases: Vec<Value> = corpus("corpus/reference/bazarr/subtitles/video.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !["BAZ-SUBTITLES-0066", "BAZ-SUBTITLES-0067"].contains(&id) || !suite.wants(id) {
            continue;
        }
        let name = case["input"]["given"]["name"].as_str().unwrap();
        let stem = std::path::Path::new(name).file_stem().unwrap().to_str().unwrap();
        let parsed = parse_release_metadata(stem);
        let expect = case["expect"].as_object().unwrap();
        let episode = expect.contains_key("series");
        let mut expected = BTreeMap::new();
        let mut actual = BTreeMap::new();
        for (key, value) in expect {
            let value = match (key.as_str(), value.as_str()) {
                ("source", Some("Web")) => json!("WEB-DL"),
                ("source", Some("Blu-ray" | "Ultra HD Blu-ray")) => json!("BluRay"),
                _ => value.clone(),
            };
            let scryer = match key.as_str() {
                "series" if episode => json!(parsed.normalized_title),
                "title" if !episode => json!(parsed.normalized_title),
                "year" => json!(parsed.year),
                "release_group" => json!(parsed.release_group),
                "resolution" => json!(parsed.quality),
                "season" => json!(parsed.episode.as_ref().and_then(|e| e.season)),
                "episode" => json!(
                    parsed
                        .episode
                        .as_ref()
                        .and_then(|e| e.episode_numbers.first().copied())
                ),
                "source" => json!(parsed.source.as_ref().map(|s| s.as_str())),
                "video_codec" => json!(parsed.video_codec.as_ref().map(|c| c.as_str())),
                "audio_codec" => json!(
                    parsed
                        .audio
                        .as_ref()
                        .or_else(|| parsed.audio_codecs.first())
                        .map(|c| c.as_str())
                ),
                _ => json!("<no Scryer field>"),
            };
            expected.insert(key.clone(), value);
            actual.insert(key.clone(), scryer);
        }
        suite.check(id, expected, actual);
    }
    suite.finish();
}

#[test]
fn baz_subtitles_config() {
    let Some(mut suite) = Suite::open("scryer-application::parity_subtitles::baz_subtitles_config") else { return };
    let catalogue = Catalogue::load();
    let scores: Value = corpus("corpus/captured/bazarr/config/scores.json");
    let settings: Value = corpus("corpus/captured/bazarr/config/settings.json");
    let profiles: Vec<Value> = corpus("corpus/captured/bazarr/config/languages-profiles.json");

    let id = "BAZ-SUBTITLES-5001";
    if suite.wants(id) {
        let reference = catalogue_ref(&catalogue, id);
        let profile_id: i64 = reference
            .rsplit_once("#profileId=")
            .unwrap()
            .1
            .parse()
            .unwrap();
        let profile = profiles
            .iter()
            .find(|p| p["profileId"].as_i64() == Some(profile_id))
            .unwrap();
        let prefs = prefs(&profile["items"]);
        let mut expected = Vec::new();
        let mut actual = Vec::new();
        for item in profile["items"].as_array().unwrap() {
            let language = item["language"].as_str().unwrap().to_string();
            // Bazarr: hi False accepts plain and HI; hi True only HI.
            let hi_only = flag(&item["hi"]);
            let forced = flag(&item["forced"]);
            expected.push((language.clone(), !hi_only, true));
            actual.push((
                language.clone(),
                satisfies(&prefs, &language, false, forced),
                satisfies(&prefs, &language, true, forced),
            ));
        }
        suite.check(id, expected, actual);
    }

    for (id, list, k) in [
        (
            "BAZ-SUBTITLES-5006",
            "series_scores",
            SubtitleScoreKind::Episode,
        ),
        (
            "BAZ-SUBTITLES-5008",
            "movie_scores",
            SubtitleScoreKind::Movie,
        ),
    ] {
        if !suite.wants(id) {
            continue;
        }
        assert!(catalogue_ref(&catalogue, id).ends_with(&format!("#{list}")));
        let (expected, actual) = weights_pair(k, score_list(&scores[list]));
        suite.check(id, expected, actual);
    }

    for (id, setting, list, k) in [
        (
            "BAZ-SUBTITLES-5007",
            "minimum_score",
            "series_scores",
            SubtitleScoreKind::Episode,
        ),
        (
            "BAZ-SUBTITLES-5009",
            "minimum_score_movie",
            "movie_scores",
            SubtitleScoreKind::Movie,
        ),
    ] {
        if !suite.wants(id) {
            continue;
        }
        assert!(catalogue_ref(&catalogue, id).ends_with(&format!("#general/{setting}")));
        let percent = settings["general"][setting].as_i64().unwrap();
        let max = bazarr_max(&score_list(&scores[list]));
        let expected = (max, max as f64 * percent as f64 / 100.0);
        let actual = (
            i64::from(k.max_score()),
            f64::from(percent_to_raw_threshold(k, i32::try_from(percent).unwrap())),
        );
        suite.check(id, expected, actual);
    }
    suite.finish();
}

#[test]
fn baz_subtitles_captured() {
    let Some(mut suite) = Suite::open("scryer-application::parity_subtitles::baz_subtitles_captured") else { return };
    let scores: Value = corpus("corpus/captured/bazarr/config/scores.json");
    let settings: Value = corpus("corpus/captured/bazarr/config/settings.json");
    for file in ["history-episodes.json", "history-movies.json"] {
        let cases: Vec<Value> = corpus(&format!("corpus/captured/bazarr/data/cases/{file}"));
        for case in &cases {
            let id = case["id"].as_str().unwrap();
            if !suite.wants(id) {
                continue;
            }
            let input = &case["input"];
            let expect = &case["expect"];
            let item_type = input["item_type"].as_str().unwrap();
            let k = kind(item_type);
            let (list, setting) = match k {
                SubtitleScoreKind::Episode => ("series_scores", "minimum_score"),
                SubtitleScoreKind::Movie => ("movie_scores", "minimum_score_movie"),
            };
            let max = bazarr_max(&score_list(&scores[list]));
            let percent: f64 = input["score"]
                .as_str()
                .unwrap()
                .trim_end_matches('%')
                .parse()
                .unwrap();
            let raw = (percent * max as f64 / 100.0).round() as i32;
            let minimum = i32::try_from(settings["general"][setting].as_i64().unwrap()).unwrap();
            let threshold = percent_to_raw_threshold(k, minimum);
            let score_ok = raw >= threshold;
            let prefs = prefs(&input["language_profile"]["items"]);
            let language_ok = satisfies(
                &prefs,
                expect["language"].as_str().unwrap(),
                expect["hi"].as_bool().unwrap(),
                expect["forced"].as_bool().unwrap(),
            );
            let expected = expect["accepted"].as_bool().unwrap();
            let accepted = score_ok && language_ok;
            suite.record(
                id,
                expected == accepted,
                if expected == accepted {
                    String::new()
                } else {
                    format!(
                        "expected accepted {expected}, got {accepted} (raw {raw} vs threshold {threshold}: {score_ok}; profile satisfied: {language_ok})"
                    )
                },
            );
        }
    }

    let cases: Vec<Value> = corpus("corpus/captured/bazarr/data/cases/wanted-movies.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let prefs = prefs(&case["input"]["language_profile"]["items"]);
        let expected: Vec<(Option<String>, bool, bool)> = case["expect"]["missing_languages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                (
                    normalize_subtitle_language_code(m["code2"].as_str().unwrap()),
                    m["forced"].as_bool().unwrap(),
                    m["hi"].as_bool().unwrap(),
                )
            })
            .collect();
        let actual: Vec<(Option<String>, bool, bool)> = compute_missing_subtitles(&prefs, &[], &[])
            .into_iter()
            .map(|p| (Some(p.code), p.forced, p.hearing_impaired))
            .collect();
        suite.check(id, expected, actual);
    }
    suite.finish();
}
