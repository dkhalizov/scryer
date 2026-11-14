//! Parity net, parse area, stream parse_sublang (Stages 2-5).
//!
//! Suites (one test fn each), all in `LanguageParserFixture.json`:
//! - `son_subtitle_language`: Sonarr `LanguageParser.ParseSubtitleLanguage`,
//!   SON-PARSE-0342..0353.
//! - `rad_subtitle_language`: Radarr `LanguageParser.ParseSubtitleLanguage`,
//!   RAD-PARSE-0174..0183, 0326..0330.
//! - `son_subtitle_info`: Sonarr `LanguageParser.ParseSubtitleLanguageInformation`,
//!   SON-PARSE-0489..0509.
//! - `rad_subtitle_info`: Radarr `LanguageParser.ParseSubtitleLanguageInformation`,
//!   RAD-PARSE-0340..0360.
//!
//! Function under test: `scryer_application::parity_seams::parse_discovered_external_subtitle`
//! (`subtitles/external.rs`), the library scan's per-file sidecar parse
//! (video stem + sidecar path -> language, forced, hearing_impaired). It is
//! pure and synchronous. The import path copies the raw suffix instead of parsing it.
//!
//! Input glue: the reference parses a bare sidecar name, Scryer reads a sidecar
//! only next to its video. The harness gives each case a video stem from
//! `VIDEO_STEMS` (the fixtures' media names): the longest one that is a plain
//! prefix of the name's file stem. The stem never comes from an expected value.
//! A name whose file stem equals a `VIDEO_STEMS` entry (SON-PARSE-0342, 0508;
//! RAD-PARSE-0330, 0359) is read through the equal-stem branch of
//! `parse_discovered_external_subtitle` (no suffix -> no language); those greens
//! rest on that branch, not on token parsing.
//!
//! Translation table:
//! | Reference | Scryer |
//! |---|---|
//! | input `name` | sidecar path as written (bare file name) |
//! | `expect.subtitle_language` / `subtitle_info.language` (Language name) | `candidate.language` named with the root `LANGUAGES` table (an unlisted code stays the code, so fails); `None` -> "Unknown" |
//! | `subtitle_info.language_tags` | `forced` -> "forced", `hearing_impaired` -> "hearing_impaired". Scryer folds forced/foreign and hi/cc/sdh/hoh into those two flags, so an expected cc/sdh/psdh/foreign fails; it has no "default" tag, so an expected "default" fails (D29: missing value, red) |
//! | `subtitle_info.title`, `.raw_title` | always null: Scryer keeps no subtitle title or copy number (missing field, red under D29). `raw_title: null` is trivially equal, so the 8 `should_not_parse_false_title` cases are decided by language and tags alone |
//! | case `compare` modes (`unordered`, `ignore_case`) | applied per key through `Check::eq` |
//! | call returns `None` (name is not a sidecar of the video) | fails with "<not a sidecar of VIDEO>" (Radarr `Movie-Title-eng-forced.sub`: Scryer needs "<stem>.") |
//!
//! The Scryer object is built in full before the expected keys are read; an
//! expected key outside {language, language_tags, title, raw_title} panics.
//!
//! Ignored keys: input `facet`, input `fn` (selection only), case `note`.
//!
//! Unmapped (tests: []), with reasons: none.

use scryer_application::parity_seams::parse_discovered_external_subtitle;
use scryer_parity::Suite;
use serde_json::{Value, json};

use super::{Check, LANGUAGES};

const VIDEO_STEMS: &[&str] = &[
    "Series Title - S01E01 - Pilot",
    "Movie Title - 2022",
    "Movie Title",
    "Movie.Title",
    "Movie-Title",
    "Name (2020) - S01E20 - [AAC 2.0]",
    "Name (2020) - [AAC 2.0]",
];

const LANG_FN: &str = "LanguageParser.ParseSubtitleLanguage";
const INFO_FN: &str = "LanguageParser.ParseSubtitleLanguageInformation";

/// (language name, forced, hearing_impaired) or the video stem it is not a sidecar of.
fn scryer(name: &str) -> Result<(String, bool, bool), String> {
    let stem = std::path::Path::new(name).file_stem().unwrap().to_str().unwrap();
    let video = VIDEO_STEMS
        .iter()
        .filter(|v| stem.starts_with(**v))
        .max_by_key(|v| v.len())
        .unwrap_or_else(|| panic!("no VIDEO_STEMS entry for {name}"));
    let c = parse_discovered_external_subtitle(video, std::path::Path::new(name))
        .ok_or_else(|| format!("<not a sidecar of {video}>"))?;
    let lang = match c.language {
        None => "Unknown".to_string(),
        Some(code) => LANGUAGES.iter().find(|(c, _)| *c == code).map_or(code.clone(), |(_, n)| n.to_string()),
    };
    Ok((lang, c.forced, c.hearing_impaired))
}

fn run(suite_name: &str, file: &str, func: &str) {
    let Some(mut suite) = Suite::open(suite_name) else { return };
    let cases: Vec<Value> = scryer_parity::corpus(file);
    for case in &cases {
        if case["input"]["fn"].as_str() != Some(func) {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let name = case["input"]["name"].as_str().unwrap();
        let actual = scryer(name);
        if func == LANG_FN {
            let exp = case["expect"]["subtitle_language"].clone();
            let act = match actual {
                Ok((l, ..)) => Value::String(l),
                Err(e) => Value::String(e),
            };
            suite.check(id, exp, act);
            continue;
        }
        let mut out = Check::default();
        match actual {
            Err(e) => out.0.push(e),
            Ok((lang, forced, hi)) => {
                let mut tags = vec![];
                if forced {
                    tags.push("forced");
                }
                if hi {
                    tags.push("hearing_impaired");
                }
                let full = json!({"language": lang, "language_tags": tags, "title": null, "raw_title": null});
                for (k, exp) in case["expect"]["subtitle_info"].as_object().unwrap() {
                    let act = full.get(k).unwrap_or_else(|| panic!("{id}: unknown key {k}")).clone();
                    let key = format!("subtitle_info.{k}");
                    out.eq(&key, case["compare"][&key].as_str(), exp, act);
                }
            }
        }
        suite.record(id, out.0.is_empty(), &out.0.join("; "));
    }
    suite.finish();
}

const SON: &str = "corpus/reference/sonarr/parse/LanguageParserFixture.json";
const RAD: &str = "corpus/reference/radarr/parse/LanguageParserFixture.json";

#[test]
fn son_subtitle_language() {
    run("scryer-application::parity_parse::son_subtitle_language", SON, LANG_FN);
}

#[test]
fn rad_subtitle_language() {
    run("scryer-application::parity_parse::rad_subtitle_language", RAD, LANG_FN);
}

#[test]
fn son_subtitle_info() {
    run("scryer-application::parity_parse::son_subtitle_info", SON, INFO_FN);
}

#[test]
fn rad_subtitle_info() {
    run("scryer-application::parity_parse::rad_subtitle_info", RAD, INFO_FN);
}
