//! Parity net, stream `priv_decision`: the private config entries of the
//! decision area (catalogue `decision.*-config.yaml`, data under
//! `corpus/captured/<app>/config/`). Nothing captured is written in this file;
//! every compared value is read from the corpus at run time (D45).
//!
//! Suites (nextest names `priv_decision::<fn>`):
//!
//! - `rad_decision_config_formats`: RAD-DECISION-5008..5032 (`qualityprofile.json`
//!   `formatItems/format=N`): the score a format gets in the profiles in use.
//! - `son_decision_config_settings`: SON-DECISION-5013 (`mediamanagement.json`
//!   `downloadPropersAndRepacks`), 5016 (`indexerconfig.json` `maximumSize`),
//!   5017 (`releaseprofile.json`).
//! - `rad_decision_config_settings`: RAD-DECISION-5034 (propers), 5037 (`maximumSize`),
//!   5038 (`allowHardcodedSubs`), 5039 (`preferIndexerFlags`), 5040
//!   (`movie-summary.json` `minimumAvailability`), 5042 (`releaseprofile.json`),
//!   5043 (`tag.json`).
//!
//! Selection happens before `wants` (under `PARITY_ADOPT=1` it is true for every
//! unmapped ID): catalogue area `decision`, source `config`, then the reference's
//! file and fragment.
//!
//! # Scryer functions under test
//!
//! - formats: `translate_arr_custom_formats` (Scryer's arr import in review mode,
//!   the same call as `parity_quality` `son_quality_config`), run once on the
//!   whole `customformat.json` with an empty score map.
//! - propers: the root `grab_refusal` (cutoff gate + `evaluate_admission`) for the
//!   "grabbed as an upgrade" half; `compare_release_search_results` with the
//!   `revision_preference` production sets (`release_search.rs`, "decision.revision_preference")
//!   and `apply_min_score_gate` for the "preferred over the original" half.
//! - minimum availability: `MovieAvailabilitySettings::policy`,
//!   `movie_availability_decision`, `ReleaseAutoDecisionCode::with_movie_availability`
//!   (composed as the call site in `release_search.rs` does).
//! - hardcoded subs: `evaluate_auto_candidate_with_step` on a standalone movie
//!   (`decision_context::movie_code`).
//!
//! # Translation table (reference term -> Scryer term)
//!
//! | Reference | Scryer |
//! |---|---|
//! | profile `formatItems[]` {format, score} | no Scryer field: the arr import reads only a profile's id, name and language (`external_import.rs:574`); a rule carries one score. Actual = `ArrTranslatedFormat.suggested_scores` of the format named by `customformat.json` `id` == N (what the export itself offers) |
//! | profiles in use | keys of `movie-summary.json` / `series-summary.json` key `qualityProfileId` |
//! | expected score | the `formatItems` score of the in-use profiles (missing entry = 0), compared as f64; profiles that disagree -> red (one score per rule) |
//! | `downloadPropersAndRepacks` | root `propers()` -> `ignore_proper_repack` through `captured_spec` (DoNotUpgrade -> no field, red). PreferAndUpgrade -> {preferred, upgrades}; DoNotUpgrade -> {preferred, not upgrades}; DoNotPrefer -> neither |
//! | "preferred over the original" | `compare_release_search_results` (rank head: revision step, then score), same tier and group, empty title-scope subject (no file on disk). The reference compares quality and revision before the custom-format score (`DownloadDecisionComparer` `CompareQuality` before `CompareCustomFormatScore`, both apps), so the original matches a format worth 100 more than the proper (a rule score contribution, as the root's custom-format glue logs it). 100 exceeds Scryer's only fileless proper preference, the bundled default TRaSH `trash-guides-features` +30/+50 for `is_proper_upload` / `is_repack`, so leaving that rule out of the harness cannot change the order. Both scores start at the profile's min format score (when positive) so the min-score gate refuses neither |
//! | "grabbed as an upgrade" | root `grab_refusal` of a same-tier same-group revision over the held original, title scope, score 0 |
//! | `allowHardcodedSubs` / `whitelistedHardcodedSubs` | `evaluate_auto_candidate_with_step` on a standalone movie: an `HC` release -> `HardcodedSubs` (fixed refusal, P20, D72), its plain twin is not refused. Expected refused = !allowHardcodedSubs. No Scryer setting: allowed=true reds through the call; a non-empty whitelist is red by reason |
//! | `preferIndexerFlags` | no setting (red by reason): `SearchRank` has no flag step (`acquisition/scoring.rs`); flags change the order only through user rules / imported `IndexerFlagSpecification` (`rules/arr_custom_format/specifications.rs:336`) |
//! | movie `minimumAvailability` announced / tba | `announced` (Radarr `Movie.cs`: both are MinValue) |
//! | inCinemas | `in_cinemas` |
//! | released | `released` |
//! | other values | red: no Scryer threshold |
//! | `availabilityDelay` | `MovieAvailabilitySettings.delay_days` |
//! | "only grabbed once available" | `with_movie_availability` -> `MovieAvailabilityBlocked` |
//! | `maximumSize`, `releaseprofile.json`, `tag.json` | no Scryer field (red by reason, D29, same convention as `decision_absent`; red even for an off/empty value). `tag.json`: delay-profile tags exist in Scryer (`resolve_delay_profile`, covered by the delay-profile config entries); the red is for indexer / download-client tags |
//!
//! Availability scenarios (expected column = Radarr `Movie.IsAvailable`,
//! `Movie.cs:77-119`; days are off-boundary by one so the time of day cannot decide):
//! released: home date delay+1 days ago / delay-1; no home date, theatrical 90+delay+1 / 90+delay-1
//! days ago; in_cinemas: theatrical delay+1 / delay-1 days ago; announced: no dates, available.
//!
//! Ignored keys: `movie-summary.json` / `series-summary.json` counts, every
//! other key of `qualityprofile.json`, `indexerconfig.json`, `mediamanagement.json`.
//!
//! # Unmapped (tests: []), with reasons
//!
//! - RAD-DECISION-5036, SON-DECISION-5015 (retention): no retention setting and no
//!   built-in age refusal (age only orders results, `SearchRank.usenet_age_hours`,
//!   `acquisition/scoring.rs:174`); age reaches only user rules as `release.age_days`
//!   (`rules/user_rule_input.rs:10`). Kept unmapped like the reference Retention
//!   fixtures in `decision_absent`; mapping both red by reason (as `maximumSize`) is
//!   the stage owner's call.
//! - RAD-DECISION-5041, SON-DECISION-5018 (title / season / single-episode monitoring):
//!   `Title.monitored` and season / single-episode monitoring are applied by target
//!   derivation (`derive_missing_targets_for_search`, `acquisition/targets.rs:752`) and
//!   the RSS scope filter (`rss_episode_scope_is_target`, `acquisition/rss.rs:2790`),
//!   both AppUseCase / repository-backed. The grab evaluator reads only episode-level
//!   unmonitored ids for EpisodeSet coverage (`release_search.rs:2415`, mapped in
//!   `decision_context`). More than a visibility seam.
//! - RAD-DECISION-5044 (auto-tagging): no condition-based tagging of library titles
//!   (request rules add tags at request time behind the request service).
//!
//! Known reds: the 25 format scores and the constant reds above (`maximumSize`,
//! `preferIndexerFlags`, `releaseprofile.json`, `tag.json`).

