//! Parity net, stream `subtitles`: custom score weights and the edition /
//! `video_fromguess` gaps, mapped red (D29).
//!
//! Suites (nextest names `subtitles::<fn>`):
//! - `baz_subtitles_custom_weights`: BAZ-SUBTITLES-0058, 0059 (score.json,
//!   kept when `input.op == "compute_score_weights"` and `input.given.custom`
//!   is not null; before `wants`).
//! - `baz_subtitles_absent`: BAZ-SUBTITLES-0060..0063 (subtitle.json
//!   `guess_matches`), 0064, 0065 (video.json `video_fromguess`), filtered by
//!   op before `wants`.
//!
//! # Scryer functions under test
//!
//! custom_weights: `SubtitleScoreKind::weights()` (`subtitles/scoring.rs`)
//! through the root `weights_pair`. absent: `SubtitleScoreKind::weights()` (an
//! edition factor lookup, `None` today: 0060..0063 compare the reference bool
//! with `None`, so even 0060/0061 are red until Scryer can answer) and
//! `parse_release_metadata` on the case file name (0064/0065: kind,
//! streaming_service, edition, `is_proper_upload` -> "Proper"). The parser
//! gets the NAME, not the guess dict (Scryer has no such input), so 0064's
//! `streaming_service: "foo"` and 0065's `edition: "foo"` stay red.
//!
//! # Translation table (reference term -> Scryer term)
//!
//! - weight names, sections `episode`/`movie`: as in the root header
//!   (`series` -> `title`; `edition`, `streaming_service` -> no factor, fails).
//! - `input.given.custom` (custom weight hash) -> no Scryer setting; weights
//!   are fixed, so the case is red.
//! - 0058 duplicates 0057's table check: its expect is the default table, and
//!   the custom-hash rejection it tests is unobservable because Scryer has no
//!   custom-weight setting (fixed tables in `subtitles/scoring.rs`; the plugin
//!   adapter always uses `MOVIE_WEIGHTS`/`SERIES_WEIGHTS`). If 0057 goes
//!   green, revisit 0058 (waive it, or record it red with that reason). 0059
//!   can only pass once Scryer accepts custom weights.
//! - guess `edition`, `streaming_service`, `other`, video `edition` -> no
//!   Scryer field (red).
//! - custom_weights key set: taken from `expect.weights` (0059's expect is a diff
//!   of two names, so only those are compared); Scryer has no custom-weight
//!   input, so `input.given.custom` cannot be fed to any function.
//! - Ignored keys: `input.given.custom`, `note`, subtitle.json
//!   `input.given.video` / `input.given.guess`, video.json `input.given.guess`
//!   / `input.given.name`; `random_key`.
//!
//! # Unmapped (tests: []), with reasons
//!
//! - BAZ-SUBTITLES-0001..0018: no corpus case (expectation only in catalogue
//!   prose). 0004 is checked by 5007/5009, 0008 by 5001 (P7, red).
//! - BAZ-SUBTITLES-0023, 0045, 0049, 0050: Scryer has no language-equivalence
//!   concept; a test for 0049/0050 could only echo the input set.
//! - BAZ-SUBTITLES-0037, 0038: the kind comes from the title facet
//!   (`subtitle_media_kind`, `subtitles/orchestration.rs`), never from a name.
//! - BAZ-SUBTITLES-0039..0042: provider pool with `update(enabled)` does not
//!   exist; providers are loaded per cycle by async
//!   `configured_runtime_subtitle_providers(AppUseCase)` (repositories needed).

use std::collections::BTreeMap;

use scryer_application::parse_release_metadata;
use scryer_application::subtitles::scoring::edition_matches;
use scryer_parity::{Suite, corpus};
use serde_json::{Value, json};

fn cases(file: &str, keep: impl Fn(&Value) -> bool) -> Vec<Value> {
    corpus::<Vec<Value>>(file)
        .into_iter()
        .filter(|c| keep(&c["input"]))
        .collect()
}

#[test]
fn baz_subtitles_custom_weights() {
    let Some(mut suite) = Suite::open("scryer-application::parity_subtitles::baz_subtitles_custom_weights") else {
        return;
    };
    let sel = cases("corpus/reference/bazarr/subtitles/score.json", |i| {
        i["op"] == "compute_score_weights" && !i["given"]["custom"].is_null()
    });
    for case in sel {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let mut expected: BTreeMap<&str, BTreeMap<String, Option<i64>>> = BTreeMap::new();
        let mut actual = BTreeMap::new();
        for (section, table) in case["expect"]["weights"].as_object().unwrap() {
            let pairs = table
                .as_object()
                .unwrap()
                .iter()
                .map(|(n, s)| (n.clone(), s.as_i64().unwrap()));
            let (e, a) = super::weights_pair(super::kind(section), pairs);
            expected.insert(section, e);
            actual.insert(section.as_str(), a);
        }
        suite.check(id, expected, actual);
    }
    suite.finish();
}

#[test]
fn baz_subtitles_absent() {
    let Some(mut suite) = Suite::open("scryer-application::parity_subtitles::baz_subtitles_absent") else {
        return;
    };
    for case in cases("corpus/reference/bazarr/subtitles/subtitle.json", |i| i["op"] == "guess_matches") {
        let id = case["id"].as_str().unwrap();
        if suite.wants(id) {
            let expected = case["expect"]["edition_in_matches"].as_bool();
            let side = |v: &Value| v["edition"].as_str().map(str::to_string);
            let given = &case["input"]["given"];
            let actual = edition_matches(side(&given["video"]).as_deref(), side(&given["guess"]).as_deref());
            suite.check(id, expected, Some(actual));
        }
    }
    // `video_fromguess`: the reference builds a video from a guessit dict;
    // Scryer has no such input, so the production parser runs on the file
    // name and its fields are compared (kind, streaming_service, edition,
    // other = Proper).
    for case in cases("corpus/reference/bazarr/subtitles/video.json", |i| i["op"] == "video_fromguess") {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let name = case["input"]["given"]["name"].as_str().unwrap();
        let stem = std::path::Path::new(name).file_stem().unwrap().to_str().unwrap();
        let parsed = parse_release_metadata(stem);
        let mut expected = BTreeMap::new();
        let mut actual = BTreeMap::new();
        for (key, value) in case["expect"].as_object().unwrap() {
            let scryer = match key.as_str() {
                "kind" => json!(if parsed.episode.is_some() { "Episode" } else { "Movie" }),
                "streaming_service" => json!(parsed.streaming_service.as_ref().map(|s| format!("{s:?}"))),
                "edition" => json!(parsed.edition),
                "other" => json!(parsed.is_proper_upload.then_some("Proper")),
                other => panic!("unmapped video_fromguess key {other}"),
            };
            expected.insert(key.clone(), value.clone());
            actual.insert(key.clone(), scryer);
        }
        suite.check(id, expected, actual);
    }
    suite.finish();
}
