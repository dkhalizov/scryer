//! Parity net, parse area (notes/11-parity-net-design.md, parity/README.md).
//!
//! Suites (one test fn each):
//! - `son_parse`: `parity/corpus/reference/sonarr/parse/*.json` (SON-PARSE-0001..)
//! - `rad_parse`: `parity/corpus/reference/radarr/parse/*.json` (RAD-PARSE-0001..)
//! - `son_parse_captured`: `parity/corpus/captured/sonarr/data/cases/parse.json` (SON-PARSE-6001..)
//! - `rad_parse_captured`: `parity/corpus/captured/radarr/data/cases/parse.json` (RAD-PARSE-6001..)
//! - `parse_title.rs`: son_clean_title, rad_clean_title, son_series_name, rad_movie_title_clean, rad_normalize_imdb_id (see that header)
//! - `parse_path::son_parse_path`, `parse_path::rad_parse_path`: path parsing, see `parity_parse/parse_path.rs`
//! - `parse_lang::son_iso_languages`, `parse_lang::rad_iso_languages`: IsoLanguages.Find, see `parity_parse/parse_lang.rs`
//!
//! The reference calls ParseTitle / ParseMovieTitle with no target, so every
//! case runs Scryer's untargeted production path:
//! `scryer_application::parse_release_metadata` (the parse) and
//! `scryer_application::neutral_release_analysis` (its analysis, for title
//! segments and the hash token). Both build the parse context from the raw
//! name only (`synthesize_release_parse_context`: guessed title, years, imdb
//! ids, facet), so no expected value reaches the parser. The case's `facet`
//! input is ignored: production guesses the facet from the name. A case with
//! `names` passes only if every name passes. A parser panic fails the case.
//!
//! Translation table (reference expect key -> Scryer output compared):
//!
//! | Reference | Scryer |
//! |---|---|
//! | result is null (`should_parse: false`) | `disposition == Unparseable` (`should_parse` = not unparseable) |
//! | any other key of ParseTitle / ParseMovieTitle / a captured case | needs a parse: an unparseable result fails the case (except a case whose only key is `no_error`) |
//! | `title` (SeriesTitle, PrimaryMovieTitle) | raw text of the best candidate's first `ObservedPrimary` title segment; none = `""` where the reference uses `""`. Sonarr only: SeriesTitle keeps the year as written (`Series 2009`, `Series (2009)`) where Scryer splits it into `year`, so when Scryer has year Y and the expected title ends in ` Y` or ` (Y)`, Scryer's title gets that suffix before comparing |
//! | `movie_titles`, `series_title_info.all_titles` | raw text of every `ObservedPrimary`/`ObservedAlternate` segment, in order |
//! | `series_title_info.title`, `.title_without_year` | the same title; `.title` gets the Sonarr year suffix as for `title` |
//! | `series_title_info.year`, `year` | `year`, absent = 0 |
//! | `season` | `episode.season`, absent = 0 (no fallback to `season_numbers`) |
//! | `episodes`, `absolute_episodes`, `special_absolute_episodes` | `episode_numbers`, `absolute_episode_numbers`, `special_absolute_episode_numbers` (integers: 12.5 cannot match) |
//! | `full_season`, `is_partial_season`, `multi_season`, `is_season_extra`, `is_split_episode` | `full_season`, `is_partial_season`, `is_multi_season`, `is_season_extra`, `is_split_episode` |
//! | `season_part`, `daily_part`, `air_date` | `season_part`, `daily_part`, `air_date` (YYYY-MM-DD) |
//! | `is_special` | `episode.special_kind` is set |
//! | `release_group` | `release_group` |
//! | `release_hash` | raw text of the first token annotated `TokenRole::ChecksumOrHash`; none = `""` where the reference uses `""` |
//! | `release_title` | `raw_title` |
//! | `no_error` | the parse does not panic |
//! | `edition` | `edition` (absent = `""` where the reference uses `""`) |
//! | `hardcoded_subs` | `is_hardcoded_subs`: false = null; true cannot name the tag, so a named tag fails |
//! | `tmdb_id`, `imdb_id` | `tmdb_id` (number, absent = 0), `imdb_id` |
//! | `languages` (set), `languages_include` (subset), `languages_count`, `languages_first` | `languages_audio` named by LANGUAGES below; empty = `["Unknown"]`. Scryer has no "Original" language, so a case expecting it fails (`is_multi_audio` is not translated: the reference reports Unknown for MULTi) |
//! | `quality.source` / `.name` / `.modifier` / `.resolution` | see quality below |
//! | `quality.revision` | see revision below |
//!
//! Quality: both sides become (source family, resolution, modifier) in Scryer
//! terms, comparing only what the case asserts. Scryer side: family from
//! `source` (`BrDisk` counts as `bluray`), resolution = leading digits of
//! `quality` (absent = 0), modifier = `remux` if `is_remux`, else `brdisk` if
//! `is_bd_disk` or source `BrDisk`, else `none` (Scryer has no Raw-HD, so
//! `rawhd` never matches). Reference side:
//!
//! | Sonarr source | family, modifier | Radarr source | family |
//! |---|---|---|---|
//! | Television | hdtv, none | TV | hdtv |
//! | TelevisionRaw | hdtv, rawhd | WEBDL | webdl |
//! | Web | webdl, none | WEBRIP | webrip |
//! | WebRip | webrip, none | BLURAY | bluray |
//! | Bluray | bluray, none | DVD | dvd |
//! | BlurayRaw | bluray, remux | CAM, TELESYNC, TELECINE, DVDSCR, WORKPRINT | same name |
//! | DVD | dvd, none | UNKNOWN | null |
//! | Unknown | null, none | | |
//!
//! Source names are looked up ignoring case (the captured APIs spell the same
//! enums `webRip`, `bluray`). A Sonarr source fixes the modifier; a Radarr
//! case asserts the modifier only when it names one (REMUX, BRDISK, RAWHD,
//! none -> lower case). A quality `name` alone (Radarr) stands for all three:
//! `<Src>-<N>p` (Bluray, WEBDL, WEBRip, HDTV), `Remux-<N>p`, `Bluray-<N>p
//! Remux`, `SDTV` (hdtv 480), `DVD` (dvd 480), `DVD-R` (dvd 480 remux),
//! `Raw-HD` (hdtv 1080 rawhd), `BR-DISK` (bluray 1080 brdisk), `Unknown`
//! (null 0). With a source present, `name` is redundant and not compared.
//!
//! Revision: Scryer folds every REAL marker into `revision` (REAL.PROPER = 2)
//! and keeps anime `vN` in `anime_version`. Scryer version =
//! max(`revision` + 1, `anime_version` or 1). The reference splits PROPER
//! (version) from REAL (real), so: version and real both asserted -> compare
//! version + real; version alone -> compare version; real alone -> Scryer real
//! is 0 when `revision` is 0, otherwise unknown (fails); `is_repack` ->
//! `is_repack`.
//!
//! LANGUAGES (Scryer ISO 639-2 code -> reference name; a code not listed is
//! compared as the code, so it fails): see `LANGUAGES` below. Scryer folds
//! Portuguese (Brazil) into `por` and Spanish (Latino) into `spa`, so those
//! names fail.
//!
//! Comparison: exact, except a case's `"compare"` field (D19; generated by the
//! extractors from FluentAssertions BeEquivalentTo) maps an expect key (dotted
//! for nested) to `unordered` or `ignore_case`; `languages` is always set
//! equality (extractor rule).
//!
//! Inputs ignored: `is_dir` (Radarr ParseMovieTitle's folder flag; Scryer's
//! parser has one entry point for file and folder names).
//!
//! Ignored keys in captured cases (not parser output): `resolved_languages`,
//! `matched_movie_id`, `matched_movie_title`, `matched_movie_year`,
//! `custom_formats`, `custom_format_score` (Radarr), and the note's matched
//! series id (Sonarr). Every other captured key is compared.
//!
//! Unmapped (tests: []), with reasons:
//! - Parser.ParsePath / Parser.ParseMoviePath (Sonarr 48, Radarr 11) are mapped
//!   in `parity_parse/parse_path.rs` (suites `son_parse_path`, `rad_parse_path`).
//! - Parser.CleanSeriesTitle, Parser.ParseSeriesName (`series_name_clean`),
//!   Parser.CleanMovieTitle, ParseMovieTitle `movie_title_clean` (Sonarr 76,
//!   Radarr 59): mapped in `parity_parse/parse_title.rs`.
//! - IsoLanguages.Find (Sonarr 13, Radarr 43) is mapped in
//!   `parity_parse/parse_lang.rs`.
//! - LanguageParser.ParseSubtitleLanguage, ParseSubtitleLanguageInformation
//!   (Sonarr 33, Radarr 36): subtitle sidecar names are parsed in
//!   scryer-application `subtitles/` (later stage).
//! - Parser.NormalizeImdbId (Radarr 6): mapped in `parity_parse/parse_title.rs`.
//!   ParsingService.Map (Radarr 4): matching a parse to a library movie, application level.
//! - Absent concepts: SceneChecker (Sonarr 13, Radarr 12), Parser.ToUrlSlug
//!   (Radarr 79), RomanNumeral (Radarr 40), Parser.NormalizeEpisodeTitle
//!   (Sonarr 6), ValidateParsedEpisodeInfo (Sonarr 3), IsPossibleSpecialEpisode
//!   (Sonarr 7), quality detection sources `source_detection_source` /
//!   `resolution_detection_source` (Sonarr 25, Radarr 22), and
//!   `series_title_info.title_equals_title_without_year` (Sonarr 1: Scryer has
//!   one title, comparing it with itself would be vacuous).
//! - `name_template` + `now_offset_days` (Sonarr 8): the date is
//!   DateTime.Now + 2 days. The parser has no clock input and the harness may
//!   not read the wall clock.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use scryer_parity::{Suite, parity_dir};
use scryer_release_parser::{
    ParseDisposition, ParsedEpisodeMetadata, ParsedReleaseMetadata, ReleaseParseAnalysis, ReleaseSource,
    TitleSegmentKind, TokenRole,
};
use serde_json::{Map, Value, json};