use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::Duration;
use scryer_application::arr_custom_format::{ArrSourceApplication, translate_arr_custom_formats};
use scryer_application::parity_seams::{
    AdmissionScope, AdmissionSubject, CandidateFacts, Incumbent, ReleaseAutoDecisionCode,
    apply_min_score_gate, compare_release_search_results, movie_availability_decision,
    revision_preference,
};
use scryer_application::{
    IndexerSearchResult, MovieAvailabilitySettings, MovieAvailabilityStatus, QualityProfile,
    ScoringSource,
};
use scryer_domain::MovieReleaseDates;
use scryer_parity::{Catalogue, Source, Suite, corpus};
use serde_json::{Value, json};

use super::{Held, PROPER_GROUP, Side};

const MAX_SIZE: &str = "no Scryer field for an absolute maximum release size (Settings > Indexers > Maximum Size); Scryer caps size only per quality and runtime (size_implausible_for_quality, quality/profile.rs) or through user rules / imported custom formats (SizeSpecification -> size_bytes range, rules/arr_custom_format/specifications.rs:444)";
const TERMS: &str = "no Scryer field for release-profile required/ignored terms; Scryer filters titles only through user rules / imported custom formats (ReleaseTitleSpecification -> regex on raw_title, rules/arr_custom_format/translate.rs:595)";
const TAGS: &str = "delay-profile tags exist (resolve_delay_profile, covered by the delay-profile config entries); no Scryer field for indexer/download-client tags: Scryer restricts indexers per library scope (indexer.routing:<scope> -> IndexerRoutingPlan, indexer_search_eligibility, contracts.rs)";
const HARD_SUBS_WHITELIST: &str = "no whitelistedHardcodedSubs setting: Scryer refuses every hardcoded-sub movie release (fixed, P20)";
const INDEXER_FLAGS: &str = "no Scryer setting to prefer indexer flags in the rank (SearchRank has no flag step, acquisition/scoring.rs); flags change the order only through user rules / imported IndexerFlagSpecification (rules/arr_custom_format/specifications.rs:336)";
const NO_FORMAT_ITEMS: &str = "Scryer's arr import reads no profile formatItems (external_import.rs:574 keeps id, name, language); its only score source is the format export";
/// A format score above Scryer's bundled fileless proper bonus (+30/+50).
const BIGGER_FORMAT: i32 = 100;

