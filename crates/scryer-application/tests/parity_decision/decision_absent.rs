//! Parity net, stream `decision_absent`: MaximumSize, ReleaseRestrictions and
//! IndexerTag, recorded red (D29: Scryer has the behaviour, not the
//! reference's setting).
//!
//! Suites (nextest names `decision_absent::<fn>`):
//!
//! - `son_decision_absent`: SON-DECISION-0094..0098, 1074..1091, 1204..1210.
//! - `rad_decision_absent`: RAD-DECISION-0065..0069, 1061..1078, 0090..0096.
//!
//! Selection (before `wants`): the corpus files
//! `MaximumSizeSpecificationFixture.json`, `ReleaseRestrictionsSpecificationFixture.json`
//! and the IndexerTag fixture (`RssSync.IndexerTagSpecificationFixture.json` for
//! Sonarr, `IndexerTagSpecificationFixture.json` for Radarr) of the app, kept
//! only when `missing_field` is `Some`.
//!
//! # Scryer functions under test
//!
//! None. A call would be vacuous: `apply_implausible_size_limits`
//! (`quality/profile.rs`) returns early without a runtime and a quality,
//! which the fixtures do not give; there is no setting to put release-profile
//! terms in (translating them into Rego would put the matching logic in the
//! test); and `indexer_search_eligibility` (`contracts.rs`) takes a per-scope
//! routing plan, so turning indexer/title tags into one would be harness
//! logic. Every case is recorded as a failure with the reason, as the
//! root AcceptableSize branch does (including cases the reference accepts).
//!
//! # Translation table (reference term -> Scryer term)
//!
//! - `config.maximum_size_mb` (Settings > Indexers > Maximum Size, 0 = off) ->
//!   no Scryer field (red). Scryer caps size only per quality and runtime
//!   (`size_implausible_for_quality`, `quality/profile.rs`) or through user
//!   rules / imported custom formats (`SizeSpecification` -> `size_bytes`
//!   range, `rules/arr_custom_format/specifications.rs:444`).
//! - `release_profiles[].required` / `.ignored` (plain = contains, `/re/` =
//!   regex) -> no Scryer field (red). Scryer filters titles only through user
//!   rules / imported custom formats (`ReleaseTitleSpecification` -> regex on
//!   `raw_title`, `rules/arr_custom_format/translate.rs:595`; BLOCK_SCORE or
//!   min score).
//! - `indexers.<id>.tags` / `indexer.tags` intersected with `series.tags` /
//!   `movie.tags` -> no Scryer field (red). Scryer restricts indexers per
//!   library/facet scope (`indexer.routing:<scope>` setting ->
//!   `IndexerRoutingPlan`, `IndexerRoutingEntry.enabled`, checked by
//!   `indexer_search_eligibility`, `contracts.rs`, at query time).
//! - Ignored input keys: `release.size_mb`, `release.title`, `release.indexer_id`,
//!   `series.tags` / `movie.tags`, `episodes`, `search`, `unknown_indexer_ids`,
//!   `note`. Expect is not read.
//!
//! Known reds: all 60, including the cases the reference accepts (cap off,
//! empty profiles, untagged or unknown indexers).
//!
//! # Unmapped (tests: []), with reasons
//!
//! - Retention (SON 1102..1107, RAD 1089..1094): Scryer has no usenet retention
//!   setting and never refuses a release for its age; age only orders results
//!   (`SearchRank.usenet_age_hours`, `acquisition/scoring.rs:174`).
//! - FreeSpace at grab time (SON 0083..0088, RAD 0042..0047): free space is
//!   checked only at import (`import/checks.rs:97`, mapped in `parity_import`).
//! - BlockedIndexer (SON 0057, 0058, RAD 0021, 0022): backoff skips the indexer
//!   before the query, in an async repository-backed method
//!   (`acquisition/convergence.rs:291-303`); no per-release check to call.
//! - DownloadDecisionMaker (SON 0068-0071, 0073, 0077-0082; RAD 0030-0033,
//!   0038, 0039, 0041): mock specification pipeline, mocked mapping and logs;
//!   Scryer's checks are a fixed sequence (`evaluate_auto_candidate_recording`,
//!   `acquisition/release_search.rs`). The `evaluate_auto_candidate_with_step`
//!   seam (stream `decision_context`) does not help: the cases assert
//!   mock-pipeline interactions, not one candidate's verdict.
//! - SingleEpisodeAge (SON 1112..1121): Scryer never refuses a single in a
//!   season search; ranking is built in private `evaluate_candidate`.
//! - Pending (RAD 1147..1150): `active_pending_release_delay_code`
//!   (`acquisition/release_search.rs`) is private and its overlap flag
//!   comes from the pending repository inside AppUseCase; every expect carries
//!   `pending_queue_reads`, a mock call count.

use scryer_parity::{Suite, corpus};
use serde_json::Value;

const MAX_SIZE: &str = "no Scryer field for an absolute maximum release size (Settings > Indexers > Maximum Size); Scryer caps size only per quality and runtime (size_implausible_for_quality, quality/profile.rs) or through user rules / imported custom formats (SizeSpecification -> size_bytes range, rules/arr_custom_format/specifications.rs:444)";
const TERMS: &str = "no Scryer field for release-profile required/ignored terms; Scryer filters titles only through user rules / imported custom formats (ReleaseTitleSpecification -> regex on raw_title, rules/arr_custom_format/translate.rs:595)";
const TAGS: &str = "no Scryer field for indexer/title tags; Scryer restricts indexers per library scope (indexer.routing:<scope> -> IndexerRoutingPlan, indexer_search_eligibility, contracts.rs)";

#[test]
fn son_decision_absent() {
    absent_suite("scryer-application::parity_decision::son_decision_absent", "sonarr");
}

#[test]
fn rad_decision_absent() {
    absent_suite("scryer-application::parity_decision::rad_decision_absent", "radarr");
}

fn absent_suite(name: &str, app: &str) {
    let Some(mut suite) = Suite::open(name) else { return };
    let tag_fixture = if app == "sonarr" {
        "RssSync.IndexerTagSpecificationFixture"
    } else {
        "IndexerTagSpecificationFixture"
    };
    for fixture in ["MaximumSizeSpecificationFixture", "ReleaseRestrictionsSpecificationFixture", tag_fixture] {
        let cases: Vec<Value> = corpus(&format!("corpus/reference/{app}/decision/{fixture}.json"));
        for case in &cases {
            let Some(reason) = missing_field(&case["input"]) else { continue };
            let id = case["id"].as_str().unwrap();
            if !suite.wants(id) {
                continue;
            }
            super::record(&mut suite, id, &case["expect"], Some(Err(reason.into())));
        }
    }
    suite.finish();
}

fn missing_field(input: &Value) -> Option<&'static str> {
    let g = &input["given"];
    match input["spec"].as_str()? {
        "MaximumSizeSpecification" if !g["config"]["maximum_size_mb"].is_null() => Some(MAX_SIZE),
        "ReleaseRestrictionsSpecification" if g["release_profiles"].is_array() => Some(TERMS),
        "RssSync.IndexerTagSpecification" | "IndexerTagSpecification"
            if !g["release"]["indexer_id"].is_null() =>
        {
            Some(TAGS)
        }
        _ => None,
    }
}