#[path = "parity_parse/parse_lang.rs"]
mod parse_lang;
#[path = "parity_parse/parse_path.rs"]
mod parse_path;
#[path = "parity_parse/parse_title.rs"]
mod parse_title;

#[derive(Clone, Copy, PartialEq)]
enum App {
    Sonarr,
    Radarr,
}

const LANGUAGES: &[(&str, &str)] = &[
    ("afr", "Afrikaans"), ("ara", "Arabic"), ("ben", "Bengali"), ("bul", "Bulgarian"),
    ("cat", "Catalan"), ("ces", "Czech"), ("dan", "Danish"), ("deu", "German"),
    ("ell", "Greek"), ("eng", "English"), ("fas", "Persian"), ("fin", "Finnish"),
    ("fra", "French"), ("heb", "Hebrew"), ("hin", "Hindi"), ("hun", "Hungarian"),
    ("isl", "Icelandic"), ("ita", "Italian"), ("jpn", "Japanese"), ("kan", "Kannada"),
    ("kat", "Georgian"), ("kor", "Korean"), ("lav", "Latvian"), ("lit", "Lithuanian"),
    ("mal", "Malayalam"), ("mar", "Marathi"), ("mon", "Mongolian"), ("nld", "Dutch"),
    ("nor", "Norwegian"), ("pol", "Polish"), ("por", "Portuguese"), ("roh", "Romansh"),
    ("ron", "Romanian"), ("rus", "Russian"), ("slk", "Slovak"), ("spa", "Spanish"),
    ("sqi", "Albanian"), ("swe", "Swedish"), ("tel", "Telugu"), ("tgl", "Tagalog"),
    ("tha", "Thai"), ("tur", "Turkish"), ("ukr", "Ukrainian"), ("urd", "Urdu"),
    ("vie", "Vietnamese"), ("zho", "Chinese"),
];

