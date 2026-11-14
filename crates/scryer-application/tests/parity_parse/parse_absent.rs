//! Parity net, parse area, stream parse_absent (Stages 2-5).
//!
//! Suites (one test fn each; each selects its own corpus cases before
//! `wants()`, so PARITY_ADOPT never adopts foreign IDs):
//! - `son_scene_title`: SceneChecker.IsSceneTitle, SON-PARSE-1189..1201 (13)
//! - `rad_url_slug`: Parser.ToUrlSlug, RAD-PARSE-1017..1091 (75)
//! - `rad_roman_to_int`: RomanNumeral.ToInt, RAD-PARSE-0985..1004 (20)
//! - `son_normalize_episode_title`: Parser.NormalizeEpisodeTitle, SON-PARSE-0587..0592 (6)
//! - `son_possible_special`: ParseTitle IsPossibleSpecialEpisode, SON-PARSE-0324..0328 (5)
//! - `rad_map_search_criteria`: ParsingService.Map (search criteria movie), RAD-PARSE-0520..0523 (4)
//! - `son_title_without_year`: SeriesTitleInfo title_equals_title_without_year, SON-PARSE-1253 (1)
//! - `son_quality_from_extension`: ParserFixture quality detection source, SON-PARSE-0673, 0674 (2)
//!
//! Scryer functions under test: `parity_seams::scene_titled_file_episode`
//! (import/workflow/series.rs), `parity_seams::slug_from_library_name`
//! (library/library.rs), `scryer_domain::season_ordinal_from_title`,
//! `parity_seams::canonical_lookup_key`, the parser's `special_kind` (root
//! `View`), `parity_seams::{canonical_title_evidence,
//! parsed_release_matches_title_evidence}` plus `parse_release_metadata`.
//!
//! Translation table:
//! | Reference | Scryer |
//! |---|---|
//! | `SceneChecker.IsSceneTitle(name)` == `is_scene_title` | `scene_titled_file_episode("<name>.mkv").is_some()`. The reference gets a name without extension; Scryer reads `file_stem`, so `.mkv` is appended |
//! | `Parser.ToUrlSlug(name, ..)` == `slug` | `slug_from_library_name(name)`, compared exactly with the raw literal. Scryer's only free-text slug function; it has none of the reference's args (`invalid_dash_replacement`, `trim_end_chars`, `deduplicate_chars`). The args are ignored and Scryer's fixed output is compared, so a case is green when that output matches what the setting asks for (dash replacement true, trim/dedupe lists that agree with Scryer) and red otherwise (D29). See the slug notes below |
//! | `new RomanNumeral(name).ToInt()` == `number` | `season_ordinal_from_title("Season <name>")`, none = null. The prefix is glue: the reader only takes a numeral in a season position. Scryer reads I..X only |
//! | `Parser.NormalizeEpisodeTitle(name)` == `normalized_episode_title` | `canonical_lookup_key(name)`, compared exactly (no space stripping) |
//! | `ParseTitle(name).IsPossibleSpecialEpisode` | the untargeted parse sets `episode.special_kind`; unparseable fails |
//! | `Map(parsed, .., MovieSearchCriteria{movie}).Movie == movie` | `canonical_title_evidence(Title)` + `parsed_release_matches_title_evidence(parse_release_metadata("<title> <year>"))`; all `parsed` entries must match. Title: name, year, aliases = alternative_titles ++ translation titles, `original_language` "English" -> `Title.language` "en" (Title.language is the original language, ISO 639-1), facet Movie, no tags. The title-identity proof reads `metadata_language`, not `Title.language`, and the corpus gives no metadata language, so `metadata_language` stays None |
//! | `SeriesTitleInfo.Title == .TitleWithoutYear` | Scryer parsed a title and no year (`p.year.is_none()`) |
//! | `Quality.is_unknown`, `*_detection_source` ("Extension") | is_unknown: no source and no resolution; Scryer has no detection-source field, so the actual is null (D29: mapped, red) |
//!
//! Ignored input keys: `facet`, `fn`, `args` (slug), `movie.clean_title`,
//! `parsed[].languages`, `imdb_id`, `tmdb_id`, `search_criteria_movie` (Map).
//!
//! Slug notes (`rad_url_slug`):
//! - RAD-PARSE-1027..1030 (leading ß/œ/Œ/Ø) are green only because Scryer
//!   drops a non-ASCII letter at the start without writing a dash. Scryer does
//!   not fold accents: a non-ASCII letter mid-word becomes '-' (`tèst` ->
//!   `t-st`), where the reference strips the accent (`test`).
//! - RAD-PARSE-1092..1095 (null/empty `trim_end_chars` / `deduplicate_chars`)
//!   are left out of the suite (unmapped): the input `test` equals the
//!   expected slug `test`, so any slugifier passes; the case tests only that
//!   the reference handles a null/empty parameter, which Scryer lacks.
//!
//! Episode title notes (`son_normalize_episode_title`):
//! - SON-PARSE-0590 red: Scryer keeps the `(1)` part number (`french title 1`).
//! - SON-PARSE-0591/0592 red: Sonarr's normaliser also extracts a special's
//!   episode title from a full release name (SpecialEpisodeTitleRegex).
//!   Scryer keys the whole name here; it finds a special's episode title by
//!   context-title matching against the catalogue episode
//!   (import/workflow/series.rs `source_fuzzily_matches_catalog_episode_title`,
//!   `ContextTitleMatchKind::EpisodeTitle`), not through the normaliser.
//!   Feeding the expected title in as context would derive actual from
//!   expected, so the red stays.
//!
//! Unmapped (tests: []), with reasons:
//! - Slug null/empty-args cases (RAD-PARSE-1092..1095, 4): echo-the-input,
//!   see the slug notes.
//! - Radarr SceneChecker.IsSceneTitle / GetSceneTitle (RAD-PARSE-1005..1016,
//!   12): Scryer's only scene check is the series sole-video guard and needs
//!   episode numbering, so for a movie name it is always false; mapping would
//!   give vacuous greens. Movie import has no scene check.
//! - RomanNumeral.ToRomanNumeral (RAD-PARSE-0965..0984, 20): Scryer never
//!   writes an integer as a Roman numeral.
//! - ParsedEpisodeInfo.IsPossibleSpecialEpisode on a hand-built parse
//!   (SON-PARSE-0322, 0323): no predicate over a parse result; specialness is
//!   the parser's own output.
//! - ValidateParsedEpisodeInfo.ValidateForSeriesType (SON-PARSE-1513..1515,
//!   3): Scryer has no series type and no such validator.
//! - `name_template` + `now_offset_days` (SON-PARSE-0264..0271, 8): the date
//!   is DateTime.Now + 2 days; the parser takes no reference date and the
//!   harness may not read the wall clock.

