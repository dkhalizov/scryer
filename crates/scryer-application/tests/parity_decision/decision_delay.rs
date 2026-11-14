//! Parity net, stream `decision_delay`: the DelaySpecification cases whose
//! verdict rests on facts Scryer computes inside `automatic_candidate_delay_decision`
//! (highest quality in the profile with bypass on, same-tier revision over the
//! file on disk). The root `delay()` skips these.
//!
//! Suites (nextest names `decision_delay::<fn>`):
//!
//! - `son_decision_delay`: SON-DECISION-1174, 1177, 1178, 1179
//!   (`RssSync.DelaySpecificationFixture.json`).
//! - `rad_decision_delay`: RAD-DECISION-0081, 0084, 0085, 0086
//!   (`DelaySpecificationFixture.json`).
//!
//! Selection (before `wants`): the two `None` branches of the root `delay()`,
//! i.e. `delay_profile.bypass_if_highest_quality == true` or a file on disk
//! (Sonarr: an episode with a non-zero `episode_file_id`; Radarr:
//! `existing_file` or `stubs`).
//!
//! # Scryer function under test
//!
//! `automatic_candidate_delay_decision` (`acquisition/release_search.rs`, seam
//! in `parity_seams`): candidate tier index == best tier, `revision_upgrade_over`
//! the incumbents, `resolve_delay_profile`, then the delay evaluation.
//!
//! # Translation table (reference term -> Scryer term)
//!
//! - DelayProfile UsenetDelay/TorrentDelay (minutes) -> `usenet_delay_minutes` /
//!   `torrent_delay_minutes`; PreferredProtocol -> `PreferredProtocol`;
//!   BypassIfHighestQuality -> `bypass_if_highest_quality` (absent = false).
//!   The profile is resolved from a one-profile list with no tags, facets or
//!   libraries.
//! - Custom-format score (`release.custom_format_score`,
//!   BypassIfAboveCustomFormatScore + MinimumCustomFormatScore): not translated
//!   here. The candidate carries no `quality_profile_decision`, so the fn sees
//!   score 0; a case that sets either key is recorded red ("custom-format score
//!   not translated in this stream") rather than decided on a fake score. No
//!   selected case sets them; the score cases (SON 1180-1182, RAD 0087-0089)
//!   stay with the root suites.
//! - Release protocol -> `source_kind`; `age_minutes` (or `age_hours` * 60) ->
//!   `published_at` = NOW - age (RFC3339); no age -> no `published_at`.
//! - Quality names, revision v -> Scryer v - 1, `real` > 0 -> red: the root
//!   `side()` table. Profile -> `QualityProfile` through the root
//!   `fixture_spec` + `build_profile`.
//! - Files on disk -> `Incumbent` (`super::incumbent`) in an `AdmissionSubject`
//!   (Sonarr scope `Episodes`, Radarr scope `Title`).
//! - A group-less release or file gets the group `PARITY` (`super::PROPER_GROUP`):
//!   the reference revision check (IsRevisionUpgrade) never reads a group,
//!   Scryer's revision bypass needs the same group on both sides (else the group
//!   gate, not the revision, would decide). The Sonarr file's `scene_name`
//!   (`...x264-Sonarr`, group `Sonarr`) is deliberately not used for the group:
//!   keeping it would turn 1177 red and 1179 green, both for a group reason.
//! - UserInvokedSearch -> `user_invoked`; OldestPendingRelease -> None
//!   (`pending_releases` is always empty).
//! - Comparison: `{accepted: reason not in {PendingDelay, ReleaseAgeUnknown}}`,
//!   the root `delay()` mapping. No delay profile resolved -> red.
//!
//! Known reds: a REAL revision has no Scryer field (SON 1178, RAD 0085);
//! Scryer tiers are resolution-only, so WEBDL-720p vs HDTV-720p is a same-tier
//! revision and bypasses (SON 1179).
//!
//! The same tier collapse weakens the highest-quality greens (SON 1174,
//! RAD 0081): HDTV/WEBDL/Bluray-720p are all tier `720P` (index 0), so any 720p
//! release would pass. They confirm the bypass flag (the bypass-off twins 1173 /
//! 0080 in the root suites go the other way), not the LastAllowedQuality
//! comparison.
//!
//! # Ignored keys
//!
//! `release.languages`, `episodes[].file.languages`, `episodes[].id`,
//! `pending_releases`, `note`, `profile.cutoff` (no effect on the delay),
//! Radarr `stubs` (verdict computed from the real inputs; the stubbed values
//! match what they give), `search.kind`.
//!
//! # Unmapped (tests: []), with reasons
//!
//! None: every selected case is mapped. The other Delay cases stay with the
//! root suites.

use scryer_application::parity_seams::{
    AdmissionScope, AdmissionSubject, DelayDecisionReason, automatic_candidate_delay_decision,
};
use scryer_application::{DelayProfile, IndexerSearchResult};
use scryer_domain::{MediaFacet, Title};
use scryer_parity::{Suite, corpus};
use serde_json::Value;

#[test]
fn son_decision_delay() {
    delay_suite(
        "scryer-application::parity_decision::son_decision_delay",
        "corpus/reference/sonarr/decision/RssSync.DelaySpecificationFixture.json",
        false,
    );
}

#[test]
fn rad_decision_delay() {
    delay_suite(
        "scryer-application::parity_decision::rad_decision_delay",
        "corpus/reference/radarr/decision/DelaySpecificationFixture.json",
        true,
    );
}