/// Captured keys that are not parser output (header: ignored keys).
const CAPTURED_IGNORED: &[&str] = &[
    "resolved_languages", "matched_movie_id", "matched_movie_title", "matched_movie_year",
    "custom_formats", "custom_format_score",
];

#[test]
fn son_parse() {
    run("scryer-application::parity_parse::son_parse", App::Sonarr, false, dir_cases("corpus/reference/sonarr/parse"));
}

#[test]
fn rad_parse() {
    run("scryer-application::parity_parse::rad_parse", App::Radarr, false, dir_cases("corpus/reference/radarr/parse"));
}

#[test]
fn son_parse_captured() {
    run(
        "scryer-application::parity_parse::son_parse_captured",
        App::Sonarr,
        true,
        scryer_parity::corpus("corpus/captured/sonarr/data/cases/parse.json"),
    );
}

#[test]
fn rad_parse_captured() {
    run(
        "scryer-application::parity_parse::rad_parse_captured",
        App::Radarr,
        true,
        scryer_parity::corpus("corpus/captured/radarr/data/cases/parse.json"),
    );
}

fn dir_cases(rel: &str) -> Vec<Value> {
    let dir = parity_dir().join(rel);
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .flat_map(|f| scryer_parity::corpus::<Vec<Value>>(&format!("{rel}/{}", f.file_name().unwrap().to_string_lossy())))
        .collect()
}

