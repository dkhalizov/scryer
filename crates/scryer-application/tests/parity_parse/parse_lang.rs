//! Parity net, parse area, stream parse_lang (Stage 1, D41).
//!
//! Suites (one test fn each):
//! - `son_iso_languages`: `corpus/reference/sonarr/parse/IsoLanguagesFixture.json`,
//!   SON-PARSE-0329..0341 (`IsoLanguages.Find`).
//! - `rad_iso_languages`: `corpus/reference/radarr/parse/IsoLanguagesFixture.json`,
//!   RAD-PARSE-0123..0165 (`IsoLanguages.Find`).
//!
//! Function under test: `scryer_application::normalize_detected_audio_language_code`
//! (`media/language.rs`, pub at the crate root). It is Scryer's shared ISO 639
//! lookup (code, bibliographic code, two-letter code, name; release-token table
//! fallback; primary-subtag fallback; unknown 3-letter passthrough). Real callers
//! match the reference callers of `IsoLanguages.Find`: media-info audio languages
//! (`media_file_store.rs`, reference AugmentLanguageFromMediaInfo) and metadata
//! original language (`catalog/facets/handler.rs`, reference SkyHook).
//! Why not the D41 seam `normalize_language_token` in scryer-release-parser: that
//! table reads languages from release titles (32 languages, upper-case tokens, no
//! region-subtag fallback), so it is the wrong counterpart and would turn cases red for
//! the wrong reason.
//!
//! Translation table:
//! | Reference | Scryer |
//! |---|---|
//! | input `name` | argument, as written |
//! | `expect.language` (Language name) | `Some(code)` named with the root `LANGUAGES` table; a code not listed stays the code (fails) |
//! | `expect.language: null`, Radarr `expect.result: null` | `None` -> `Value::Null` |
//!
//! Ignored keys: input `facet` (always "unknown"), input `fn` (case selection
//! only), Radarr case `note`. A missing `language` and `result` key panics.
//!
//! Expected reds (Scryer knows more or falls back more loosely): `wo` (wol),
//! `fr-CA` (frc), `de-AU` (German via primary subtag), Radarr `khk`/`mvf`
//! (unknown 3-letter codes pass through unchanged).
//!
//! Unmapped (tests: []), with reasons: none. LanguageParser.ParseSubtitleLanguage*
//! stays in the root list (later stage).

use scryer_parity::Suite;
use serde_json::Value;

use super::LANGUAGES;

fn run(suite_name: &str, file: &str) {
    let Some(mut suite) = Suite::open(suite_name) else { return };
    let cases: Vec<Value> = scryer_parity::corpus(file);
    for case in &cases {
        if case["input"]["fn"].as_str() != Some("IsoLanguages.Find") {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let expect = &case["expect"];
        let expected = match expect.get("language") {
            Some(v) => v.clone(),
            None => {
                let r = expect.get("result").unwrap_or_else(|| panic!("{id}: no language/result"));
                assert!(r.is_null(), "{id}: result must be null");
                Value::Null
            }
        };
        let name = case["input"]["name"].as_str().unwrap();
        let actual = match scryer_application::normalize_detected_audio_language_code(name) {
            None => Value::Null,
            Some(code) => Value::String(
                LANGUAGES.iter().find(|(c, _)| *c == code).map_or(code.clone(), |(_, n)| n.to_string()),
            ),
        };
        suite.check(id, expected, actual);
    }
    suite.finish();
}

#[test]
fn son_iso_languages() {
    run(
        "scryer-application::parity_parse::son_iso_languages",
        "corpus/reference/sonarr/parse/IsoLanguagesFixture.json",
    );
}

#[test]
fn rad_iso_languages() {
    run(
        "scryer-application::parity_parse::rad_iso_languages",
        "corpus/reference/radarr/parse/IsoLanguagesFixture.json",
    );
}