fn app_source(app: &str) -> ArrSourceApplication {
    if app == "sonarr" { ArrSourceApplication::Sonarr } else { ArrSourceApplication::Radarr }
}

/// Profile ids in use: keys of `<summary>.json` key `qualityProfileId`, ascending.
fn profiles_in_use(app: &str) -> Vec<i64> {
    let summary = if app == "sonarr" { "series-summary" } else { "movie-summary" };
    let v: Value = corpus(&format!("corpus/captured/{app}/config/{summary}.json"));
    let mut ids: Vec<i64> = v["qualityProfileId"]
        .as_object()
        .map(|m| m.keys().filter_map(|k| k.parse().ok()).collect())
        .unwrap_or_default();
    ids.sort_unstable();
    ids
}

// ── formats ────────────────────────────────────────────────────────────────

#[test]
fn rad_decision_config_formats() {
    let Some(mut suite) = Suite::open("scryer-application::parity_decision::rad_decision_config_formats") else {
        return;
    };
    let app = "radarr";
    let catalogue = Catalogue::load();
    let formats: Vec<Value> = corpus(&format!("corpus/captured/{app}/config/customformat.json"));
    let profiles: Vec<Value> = corpus(&format!("corpus/captured/{app}/config/qualityprofile.json"));
    let in_use = profiles_in_use(app);
    let paste = Value::from(formats.clone()).to_string();
    let translation = translate_arr_custom_formats(app_source(app), &paste, &HashMap::new());
    for (area, entry) in &catalogue.entries {
        if area != "decision" || entry.source != Source::Config || !entry.id.starts_with("RAD-DECISION-5") {
            continue;
        }
        let Some((path, fragment)) = entry.reference.split_once('#') else { continue };
        let Some(n) = fragment.strip_prefix("formatItems/format=").and_then(|n| n.parse::<i64>().ok()) else {
            continue;
        };
        if path.rsplit('/').next() != Some("qualityprofile.json") || !suite.wants(&entry.id) {
            continue;
        }
        let id = entry.id.as_str();
        let scores: Vec<f64> = in_use
            .iter()
            .map(|pid| {
                profiles
                    .iter()
                    .find(|p| p["id"].as_i64() == Some(*pid))
                    .and_then(|p| p["formatItems"].as_array())
                    .and_then(|items| items.iter().find(|i| i["format"].as_i64() == Some(n)))
                    .and_then(|i| i["score"].as_f64())
                    .unwrap_or(0.0)
            })
            .collect();
        let Some(&expected) = scores.first() else {
            suite.record(id, false, "harness: no profile in use");
            continue;
        };
        if scores.iter().any(|s| *s != expected) {
            suite.record(id, false, format!("per-profile scores differ ({scores:?}); Scryer rules carry one score per rule"));
            continue;
        }
        let translation = match &translation {
            Ok(t) => t,
            Err(e) => {
                suite.record(id, false, format!("harness: translation refused: {e:?}"));
                continue;
            }
        };
        let name = formats.iter().find(|f| f["id"].as_i64() == Some(n)).and_then(|f| f["name"].as_str());
        let Some(format) = name.and_then(|name| translation.formats.iter().find(|f| f.name == name)) else {
            suite.record(id, false, format!("format {n} not in the translation (refused or renamed)"));
            continue;
        };
        let actual = format.suggested_scores.first().map(|(_, s)| *s);
        let pass = actual == Some(expected);
        let detail = match actual {
            _ if pass => String::new(),
            None => format!("{NO_FORMAT_ITEMS}; expected {expected}, actual None"),
            Some(a) => format!("expected {expected}, actual {a}"),
        };
        suite.record(id, pass, detail);
    }
    suite.finish();
}

