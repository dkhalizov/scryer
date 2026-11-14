//! Parity net, area `quality` (notes/11-parity-net-design.md). Six suites:
//!
//! | suite                    | cases                                                    |
//! |--------------------------|----------------------------------------------------------|
//! | son_quality / rad_quality | reference ports, `corpus/reference/<app>/quality/*.json` (0001..) |
//! | son_quality_config / rad_quality_config | captured config facts (5001..), checked from the ref'd `corpus/captured/<app>/config/*.json` |
//! | son_quality_captured / rad_quality_captured | quality each real file was assigned (6001..), `corpus/captured/<app>/data/cases/quality.json` |
//!
//! # Translation tables
//!
//! Sonarr/Radarr quality name -> Scryer `(source, is_remux, resolution lines)`
//! (`QUALITIES` below). Scryer has no closed quality catalogue: a file's
//! quality is its parsed `ReleaseSource` (`ReleaseSource::as_str`), the
//! parser's `is_remux`, and the resolution tier from
//! `derive_primary_quality_label` (media dimensions first, the parsed label as
//! fallback). Television maps to `HDTV`, web to `WEB-DL`/`WEBRip`, Bluray to
//! `BluRay`, `Bluray-<n>p Remux` / `Remux-<n>p` to `BluRay` + remux, Radarr
//! `DVD-R` (DVD + REMUX modifier) to `DVD` + remux, `BR-DISK` to `BRDISK`.
//! `Unknown`, `Raw-HD` and Radarr `REGIONAL` have no Scryer equivalent: a
//! case expecting one fails (red) with that reason.
//!
//! Scryer resolution label -> lines: `"4K"` is 2160 (Scryer's spelling of
//! 2160p, types.rs:4220), otherwise the number before a trailing `p`.
//!
//! Revision: Sonarr/Radarr `{version, real, is_repack}` -> Scryer
//! `(revision, is_repack)` with `revision = version - 1 + real`. Scryer's
//! parser folds PROPER/REPACK/REAL into one count where 0 is an original
//! (scryer-release-parser enrichment.rs:47-123: PROPER is 1, REPACK2 is 2,
//! each adjacent REAL adds 1).
//!
//! Language (custom-format language cases): English/French/Spanish ->
//! Sonarr/Radarr language ids 1/2/3 (the spec value a real export carries)
//! and ISO 639-2 `eng`/`fra`/`spa` on the release; `Original` is id -2.
//!
//! # How each mapped group runs Scryer
//!
//! - `format_matches` (0001-0016): the spec is pasted as a one-format custom
//!   format into `arr_custom_format::translate_arr_custom_formats` (Scryer's
//!   arr import), the resulting Rego is loaded into `scryer_rules`'
//!   `UserRulesEngine` and evaluated on a release whose `languages_audio` and
//!   `context.original_language` carry the case's languages. Matches means
//!   the format's score entry fired. A disabled translation fails the case.
//! - Config custom-format definitions (SON 5032-5055, RAD 5037-5061): the
//!   whole captured `customformat.json` is pasted into Scryer's import (every
//!   format scored 1), the Rego is loaded into `UserRulesEngine`, and every
//!   title of a sample of the captured `data/parse.json` (the arr's parse
//!   API, which lists the formats the arr matched on that title) is
//!   evaluated: every k-th title (about 40) plus each format's first 3
//!   matched titles, 67 of 1269 Sonarr, all 23 Radarr (`verdict_sample`; the
//!   whole set costs ~40 CPU-min in a debug build). Green needs status
//!   Translated and Scryer's rule to fire on exactly the sampled titles the
//!   arr matched it on. Entries are taken as the live scorer takes them, rule
//!   errors or not (quality/canonical.rs `append_rule_scores`): a translated
//!   ReleaseGroup spec errors on a release with no parsed group (`regex.match`
//!   on null), which empties every format's entries for that title, so the
//!   reds are mostly that one cause. The 1 s rule time budget is lifted
//!   (not a parity behaviour; a loaded debug build would trip it). The release carries
//!   `raw_title` and `release_group` from Scryer's parse, the two fields the
//!   translated title/group specs read (translate.rs:595-624), set as
//!   `build_rule_input` sets them (user_rule_input.rs:141,183); a format with
//!   another spec type fails (none in the captured config). "Renaming includes it: on" fails: Scryer's renamer has no
//!   custom-format token (all captured formats are off).
//! - Config custom-format scores (SON 5008-5031): Scryer's side is the score
//!   its import derives for the format from the captured export
//!   (`suggested_scores`, from a `score` / `trash_scores` key). Scryer's arr
//!   import never reads a profile's `formatItems` (external_import.rs:574
//!   keeps id, name, language) and Scryer rules carry one score per rule, not
//!   per profile, so these are red.
//! - Captured files (6001..): Scryer parses the scene name the arr stored
//!   (the file name when there is none) and derives the resolution from the
//!   stored media dimensions, the same inputs Scryer's import and scan use.
//!   Caveat: 193 of the 363 Sonarr files have no scene name, so the input is
//!   Sonarr's renamed file name, which carries Sonarr's own quality token
//!   (`Show - S01E02 - Title Bluray-1080p.mkv`). Those cases show Scryer
//!   reads arr quality tokens back from a Sonarr-named library (scan
//!   migration), not that it assigns quality from a release. All 6 Radarr
//!   files have a scene name. `episodefile.json` has no original file path.
//!
//! # Unmapped (tests: []) and why
//!
//! Reference (both apps):
//! - `quality_declaration`, `all_qualities`, `default_definition`, `from_id`,
//!   `to_id`, `find_by_source_and_resolution`, `definitions_init` /
//!   `definition_init`: Sonarr/Radarr's closed quality catalogue (numeric
//!   ids, weights, per-quality size definitions, source+resolution lookup).
//!   Scryer has no such catalogue: quality is an open (source, resolution)
//!   pair parsed per release. Concept absent.
//! - `quality_index_compare`: `QualityIndex` with quality-group indices.
//!   Scryer profiles have no quality groups (a flat tier list). Concept absent.
//! - `revision_compare`: Scryer has no Revision value type; the folded i32 is
//!   compared inside the pub(crate) decision pipeline
//!   (acquisition/decision_pipeline.rs). Comparing two translated ints in the
//!   harness would test nothing of Scryer. Revision ordering is exercised by
//!   the decision suites.
//! - `compare` (QualityModelComparer, SON 0078-0083, RAD 0085-0090): Scryer's
//!   tier comparison `quality_meets_or_exceeds_cutoff` (quality/profile.rs:993)
//!   is `pub` inside the pub(crate) `quality` module and not re-exported, so an
//!   integration test cannot call it. Needs seam (see below). Note 0083/0090
//!   (`respect_group_order`) would stay red: no groups in Scryer.
//! - `default_profiles`: Scryer's built-in profiles are code constants
//!   (quality/profile.rs:536-683), never seeded into a store. Concept absent.
//! - `profile_delete`: Scryer has the guard
//!   (settings/runtime/quality_profiles.rs:841 `ensure_quality_profiles_are_unreferenced`)
//!   but only behind a full `AppUseCase` over stores; the in-crate fixtures
//!   (lib_tests) are not reachable from an integration test.
//! - `delay_profile_reorder`: Scryer delay profiles carry an explicit
//!   `priority` (acquisition/delay_profile.rs:121); there is no move-after
//!   reorder operation. Concept absent.
//! - `acceptable_languages` (RAD 0029-0030): Radarr derives a profile's
//!   acceptable language set from its language plus language-format scores.
//!   Scryer profiles have no profile language; `required_audio_languages` is
//!   a filter, not derived from format scores. Concept absent.
//!
//! Config:
//! - Custom-format definitions the arr matched on no sampled parse title
//!   while Scryer matches none either: no positive verdict, so matching
//!   would go untested and a green would be vacuous (a Scryer false positive
//!   records red instead). SON 5032 5035 5036 5040 5041 5043 5044 5046 5049
//!   5051 5052; RAD 5037 5039-5045 5047 5049 5051 5052 5054-5058.
//! - Profile definitions (SON 5007, RAD 5002, RAD 5005): Scryer's arr import
//!   reads only a profile's id, name and language (external_import.rs:574);
//!   nothing converts arr quality items, groups, cutoff or upgrade flag into a
//!   Scryer `QualityProfile`, so there is no Scryer output to compare. A
//!   hand-built Scryer profile would test the harness. Open decision.
//! - Quality definitions (RAD 5025-5034, MB per minute bounds): Scryer has no
//!   per-quality size table; size plausibility comes from a bitrate model
//!   (quality/profile.rs:1217). Concept absent.
//!
//! # Ignored keys
//!
//! - Captured Sonarr `media_info`: `codec`, `dynamic_range`, `audio_*`,
//!   `subtitles`, `run_time` (not part of the assigned quality).
//! - Captured Radarr `given.release_group`, `given.video_codec`.
//! - `expect.source` / `expect.modifier`: implied by the quality name, which
//!   is what is translated; `expect.resolution` is asserted equal to the
//!   table's lines (a harness-table check that panics, not a case outcome).
//! - Reference `note` fields (provenance only).
//!
//! # Seams
//!
//! None used (`PolicyEngine::build_with_limits` and `RuntimeLimits` are
//! already pub). Needed to map `compare`: re-export
//! `quality_meets_or_exceeds_cutoff` from crate root (lib.rs:759 `pub use
//! quality_profile::{...}`).

