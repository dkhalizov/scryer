//! Parity net, parse area, stream parse_title (Stage 1, D41).
//!
//! Suites (one test fn each):
//! - `son_clean_title`: Parser.CleanSeriesTitle, SON-PARSE-0593..0634 + 0670 (43)
//! - `rad_clean_title`: Parser.CleanMovieTitle, RAD-PARSE-0361..0404 (44)
//! - `son_series_name`: Parser.ParseSeriesName (`series_name_clean`), SON-PARSE-0635..0651 + 1482..1497 (33)
//! - `rad_movie_title_clean`: Parser.ParseMovieTitle (`movie_title_clean`), RAD-PARSE-1096..1110 (15)
//! - `rad_normalize_imdb_id`: Parser.NormalizeImdbId, RAD-PARSE-0510..0515 (6)
//!
//! Scryer functions under test: `parity_seams::canonical_lookup_key` (the
//! application's title lookup key, delegating to scryer_domain
//! `title_lookup_form`), `parity_seams::normalize_imdb_id`, and for the two
//! parse-derived suites the root `View` (`neutral_release_analysis` title
//! segment, `parse_release_metadata` year).
//!
//! Key format decision: Scryer's key is lowercase words separated by single
//! spaces; the reference's clean title runs letters together. Comparing
//! literally would be red for a format reason, so both sides are compared with
//! spaces removed (`strip`). Limitation: word boundaries are not compared, as
//! the reference key has none (`peri.od` -> `peri od` passes against `period`;
//! `word.theword` -> `word theword` and `wordthe.word` -> `wordthe word` both
//! pass against `wordtheword`, although Scryer would not match the two names to
//! each other). Each name of a several-name case is checked on its own, which is
//! what the reference asserts.
//!
//! Anti-vacuity: the Clean suites compare against the RAW expected literal
//! (never passed through Scryer code), so an empty or identity key fails. The
//! parse suites apply the key to both sides like the reference's
//! `Clean(actual) == Clean(expected)`, and fail on an empty expected key, no
//! title, or an empty actual key.
//!
//! Translation table:
//! | Reference | Scryer |
//! |---|---|
//! | `name.CleanSeriesTitle()` / `CleanMovieTitle()` == `clean_title` | `strip(canonical_lookup_key(name)) == clean_title` for every name |
//! | `ParseSeriesName(name).CleanSeriesTitle() == series_name_clean.CleanSeriesTitle()` | `strip(key(title))` == `strip(key(expected))`; Sonarr SeriesTitle keeps the year as written, so when Scryer parsed year Y, Y is not the year of Scryer's daily `air_date`, and Scryer's key does not end in Y, Y is appended. The rule reads only Scryer's parse, never the expected value |
//! | `ParseMovieTitle(name).MovieTitle.CleanMovieTitle() == movie_title_clean.CleanMovieTitle()` (compare `clean_title`) | same with no year rule (MovieTitle excludes the year) |
//! | `NormalizeImdbId(x)` (compare `ignore_case`) | `normalize_imdb_id(x)`, None = null |
//!
//! `title` is the best candidate's first `ObservedPrimary` segment (root
//! `View::title`); the reference falls back to cleaning the whole input when
//! ParseTitle returns null, Scryer has no such fallback, so the harness never
//! uses the raw input as the actual value.
//!
//! Ignored input keys: `facet`, `fn` (selection only). The compare
//! mode `clean_title` is consumed here and never passed to `Check::eq`.
//!
//! Expected reds (decided by adoption): stop words and/or/of/a/an/the kept in
//! Scryer's key; trailing article moved to the front; `%` dropped, not
//! `percent`; diacritics kept in the key (collation equates them, the key does
//! not); `&` rewritten to `and` by the release sanitizer
//! (scryer-release-parser sanitize.rs:21), then kept as a stop word
//! (SON-PARSE-0636); numeric title word dropped from the parsed title
//! (SON-PARSE-0650, `Series.911.S01` -> `Series`); unbracketed site prefixes
//! kept in the parse title; a bracketed site prefix followed by a title with a
//! parenthesised year gives the site name as the title (SON-PARSE-1494,
//! RAD-PARSE-1107, `[www.test-hyphen.ca] - ...` -> `www test`); no 7-digit zero
//! padding of IMDb ids.
//!
//! Unmapped (tests: []), with reasons: none.

use std::panic::{AssertUnwindSafe, catch_unwind};

use chrono::Datelike;

use scryer_application::parity_seams::{canonical_lookup_key, normalize_imdb_id};
use scryer_parity::Suite;
use serde_json::{Value, json};

use super::{Check, View, dir_cases};

fn strip(s: &str) -> String {
    s.replace(' ', "")
}

fn select(dir: &str, f: &str, expect_key: Option<&str>) -> Vec<Value> {
    dir_cases(dir)
        .into_iter()
        .filter(|c| c["input"]["fn"].as_str() == Some(f))
        .filter(|c| expect_key.is_none_or(|k| c["expect"].get(k).is_some()))
        .collect()
}