// ── settings ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Kind {
    Propers,
    MaxSize,
    HardSubs,
    IndexerFlags,
    Availability,
    Terms,
    Tags,
}

#[test]
fn son_decision_config_settings() {
    settings_suite("scryer-application::parity_decision::son_decision_config_settings", "sonarr", "SON-DECISION-5");
}

#[test]
fn rad_decision_config_settings() {
    settings_suite("scryer-application::parity_decision::rad_decision_config_settings", "radarr", "RAD-DECISION-5");
}

fn settings_suite(name: &str, app: &str, prefix: &str) {
    let Some(mut suite) = Suite::open(name) else { return };
    let catalogue = Catalogue::load();
    for (area, entry) in &catalogue.entries {
        if area != "decision" || entry.source != Source::Config || !entry.id.starts_with(prefix) {
            continue;
        }
        let (path, fragment) = entry.reference.split_once('#').unwrap_or((&entry.reference, ""));
        if path.split('/').nth(2) != Some(app) {
            continue;
        }
        let kind = match (path.rsplit('/').next().unwrap_or(""), fragment) {
            ("mediamanagement.json", "downloadPropersAndRepacks") => Kind::Propers,
            ("indexerconfig.json", "maximumSize") => Kind::MaxSize,
            ("indexerconfig.json", "allowHardcodedSubs") if app == "radarr" => Kind::HardSubs,
            ("indexerconfig.json", "preferIndexerFlags") if app == "radarr" => Kind::IndexerFlags,
            ("movie-summary.json", "minimumAvailability") => Kind::Availability,
            ("releaseprofile.json", _) => Kind::Terms,
            ("tag.json", _) if app == "radarr" => Kind::Tags,
            _ => continue,
        };
        if !suite.wants(&entry.id) {
            continue;
        }
        let id = entry.id.as_str();
        let outcome = match kind {
            Kind::Propers => propers_case(app),
            Kind::MaxSize => Err(MAX_SIZE.into()),
            Kind::Terms => Err(TERMS.into()),
            Kind::Tags => Err(TAGS.into()),
            Kind::HardSubs => hard_subs_case(app),
            Kind::IndexerFlags => Err(INDEXER_FLAGS.into()),
            Kind::Availability => availability_case(app),
        };
        match outcome {
            Ok((expected, actual)) => suite.check(id, expected, actual),
            Err(why) => suite.record(id, false, why),
        }
    }
    suite.finish();
}