use scryer_application::arr_custom_format::{
    ArrCustomFormatTranslation, ArrFormatStatus, ArrSourceApplication,
    translate_arr_custom_formats,
};
use scryer_application::{derive_primary_quality_label, parse_release_metadata};
use scryer_parity::{Catalogue, Suite, corpus};
use scryer_rules::{
    BuiltinScoreDoc, ContextDoc, PolicyOrigin, ProfileDoc, ReleaseDoc, ScoringOverridesDoc,
    UserPolicy, UserRuleInput, UserRulesEngine, rewrite_package_declaration,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Debug;

/// Arr quality name -> (Scryer `ReleaseSource::as_str`, is_remux, lines).
/// `None` source: no Scryer equivalent.
const QUALITIES: &[(&str, Option<&str>, bool, Option<u32>)] = &[
    ("SDTV", Some("HDTV"), false, Some(480)),
    ("DVD", Some("DVD"), false, Some(480)),
    ("DVD-R", Some("DVD"), true, Some(480)),
    ("WEBDL-480p", Some("WEB-DL"), false, Some(480)),
    ("WEBRip-480p", Some("WEBRip"), false, Some(480)),
    ("Bluray-480p", Some("BluRay"), false, Some(480)),
    ("Bluray-576p", Some("BluRay"), false, Some(576)),
    ("HDTV-720p", Some("HDTV"), false, Some(720)),
    ("WEBDL-720p", Some("WEB-DL"), false, Some(720)),
    ("WEBRip-720p", Some("WEBRip"), false, Some(720)),
    ("Bluray-720p", Some("BluRay"), false, Some(720)),
    ("HDTV-1080p", Some("HDTV"), false, Some(1080)),
    ("WEBDL-1080p", Some("WEB-DL"), false, Some(1080)),
    ("WEBRip-1080p", Some("WEBRip"), false, Some(1080)),
    ("Bluray-1080p", Some("BluRay"), false, Some(1080)),
    ("Bluray-1080p Remux", Some("BluRay"), true, Some(1080)),
    ("Remux-1080p", Some("BluRay"), true, Some(1080)),
    ("HDTV-2160p", Some("HDTV"), false, Some(2160)),
    ("WEBDL-2160p", Some("WEB-DL"), false, Some(2160)),
    ("WEBRip-2160p", Some("WEBRip"), false, Some(2160)),
    ("Bluray-2160p", Some("BluRay"), false, Some(2160)),
    ("Bluray-2160p Remux", Some("BluRay"), true, Some(2160)),
    ("Remux-2160p", Some("BluRay"), true, Some(2160)),
    ("BR-DISK", Some("BRDISK"), false, Some(1080)),
    ("CAM", Some("CAM"), false, None),
    ("TELESYNC", Some("TELESYNC"), false, None),
    ("TELECINE", Some("TELECINE"), false, None),
    ("WORKPRINT", Some("WORKPRINT"), false, None),
    ("DVDSCR", Some("DVDSCR"), false, None),
    ("Unknown", None, false, None),
    ("Raw-HD", None, false, Some(1080)),
    ("REGIONAL", None, false, Some(480)),
];

/// Language name -> (arr language id, ISO 639-2 code on the release).
const LANGUAGES: &[(&str, i64, &str)] = &[
    ("English", 1, "eng"),
    ("French", 2, "fra"),
    ("Spanish", 3, "spa"),
    ("Original", -2, ""),
];

#[derive(Debug, PartialEq)]
struct FileQuality {
    source: Option<String>,
    remux: bool,
    lines: Option<u32>,
    revision: i64,
    repack: bool,
}

fn record<T: PartialEq + Debug>(suite: &mut Suite, id: &str, expected: T, actual: T, context: &str) {
    let pass = expected == actual;
    let detail = if pass { String::new() } else { format!("expected {expected:?}, got {actual:?}; {context}") };
    suite.record(id, pass, detail);
}

// ── Captured file qualities ─────────────────────────────────────────────────

/// "4K" is Scryer's label for 2160p; otherwise the number before `p`.
fn label_lines(label: &str) -> Option<u32> {
    let label = label.trim();
    if label.eq_ignore_ascii_case("4k") {
        return Some(2160);
    }
    label.strip_suffix(['p', 'P'])?.parse().ok()
}

fn scryer_file_quality(title: &str, width: Option<i64>, height: Option<i64>) -> FileQuality {
    let parsed = parse_release_metadata(title);
    let dim = |v: Option<i64>| v.and_then(|v| i32::try_from(v).ok());
    let label = derive_primary_quality_label(dim(width), dim(height), parsed.quality.as_deref(), None);
    FileQuality {
        source: parsed.source.as_ref().map(|s| s.as_str().to_string()),
        remux: parsed.is_remux,
        lines: label.as_deref().and_then(label_lines),
        revision: i64::from(parsed.revision),
        repack: parsed.is_repack,
    }
}

/// The arr's stored quality in Scryer's terms, or why it has none.
fn arr_file_quality(name: &str, resolution: i64, revision: &Value) -> Result<FileQuality, String> {
    let (_, source, remux, lines) = QUALITIES
        .iter()
        .find(|(n, ..)| *n == name)
        .unwrap_or_else(|| panic!("quality `{name}` missing from QUALITIES"));
    assert_eq!(
        i64::from(lines.unwrap_or(0)),
        resolution,
        "QUALITIES disagrees with the corpus on `{name}`"
    );
    let source = source.ok_or_else(|| format!("arr quality `{name}` has no Scryer equivalent"))?;
    Ok(FileQuality {
        source: Some(source.to_string()),
        remux: *remux,
        lines: *lines,
        revision: revision["version"].as_i64().unwrap() - 1 + revision["real"].as_i64().unwrap(),
        repack: revision["is_repack"].as_bool().unwrap(),
    })
}

fn record_file(suite: &mut Suite, id: &str, expected: Result<FileQuality, String>, actual: FileQuality, title: &str) {
    match expected {
        Ok(expected) => record(suite, id, expected, actual, &format!("parsed `{title}`")),
        Err(why) => suite.record(id, false, why),
    }
}

/// The arr stored the scene name when it had one; otherwise the file name.
fn import_title<'a>(scene_name: &'a Value, relative_path: &'a Value) -> &'a str {
    scene_name
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| relative_path.as_str().unwrap().rsplit('/').next().unwrap())
}