fn run(name: &str, app: App, captured: bool, cases: Vec<Value>) {
    let Some(mut suite) = Suite::open(name) else { return };
    for case in &cases {
        let id = case["id"].as_str().expect("case id");
        if !suite.wants(id) || unmapped(case).is_some() {
            continue;
        }
        let (pass, detail) = check_case(app, captured, case);
        suite.record(id, pass, detail);
    }
    suite.finish();
}

/// Why a case has no faithful Scryer counterpart (header: unmapped).
fn unmapped(case: &Value) -> Option<&'static str> {
    let input = &case["input"];
    let expect = &case["expect"];
    match input["fn"].as_str().unwrap_or_default() {
        "Parser.ParsePath" | "Parser.ParseMoviePath" => return Some("path parsing is scryer-application filename_parser"),
        "Parser.CleanSeriesTitle" | "Parser.ParseSeriesName" | "Parser.CleanMovieTitle" => {
            return Some("title cleaning is scryer-application title_matching");
        }
        "IsoLanguages.Find" | "LanguageParser.ParseSubtitleLanguage" | "LanguageParser.ParseSubtitleLanguageInformation" => {
            return Some("subtitle/ISO language lookup has no public parser API");
        }
        "Parser.NormalizeImdbId" | "ParsingService.Map" => return Some("application level"),
        "SceneChecker.IsSceneTitle" | "SceneChecker.GetSceneTitle" | "Parser.ToUrlSlug" | "RomanNumeral.ToRomanNumeral"
        | "RomanNumeral.ToInt" | "Parser.NormalizeEpisodeTitle" | "ValidateParsedEpisodeInfo.ValidateForSeriesType"
        | "ParsedEpisodeInfo.IsPossibleSpecialEpisode" => return Some("concept absent in Scryer"),
        _ => {}
    }
    if input.get("name_template").is_some() {
        return Some("needs DateTime.Now");
    }
    if expect.get("is_possible_special_episode").is_some()
        || expect.get("movie_title_clean").is_some()
        || expect["quality"].get("source_detection_source").is_some()
        || expect["series_title_info"].get("title_equals_title_without_year").is_some()
    {
        return Some("concept absent in Scryer");
    }
    None
}

fn check_case(app: App, captured: bool, case: &Value) -> (bool, String) {
    let input = &case["input"];
    let expect = case["expect"].as_object().expect("expect object");
    let compare: BTreeMap<String, String> = case
        .get("compare")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect())
        .unwrap_or_default();
    let names: Vec<&str> = match input.get("names") {
        Some(list) => list.as_array().unwrap().iter().map(|n| n.as_str().unwrap()).collect(),
        None => vec![input["name"].as_str().expect("input name")],
    };
    let func = input["fn"].as_str().unwrap_or("captured");
    // A case asserting only `no_error` (the reference just calls ParseTitle) needs no result.
    let needs_parse = (captured || matches!(func, "Parser.ParseTitle" | "Parser.ParseMovieTitle"))
        && !expect.keys().all(|k| k == "no_error");
    let mut failures = Vec::new();
    for name in &names {
        let mut out = Check::default();
        match catch_unwind(AssertUnwindSafe(|| View::new(name))) {
            Err(_) => out.fail("parser", "no panic", "panicked"),
            Ok(view) if needs_parse && !view.parsed() && !expect.contains_key("should_parse") => {
                out.fail("parse", "a result", "Unparseable");
            }
            Ok(view) => {
                for (key, exp) in expect {
                    if captured && CAPTURED_IGNORED.contains(&key.as_str()) {
                        continue;
                    }
                    view.compare(app, &mut out, key, exp, &compare);
                }
            }
        }
        if !out.0.is_empty() {
            let who = if names.len() > 1 { format!("{name:?}: ") } else { String::new() };
            failures.push(format!("{who}{}", out.0.join("; ")));
        }
    }
    (failures.is_empty(), failures.join(" | "))
}

