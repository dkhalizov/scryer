//! Parity net, stream `decision_rank`: PrioritizeDownloadDecisionFixture.
//!
//! Suites (nextest names `decision_rank::<fn>`):
//!
//! - `son_decision_rank`: SON-DECISION-1000, 1001, 1002, 1006, 1007, 1020..1026.
//! - `rad_decision_rank`: RAD-DECISION-1000, 1001, 1002, 1004..1007, 1016..1022.
//!
//! # Scryer function under test
//!
//! `compare_release_search_results` (`catalog/release_search.rs`, `RankHead::compare`,
//! seam in `parity_seams`): the head of the search rank key (blocked, profile
//! tier, language, revision, score, size fit). Production also uses it to
//! re-sort automatic results. The decisions it reads come from
//! `evaluate_profile_requirements` + a `UserRule` custom-format contribution +
//! `apply_min_score_gate`, all through the root helpers. Candidates are sorted
//! with the stable `slice::sort_by`, as production does; no tail tie-break is
//! added. A head tie between the first two sorted candidates is reported as
//! `"tie (no Scryer preference): a = b"` instead of a label: production
//! would keep indexer arrival order there, so the verdict must not depend on
//! corpus order. `rank_case` never sees the case's `expect`.
//!
//! # Translation table (reference term -> Scryer term)
//!
//! - Quality name -> root `quality()` table; revision version v -> Scryer
//!   revision v - 1; `real` > 0 -> no Scryer field (red).
//! - `custom_format_score` / `custom_formats` -> one `UserRule` contribution on
//!   the decision (root `cf_score`), then `apply_min_score_gate`.
//! - `config.download_propers_and_repacks` -> root `propers()`
//!   (DoNotPrefer -> `ignore_proper_repack`, DoNotUpgrade -> red).
//! - Revision preference -> production `revision_preference` over an empty
//!   title subject, as `release_search.rs` sets it (no file on disk).
//! - `quality_definition.preferred_size` non-null -> red: no per-quality
//!   preferred size setting (Scryer's size fit uses its own bitrate).
//! - `protocol` -> `source_kind`; `size_mb` -> `size_bytes`; `age_days` ->
//!   `published_at` offset from the fixed NOW. These are carried onto the
//!   result for completeness only: `RankHead::compare` never reads them (its
//!   size fit is a decision field, 0 without `apply_size_requirement`, which
//!   is not called; production's `size_fit_penalty` is 0 at size 0 too). Every
//!   mapped case has size 0 except SON 1020, decided by tier first.
//! - Comparison: the shape is fixed per suite, never read from `expect`.
//!   Sonarr -> actual `{first: sorted[0].label}`; Radarr -> actual
//!   `{order: [sorted[0].label]}`. Every mapped Radarr case asserts only
//!   `qualifiedReports.First()` in the reference (the full-order tests RAD
//!   1023/1024 are unmapped), so the reported prefix is one label; an expected
//!   list of any other length would fail, not be truncated to match.
//!
//! # Ignored keys
//!
//! Not read by the head; identical across candidates in every mapped case, or
//! dominated by a strict tier order: `languages` (neutral rank),
//! `series.runtime`, `movie.*`, `title`, `series_type`,
//! `delay_profile.preferred_protocol`, `indexer_priority`, `episodes`,
//! `seeders`, `peers`, `release_class` (plus `protocol`, `size_mb`,
//! `age_days`, carried but unread, see above). A guard asserts that any
//! head-tied adjacent pair has equal listing facts (read from the corpus), so
//! the static ID lists stay honest.
//!
//! # Unmapped (tests: []), with reasons
//!
//! - Order decided by a `SearchRank` listing step (episode number, usenet age,
//!   preferred protocol, coverage distance, seeders, indexer priority): SON
//!   1003, 1004, 1008, 1010-1015, 1027, 1028; RAD 1008-1011, 1023, 1024.
//!   Production builds `SearchRank` only inline in the private
//!   `evaluate_candidate` (`catalog/release_search.rs:433`), which needs
//!   `PreparedReleaseScoringInputs` (`:36`) built by the async AppUseCase
//!   scoring method. Unlock: extract a `SearchRank` constructor (product
//!   refactor with a ledger entry, not a visibility seam).
//! - Order rests on peers at equal seeders: SON 1016, 1017; RAD 1012, 1013.
//!   Same reason; Scryer's swarm step reads seeders only, no peers field.
//! - Same quality differing in both size and age: SON 1005, 1019; RAD 1003,
//!   1015. The age step is a `SearchRank` step (not callable); mapping only the
//!   size half would report a tie production does not have.
//! - Same quality differing in both size and seeders: SON 1018, RAD 1014
//!   (250 MB / 1000 seeders vs 200 MB / 1100). The head's size fit (needs
//!   `apply_size_requirement` with a runtime basis) may decide before the
//!   seeders step; the seeders step is in `SearchRank` (not callable).
//! - SON 1009 (no episodes does not throw): the head comparator cannot fail,
//!   a `{throws: false}` result would be vacuous.

use std::cmp::Ordering;

use scryer_application::parity_seams::{
    AdmissionScope, AdmissionSubject, CandidateFacts, Incumbent, apply_min_score_gate,
    compare_release_search_results, revision_preference,
};
use scryer_application::{IndexerSearchResult, ScoringSource};
use scryer_parity::{Suite, corpus};
use serde_json::{Value, json};

