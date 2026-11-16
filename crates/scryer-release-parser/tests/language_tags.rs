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

fn original(release: &str) -> bool {
    let target: ReleaseParseContext = serde_json::from_value(serde_json::json!({
        "facet_hint": "movie", "title": {"name": "Movie Title"}, "aliases": [],
        "known_years": [], "imdb_ids": [], "episodes": []
    }))
    .expect("context");
    best_parse_for_target(release, &target).is_original_audio
}

// fork (P27b): Original marker and the German DL/ML rule.
#[test]
fn original_audio_marker() {
    assert!(original("Movie.Title.2025.Orig.1080P.WEB.H264-Grp"));
    assert!(original("Movie.Title.2016.German.DTS.DL.720p.BluRay.x264-Grp"));
    assert_eq!(audio("Movie.Title.2023.German.ML.EAC3.720p.NF.WEB.H264-Grp"), ["deu", "eng"]);
    for name in [
        "Movie.Title.2016.German.DTS.720p.WEB-DL.x264-Grp",
        "Movie.Title.2016.German.DTS.720p.WEB.DL.x264-Grp",
        "Movie.Title.2016.German.English.DL.720p.BluRay.x264-Grp",
    ] {
        assert!(!original(name) || name.contains("English"), "{name}");
    }
}

// fork (P27e): '+' lists and multipliers.
#[test]
fn plus_separated_language_lists() {
    assert_eq!(audio("Movie.Title.2010.ENG+RUS.1080p.BluRay.x264-Grp"), ["eng", "rus"]);
    assert_eq!(audio("Movie.Title.2010.2xUKR+ENG.1080p.BluRay.x264-Grp"), ["ukr", "eng"]);
}

fn audio_for(facet: &str, title: &str, release: &str) -> Vec<String> {
    let target: ReleaseParseContext = serde_json::from_value(serde_json::json!({
        "facet_hint": facet, "title": {"name": title}, "aliases": [],
        "known_years": [], "imdb_ids": [], "episodes": []
    }))
    .expect("context");
    best_parse_for_target(release, &target).languages_audio
}

fn has(fails: &std::cell::RefCell<Vec<String>>, facet: &str, title: &str, release: &str, codes: &[&str]) {
    let got = audio_for(facet, title, release);
    for code in codes {
        if !got.iter().any(|g| g == code) {
            fails.borrow_mut().push(format!("{release}: want {code}, got {got:?}"));
        }
    }
}

// fork (P39): language tokens remainder (census A5).
#[test]
fn p39_movie_language_shapes() {
    let fails = std::cell::RefCell::new(Vec::new());
    let m = |release: &str, codes: &[&str]| has(&fails, "movie", "Movie Title", release, codes);
    m("Movie.Name.2022.BluRay.1080p.H264.DTS[EN+ES].[EN+ES+IT]", &["spa"]);
    m("Movie Title 2016 - Kampfhaehne - mkv - by Videomann", &["deu"]);
    m("Movie.Title.2016.1080p.10Bit.HEVC.WEBRip.HIN-ENG-GER.DD5.1.H.265", &["deu"]);
    m("Movie Title (1985) (1080p.AC3 ITA-ENG-JAP)", &["jpn"]);
    m("Movie.Title (2006) BDRemux 1080p 2xUkr | Sub Ukr", &["ukr"]);
    m("Movie.Title [1984, BDRip 720p] MVO + MVO + Dub + AVO + 3xUkr", &["ukr"]);
    m("Movie.Title.[2003.BDRemux.1080p].Dub.MVO.(2xUkr/Fra).Sub.(Rus/Fra)", &["ukr"]);
    assert!(fails.borrow().is_empty(), "{}", fails.borrow().join("\n"));
}

#[test]
fn p39_series_language_shapes() {
    let fails = std::cell::RefCell::new(Vec::new());
    let s = |release: &str, codes: &[&str]| has(&fails, "series", "Series", release, codes);
    let has_c = |title: &str, release: &str, codes: &[&str]| has(&fails, "series", title, release, codes);
    s("Series S02 (1999–2003)[BDRemux 1080p AVC Esp DD2.0,Ing DTS-HD5.1,Cat DD2.0 Subs][HD-Olimpo][PACK]", &["spa", "eng", "cat"]);
    s("Series S02 (2009–2013)[BDRemux 1080p AVC Esp.DD2.0,Ing.DD2.0,Cat.DD2.0,Subs][HD-Olimpo][PACK]", &["spa", "eng", "cat"]);
    s("The Series Title - S02E16 - Kampfhaehne - mkv - by Videomann", &["deu"]);
    s("Title.the.Series.S01.COMPLETE.2009.1080p.WEB-DL.x264.AVC.AAC.LT.LV.RU", &["rus"]);
    s("Title.the.Series.S07E11.WEB Rip.XviD.Louige-CZ.EN.5.1", &["ces"]);
    has_c("Гало Сезон 1 Серії 1 5 SeriesTitle", "Гало(Сезон 1, серії 1-5) / SeriesTitle(Season 1, episodes 1-5) (2022) WEBRip-AVC Ukr/Eng", &["ukr"]);
    has_c("Остання серія Сезон 1 The Last Series", "Остання серія (Сезон 1) / The Last Series (Season 1) (2024) WEB-DLRip-AVC 2xUkr/Eng | Sub Ukr/Eng", &["ukr", "eng"]);
    has_c("Серія", "Серія (Сезон 1-3) / The Series (Seasons 1-3) (2019-2022) BDRip-AVC 4xUkr/Eng | Sub 2xUkr/Eng", &["ukr", "eng"]);
    assert!(fails.borrow().is_empty(), "{}", fails.borrow().join("\n"));
}

#[test]
fn p39_original_after_episode() {
    let mut fails = Vec::new();
    for (facet, release) in [
        ("series", "Series.Title.S01E01.Original.1080P.WEB.H264-RlsGrp"),
        ("series", "Series.Title.S01E01.Orig.1080P.WEB.H264-RlsGrp"),
        ("series", "Series / S1E1-10 of 10 [2023, HEVC, HDR10, Dolby Vision, WEB-DL 2160p] [Hybrid] 3 XX + Original"),
        ("movie", "Movie Title 2025 [HEVC, HDR10, Dolby Vision, WEB-DL 2160p] [Hybrid] 3 XX + Original"),
    ] {
        let target: ReleaseParseContext = serde_json::from_value(serde_json::json!({
            "facet_hint": facet, "title": {"name": if facet == "movie" { "Movie Title" } else { "Series" }}, "aliases": [],
            "known_years": [], "imdb_ids": [], "episodes": []
        }))
        .expect("context");
        if !best_parse_for_target(release, &target).is_original_audio {
            fails.push(release);
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

// fork (P39): no panic on non-ASCII around the new list handling.
#[test]
fn p39_non_ascii_no_panic() {
    for release in ["Фильм.2020.HIN-ЕNG-GER.DD5.1", "Série.S01E01.Esp.DD2.0,Ïng.DD2.0", "Тест [EN+ЕS] ｜ 3 ХХ + Оriginal"] {
        let _ = audio_for("series", "Série", release);
    }
}