/// Mismatches of one name, as "key: expected X, got Y".
#[derive(Default)]
struct Check(Vec<String>);

impl Check {
    fn fail(&mut self, key: &str, exp: impl std::fmt::Display, act: impl std::fmt::Display) {
        self.0.push(format!("{key}: expected {exp}, got {act}"));
    }

    /// Compares under the case's mode for `key` (exact when none).
    fn eq(&mut self, key: &str, mode: Option<&str>, exp: &Value, act: Value) {
        let norm = |v: &Value| -> Value {
            match (mode, v) {
                (Some("unordered"), Value::Array(a)) => {
                    let mut a = a.clone();
                    a.sort_by_key(Value::to_string);
                    Value::Array(a)
                }
                (Some("ignore_case"), Value::String(s)) => Value::String(s.to_lowercase()),
                (None | Some("unordered" | "ignore_case"), _) => v.clone(),
                (Some(m), _) => panic!("{key}: unknown compare mode {m}"),
            }
        };
        if norm(exp) != norm(&act) {
            self.fail(key, exp, act);
        }
    }
}

struct View {
    analysis: ReleaseParseAnalysis,
    p: ParsedReleaseMetadata,
}

impl View {
    fn new(name: &str) -> Self {
        Self {
            analysis: scryer_application::neutral_release_analysis(name),
            p: scryer_application::parse_release_metadata(name),
        }
    }

    fn parsed(&self) -> bool {
        self.p.disposition != ParseDisposition::Unparseable
    }

    fn ep(&self) -> ParsedEpisodeMetadata {
        self.p.episode.clone().unwrap_or_default()
    }

    fn titles(&self, primary_only: bool) -> Vec<String> {
        let Some(candidate) = self.analysis.best_candidate() else { return Vec::new() };
        candidate
            .title_segments
            .iter()
            .filter(|s| {
                s.kind == TitleSegmentKind::ObservedPrimary
                    || (!primary_only && s.kind == TitleSegmentKind::ObservedAlternate)
            })
            .map(|s| s.raw.clone())
            .collect()
    }

    fn title(&self) -> Value {
        self.titles(true).into_iter().next().map_or(Value::Null, Value::String)
    }

    /// Sonarr's SeriesTitle with the year as written: Scryer's title plus
    /// ` Y` / ` (Y)` when Scryer parsed year Y and the expected title ends so.
    fn title_with_year(&self, app: App, exp: &Value) -> Value {
        let title = self.title();
        if let (App::Sonarr, Value::String(t), Some(y), Some(e)) = (app, &title, self.p.year, exp.as_str()) {
            for suffix in [format!(" {y}"), format!(" ({y})")] {
                if e.ends_with(&suffix) && !t.ends_with(&suffix) {
                    return json!(format!("{t}{suffix}"));
                }
            }
        }
        title
    }

    fn release_hash(&self) -> Value {
        self.analysis
            .annotations
            .iter()
            .zip(&self.analysis.tokens)
            .find(|(a, _)| a.primary_role == TokenRole::ChecksumOrHash)
            .map_or(Value::Null, |(_, t)| Value::String(t.raw.clone()))
    }

    fn languages(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .p
            .languages_audio
            .iter()
            .map(|code| LANGUAGES.iter().find(|(c, _)| c == code).map_or(code.clone(), |(_, n)| n.to_string()))
            .collect();
        if out.is_empty() {
            out.push("Unknown".into());
        }
        out
    }