const SON_RANK: &[&str] = &[
    "SON-DECISION-1000", "SON-DECISION-1001", "SON-DECISION-1002", "SON-DECISION-1006",
    "SON-DECISION-1007", "SON-DECISION-1020", "SON-DECISION-1021", "SON-DECISION-1022",
    "SON-DECISION-1023", "SON-DECISION-1024", "SON-DECISION-1025", "SON-DECISION-1026",
];
const RAD_RANK: &[&str] = &[
    "RAD-DECISION-1000", "RAD-DECISION-1001", "RAD-DECISION-1002", "RAD-DECISION-1004",
    "RAD-DECISION-1005", "RAD-DECISION-1006", "RAD-DECISION-1007", "RAD-DECISION-1016",
    "RAD-DECISION-1017", "RAD-DECISION-1018", "RAD-DECISION-1019", "RAD-DECISION-1020",
    "RAD-DECISION-1021", "RAD-DECISION-1022",
];

#[test]
fn son_decision_rank() {
    rank_suite(
        "corpus/reference/sonarr/decision/PrioritizeDownloadDecisionFixture.json",
        SON_RANK,
        "candidates",
        false,
        "scryer-application::parity_decision::son_decision_rank",
    );
}

#[test]
fn rad_decision_rank() {
    rank_suite(
        "corpus/reference/radarr/decision/PrioritizeDownloadDecisionFixture.json",
        RAD_RANK,
        "releases",
        true,
        "scryer-application::parity_decision::rad_decision_rank",
    );
}

fn rank_suite(file: &str, mapped: &[&str], key: &str, order: bool, name: &str) {
    let Some(mut suite) = Suite::open(name) else { return };
    let cases: Vec<Value> = corpus(file);
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !mapped.contains(&id) || !suite.wants(id) {
            continue;
        }
        let outcome = rank_case(&case["input"]["given"], key, order);
        super::record(&mut suite, id, &case["expect"], Some(outcome));
    }
    suite.finish();
}

fn rank_case(g: &Value, key: &str, order: bool) -> Result<Value, String> {
    // Every candidate names its quality (header, D54).
    let named: Vec<_> = g[key].as_array().into_iter().flatten()
        .filter_map(|c| super::side(&c["quality"], None, 0).ok())
        .map(|s| s.q)
        .collect();
    let profile = super::build_profile(&super::fixture_spec(&g["profile"], &g["config"])?, &named)?;
    if !g["quality_definition"]["preferred_size"].is_null() {
        return Err("no Scryer setting for a per-quality preferred size (quality definition preferredSize, MB/min); Scryer's size fit uses its own expected bitrate".into());
    }
    let mut ranked: Vec<(String, IndexerSearchResult)> = Vec::new();
    for cand in g[key].as_array().ok_or("no candidate list")? {
        let label = cand["label"].as_str().unwrap().to_string();
        let side = super::side(&cand["quality"], None, 0)?;
        let mut decision = super::requirements(&profile, &side);
        let score = super::cf_score(&g["profile"], cand);
        if score != 0 {
            decision.log_with_source(
                "custom_format_score",
                score,
                ScoringSource::UserRule {
                    id: "custom_format_score".into(),
                    name: "custom_format_score".into(),
                },
            );
        }
        apply_min_score_gate(&profile, &mut decision);
        // As release_search.rs sets it: production `revision_preference` over
        // the title subject (empty here: no file on disk).
        let facts = CandidateFacts::new(decision.tier_index, side.revision, decision.preference_score)
            .with_profile_policy(side.group.as_deref(), &profile.criteria);
        decision.revision_preference =
            revision_preference(facts, &AdmissionSubject::new(AdmissionScope::Title, Vec::<(Incumbent, bool)>::new()));
        let size_mb = cand["size_mb"].as_i64().unwrap_or(0);
        let age = cand["age_days"].as_i64().unwrap_or(0);
        ranked.push((
            label.clone(),
            IndexerSearchResult {
                indexer_id: None,
                source: String::new(),
                title: label.clone(),
                link: None,
                download_url: Some(format!("parity://{label}")),
                source_kind: super::source_kind(&cand["protocol"]),
                size_bytes: Some(size_mb * 1_048_576),
                published_at: Some((super::now() - chrono::Duration::days(age)).to_rfc3339()),
                thumbs_up: None,
                thumbs_down: None,
                indexer_languages: None,
                indexer_subtitles: None,
                indexer_grabs: None,
                password_hint: None,
                parsed_release_metadata: Some(super::parsed(&side)),
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
            },
        ));
    }
    ranked.sort_by(|a, b| compare_release_search_results(&a.1, &b.1));

    // Guard: a head tie must leave every SearchRank listing step tied too.
    let list = g[key].as_array().unwrap();
    let facts = |label: &str| {
        let c = list.iter().find(|c| c["label"] == label).unwrap();
        ["protocol", "episodes", "full_season", "indexer_priority", "seeders", "peers", "age_days", "size_mb"]
            .map(|k| c[k].clone())
    };
    for pair in ranked.windows(2) {
        if compare_release_search_results(&pair[0].1, &pair[1].1) == Ordering::Equal {
            assert_eq!(
                facts(&pair[0].0),
                facts(&pair[1].0),
                "harness bug: head tie with differing listing facts ({} vs {})",
                pair[0].0,
                pair[1].0
            );
        }
    }

    // A head tie at the top: Scryer has no preference there.
    let top = match &ranked[..] {
        [a, b, ..] if compare_release_search_results(&a.1, &b.1) == Ordering::Equal => {
            format!("tie (no Scryer preference): {} = {}", a.0, b.0)
        }
        [a, ..] => a.0.clone(),
        [] => return Err("no candidates".into()),
    };
    Ok(if order { json!({ "order": [top] }) } else { json!({ "first": top }) })
}
