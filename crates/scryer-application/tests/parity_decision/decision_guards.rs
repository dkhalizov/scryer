//! Parity net, stream `decision_guards`: grab-decision cases the root harness
//! skips because the reference spec "stands aside" (guard) or because the
//! outcome rests on which rows the loader counts (history, queue state).
//!
//! Suites (nextest names `decision_guards::<fn>`):
//!
//! - `son_decision_history`: SON-DECISION-1189..1203
//!   (`RssSync.HistorySpecificationFixture.json`, every case).
//! - `rad_decision_history`: RAD-DECISION-0048..0058
//!   (`HistorySpecificationFixture.json`, every case).
//! - `son_decision_guards`: SON-DECISION-0049, 0050, 1092, 1093, 1214, 1215, 1218
//!   (Repack, AnimeVersionUpgrade, Proper fixtures).
//! - `rad_decision_guards`: RAD-DECISION-1079, 1080, 1152, 1153
//!   (Repack, Proper fixtures).
//! - `son_decision_queue_state`: SON-DECISION-1055.
//! - `rad_decision_queue_state`: RAD-DECISION-1042.
//!
//! # Scryer functions under test
//!
//! - History: the loader's row selection `grabbed_release_record` (no age
//!   bound) then `recent_missing_grab_record` (12 h window, missing downloads
//!   only), in the order `grabbed_release_claims_for_scope` calls them for the
//!   RSS lane. Each claim becomes a `QueuedRelease`; the verdict is the root
//!   `grab_refusal` (cutoff gate, then `evaluate_admission`).
//! - Guards: the root `grab_refusal` ladder (Repack, AnimeVersion: whole
//!   verdict; Proper: only `ProperForOldFile`, as the root `proper()`).
//! - Queue state: `submission_is_queued` (the loader's liveness filter), then
//!   the root `queue()` on the live items.
//!
//! # Translation table (reference term -> Scryer term)
//!
//! - Profile, quality names, revision v -> v - 1, groups, `release`/
//!   `parsed_quality`: the root `fixture_spec`, `build_profile`, `side`.
//! - `config.enable_completed_download_handling` true or absent: Scryer's only
//!   mode (it tracks every grab it submits). `false`: Scryer has no setting, so
//!   the case is recorded red before anything is built (D29).
//! - `most_recent_history` (Sonarr: map episode id -> row, `event_type`;
//!   Radarr: one row or null, `event`): `Grabbed` -> scope row status
//!   `Grabbed`; `DownloadFailed` -> status `Wanted` with no missing-download
//!   id (Scryer's failure handler moves the row out of grabbed), so the seams
//!   return no claim; any other event -> red. Age = `age_hours`, or
//!   `age_days` * 24, or 0, written as `grabbed_at` = NOW - age.
//! - A claim -> `QueuedRelease`, harness glue: the loader re-parses the
//!   record title (`score_parked_release_title`) for tier, revision, score and
//!   group and sets `release_key(&title)`; a history row has no title to
//!   parse, so tier, revision and score come from the row, `release_group` is
//!   None (the reference rows carry none), `release_key` is None (inert: the
//!   root candidate has no key either), default language. Status is Grabbed,
//!   so `missing_history` and `history_cutoff_reached` are false, as in the
//!   loader. Sonarr covers = the history map key; Radarr covers = [].
//! - Sonarr rows pass the loader's scope filter (`submission_scope_intersects`
//!   of `SubmissionScope::Episode { key }` with the episode membership), as
//!   `grabbed_release_claims_for_scope` does; today every history key is a
//!   member.
//! - `search` Sonarr: null = RSS; Radarr: `search.kind == "rss"`.
//! - Queue `TrackedDownloadState` names map 1:1 to `scryer_domain::TrackedDownloadState`
//!   (explicit match). Items the seam says are not live are dropped; live ones
//!   get state "Downloading" so the root `queue()` builds the pseudo-incumbent.
//! - Guards: the root Repack / AnimeVersion / Proper translation unchanged
//!   (`PROPER_GROUP` fill, no file when `episode_file_id` / `movie_file_id` is 0).
//!
//! Selection (before `wants`): history cases by file; guard cases are the
//! ones the root selectors return `None` for, minus the cases that are not
//! comparable (DoNotPrefer, or a file the release is not a same-quality newer
//! revision of, where the reference's own pipeline refuses but its spec stands
//! aside). Queue cases: the root `queue()` returns `None` (state other than
//! Downloading).
//!
//! # Ignored keys
//!
//! `languages` (release and history), `download_id`, `custom_formats` (all
//! empty), `search.kind` beyond rss-or-not, `episodes[].season/episode`,
//! `movie.id`, `series.series_type` (Scryer has no anime-only branch), `note`,
//! `scene_name`.
//!
//! # Known reds
//!
//! - SON 1214, RAD 1152: SDTV and DVD share Scryer's 480P tier, so DVD v2 reads
//!   as a PROPER of a 30-day-old SDTV file (`ProperForOldFile`).
//! - SON 1192, RAD 0051: a grab 13 h old still claims the scope (no age bound).
//! - SON 1193, 1194, RAD 0052: SDTV and DVD collapse into Scryer's 480P tier,
//!   so DVD v2 is no quality upgrade over the SDTV grab and
//!   `QueuedEqualOrBetter` refuses. The group-less claim (glue above; a
//!   queued release carries no exact quality, P30b) also keeps it from
//!   counting as a related revision; with a shared group Scryer
//!   would admit it as a revision upgrade, still not for the reference's
//!   reason (a better quality).
//! - SON 1201-1203, RAD 0056-0058: no setting for completed download handling off.
//!
//! # Weak greens
//!
//! No-file and no-history greens (SON 1092, 1093, 0049, 1189, 1190; RAD 1079,
//! 1080, 0048, 0049) decide by "nothing to compare"; they are non-vacuous
//! only through the guard-active siblings on the same `grab_refusal` path
//! (root SON 1096-1098, RAD 1083-1085, and the refused history
//! cases here). SON 1191 and RAD 0050 are decided by the DownloadFailed ->
//! Wanted translation (no claim), which matches the failure handler's reopen
//! (`post_download_gate.rs` `reset_scopes_for_retry`); `missing_download_id`
//! is written only for Missing downloads (`tracked_commands.rs`), and the
//! Grabbed siblings on the same path are refused. SON 1218 holds two guards
//! (DoNotPrefer and a fresh file). The
//! History search guard is not exercised: no search case carries a grab.
//!
//! # Unmapped (tests: []), with reasons
//!
//! - SON-DECISION-1099, RAD-DECISION-1088, SON-DECISION-0056: DoNotPrefer
//!   applicability guard. The reference's own `IsUpgradable` refuses the same
//!   release: DoNotPrefer skips the revision accept, and the fixtures set no
//!   profile, so the reference model's UpgradeAllowed false gives
//!   UpgradesNotAllowed (Sonarr UpgradableSpecification.cs:53-64; with
//!   upgrades on, CustomFormatScore at :86-93). Scryer's ladder refuses too;
//!   Scryer has no separate repack/version refusal, so a whole-verdict red
//!   would record agreement as a gap.
//! - SON-DECISION-0051: not-a-newer-revision guard (anime v1 vs cross-group v1
//!   file): the reference's whole pipeline refuses it too (same quality and
//!   revision, upgrades off: UpgradesNotAllowed, UpgradableSpecification.cs:60-64);
//!   Scryer has no AnimeVersionUpgrade-only verdict.
//! - SON-DECISION-1094, RAD-DECISION-1081: different-quality guard (repack SDTV
//!   v2 vs DVD v1 file): the reference's UpgradeDisk refuses it, Scryer's whole
//!   verdict would admit it (shared 480P tier); no repack-only gate to compare.
//!   The tier collapse is already red in the Upgrade families.