    fn compare(&self, app: App, out: &mut Check, key: &str, exp: &Value, modes: &BTreeMap<String, String>) {
        let mode = modes.get(key).map(String::as_str);
        let ep = self.ep();
        let p = &self.p;
        let act = match key {
            "should_parse" => json!(self.parsed()),
            "title" => or_empty(self.title_with_year(app, exp), exp),
            "movie_titles" => json!(self.titles(false)),
            "year" => json!(p.year.unwrap_or(0)),
            "season" => json!(ep.season.unwrap_or(0)),
            "episodes" => json!(ep.episode_numbers),
            "absolute_episodes" => json!(ep.absolute_episode_numbers),
            "special_absolute_episodes" => {
                let as_f64 = |v: &Value| -> Vec<f64> { v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect() };
                let act: Vec<f64> = ep.special_absolute_episode_numbers.iter().map(|&n| f64::from(n)).collect();
                if as_f64(exp) != act {
                    out.fail(key, exp, json!(act));
                }
                return;
            }
            "full_season" => json!(ep.full_season),
            "is_partial_season" => json!(ep.is_partial_season),
            "multi_season" => json!(ep.is_multi_season),
            "is_season_extra" => json!(ep.is_season_extra),
            "is_split_episode" => json!(ep.is_split_episode),
            "season_part" => json!(ep.season_part),
            "daily_part" => json!(ep.daily_part),
            "air_date" => json!(ep.air_date.map(|d| d.format("%Y-%m-%d").to_string())),
            "is_special" => json!(ep.special_kind.is_some()),
            "release_group" => json!(p.release_group),
            "release_hash" => or_empty(self.release_hash(), exp),
            "release_title" => json!(p.raw_title),
            "no_error" => json!(true), // reached only when the parse did not panic
            "edition" => or_empty(json!(p.edition), exp),
            "hardcoded_subs" => {
                if p.is_hardcoded_subs { json!("<hardcoded subs, tag not modelled>") } else { Value::Null }
            }
            "tmdb_id" => match &p.tmdb_id {
                None => json!(0),
                Some(s) => s.parse::<i64>().map_or_else(|_| json!(s), |n| json!(n)),
            },
            "imdb_id" => json!(p.imdb_id),
            "languages" => {
                let mut e: Vec<String> = serde_json::from_value(exp.clone()).unwrap();
                let mut a = self.languages();
                e.sort();
                e.dedup();
                a.sort();
                a.dedup();
                if e != a {
                    out.fail(key, exp, json!(self.languages()));
                }
                return;
            }
            "languages_include" => {
                let a = self.languages();
                if !exp.as_array().unwrap().iter().all(|l| a.iter().any(|x| Some(x.as_str()) == l.as_str())) {
                    out.fail(key, format!("a superset of {exp}"), json!(a));
                }
                return;
            }
            "languages_count" => json!(self.languages().len()),
            "languages_first" => json!(self.languages().first()),
            "series_title_info" => {
                for (sub, e) in exp.as_object().unwrap() {
                    let full = format!("{key}.{sub}");
                    let a = match sub.as_str() {
                        "year" => json!(p.year.unwrap_or(0)),
                        "title" => self.title_with_year(app, e),
                        "title_without_year" => self.title(),
                        "all_titles" => json!(self.titles(false)),
                        other => panic!("series_title_info.{other} not mapped"),
                    };
                    out.eq(&full, modes.get(&full).map(String::as_str), e, a);
                }
                return;
            }
            "quality" => {
                let (e, a) = quality(app, exp.as_object().unwrap(), p);
                out.eq(key, None, &e, a);
                return;
            }
            other => panic!("expect key {other} not mapped"),
        };
        out.eq(key, mode, exp, act);
    }
}

/// Scryer's "none" (null) is the reference's `""` where the reference reports
/// absence as an empty string (SeriesTitle, ReleaseHash, Edition).
fn or_empty(act: Value, exp: &Value) -> Value {
    if act.is_null() && exp == "" { json!("") } else { act }
}