use std::panic::{AssertUnwindSafe, catch_unwind};

use scryer_application::parity_seams::{
    canonical_lookup_key, canonical_title_evidence, parsed_release_matches_title_evidence,
    scene_titled_file_episode, slug_from_library_name,
};
use scryer_domain::{MediaFacet, Title};
use scryer_parity::Suite;
use serde_json::{Value, json};

use super::{Check, View};

fn select(file: &str, keep: impl Fn(&Value) -> bool, n: usize) -> Vec<Value> {
    let cases: Vec<Value> = scryer_parity::corpus::<Vec<Value>>(file).into_iter().filter(|c| keep(c)).collect();
    assert_eq!(cases.len(), n, "{file}: selected case count drifted");
    cases
}

fn is_fn(c: &Value, f: &str) -> bool {
    c["input"]["fn"].as_str() == Some(f)
}

fn run(name: &str, cases: Vec<Value>, f: impl Fn(&Value) -> Check) {
    let Some(mut suite) = Suite::open(name) else { return };
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let out = f(case);
        suite.record(id, out.0.is_empty(), out.0.join("; "));
    }
    suite.finish();
}

/// Runs `f`, a panic becomes `Err`.
fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, ()> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|_| ())
}

fn compare(key: &str, expected: &Value, actual: Result<Value, ()>) -> Check {
    let mut out = Check::default();
    match actual {
        Ok(a) => out.eq(key, None, expected, a),
        Err(()) => out.fail(key, expected, "panic"),
    }
    out
}

