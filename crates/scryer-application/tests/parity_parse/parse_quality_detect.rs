//! Parity net, parse area, stream parse_quality_detect (Stages 2-5).
//!
//! Suites (one test fn each):
//! - `son_quality_detection_source`: QualityParser.ParseQuality, SON-PARSE-1045..1067 (23)
//! - `rad_quality_detection_source`: QualityParser.ParseQuality, RAD-PARSE-0783..0804 (22)
//!
//! Function under test: `scryer_application::parse_release_metadata` through
//! the root `View` (untargeted, as son_parse/rad_parse). Cases are selected by
//! corpus file, `input.fn` and the presence of a detection key before `wants`.
//!
//! Translation (reference -> Scryer, read from `View.p` of the name):
//! | source_detection_source = Name       | `p.source.is_some()` |
//! | source_detection_source = Unknown    | `p.source.is_none()` |
//! | resolution_detection_source = Name   | `p.quality.is_some()` (a resolution string) |
//! | resolution_detection_source = Unknown| `p.quality.is_none()` |
//! | either = Extension                   | never produced: Scryer has no extension->quality
//! |   table (reference .avi->SDTV, .mkv->HDTV-720p / WEBDL-720p), so red (D29) |
//! Expected is the case's two detection keys as written; actual is built only
//! from the parse. The case asserts no quality value, so `super::quality` is
//! not used. A quality key other than the two detection keys, or any expect key
//! other than `quality`, panics.
//!
//! Ignored input keys: `facet` (production guesses it), `fn` (selection only).
//! If Scryer gains an extension fallback the translation needs an Extension
//! rule and the Extension reds should be revisited.
//!
//! Unmapped (tests: []), with reasons: none in this stream. ParserFixture
//! SON-PARSE-0673/0674 (Parser.ParseTitle detection sources) are not selected
//! here; they are mapped (red, same missing Extension rule) in
//! `parity_parse/parse_absent.rs` (`son_quality_from_extension`).

use std::panic::{AssertUnwindSafe, catch_unwind};

use scryer_parity::Suite;
use serde_json::{Value, json};

use super::View;

const KEYS: [&str; 2] = ["source_detection_source", "resolution_detection_source"];

fn run(suite_name: &str, file: &str) {
    let Some(mut suite) = Suite::open(suite_name) else { return };
    let cases: Vec<Value> = scryer_parity::corpus(file);
    for case in &cases {
        if case["input"]["fn"].as_str() != Some("QualityParser.ParseQuality") {
            continue;
        }
        let Some(q) = case["expect"].get("quality").and_then(Value::as_object) else { continue };
        if !KEYS.iter().any(|k| q.contains_key(*k)) {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        for k in case["expect"].as_object().unwrap().keys() {
            assert_eq!(k, "quality", "{id}: expect key {k} not mapped");
        }
        for k in q.keys() {
            assert!(KEYS.contains(&k.as_str()), "{id}: quality.{k} not mapped");
        }
        let name = case["input"]["name"].as_str().unwrap();
        let Ok(view) = catch_unwind(AssertUnwindSafe(|| View::new(name))) else {
            suite.record(id, false, "parser panicked");
            continue;
        };
        let read = |present: bool| if present { "Name" } else { "Unknown" };
        let full = json!({
            "source_detection_source": read(view.p.source.is_some()),
            "resolution_detection_source": read(view.p.quality.is_some()),
        });
        let actual: serde_json::Map<String, Value> =
            q.keys().map(|k| (k.clone(), full[k].clone())).collect();
        suite.check(id, Value::Object(q.clone()), Value::Object(actual));
    }
    suite.finish();
}

#[test]
fn son_quality_detection_source() {
    run(
        "scryer-application::parity_parse::son_quality_detection_source",
        "corpus/reference/sonarr/parse/QualityParserFixture.json",
    );
}

#[test]
fn rad_quality_detection_source() {
    run(
        "scryer-application::parity_parse::rad_quality_detection_source",
        "corpus/reference/radarr/parse/QualityParserFixture.json",
    );
}