fn selected(radarr: bool, g: &Value) -> bool {
    if g["delay_profile"]["bypass_if_highest_quality"].as_bool() == Some(true) {
        return true;
    }
    if radarr {
        !g["existing_file"].is_null() || !g["stubs"].is_null()
    } else {
        g["episodes"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|e| e["episode_file_id"].as_i64().unwrap_or(0) != 0)
    }
}

fn delay_suite(name: &str, file: &str, radarr: bool) {
    let Some(mut suite) = Suite::open(name) else { return };
    let cases: Vec<Value> = corpus(file);
    for case in &cases {
        let g = &case["input"]["given"];
        if !selected(radarr, g) {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let outcome = delay_case(radarr, g);
        super::record(&mut suite, id, &case["expect"], Some(outcome));
    }
    suite.finish();
}

fn delay_case(radarr: bool, g: &Value) -> Result<Value, String> {
    let d = &g["delay_profile"];
    if !g["release"]["custom_format_score"].is_null()
        || d["bypass_if_above_custom_format_score"].as_bool() == Some(true)
    {
        return Err("custom-format score not translated in this stream".into());
    }
    let profile = super::build_profile(&super::fixture_spec(&g["profile"], &g["config"])?)?;
    let release = &g["release"];
    let (release_fact, facet) = if radarr {
        (&g["parsed_quality"], MediaFacet::Movie)
    } else {
        (release, MediaFacet::Series)
    };
    let release_group = if radarr { &g["parsed_release_group"] } else { &release["release_group"] };
    let release_side = super::side(release_fact, super::proper_group(release_group), 0)?;

    let (scope, incumbents) = if radarr {
        let mut v = Vec::new();
        if !g["existing_file"].is_null() {
            let f = &g["existing_file"];
            let s = super::side(f, super::proper_group(&f["release_group"]), 0)?;
            v.push(super::incumbent(&profile, "f1", &s, None, 0));
        }
        (AdmissionScope::Title, v)
    } else {
        let (members, held) = super::episode_files(g, &g["profile"])?;
        let v = held
            .into_iter()
            .map(|mut h| {
                h.side.group.get_or_insert_with(|| super::PROPER_GROUP.into());
                super::incumbent(&profile, &h.id, &h.side, h.episode.as_deref(), h.added_days_ago)
            })
            .collect();
        (AdmissionScope::Episodes(members), v)
    };
    let subject = AdmissionSubject::new(scope, incumbents.into_iter().map(|i| (i, true)));

    let age = release["age_minutes"]
        .as_i64()
        .or_else(|| release["age_hours"].as_i64().map(|h| h * 60));
    let candidate = IndexerSearchResult {
        indexer_id: None,
        source: "parity".into(),
        title: "parity-release".into(),
        link: None,
        download_url: None,
        source_kind: super::source_kind(&release["protocol"]),
        size_bytes: None,
        published_at: age.map(|m| (super::now() - chrono::Duration::minutes(m)).to_rfc3339()),
        thumbs_up: None,
        thumbs_down: None,
        indexer_languages: None,
        indexer_subtitles: None,
        indexer_grabs: None,
        password_hint: None,
        parsed_release_metadata: Some(super::parsed(&release_side)),
        quality_profile_decision: None,
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
    };

    let delay_profile = DelayProfile {
        usenet_delay_minutes: d["usenet_delay"].as_i64().unwrap_or(0),
        torrent_delay_minutes: d["torrent_delay"].as_i64().unwrap_or(0),
        preferred_protocol: super::preferred(&d["preferred_protocol"]),
        bypass_if_highest_quality: d["bypass_if_highest_quality"].as_bool().unwrap_or(false),
        ..DelayProfile::default()
    };
    let title = title(facet);
    let user_invoked = g["search"]["user_invoked"].as_bool().unwrap_or(false);
    let decision = automatic_candidate_delay_decision(
        &candidate,
        &title,
        &subject,
        &profile,
        &[delay_profile],
        user_invoked,
        None,
        &super::now(),
    )
    .ok_or("no delay profile resolved")?;
    Ok(super::accepted(!matches!(
        decision.reason,
        DelayDecisionReason::PendingDelay | DelayDecisionReason::ReleaseAgeUnknown
    )))
}

fn title(facet: MediaFacet) -> Title {
    Title {
        id: "parity-title".into(),
        library_id: scryer_domain::default_library_id_for_facet(&facet),
        name: "parity".into(),
        facet,
        monitored: true,
        tags: vec![],
        external_ids: vec![],
        root_folder_id: scryer_domain::root_folder_id_for_path("/parity"),
        created_by: None,
        created_at: super::now(),
        year: None,
        overview: None,
        poster_url: None,
        poster_source_url: None,
        background_url: None,
        background_source_url: None,
        sort_title: None,
        catalog_sort_key: String::new(),
        slug: None,
        imdb_id: None,
        runtime_minutes: None,
        popularity: None,
        canonical_tags: vec![],
        content_status: None,
        language: None,
        first_aired: None,
        network: None,
        studio: None,
        country: None,
        aliases: vec![],
        tagged_aliases: vec![],
        metadata_language: None,
        metadata_fetched_at: None,
        min_availability: None,
        digital_release_date: None,
        movie_release_dates: None,
        folder_path: None,
    }
}