#[test]
fn son_quality_captured() {
    let Some(mut suite) = Suite::open("scryer-application::parity_quality::son_quality_captured") else { return };
    let cases: Vec<Value> = corpus("corpus/captured/sonarr/data/cases/quality.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let input = &case["input"];
        let title = import_title(&input["scene_name"], &input["relative_path"]);
        let media = &input["media_info"];
        let actual = scryer_file_quality(title, media["width"].as_i64(), media["height"].as_i64());
        let expect = &case["expect"];
        let expected = arr_file_quality(
            expect["quality"].as_str().unwrap(),
            expect["resolution"].as_i64().unwrap(),
            &expect["revision"],
        );
        record_file(&mut suite, id, expected, actual, title);
    }
    suite.finish();
}

#[test]
fn rad_quality_captured() {
    let Some(mut suite) = Suite::open("scryer-application::parity_quality::rad_quality_captured") else { return };
    let cases: Vec<Value> = corpus("corpus/captured/radarr/data/cases/quality.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let given = &case["input"]["given"];
        let title = import_title(&given["scene_name"], &given["relative_path"]);
        let (width, height) = given["resolution"]
            .as_str()
            .and_then(|r| r.split_once('x'))
            .map(|(w, h)| (w.trim().parse().ok(), h.trim().parse().ok()))
            .unwrap_or((None, None));
        let actual = scryer_file_quality(title, width, height);
        let expect = &case["expect"]["quality"];
        let expected = arr_file_quality(
            expect["name"].as_str().unwrap(),
            expect["resolution"].as_i64().unwrap(),
            &expect["revision"],
        );
        record_file(&mut suite, id, expected, actual, title);
    }
    suite.finish();
}