type Case = Result<(Value, Value), String>;

fn search_result(profile: &QualityProfile, side: &Side) -> IndexerSearchResult {
    let mut decision = super::requirements(profile, side);
    if side.score != 0 {
        // A matched format's score is a rule score contribution (root glue).
        decision.log_with_source(
            "parity-format",
            side.score,
            ScoringSource::UserRule { id: "parity-format".into(), name: "parity-format".into() },
        );
    }
    apply_min_score_gate(profile, &mut decision);
    // As release_search.rs sets it: production `revision_preference` over the
    // title subject (empty here: no file on disk).
    let facts = CandidateFacts::new(decision.tier_index, side.revision, decision.preference_score)
        .with_profile_policy(side.group.as_deref(), &profile.criteria);
    decision.revision_preference =
        revision_preference(facts, &AdmissionSubject::new(AdmissionScope::Title, Vec::<(Incumbent, bool)>::new()));
    IndexerSearchResult {
        indexer_id: None,
        source: String::new(),
        title: format!("rev{}", side.revision),
        link: None,
        download_url: Some(format!("parity://rev{}", side.revision)),
        source_kind: None,
        size_bytes: None,
        published_at: None,
        thumbs_up: None,
        thumbs_down: None,
        indexer_languages: None,
        indexer_subtitles: None,
        indexer_grabs: None,
        password_hint: None,
        parsed_release_metadata: Some(super::parsed(side)),
        quality_profile_decision: Some(decision),
        extra: Default::default(),
        response_attributes: Default::default(),
        guid: None,
        info_url: None,
        provenance: None,
        candidate_token: None,
        queue_scope: None,
        coverage_scope: None,
        auto_eligible: None,
        auto_decision_code: None,
        auto_decision_summary: None,
        release_listing_json: None,
        http: None,
    }
}

/// Radarr `HardcodeSubsSpecification`: an HC release is refused unless allowed;
/// its plain twin never is.
fn hard_subs_case(app: &str) -> Case {
    let cfg: Value = corpus(&format!("corpus/captured/{app}/config/indexerconfig.json"));
    if cfg["whitelistedHardcodedSubs"].as_str().is_some_and(|w| !w.trim().is_empty()) {
        return Err(HARD_SUBS_WHITELIST.into());
    }
    let allowed = cfg["allowHardcodedSubs"].as_bool().unwrap_or(false);
    let refused =
        |t: &str| super::decision_context::movie_code(1, t) == ReleaseAutoDecisionCode::HardcodedSubs;
    Ok((
        json!({ "hc_refused": !allowed, "plain_refused": false }),
        json!({
            "hc_refused": refused("Amber.Circuit.2018.HC.1080p.WEB-DL.x264-GRP"),
            "plain_refused": refused("Amber.Circuit.2018.1080p.WEB-DL.x264-GRP"),
        }),
    ))
}