use scryer_application::parity_seams::{
    QueuedRelease, ReleaseAutoDecisionCode, grabbed_release_record, recent_missing_grab_record,
    submission_is_queued,
};
use scryer_application::{AcquisitionScopeState, AcquisitionScopeStatus, AudioLanguageRank};
use scryer_domain::TrackedDownloadState;
use scryer_parity::{Suite, corpus};
use serde_json::{Value, json};

use super::{Held, Refusal, Side};

const SON: &str = "corpus/reference/sonarr/decision/";
const RAD: &str = "corpus/reference/radarr/decision/";

#[test]
fn son_decision_history() {
    history_suite("son_decision_history", &format!("{SON}RssSync.HistorySpecificationFixture.json"), false);
}

#[test]
fn rad_decision_history() {
    history_suite("rad_decision_history", &format!("{RAD}HistorySpecificationFixture.json"), true);
}

#[test]
fn son_decision_guards() {
    guards_suite("son_decision_guards", false);
}

#[test]
fn rad_decision_guards() {
    guards_suite("rad_decision_guards", true);
}

#[test]
fn son_decision_queue_state() {
    queue_suite("son_decision_queue_state", &format!("{SON}QueueSpecificationFixture.json"), false);
}

#[test]
fn rad_decision_queue_state() {
    queue_suite("rad_decision_queue_state", &format!("{RAD}QueueSpecificationFixture.json"), true);
}