// ── Custom formats through Scryer's arr import ──────────────────────────────

/// Translates a one-format paste the way the import UI does: review to learn
/// the format id, then translate with a score for it.
fn translate_one(
    source: ArrSourceApplication,
    paste: &str,
    score: i32,
) -> Result<ArrCustomFormatTranslation, String> {
    let review = translate_arr_custom_formats(source, paste, &HashMap::new())
        .map_err(|e| format!("import refused: {e:?}"))?;
    let format = review
        .formats
        .first()
        .ok_or_else(|| format!("format not readable: {:?}", review.inspection_diagnostics))?;
    let scores = HashMap::from([(format.id.clone(), score)]);
    let translation = translate_arr_custom_formats(source, paste, &scores)
        .map_err(|e| format!("import refused: {e:?}"))?;
    if translation.formats.is_empty() {
        return Err(format!("format not readable: {:?}", translation.inspection_diagnostics));
    }
    Ok(translation)
}

fn release_input(
    raw_title: String,
    release_group: Option<String>,
    languages: Vec<String>,
    original: &str,
    facet: &str,
) -> UserRuleInput {
    UserRuleInput {
        release: ReleaseDoc {
            protocol: None,
            stereoscopy_tokens: vec![],
            stereoscopy: None,
            raw_title,
            normalized_tokens: vec![],
            quality: None,
            source: None,
            video_codec: None,
            audio: None,
            audio_codecs: vec![],
            audio_channels: None,
            languages_audio: languages,
            indexer_languages: vec![],
            languages_subtitles: vec![],
            is_dual_audio: false,
            is_atmos: false,
            is_dolby_vision: false,
            has_hdr_fallback: false,
            detected_hdr: false,
            is_remux: false,
            is_bd_disk: false,
            is_proper_upload: false,
            is_repack: false,
            revision: 0,
            is_ai_enhanced: false,
            is_hardcoded_subs: false,
            is_password_protected: None,
            is_hdr10plus: false,
            is_hlg: false,
            is_10bit: false,
            is_uncensored: false,
            is_dubs_only: false,
            has_release_group: false,
            is_obfuscated: false,
            is_retagged: false,
            streaming_service: None,
            edition: None,
            anime_version: None,
            episode_release_type: None,
            is_season_pack: false,
            is_multi_episode: false,
            release_group,
            year: None,
            parse_confidence: 1.0,
            size_bytes: None,
            age_days: None,
            thumbs_up: None,
            thumbs_down: None,
            extra: HashMap::new(),
        },
        profile: ProfileDoc {
            id: "parity".to_string(),
            name: "Parity".to_string(),
            quality_tiers: vec![],
            archival_quality: None,
            allow_unknown_quality: true,
            source_allowlist: vec![],
            source_blocklist: vec![],
            video_codec_allowlist: vec![],
            video_codec_blocklist: vec![],
            audio_codec_allowlist: vec![],
            audio_codec_blocklist: vec![],
            atmos_preferred: false,
            dolby_vision_allowed: true,
            detected_hdr_allowed: true,
            prefer_remux: false,
            allow_bd_disk: false,
            allow_upgrades: true,
            prefer_dual_audio: false,
            required_audio_languages: vec![],
            scoring_persona: "balanced".to_string(),
            scoring_overrides: ScoringOverridesDoc::default(),
        },
        context: ContextDoc {
            title_id: None,
            library_name: None,
            media_type: facet.to_string(),
            category: facet.to_string(),
            original_language: Some(original.to_string()),
            original_country: None,
            inferred_original_audio_language: original.to_string(),
            tags: vec![],
            has_existing_file: false,
            existing_score: None,
            search_mode: "canonical".to_string(),
            runtime_minutes: None,
            coverage_total_runtime_minutes: None,
            coverage_member_runtime_minutes: None,
            coverage_member_count: None,
            is_anime: false,
            is_filler: false,
        },
        builtin_score: BuiltinScoreDoc { total: 0, blocked: false, codes: vec![] },
        file: None,
    }
}