fn propers_case(app: &str) -> Case {
    let media: Value = corpus(&format!("corpus/captured/{app}/config/mediamanagement.json"));
    let value = media["downloadPropersAndRepacks"].as_str().map(str::to_ascii_lowercase);
    // DownloadDecisionComparer.CompareQuality (revision unless DoNotPrefer) and
    // UpgradableSpecification / RepackSpecification.
    let (preferred, upgrades) = match value.as_deref() {
        None | Some("preferandupgrade") => (true, true),
        Some("donotupgrade") => (true, false),
        Some("donotprefer") => (false, false),
        Some(other) => return Err(format!("unknown downloadPropersAndRepacks '{other}'")),
    };
    let mut expected = Vec::new();
    let mut actual = Vec::new();
    for pid in profiles_in_use(app) {
        let profile = super::build_profile(&super::captured_spec(app, pid)?, &[])?;
        let file = super::release_at(&profile, 0, 0, Some(PROPER_GROUP)).ok_or("profile has no tier")?;
        let proper = Side { revision: 1, ..file.clone() };
        let held = [Held { id: "f1".into(), side: file.clone(), episode: None, added_days_ago: 0 }];
        let upgraded = super::grab_refusal(&profile, &proper, &held, None, Vec::new(), false).is_none();
        // Ranking: the original matches a format worth BIGGER_FORMAT more than the
        // proper; the reference still puts the proper first (revision before score).
        let base = profile.criteria.min_score_to_grab.unwrap_or(0).max(0);
        let ranked_proper = Side { score: base, ..proper.clone() };
        let ranked_original = Side { score: base + BIGGER_FORMAT, ..file.clone() };
        let proper_result = search_result(&profile, &ranked_proper);
        let original_result = search_result(&profile, &ranked_original);
        let ranked = compare_release_search_results(&proper_result, &original_result) == Ordering::Less;
        expected.push(json!({ "preferred": preferred, "upgrades": upgrades }));
        actual.push(json!({ "preferred": ranked, "upgrades": upgraded }));
    }
    if expected.is_empty() {
        return Err("harness: no profile in use".into());
    }
    Ok((Value::Array(expected), Value::Array(actual)))
}

fn availability_case(app: &str) -> Case {
    let summary: Value = corpus(&format!("corpus/captured/{app}/config/movie-summary.json"));
    let cfg: Value = corpus(&format!("corpus/captured/{app}/config/indexerconfig.json"));
    let delay = cfg["availabilityDelay"].as_i64().unwrap_or(0) as i32;
    let mut values: Vec<String> = summary["minimumAvailability"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    values.sort();
    if values.is_empty() {
        return Err("harness: no minimumAvailability captured".into());
    }
    let settings = MovieAvailabilitySettings { delay_days: delay, ..MovieAvailabilitySettings::default() };
    let now = super::now();
    let day = |n: i64| (now - Duration::days(n)).format("%Y-%m-%d").to_string();
    let d = i64::from(delay);
    let mut expected = Vec::new();
    let mut actual = Vec::new();
    for v in &values {
        // (theatrical days ago, home days ago, Radarr IsAvailable)
        let (threshold, scenarios): (&str, Vec<(Option<i64>, Option<i64>, bool)>) =
            match v.to_ascii_lowercase().as_str() {
                "announced" | "tba" => ("announced", vec![(None, None, true)]),
                "incinemas" => ("in_cinemas", vec![(Some(d + 1), None, true), (Some(d - 1), None, false)]),
                "released" => (
                    "released",
                    vec![
                        (Some(200), Some(d + 1), true),
                        (Some(200), Some(d - 1), false),
                        (Some(90 + d + 1), None, true),
                        (Some(90 + d - 1), None, false),
                    ],
                ),
                _ => return Err(format!("no Scryer threshold for '{v}'")),
            };
        for (theatrical, home, want) in scenarios {
            let dates = MovieReleaseDates {
                market: settings.release_market.clone(),
                fetched: true,
                theatrical_dates: theatrical.map(|n| vec![day(n)]).unwrap_or_default(),
                digital_dates: home.map(|n| vec![day(n)]).unwrap_or_default(),
                ..Default::default()
            };
            let decision =
                movie_availability_decision(Some(&dates), true, None, None, settings.policy(Some(threshold)), &now);
            let grabbed = ReleaseAutoDecisionCode::Eligible
                .with_movie_availability(decision.status != MovieAvailabilityStatus::Available)
                == ReleaseAutoDecisionCode::Eligible;
            expected.push(json!(want));
            actual.push(json!(grabbed));
        }
    }
    Ok((Value::Array(expected), Value::Array(actual)))
}