fn suite(name: &str) -> Option<Suite> {
    Suite::open(&format!("scryer-application::parity_decision::{name}"))
}

// ── History ────────────────────────────────────────────────────────────────

fn history_suite(name: &str, file: &str, radarr: bool) {
    let Some(mut suite) = suite(name) else { return };
    let cases: Vec<Value> = corpus(file);
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let outcome = history_case(radarr, &case["input"]["given"]);
        super::record(&mut suite, id, &case["expect"], Some(outcome));
    }
    suite.finish();
}

fn history_case(radarr: bool, g: &Value) -> Result<Value, String> {
    if g["config"]["enable_completed_download_handling"].as_bool() == Some(false) {
        return Err("no Scryer setting for completed download handling off".into());
    }
    let p = &g["profile"];
    let (release, members, is_rss, event_key) = if radarr {
        for k in ["release_custom_formats", "history_custom_formats"] {
            if g[k].as_array().is_some_and(|a| !a.is_empty()) {
                return Err("custom formats not translated in this stream".into());
            }
        }
        (
            super::side(&g["parsed_quality"], None, 0)?,
            None,
            g["search"]["kind"].as_str() == Some("rss"),
            "event",
        )
    } else {
        let members: Vec<String> = g["episodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| e["id"].as_i64().map(|n| n.to_string()))
            .collect();
        (
            super::side(&g["release"], None, super::cf_score(p, &g["release"]))?,
            Some(members),
            g["search"].is_null(),
            "event_type",
        )
    };
    // (scope key, row) pairs; Radarr has one row for the title.
    let rows: Vec<(Option<String>, &Value)> = match &g["most_recent_history"] {
        Value::Object(map) if radarr => {
            if map.is_empty() { vec![] } else { vec![(None, &g["most_recent_history"])] }
        }
        Value::Object(map) => map.iter().map(|(k, v)| (Some(k.clone()), v)).collect(),
        _ => vec![],
    };
    // History items name their quality even when the loader would skip them.
    let mut named = vec![release.q];
    named.extend(rows.iter().filter_map(|(_, item)| super::side(item, None, 0).ok()).map(|h| h.q));
    let profile = super::build_profile(&super::fixture_spec(p, &g["config"])?, &named)?;
    let mut queued = Vec::new();
    for (key, item) in rows {
        // The loader's scope filter (canonical_context.rs grabbed_release_claims_for_scope).
        if let (Some(k), Some(ids)) = (&key, &members) {
            let scope = super::SubmissionScope::Episode { episode_id: k.clone() };
            let membership = super::ScopeMembership { episode_ids: ids, ..Default::default() };
            if !super::submission_scope_intersects(&scope, &membership) {
                continue;
            }
        }
        let status = match item[event_key].as_str() {
            Some("Grabbed") => AcquisitionScopeStatus::Grabbed,
            Some("DownloadFailed") => AcquisitionScopeStatus::Wanted,
            other => return Err(format!("history event {other:?} not translated")),
        };
        let age_hours = item["age_hours"]
            .as_i64()
            .or_else(|| item["age_days"].as_i64().map(|d| d * 24))
            .unwrap_or(0);
        let h = super::side(item, None, 0)?;
        let grabbed_release = json!({
            "title": format!("parity history {}", h.name),
            "grabbed_at": (super::now() - chrono::Duration::hours(age_hours)).to_rfc3339(),
        })
        .to_string();
        let state = scope_state(radarr, key.clone(), status, grabbed_release);
        let record = match grabbed_release_record(&state) {
            Some(r) => Some(r),
            None => recent_missing_grab_record(&state, super::now()).map_err(|e| format!("{e:?}"))?,
        };
        let Some(record) = record else { continue };
        if status != AcquisitionScopeStatus::Grabbed {
            return Err("translation cannot express a missing-download claim".into());
        }
        queued.push(QueuedRelease {
            missing_history: false,
            history_cutoff_reached: false,
            release_group: None,
            title: record.title,
            covers: key.into_iter().collect(),
            tier_index: super::tier_index(&profile, &h),
            revision: h.revision,
            score: h.score,
            language: AudioLanguageRank::default(),
            release_key: None,
        });
    }
    let refusal = super::grab_refusal(&profile, &release, &[], members.as_deref(), queued, is_rss);
    Ok(super::accepted(refusal.is_none()))
}

fn scope_state(
    radarr: bool,
    episode_id: Option<String>,
    status: AcquisitionScopeStatus,
    grabbed_release: String,
) -> AcquisitionScopeState {
    AcquisitionScopeState {
        id: "scope-1".into(),
        title_id: "title-1".into(),
        title_name: None,
        title_slug: None,
        title_facet: None,
        library_id: None,
        library_name: None,
        library_slug: None,
        episode_id,
        collection_id: None,
        series_movie_link_id: None,
        season_number: None,
        episode_number: None,
        media_type: if radarr { "movie" } else { "episode" }.into(),
        last_search_at: None,
        status,
        grabbed_release: Some(grabbed_release),
        landed_bar: None,
        latest_release_decision: None,
        mismatch_recovery_eligible: false,
        created_at: super::NOW.into(),
        updated_at: super::NOW.into(),
    }
}

// ── Guards ─────────────────────────────────────────────────────────────────

type Parts = (Side, Option<Vec<String>>, Vec<Held>);

fn guards_suite(name: &str, radarr: bool) {
    let Some(mut suite) = suite(name) else { return };
    let (dir, repack_file, proper_file, anime_file) = if radarr {
        (RAD, "RepackSpecificationFixture.json", "ProperSpecificationFixture.json", None)
    } else {
        (
            SON,
            "RepackSpecificationFixture.json",
            "RssSync.ProperSpecificationFixture.json",
            Some("AnimeVersionUpgradeSpecificationFixture.json"),
        )
    };
    let app = if radarr { "radarr" } else { "sonarr" };
    let mut run = |file: &str, kind: &str| {
        let cases: Vec<Value> = corpus(&format!("{dir}{file}"));
        for case in &cases {
            let g = &case["input"]["given"];
            let selected = match kind {
                "repack" => super::repack(app, g).is_none() && repack_comparable(app, g),
                "anime" => super::anime_version(g).is_none() && anime_comparable(g),
                _ => super::proper(app, g).is_none(),
            };
            let id = case["id"].as_str().unwrap();
            if !selected || !suite.wants(id) {
                continue;
            }
            let outcome = match kind {
                "repack" => repack_case(app, g),
                "anime" => anime_case(g),
                _ => proper_case(app, g),
            };
            super::record(&mut suite, id, &case["expect"], Some(outcome));
        }
    };
    run(repack_file, "repack");
    run(proper_file, "proper");
    if let Some(f) = anime_file {
        run(f, "anime");
    }
    suite.finish();
}

fn repack_parts(app: &str, g: &Value) -> Result<Parts, String> {
    let p = &g["profile"];
    if app == "radarr" {
        let release = super::side(&g["parsed_quality"], super::group(&g["parsed_release_group"]), 0)?;
        let mut held = Vec::new();
        let file = &g["existing_file"];
        if g["movie"]["movie_file_id"].as_i64().unwrap_or(0) != 0 && !file.is_null() {
            held.push(Held {
                id: "f1".into(),
                side: super::side(file, super::group(&file["release_group"]), 0)?,
                episode: None,
                added_days_ago: 0,
            });
        }
        Ok((release, None, held))
    } else {
        let r = &g["release"];
        let release = super::side(r, super::group(&r["release_group"]), 0)?;
        let (members, held) = super::episode_files(g, p)?;
        Ok((release, Some(members), held))
    }
}

fn anime_parts(g: &Value) -> Result<Parts, String> {
    let r = &g["release"];
    let release = super::side(r, super::group(&r["release_group"]), 0)?;
    let has_file = g["episodes"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|e| e["episode_file_id"].as_i64().unwrap_or(0) != 0);
    let mut held = Vec::new();
    if has_file {
        let f = &g["existing_file"];
        held.push(Held {
            id: "f1".into(),
            side: super::side(f, super::group(&f["release_group"]), 0)?,
            episode: None,
            added_days_ago: 0,
        });
    }
    Ok((release, None, held))
}

/// Not DoNotPrefer, and either no file or a same-quality newer revision of one.
fn comparable(ignore: Result<bool, String>, parts: Result<Parts, String>) -> bool {
    match (ignore, parts) {
        (Ok(false), Ok((release, _, held))) => {
            held.is_empty() || held.iter().any(|h| super::is_revision_upgrade(&release, &h.side))
        }
        _ => false,
    }
}

fn repack_comparable(app: &str, g: &Value) -> bool {
    comparable(super::propers(&g["config"]["download_propers_and_repacks"]), repack_parts(app, g))
}

fn anime_comparable(g: &Value) -> bool {
    comparable(super::propers(&g["config"]["download_propers_and_repacks"]), anime_parts(g))
}

fn whole_verdict(g: &Value, (release, members, held): Parts) -> Result<Value, String> {
    let named: Vec<_> = std::iter::once(release.q).chain(held.iter().map(|h| h.side.q)).collect();
    let profile = super::build_profile(&super::fixture_spec(&g["profile"], &g["config"])?, &named)?;
    let refusal = super::grab_refusal(&profile, &release, &held, members.as_deref(), Vec::new(), false);
    Ok(super::accepted(refusal.is_none()))
}

fn repack_case(app: &str, g: &Value) -> Result<Value, String> {
    whole_verdict(g, repack_parts(app, g)?)
}

fn anime_case(g: &Value) -> Result<Value, String> {
    whole_verdict(g, anime_parts(g)?)
}

fn proper_case(app: &str, g: &Value) -> Result<Value, String> {
    let p = &g["profile"];
    let radarr = app == "radarr";
    let is_rss = if radarr { g["search"]["kind"].as_str() == Some("rss") } else { g["search"].is_null() };
    let (release, members, held): Parts = if radarr {
        let release = super::side(&g["parsed_quality"], super::proper_group(&g["parsed_release_group"]), 0)?;
        let file = &g["existing_file"];
        let held = vec![Held {
            id: "f1".into(),
            side: super::side(file, super::proper_group(&file["release_group"]), 0)?,
            episode: None,
            added_days_ago: file["added_days_ago"].as_i64().unwrap_or(0),
        }];
        (release, None, held)
    } else {
        let r = &g["release"];
        let release = super::side(r, super::proper_group(&r["release_group"]), 0)?;
        let (members, mut held) = super::episode_files(g, p)?;
        for h in &mut held {
            h.side.group.get_or_insert_with(|| super::PROPER_GROUP.into());
        }
        (release, Some(members), held)
    };
    let named: Vec<_> = std::iter::once(release.q).chain(held.iter().map(|h| h.side.q)).collect();
    let profile = super::build_profile(&super::fixture_spec(p, &g["config"])?, &named)?;
    let refusal = super::grab_refusal(&profile, &release, &held, members.as_deref(), Vec::new(), is_rss);
    Ok(super::accepted(!matches!(
        refusal,
        Some(Refusal::Cutoff(ReleaseAutoDecisionCode::ProperForOldFile))
    )))
}

// ── Queue state ────────────────────────────────────────────────────────────

fn queue_suite(name: &str, file: &str, radarr: bool) {
    let Some(mut suite) = suite(name) else { return };
    let app = if radarr { "radarr" } else { "sonarr" };
    let cases: Vec<Value> = corpus(file);
    for case in &cases {
        let g = &case["input"]["given"];
        let id = case["id"].as_str().unwrap();
        if super::queue(app, g).is_some() || !suite.wants(id) {
            continue;
        }
        super::record(&mut suite, id, &case["expect"], Some(queue_case(radarr, g)));
    }
    suite.finish();
}

fn tracked_state(name: &str) -> Result<TrackedDownloadState, String> {
    Ok(match name {
        "Downloading" => TrackedDownloadState::Downloading,
        "ImportPending" => TrackedDownloadState::ImportPending,
        "Importing" => TrackedDownloadState::Importing,
        "ImportBlocked" => TrackedDownloadState::ImportBlocked,
        "Imported" => TrackedDownloadState::Imported,
        "FailedPending" => TrackedDownloadState::FailedPending,
        "Failed" => TrackedDownloadState::Failed,
        "Ignored" => TrackedDownloadState::Ignored,
        other => return Err(format!("queue state {other:?} not translated")),
    })
}

fn queue_case(radarr: bool, g: &Value) -> Result<Value, String> {
    let key = if radarr { "tracked_download_state" } else { "state" };
    let mut live = Vec::new();
    for item in g["queue"].as_array().into_iter().flatten() {
        let state = tracked_state(item[key].as_str().ok_or("queue item has no state")?)?;
        if submission_is_queued(Some(state), false) {
            let mut item = item.clone();
            item[key] = json!("Downloading");
            live.push(item);
        }
    }
    let mut g2 = g.clone();
    g2["queue"] = Value::Array(live);
    super::queue(if radarr { "radarr" } else { "sonarr" }, &g2)
        .unwrap_or_else(|| Err("queue item state not translated".into()))
}