fn language(name: &str) -> Result<(i64, &'static str), String> {
    LANGUAGES
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|(_, id, iso)| (*id, *iso))
        .ok_or_else(|| format!("language `{name}` missing from LANGUAGES"))
}

fn facet(source: ArrSourceApplication) -> &'static str {
    match source {
        ArrSourceApplication::Sonarr => "series",
        ArrSourceApplication::Radarr => "movie",
    }
}

/// Scryer's rules engine with the imported Rego as one user rule set.
fn rules_engine(translation: &ArrCustomFormatTranslation) -> Result<UserRulesEngine, String> {
    let policy = UserPolicy {
        id: "parity_cf".to_string(),
        name: "Parity import".to_string(),
        rego_source: rewrite_package_declaration(&translation.rego_source, "parity_cf"),
        origin: PolicyOrigin::User,
        applied_facets: translation.applied_facets.clone(),
    };
    // The 1 s wall-clock budget guards production from runaway rules; it is
    // not a parity behaviour, and a debug build under load would trip it.
    let mut limits = scryer_rules::runtime::RuntimeLimits::release_defaults();
    limits.max_execution_time = std::time::Duration::from_secs(3600);
    UserRulesEngine::build_with_limits(&[policy], limits).map_err(|e| e.to_string())
}

/// Whether Scryer's imported language format matches the case's release.
fn language_format_matches(source: ArrSourceApplication, given: &Value) -> Result<bool, String> {
    let spec = &given["spec"];
    let release = &given["release"];
    let (value, _) = language(spec["value"].as_str().unwrap())?;
    let languages = release["languages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| language(l.as_str().unwrap()).map(|(_, iso)| iso.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let original_name = release["series_original_language"]
        .as_str()
        .or_else(|| release["movie_original_language"].as_str())
        .unwrap();
    let (_, original) = language(original_name)?;
    let paste = json!({
        "id": 1,
        "name": "Parity language",
        "specifications": [{
            "name": "Language",
            "implementation": spec["implementation"],
            "negate": spec["negate"],
            "required": false,
            "fields": { "value": value, "exceptLanguage": spec["except_language"] },
        }],
    })
    .to_string();
    let translation = translate_one(source, &paste, 1)?;
    let format = &translation.formats[0];
    if format.status != ArrFormatStatus::Translated {
        return Err(format!("format disabled: {:?}", format.diagnostics));
    }
    let facet = facet(source);
    let engine = rules_engine(&translation)?;
    let mut evaluator = engine.evaluator();
    let input = release_input("Parity.Language.Case".to_string(), None, languages, original, facet);
    let result = evaluator
        .evaluate(&input, facet)
        .map_err(|e| e.to_string())?;
    if !result.errors.is_empty() {
        let messages: Vec<_> = result.errors.iter().map(|e| e.message.as_str()).collect();
        return Err(format!("rule errors: {messages:?}"));
    }
    Ok(!result.entries.is_empty())
}

fn reference(suite_name: &str, app: &str, source: ArrSourceApplication) {
    let Some(mut suite) = Suite::open(suite_name) else { return };
    // Only this file holds a mapped op; see the header for the rest.
    let cases: Vec<Value> = corpus(&format!("corpus/reference/{app}/quality/custom_format_language.json"));
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) || case["input"]["op"] != "format_matches" {
            continue;
        }
        let expected = case["expect"]["matches"].as_bool().unwrap();
        match language_format_matches(source, &case["input"]["given"]) {
            Ok(actual) => record(&mut suite, id, expected, actual, "format_matches"),
            Err(why) => suite.record(id, false, why),
        }
    }
    suite.finish();
}