#[test]
fn son_scene_title() {
    let cases = select(
        "corpus/reference/sonarr/parse/SceneCheckerFixture.json",
        |c| is_fn(c, "SceneChecker.IsSceneTitle"),
        13,
    );
    run("scryer-application::parity_parse::son_scene_title", cases, |c| {
        let name = c["input"]["name"].as_str().unwrap();
        let act = guarded(|| json!(scene_titled_file_episode(std::path::Path::new(&format!("{name}.mkv"))).is_some()));
        compare("is_scene_title", &c["expect"]["is_scene_title"], act)
    });
}

#[test]
fn rad_url_slug() {
    let cases = select("corpus/reference/radarr/parse/SlugParserFixture.json", |c| {
        is_fn(c, "Parser.ToUrlSlug") && !matches!(c["id"].as_str(), Some("RAD-PARSE-1092" | "RAD-PARSE-1093" | "RAD-PARSE-1094" | "RAD-PARSE-1095"))
    }, 75);
    run("scryer-application::parity_parse::rad_url_slug", cases, |c| {
        let name = c["input"]["name"].as_str().unwrap();
        compare("slug", &c["expect"]["slug"], guarded(|| json!(slug_from_library_name(name))))
    });
}

#[test]
fn rad_roman_to_int() {
    let cases = select(
        "corpus/reference/radarr/parse/RomanNumeralConversionFixture.json",
        |c| is_fn(c, "RomanNumeral.ToInt"),
        20,
    );
    run("scryer-application::parity_parse::rad_roman_to_int", cases, |c| {
        let name = c["input"]["name"].as_str().unwrap();
        let act = guarded(|| {
            scryer_domain::season_ordinal_from_title(&format!("Season {name}")).map_or(Value::Null, |n| json!(n))
        });
        compare("number", &c["expect"]["number"], act)
    });
}

#[test]
fn son_normalize_episode_title() {
    let cases = select(
        "corpus/reference/sonarr/parse/NormalizeEpisodeTitleFixture.json",
        |c| is_fn(c, "Parser.NormalizeEpisodeTitle"),
        6,
    );
    run("scryer-application::parity_parse::son_normalize_episode_title", cases, |c| {
        let name = c["input"]["name"].as_str().unwrap();
        compare("normalized_episode_title", &c["expect"]["normalized_episode_title"], guarded(|| json!(canonical_lookup_key(name))))
    });
}

#[test]
fn son_possible_special() {
    let cases = select(
        "corpus/reference/sonarr/parse/IsPossibleSpecialEpisodeFixture.json",
        |c| is_fn(c, "Parser.ParseTitle"),
        5,
    );
    run("scryer-application::parity_parse::son_possible_special", cases, |c| {
        let name = c["input"]["name"].as_str().unwrap();
        let key = "is_possible_special_episode";
        let mut out = Check::default();
        match guarded(|| View::new(name)) {
            Err(()) => out.fail(key, &c["expect"][key], "panic"),
            Ok(v) if !v.parsed() => out.fail("parse", "a result", "Unparseable"),
            Ok(v) => out.eq(key, None, &c["expect"][key], json!(v.ep().special_kind.is_some())),
        }
        out
    });
}

