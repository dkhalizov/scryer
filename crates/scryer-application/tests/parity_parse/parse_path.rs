//! Parity net, parse area, stream parse_path (Stage 1, D41).
//!
//! Suites (one test fn each):
//! - `son_parse_path`: Sonarr `Parser.ParsePath` cases of
//!   `corpus/reference/sonarr/parse/*.json` (SON-PARSE-0152, 0303..0321,
//!   0740..0766, 1153; selected by `input.fn`, 48 cases).
//! - `rad_parse_path`: Radarr `Parser.ParseMoviePath` cases of
//!   `corpus/reference/radarr/parse/*.json` (RAD-PARSE-0112..0122, 11 cases).
//!
//! Function under test: `scryer_application::parity_seams::parse_library_filename`
//! (`library/filename_parser.rs`). Sonarr: mode `TitleScan` with no title or
//! catalogue, the input the series title scan builds (`library/scan/title_scan.rs`
//! ~3168) minus the catalogue. Radarr: `LibraryFilenameParseInput::title_only`
//! (mode `TitleOnly`), as the movie scan calls it (`library/scan/metadata.rs`
//! ~1497). A parser panic fails the case. No expected value reaches the parser.
//!
//! Sonarr path handling: when the corpus path has a parent, it is rebuilt under
//! a per-suite `tempfile::tempdir()` as `<tmp>/<case id>/<path>` and a 1-byte
//! file is written, because the scan's single-video-in-folder fallback reads the
//! directory. A bare file name (no parent) is passed as is and not
//! materialised. Backslashes and `:` are NOT converted (SON-PARSE-1153 is a raw
//! Windows string, one file name on unix). Radarr needs no filesystem.
//!
//! Translation table:
//! | Reference | Scryer |
//! |---|---|
//! | `Parser.ParsePath(path)` | `parse_library_filename`, mode `TitleScan`, `title`/catalogue None |
//! | `Parser.ParseMoviePath(path)` | `parse_library_filename(title_only(path, None))` |
//! | input `facet` (Sonarr) | `LibraryFilenameParseInput.facet`: anime -> Anime, series -> Series, unknown -> None |
//! | Sonarr `title` | `parsed_release.normalized_title`; both sides keyed: upper case, every non-alphanumeric -> space, whitespace collapsed. Year rule: when `parsed_release.year` is Y, Y is not the year of Scryer's daily `air_date`, and the actual key does not end in Y, Y is appended to the actual key (Sonarr keeps the year in SeriesTitle, Scryer splits it). The rule reads only Scryer's parse, never the expected value |
//! | Radarr `title` | `query_evidence.queries.first()` (the movie scan's lookup title, metadata.rs ~1505), same key, no year rule; none = "" |
//! | `season` | `episode_identity.season`, absent or no identity = 0 |
//! | `episodes` | `episode_identity.episode_numbers` (case `compare` mode, `unordered` for 0765/0766) |
//! | `absolute_episodes` | `episode_identity.absolute_episode_numbers` |
//! | `full_season` | `episode_identity.full_season`, no identity = false |
//! | `release_group` | `parsed_release.release_group` (null = None) |
//! | `quality` | root `quality()` helper (Sonarr: source/name/resolution; Radarr: name), see the root header |
//!
//! Radarr notes: `library_root` is None. Production passes the library root
//! for a file representative (metadata.rs ~1493); `build_library_query_evidence`
//! drops the folder only when the parent IS the root, never the case for these
//! paths. The movie scan's identity hint (`select_metadata_identity_hint`,
//! metadata.rs ~405-418) prefers a file or folder title walk (simple
//! `Title (Year)` shape, or one carrying an external id) over the query; the
//! file walk is pushed first into `queries` anyway, so only a folder-walk-only
//! case would diverge, and no case here produces one. TitleOnly's
//! `parsed_release` comes from the file stem alone.
//!
//! HashedRelease reds are scan-path results. The download import deliberately
//! takes the release title from grab evidence, not the folder
//! (`import/workflow/movie.rs` ~33, `series.rs` ~2934), and is not covered here.
//!
//! Ignored input: Radarr `facet` (always movie). Ignored expect keys: none (an
//! unknown key panics). `Path::file_stem` cuts extension-less corpus names at
//! the last `.`; that is production behaviour and is not corrected here.
//!
//! Unmapped (tests: []), with reasons: none.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use chrono::Datelike;

use scryer_application::parity_seams::{
    LibraryFilenameParse, LibraryFilenameParseInput, LibraryFilenameParseMode, parse_library_filename,
};
use scryer_domain::MediaFacet;
use scryer_parity::Suite;
use serde_json::{Value, json};

use super::{App, Check, dir_cases, quality};