#[test]
fn son_quality() {
    reference("scryer-application::parity_quality::son_quality", "sonarr", ArrSourceApplication::Sonarr);
}

#[test]
fn rad_quality() {
    reference("scryer-application::parity_quality::rad_quality", "radarr", ArrSourceApplication::Radarr);
}

// ── Config facts ────────────────────────────────────────────────────────────

/// `key=N` in a catalogue ref fragment.
fn fragment_number(part: &str, key: &str) -> Option<i64> {
    part.strip_prefix(key)?.strip_prefix('=')?.parse().ok()
}

fn by_id<'a>(items: &'a [Value], key: &str, id: i64, what: &str) -> &'a Value {
    items
        .iter()
        .find(|item| item[key].as_i64() == Some(id))
        .unwrap_or_else(|| panic!("{what} {id} missing from the captured config"))
}

/// Spec types whose translated rule reads only `raw_title` / `release_group`
/// (arr_custom_format/translate.rs:595-624), the two release fields
/// `release_input` fills from Scryer's parse as `build_rule_input` does
/// (user_rule_input.rs:141,183).
const FED_SPECS: &[&str] = &["ReleaseTitleSpecification", "ReleaseGroupSpecification"];

/// Verdict sample: every `stride`-th title (about 40 in all) plus each
/// format's first 3 matched titles.
// ponytail: sampled because a debug-build regorus spends ~2 s per title on the
// lookaround-heavy formats (40 CPU-min for all 1269 Sonarr titles); evaluate
// every title once regorus is optimised in the dev profile.
fn verdict_sample(verdicts: &[Value]) -> Vec<&Value> {
    let stride = verdicts.len().div_ceil(40);
    let mut firsts: HashMap<&str, usize> = HashMap::new();
    verdicts
        .iter()
        .enumerate()
        .filter(|(i, v)| {
            let mut keep = i % stride == 0;
            for name in v["customFormats"].as_array().unwrap() {
                let seen = firsts.entry(name.as_str().unwrap()).or_default();
                keep |= *seen < 3;
                *seen += 1;
            }
            keep
        })
        .map(|(_, v)| v)
        .collect()
}