fn only_expect(case: &Value, key: &str) {
    let obj = case["expect"].as_object().unwrap();
    assert!(obj.len() == 1 && obj.contains_key(key), "{}: unexpected expect keys", case["id"]);
}

fn names(case: &Value) -> Vec<String> {
    match case["input"].get("names") {
        Some(Value::Array(a)) => a.iter().map(|v| v.as_str().unwrap().to_string()).collect(),
        _ => vec![case["input"]["name"].as_str().unwrap().to_string()],
    }
}

fn finish(out: Check) -> (bool, String) {
    (out.0.is_empty(), out.0.join("; "))
}

fn clean_case(case: &Value) -> (bool, String) {
    let expected = case["expect"]["clean_title"].as_str().unwrap();
    let mut out = Check::default();
    for name in names(case) {
        match catch_unwind(AssertUnwindSafe(|| canonical_lookup_key(&name))) {
            Ok(k) if strip(&k) == expected => {}
            Ok(k) => out.fail(&format!("clean_title of {name:?}"), expected, strip(&k)),
            Err(_) => out.fail(&format!("clean_title of {name:?}"), expected, "panic"),
        }
    }
    finish(out)
}

fn name_case(case: &Value, key: &str, sonarr_year: bool) -> (bool, String) {
    let name = case["input"]["name"].as_str().unwrap();
    let expected = case["expect"][key].as_str().unwrap();
    let e = strip(&canonical_lookup_key(expected));
    let mut out = Check::default();
    if e.is_empty() {
        out.fail(key, "non-empty expected key", "expected key empty: comparison would be vacuous");
        return finish(out);
    }
    let Ok(view) = catch_unwind(AssertUnwindSafe(|| View::new(name))) else {
        out.fail(key, &e, "panic");
        return finish(out);
    };
    let Value::String(title) = view.title() else {
        out.fail(key, &e, "no series title");
        return finish(out);
    };
    let mut a = strip(&canonical_lookup_key(&title));
    if a.is_empty() {
        out.fail(key, &e, "empty key for the parsed title");
        return finish(out);
    }
    // Year rule, from Scryer's parse only (never the expected value).
    let daily = view.ep().air_date.map(|d| d.year());
    if let (true, Some(y)) = (sonarr_year, view.p.year.filter(|y| daily != Some(*y))) {
        let y = y.to_string();
        if !a.ends_with(&y) {
            a.push_str(&y);
        }
    }
    if a != e {
        out.fail(key, &e, format!("{a} (title {title:?})"));
    }
    finish(out)
}

fn run(suite_name: &str, cases: Vec<Value>, f: impl Fn(&Value) -> (bool, String)) {
    let Some(mut suite) = Suite::open(suite_name) else { return };
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let (pass, detail) = f(case);
        suite.record(id, pass, &detail);
    }
    suite.finish();
}

#[test]
fn son_clean_title() {
    let cases = select("corpus/reference/sonarr/parse", "Parser.CleanSeriesTitle", None);
    cases.iter().for_each(|c| only_expect(c, "clean_title"));
    run("scryer-application::parity_parse::son_clean_title", cases, clean_case);
}

#[test]
fn rad_clean_title() {
    let cases = select("corpus/reference/radarr/parse", "Parser.CleanMovieTitle", None);
    cases.iter().for_each(|c| only_expect(c, "clean_title"));
    run("scryer-application::parity_parse::rad_clean_title", cases, clean_case);
}

#[test]
fn son_series_name() {
    let cases = select("corpus/reference/sonarr/parse", "Parser.ParseSeriesName", None);
    cases.iter().for_each(|c| only_expect(c, "series_name_clean"));
    run("scryer-application::parity_parse::son_series_name", cases, |c| name_case(c, "series_name_clean", true));
}

#[test]
fn rad_movie_title_clean() {
    let cases = select("corpus/reference/radarr/parse", "Parser.ParseMovieTitle", Some("movie_title_clean"));
    for c in &cases {
        only_expect(c, "movie_title_clean");
        assert_eq!(c["compare"], json!({"movie_title_clean": "clean_title"}), "{}", c["id"]);
    }
    run("scryer-application::parity_parse::rad_movie_title_clean", cases, |c| name_case(c, "movie_title_clean", false));
}

#[test]
fn rad_normalize_imdb_id() {
    let cases = select("corpus/reference/radarr/parse", "Parser.NormalizeImdbId", None);
    cases.iter().for_each(|c| only_expect(c, "imdb_id"));
    run("scryer-application::parity_parse::rad_normalize_imdb_id", cases, |c| {
        let name = c["input"]["name"].as_str().unwrap();
        let mode = c["compare"]["imdb_id"].as_str();
        let act = match catch_unwind(AssertUnwindSafe(|| normalize_imdb_id(name))) {
            Ok(v) => json!(v),
            Err(_) => json!("panic"),
        };
        let mut out = Check::default();
        out.eq("imdb_id", mode, &c["expect"]["imdb_id"], act);
        finish(out)
    });
}