fn movie_title(m: &Value) -> Title {
    let mut aliases: Vec<String> = m["alternative_titles"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect();
    aliases.extend(m["translations"].as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap().to_string()));
    Title {
        id: "title-1".into(),
        library_id: scryer_domain::default_library_id_for_facet(&MediaFacet::Movie),
        name: m["title"].as_str().unwrap().into(),
        facet: MediaFacet::Movie,
        monitored: true,
        tags: vec![],
        external_ids: vec![],
        root_folder_id: scryer_domain::root_folder_id_for_path("/media"),
        created_by: None,
        created_at: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).expect("epoch"),
        year: Some(m["year"].as_i64().unwrap() as i32),
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
        canonical_tags: vec![],
        content_status: None,
        language: Some(match m["original_language"].as_str() {
            Some("English") => "en".into(),
            other => panic!("no translation for original_language {other:?}"),
        }),
        first_aired: None,
        network: None,
        studio: None,
        country: None,
        aliases,
        tagged_aliases: vec![],
        metadata_language: None,
        metadata_fetched_at: None,
        min_availability: None,
        digital_release_date: None,
        movie_release_dates: None,
        folder_path: None,
    }
}

#[test]
fn rad_map_search_criteria() {
    let cases = select(
        "corpus/reference/radarr/parse/MapFixture.json",
        |c| is_fn(c, "ParsingService.Map") && c["input"]["search_criteria_movie"] == json!(true),
        4,
    );
    run("scryer-application::parity_parse::rad_map_search_criteria", cases, |c| {
        let title = movie_title(&c["input"]["movie"]);
        let act = guarded(|| {
            let ev = canonical_title_evidence(&title);
            let all = c["input"]["parsed"].as_array().unwrap().iter().all(|p| {
                let titles = p["movie_titles"].as_array().unwrap();
                assert_eq!(titles.len(), 1);
                let release = format!("{} {}", titles[0].as_str().unwrap(), p["year"]);
                parsed_release_matches_title_evidence(&scryer_application::parse_release_metadata(&release), &ev)
            });
            json!(all)
        });
        compare("maps_to_search_criteria_movie", &c["expect"]["maps_to_search_criteria_movie"], act)
    });
}

#[test]
fn son_title_without_year() {
    let cases = select(
        "corpus/reference/sonarr/parse/SeriesTitleInfoFixture.json",
        |c| c["expect"]["series_title_info"].get("title_equals_title_without_year").is_some(),
        1,
    );
    run("scryer-application::parity_parse::son_title_without_year", cases, |c| {
        let name = c["input"]["name"].as_str().unwrap();
        let key = "series_title_info.title_equals_title_without_year";
        let exp = &c["expect"]["series_title_info"]["title_equals_title_without_year"];
        let mut out = Check::default();
        match guarded(|| View::new(name)) {
            Err(()) => out.fail(key, exp, "panic"),
            Ok(v) if !v.parsed() || v.title().is_null() => out.fail(key, exp, "no parsed series title"),
            Ok(v) => out.eq(key, None, exp, json!(v.p.year.is_none())),
        }
        out
    });
}

#[test]
fn son_quality_from_extension() {
    let cases = select(
        "corpus/reference/sonarr/parse/ParserFixture.json",
        |c| is_fn(c, "Parser.ParseTitle") && c["expect"]["quality"].get("source_detection_source").is_some(),
        2,
    );
    run("scryer-application::parity_parse::son_quality_from_extension", cases, |c| {
        let name = c["input"]["name"].as_str().unwrap();
        let exp = &c["expect"]["quality"];
        let mut out = Check::default();
        match guarded(|| View::new(name)) {
            Err(()) => out.fail("quality", exp, "panic"),
            Ok(v) if !v.parsed() => out.fail("parse", "a result", "Unparseable"),
            Ok(v) => {
                let unknown = v.p.source.is_none() && v.p.quality.is_none();
                out.eq("quality.is_unknown", None, &exp["is_unknown"], json!(unknown));
                // Scryer keeps no detection-source field.
                out.eq("quality.source_detection_source", None, &exp["source_detection_source"], Value::Null);
                out.eq("quality.resolution_detection_source", None, &exp["resolution_detection_source"], Value::Null);
            }
        }
        out
    });
}