fn key(s: &str) -> String {
    let m: String = s.chars().map(|c| if c.is_alphanumeric() { c.to_uppercase().collect::<String>() } else { " ".into() }).collect();
    m.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cases_for(dir: &str, func: &str) -> Vec<Value> {
    dir_cases(dir).into_iter().filter(|c| c["input"]["fn"].as_str() == Some(func)).collect()
}

fn run(
    name: &str,
    app: App,
    cases: Vec<Value>,
    path_for: impl Fn(&Value, &str) -> PathBuf,
    parse: impl Fn(&Value, &Path) -> LibraryFilenameParse,
) {
    let Some(mut suite) = Suite::open(name) else { return };
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let path = case["input"]["name"].as_str().expect("input name");
        // Harness glue (path materialisation) runs outside catch_unwind: a
        // failure there panics the test instead of adopting a red.
        let full = path_for(case, id);
        let mut out = Check::default();
        match catch_unwind(AssertUnwindSafe(|| parse(case, &full))) {
            Err(_) => out.fail("parser", "no panic", "panicked"),
            Ok(p) => compare(app, case, &p, &mut out),
        }
        let detail = if out.0.is_empty() { String::new() } else { format!("{path:?}: {}", out.0.join("; ")) };
        suite.record(id, out.0.is_empty(), detail);
    }
    suite.finish();
}

fn compare(app: App, case: &Value, p: &LibraryFilenameParse, out: &mut Check) {
    let ep = p.episode_identity.clone().unwrap_or_default();
    let rel = &p.parsed_release;
    let cmp = case.get("compare");
    let mode = |k: &str| cmp.and_then(|m| m.get(k)).and_then(Value::as_str);
    for (k, exp) in case["expect"].as_object().expect("expect object") {
        match k.as_str() {
            "title" => {
                let e = key(exp.as_str().unwrap());
                let a = match app {
                    App::Sonarr => {
                        let mut a = key(&rel.normalized_title);
                        // Year rule, from Scryer's parse only (never the expected value).
                        let daily = rel.episode.as_ref().and_then(|e| e.air_date).map(|d| d.year());
                        if let Some(y) = rel.year.filter(|y| daily != Some(*y)) {
                            let ys = y.to_string();
                            if !(a == ys || a.ends_with(&format!(" {ys}"))) {
                                a = format!("{a} {ys}").trim().to_string();
                            }
                        }
                        a
                    }
                    App::Radarr => key(p.query_evidence.queries.first().map(String::as_str).unwrap_or_default()),
                };
                out.eq("title", None, &json!(e), json!(a));
            }
            "season" => out.eq(k, None, exp, json!(ep.season.unwrap_or(0))),
            "episodes" => out.eq(k, mode(k), exp, json!(ep.episode_numbers)),
            "absolute_episodes" => out.eq(k, mode(k), exp, json!(ep.absolute_episode_numbers)),
            "full_season" => out.eq(k, None, exp, json!(ep.full_season)),
            "release_group" => out.eq(k, None, exp, json!(rel.release_group)),
            "quality" => {
                let (e, a) = quality(app, exp.as_object().unwrap(), rel);
                out.eq(k, None, &e, a);
            }
            other => panic!("unmapped expect key {other}"),
        }
    }
}

#[test]
fn son_parse_path() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    run(
        "scryer-application::parity_parse::son_parse_path",
        App::Sonarr,
        cases_for("corpus/reference/sonarr/parse", "Parser.ParsePath"),
        |case, id| {
            let name = case["input"]["name"].as_str().unwrap();
            if !Path::new(name).parent().is_some_and(|p| p.file_name().is_some()) {
                return PathBuf::from(name);
            }
            let full = root.join(id).join(name.trim_start_matches('/'));
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(&full, b"x").unwrap();
            full
        },
        |case, path| {
            let facet = match case["input"]["facet"].as_str().unwrap_or("unknown") {
                "anime" => Some(MediaFacet::Anime),
                "series" => Some(MediaFacet::Series),
                _ => None,
            };
            let mut input = LibraryFilenameParseInput::title_only(path, None);
            input.mode = LibraryFilenameParseMode::TitleScan;
            input.facet = facet.as_ref();
            parse_library_filename(&input)
        },
    );
}

#[test]
fn rad_parse_path() {
    run(
        "scryer-application::parity_parse::rad_parse_path",
        App::Radarr,
        cases_for("corpus/reference/radarr/parse", "Parser.ParseMoviePath"),
        |case, _| PathBuf::from(case["input"]["name"].as_str().unwrap()),
        |_, path| parse_library_filename(&LibraryFilenameParseInput::title_only(path, None)),
    );
}