/// (expected, actual) in Scryer terms, only the parts the case asserts.
fn quality(app: App, q: &Map<String, Value>, p: &ParsedReleaseMetadata) -> (Value, Value) {
    let mut e = Map::new();
    let s = |v: &Value| v.as_str().unwrap().to_lowercase();
    if let Some(src) = q.get("source") {
        let src = s(src);
        match app {
            App::Sonarr => {
                let (family, modifier) = match src.as_str() {
                    "television" => (json!("hdtv"), "none"),
                    "televisionraw" => (json!("hdtv"), "rawhd"),
                    "web" => (json!("webdl"), "none"),
                    "webrip" => (json!("webrip"), "none"),
                    "bluray" => (json!("bluray"), "none"),
                    "blurayraw" => (json!("bluray"), "remux"),
                    "dvd" => (json!("dvd"), "none"),
                    "unknown" => (Value::Null, "none"),
                    other => panic!("Sonarr quality source {other} not in the table"),
                };
                e.insert("source".into(), family);
                e.insert("modifier".into(), json!(modifier));
            }
            App::Radarr => {
                let family = match src.as_str() {
                    "tv" => json!("hdtv"),
                    "unknown" => Value::Null,
                    "webdl" | "webrip" | "bluray" | "dvd" | "cam" | "telesync" | "telecine" | "dvdscr" | "workprint" => json!(src),
                    other => panic!("Radarr quality source {other} not in the table"),
                };
                e.insert("source".into(), family);
            }
        }
    } else if let Some(name) = q.get("name") {
        let (family, resolution, modifier) = quality_name(name.as_str().unwrap());
        e.insert("source".into(), family);
        e.insert("resolution".into(), json!(resolution));
        e.insert("modifier".into(), json!(modifier));
    }
    if let Some(r) = q.get("resolution") {
        e.insert("resolution".into(), r.clone());
    }
    if let Some(m) = q.get("modifier") {
        e.insert("modifier".into(), json!(s(m)));
    }
    let version = (p.revision + 1).max(p.anime_version.map_or(1, |v| v as i32));
    let rev = q.get("revision").and_then(Value::as_object);
    match rev.map(|r| (r.get("version").and_then(Value::as_i64), r.get("real").and_then(Value::as_i64))) {
        Some((Some(v), Some(r))) => {
            e.insert("version+real".into(), json!(v + r));
        }
        Some((Some(v), None)) => {
            e.insert("version".into(), json!(v));
        }
        Some((None, Some(r))) => {
            e.insert("real".into(), json!(r));
        }
        _ => {}
    }
    if let Some(r) = rev.and_then(|r| r.get("is_repack")) {
        e.insert("is_repack".into(), r.clone());
    }
    for k in q.keys() {
        assert!(
            matches!(k.as_str(), "name" | "source" | "resolution" | "modifier" | "revision"),
            "quality.{k} not mapped"
        );
    }
    let a: Map<String, Value> = e
        .keys()
        .map(|k| {
            let v = match k.as_str() {
                "source" => match p.source {
                    None => Value::Null,
                    Some(ReleaseSource::BrDisk) => json!("bluray"),
                    Some(src) => json!(src.as_str().to_lowercase().replace('-', "")),
                },
                "resolution" => {
                    let digits: String = p.quality.as_deref().unwrap_or("").chars().take_while(char::is_ascii_digit).collect();
                    json!(digits.parse::<u32>().unwrap_or(0))
                }
                "modifier" => json!(if p.is_remux {
                    "remux"
                } else if p.is_bd_disk || p.source == Some(ReleaseSource::BrDisk) {
                    "brdisk"
                } else {
                    "none"
                }),
                "version+real" | "version" => json!(version),
                "real" => {
                    if p.revision == 0 { json!(0) } else { json!("unknown (REAL folded into revision)") }
                }
                "is_repack" => json!(p.is_repack),
                _ => unreachable!(),
            };
            (k.clone(), v)
        })
        .collect();
    (Value::Object(e), Value::Object(a))
}

/// A reference quality name -> (family, resolution, modifier) in Scryer terms.
fn quality_name(name: &str) -> (Value, u32, &'static str) {
    match name {
        "Unknown" => return (Value::Null, 0, "none"),
        "SDTV" => return (json!("hdtv"), 480, "none"),
        "DVD" => return (json!("dvd"), 480, "none"),
        "DVD-R" => return (json!("dvd"), 480, "remux"),
        "Raw-HD" => return (json!("hdtv"), 1080, "rawhd"),
        "BR-DISK" => return (json!("bluray"), 1080, "brdisk"),
        _ => {}
    }
    let (body, remux) = name.strip_suffix(" Remux").map_or((name, false), |b| (b, true));
    let (src, res) = body.split_once('-').unwrap_or_else(|| panic!("quality name {name} not in the table"));
    let res: u32 = res.strip_suffix('p').and_then(|r| r.parse().ok()).unwrap_or_else(|| panic!("quality name {name}"));
    let (family, modifier) = match src {
        "Bluray" => ("bluray", if remux { "remux" } else { "none" }),
        "Remux" => ("bluray", "remux"),
        "WEBDL" => ("webdl", "none"),
        "WEBRip" => ("webrip", "none"),
        "HDTV" => ("hdtv", "none"),
        _ => panic!("quality name {name} not in the table"),
    };
    (json!(family), res, modifier)
}
