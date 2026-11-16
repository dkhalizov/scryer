//! Release-language tags checked against annotated, anonymized release shapes.

use scryer_release_parser::{ReleaseParseContext, best_parse_for_target};
use serde_json::Value;

#[test]
fn ukrainian_language_tag_shapes() {
    let corpus: Value =
        serde_json::from_str(include_str!("corpus/languages/ukrainian.json")).expect("corpus JSON");
    assert_eq!(corpus["schema_version"], 1);
    let mut failures = Vec::new();
    for case in corpus["cases"].as_array().expect("cases") {
        let expected = case["expected"].as_object().expect("expected object");
        let evidence = case["evidence"].as_object().expect("evidence object");
        assert!(
            expected.keys().all(|field| evidence.contains_key(field)),
            "{}: every expected field needs evidence",
            case["id"]
        );
        let target: ReleaseParseContext =
            serde_json::from_value(case["context"].clone()).expect("context");
        let parsed = best_parse_for_target(case["release"].as_str().expect("release"), &target);
        let actual = serde_json::to_value(&parsed).expect("serializable parse");
        for (field, value) in expected {
            if actual.get(field) != Some(value) {
                failures.push(format!(
                    "{} {field}: expected {value}, actual {:?}",
                    case["id"],
                    actual.get(field)
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn languages(release: &str) -> (Vec<String>, Vec<String>) {
    let target: ReleaseParseContext = serde_json::from_value(serde_json::json!({
        "facet_hint": "movie", "title": {"name": "Movie Title"}, "aliases": [],
        "known_years": [], "imdb_ids": [], "episodes": []
    }))
    .expect("context");
    let parsed = best_parse_for_target(release, &target);
    (parsed.languages_audio, parsed.languages_subtitles)
}

fn audio(release: &str) -> Vec<String> {
    languages(release).0
}

// fork (P27c): French dub markers are audio even after MULTI or VOSTFR.
#[test]
fn french_dub_markers_are_audio() {
    assert_eq!(audio("Movie.Title.1956.MULTi.VF.Bluray.1080p.REMUX.AC3.x264-GRP"), ["fra"]);
    let (a, s) = languages("Movie.Title.2010.Blu-ray HD.VOSTFR.VFF.1080p.x264-GRP");
    assert_eq!((a, s), (vec!["fra".to_string()], vec!["fra".to_string()]));
}

// fork (P27d): arr CaseSensitiveLanguageRegex short codes.
#[test]
fn short_language_codes_need_upper_case() {
    assert!(audio("Movie.Title.2010.En.1080p.WEB-DL.x264-GRP").is_empty());
    assert!(audio("Movie.Title.2010.2160p.iT.WEB-DL.x264-GRP").is_empty());
    assert!(audio("Movie.Title.2010.BluRay.1080p.DTS-ES.AC3.x264-GRP").is_empty());
    assert_eq!(audio("Movie.Title.1994.CZ.1080p.BluRay.x264-GRP"), ["ces"]);
    assert_eq!(audio("Movie.Title.2010.HU-IT-DE.1080p.BluRay.x264-GRP"), ["hun", "deu"]);
}

// fork (P27f): full-name vocabulary, and words that must stay in titles.
#[test]
fn full_language_names_are_read() {
    for (name, code) in [("Greek", "ell"), ("Tamil", "tam"), ("Flemish", "nld"), ("Mongolian", "mon")] {
        assert_eq!(audio(&format!("Movie.Title.2010.{name}.1080p.BluRay.x264-GRP")), [code], "{name}");
    }
    assert!(audio("Tel.Aviv.On.Fire.2018.1080p.BluRay.x264-GRP").is_empty());
}