/// Scryer's import of the whole captured export, every format scored 1, and
/// for each sampled parse verdict (title, arr's matched format names) the
/// score keys Scryer's rules emitted for that title.
fn import_and_match(
    source: ArrSourceApplication,
    formats: &[Value],
    verdicts: &[Value],
) -> (ArrCustomFormatTranslation, Vec<(String, Vec<String>, Vec<String>)>) {
    let paste = Value::from(formats.to_vec()).to_string();
    let review = translate_arr_custom_formats(source, &paste, &HashMap::new()).expect("export readable");
    let scores = review.formats.iter().map(|f| (f.id.clone(), 1)).collect();
    let translation = translate_arr_custom_formats(source, &paste, &scores).expect("export readable");
    let facet = facet(source);
    let sample = verdict_sample(verdicts);
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let matched = std::thread::scope(|scope| {
        let handles: Vec<_> = sample
            .chunks(sample.len().div_ceil(workers))
            .map(|chunk| {
                let translation = &translation;
                scope.spawn(move || {
                    let engine = rules_engine(translation).expect("imported rules compile");
                    let mut evaluator = engine.evaluator();
                    chunk
                        .iter()
                        .map(|v| {
                            let title = v["title"].as_str().unwrap();
                            let parsed = parse_release_metadata(title);
                            let input =
                                release_input(parsed.raw_title.clone(), parsed.release_group.clone(), vec![], "", facet);
                            let result = evaluator.evaluate(&input, facet).expect("rules evaluate");
                            // Entries as the live scorer applies them, errors
                            // or not (quality/canonical.rs append_rule_scores).
                            let title = if result.errors.is_empty() {
                                format!("`{title}`")
                            } else {
                                format!("`{title}` (rule error: {})", result.errors[0].message.lines().last().unwrap_or(""))
                            };
                            let arr = v["customFormats"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|n| n.as_str().unwrap().to_string())
                                .collect();
                            (title, arr, result.entries.into_iter().map(|e| e.code).collect())
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    (translation, matched)
}

fn config(suite_name: &str, app: &str, prefix: &str, source: ArrSourceApplication) {
    let Some(mut suite) = Suite::open(suite_name) else { return };
    let catalogue = Catalogue::load();
    let formats: Vec<Value> = corpus(&format!("corpus/captured/{app}/config/customformat.json"));
    let profiles: Vec<Value> = corpus(&format!("corpus/captured/{app}/config/qualityprofile.json"));
    let verdicts: Vec<Value> = corpus(&format!("corpus/captured/{app}/data/parse.json"));
    let (translation, matched) = import_and_match(source, &formats, &verdicts);
    for (_, entry) in &catalogue.entries {
        let id = entry.id.as_str();
        if !id.starts_with(prefix) || !suite.wants(id) {
            continue;
        }
        let Some((path, fragment)) = entry.reference.split_once('#') else { continue };
        let parts: Vec<&str> = fragment.split('/').collect();
        match (path.rsplit('/').next().unwrap(), &parts[..]) {
            // Custom-format definition: Scryer's imported rule matches exactly
            // the captured titles the arr matched it on.
            ("customformat.json", [format_ref]) => {
                let Some(format_id) = fragment_number(format_ref, "id") else { continue };
                let format = by_id(&formats, "id", format_id, "custom format");
                let name = format["name"].as_str().unwrap();
                let unfed: Vec<_> = format["specifications"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|s| s["implementation"].as_str())
                    .filter(|i| !FED_SPECS.contains(i))
                    .collect();
                if !unfed.is_empty() {
                    suite.record(id, false, format!("harness feeds only title and group; format has {unfed:?}"));
                    continue;
                }
                let Some(pos) = translation.formats.iter().position(|f| f.name == name) else {
                    suite.record(id, false, format!("format not readable: {:?}", translation.inspection_diagnostics));
                    continue;
                };
                let imported = &translation.formats[pos];
                if imported.status != ArrFormatStatus::Translated {
                    suite.record(id, false, format!("format disabled: {:?}", imported.diagnostics));
                    continue;
                }
                // No custom-format token in Scryer's renamer: "renaming
                // includes it: on" has no Scryer equivalent.
                if format["includeCustomFormatWhenRenaming"].as_bool().unwrap_or(false) {
                    suite.record(id, false, "renaming includes it: Scryer's renamer has no custom-format token");
                    continue;
                }
                let key_suffix = format!("_{}", pos + 1);
                let wrong: Vec<String> = matched
                    .iter()
                    .filter_map(|(title, arr, codes)| {
                        let arr = arr.iter().any(|n| n == name);
                        let scryer = codes.iter().any(|c| c.ends_with(&key_suffix));
                        (arr != scryer).then(|| format!("{title}: arr {arr}, scryer {scryer}"))
                    })
                    .collect();
                // No positive verdict to test matching against; a false
                // positive still fails.
                if wrong.is_empty() && !matched.iter().any(|(_, arr, _)| arr.iter().any(|n| n == name)) {
                    continue;
                }
                let detail = format!("{} of {} titles disagree, e.g. {:?}", wrong.len(), matched.len(), &wrong[..wrong.len().min(3)]);
                suite.record(id, wrong.is_empty(), if wrong.is_empty() { String::new() } else { detail });
            }
            // Custom-format score in a profile: the score Scryer derives for
            // the format from the captured arr data.
            ("qualityprofile.json", [profile_ref, "formatItems", format_ref]) => {
                let (Some(profile_id), Some(format_id)) =
                    (fragment_number(profile_ref, "id"), fragment_number(format_ref, "format"))
                else {
                    continue;
                };
                let profile = by_id(&profiles, "id", profile_id, "quality profile");
                let item = by_id(profile["formatItems"].as_array().unwrap(), "format", format_id, "format item");
                let expected = item["score"].as_f64();
                let name = by_id(&formats, "id", format_id, "custom format")["name"].as_str().unwrap();
                let actual = translation
                    .formats
                    .iter()
                    .find(|f| f.name == name)
                    .and_then(|f| f.suggested_scores.first().map(|(_, s)| *s));
                record(
                    &mut suite,
                    id,
                    expected,
                    actual,
                    "Scryer's arr import reads no profile formatItems (external_import.rs:574 keeps id, name, language); its only score source is the format export",
                );
            }
            // Profile definitions and quality definitions: unmapped (header).
            _ => {}
        }
    }
    suite.finish();
}

#[test]
fn son_quality_config() {
    config(
        "scryer-application::parity_quality::son_quality_config",
        "sonarr",
        "SON-QUALITY-5",
        ArrSourceApplication::Sonarr,
    );
}

#[test]
fn rad_quality_config() {
    config(
        "scryer-application::parity_quality::rad_quality_config",
        "radarr",
        "RAD-QUALITY-5",
        ArrSourceApplication::Radarr,
    );
}
