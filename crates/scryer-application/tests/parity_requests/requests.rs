//! Parity net, area `requests`, stream `requests` (Stages 6-7).
//!
//! Suites and mapped IDs:
//! - `ser_requests_seasons`: SER-REQUESTS-0073, 0074 (source.json,
//!   `request.requested_seasons`). Scryer: `normalize_requested_monitor_selection`
//!   (request submission step, media_requests.rs:2532) then `should_monitor_season`
//!   (catalog/workflow/monitoring.rs:1340, called from collections.rs:473).
//! - `ser_requests_season_status`: SER-REQUESTS-0084, 0085 (source.json,
//!   `request.create`, `expect.season_status`). Scryer: the root's
//!   `scryer_status` (`has_library_permission` + `request_rules::legacy_outcome`).
//! - `ser_requests_watchlist`: SER-REQUESTS-0047 (watchlistsync.json).
//!   Scryer: `lists::evaluate::evaluate`.
//! - `ser_requests_watchlist_rerequest`: SER-REQUESTS-0048 (watchlistsync.json).
//!   Scryer: `lists::evaluate::evaluate` with the earlier request's membership
//!   row. The sync's `deleted_additions` step (lists/sync.rs:652) runs before
//!   `evaluate`, but it only re-weighs rows in state `Added` (sync.rs:662), so
//!   for this row it is a no-op and `evaluate` alone is the production answer.
//!
//! Translation table, Seerr -> Scryer:
//!
//! | Seerr | Scryer |
//! |---|---|
//! | seasons "all" | monitor type "allepisodes", no selection |
//! | seasons [n,...] | monitor type "advanced" with `MonitorSelection { seasons }`, normalised |
//! | enableSpecialEpisodes | `monitor_specials` argument |
//! | tmdb_seasons[].season_number | candidate seasons (the title's metadata seasons); with none, the range 0..=max(requested)+1, so the selection must drop the unrequested seasons (the requested list is never the candidate set) |
//! | Seerr permission names | the root's `authorization` grants (REQUEST -> Request, AUTO_APPROVE -> AutoApproveRequests per library); the status itself is `has_library_permission` + `legacy_outcome`, production code |
//! | requested_seasons | sorted candidate seasons for which `should_monitor_season` is true |
//! | request season_status | the request's status (a series request is one row; its seasons share it) |
//! | Plex watchlist sync | personal `ListSubscription` (plex, watchlist, mode Request) |
//! | media AVAILABLE | `ResolvedItem.library_title_id` set (InLibrary) |
//! | media BLOCKLISTED | `ListExclusion` scope AllLists on the tmdb id |
//! | media UNKNOWN / DELETED | no library title, no membership |
//! | requested | items decided `ItemDecision::Candidate` |
//! | earlier request COMPLETED, is_auto_request, by a user without auto-approve | membership row in state `InLibrary` with `title_id` set: a Request-mode member's item is filed `Requested` (lists/act.rs:231-236), and the sync after fulfilment rewrites it to `InLibrary` (evaluate.rs:142-155, sync.rs:521). The title id survives the title's deletion (list_memberships.title_id has no FK). A row still `Requested` with no title id gives the same `Keep` (evaluate.rs:158) |
//! | request_calls_for_movie_100 | number of `Candidate` decisions for item `movie:100` |
//!
//! Ignored keys: `tmdb_seasons[].episode_count` (Scryer has no empty-season
//! rule above season 0; that is why 0073 is red), `body.is4k`, `body.seasons`
//! (0084/0085), `user.permissions`, `user.plex_token` (0047, 0048),
//! `requests[].is4k`, `requests[].requested_by` (0048),
//! `expect.each_requested_once` (0047: `evaluate` returns one decision per
//! item, the sync dedupes the fetched list and a second item with the same
//! SMG target is Filtered `duplicate_target` (evaluate.rs:76-82); with the
//! case's distinct items the comparison could not fail, so it is not made).
//!
//! Expected reds: 0073 (Scryer monitors empty season 3), 0074 (under "advanced"
//! the selection replaces the monitor-specials rule by design, monitoring.rs:1346-1349;
//! waiver candidate for the user), 0048 (Scryer keeps the earlier request's
//! membership (`Keep { InLibrary }`, evaluate.rs:116-127) and does not request
//! the deleted movie again: 0 calls, Seerr 1).
//!
//! Unmapped (tests: []), with reasons:
//! - SER-REQUESTS-0002..0005: concurrent/duplicate submissions are decided by the
//!   SQL store (scryer-infrastructure-library src/media/requests.rs:60), not an
//!   application function; `AppServices::builder` needs 10 repository objects and
//!   a stateful `MediaRequestRepository` (only the SQL store and cfg(test) doubles).
//! - SER-REQUESTS-0007..0012: delete permissions are inline checks in async
//!   `cancel_my_media_request` / `dismiss_media_request` after a repository get.
//! - SER-REQUESTS-0013..0019, 0021, 0022: edits run through async
//!   `update_my_media_request` / `approve_media_request`; server, root folder,
//!   tags and language profile have no Scryer counterpart; no season-merge function.
//! - SER-REQUESTS-0023..0028: approve/decline check NO_LONGER_PENDING inline after
//!   a repository get; unknown action strings are an HTTP route concern; 0027 is a race.
//! - SER-REQUESTS-0029..0031: concept absent: `MediaRequestStatus` has no FAILED
//!   state and no retry (`reopen_media_request` is Rejected to Pending, async).
//! - SER-REQUESTS-0032, 0034, 0036, 0037, 0043..0045: concept absent: no media or
//!   season request status (UNKNOWN, PENDING, PROCESSING, PARTIALLY_AVAILABLE,
//!   DELETED) that deleting a request resets.
//! - SER-REQUESTS-0038, 0039, 0041, 0042: per-request server/root folder/tags/
//!   language profile and REQUEST_ADVANCED; the pick resolves in async
//!   `request_quality_profile_snapshot_for_submission` over the profile repository.
//! - SER-REQUESTS-0075, 0076: no per-season exclusion; the title-level refusal is
//!   async `ensure_request_subject_is_not_in_library`, no pure remaining-seasons function.
//! - SER-REQUESTS-0079, 0080: the gate is read inline in async list sync
//!   (lists/sync.rs:244-256); the request is built in async `AppListActions::submit_request`.
//! - SER-REQUESTS-0049, 0050, 0052, 0056, 0058, 0059, 0062, 0065: media-server
//!   scan and availability sync; no availability states on requests, no scan path
//!   writes a request, no rollup.
//! - SER-REQUESTS-0067..0070, 0082, 0083: Radarr/Sonarr add-and-search; approval
//!   always triggers a wanted search inside async `approve_media_request`; no
//!   preventSearch setting, no pure predicate.
//! - SER-REQUESTS-0081: first Plex sign-in creates the user in async
//!   security/external_accounts.rs; `build_user_authorization` is private.

use std::collections::{BTreeMap, HashMap};

use chrono::{TimeZone, Utc};
use scryer_application::lists::evaluate::{ItemDecision, evaluate};
use scryer_application::lists::resolve::ResolvedItem;
use scryer_application::parity_seams::{
    normalize_requested_monitor_selection, should_monitor_season,
};
use scryer_domain::{
    ExternalId, ListCounts, ListExclusion, ListExclusionScope, ListMembership, ListMembershipState, ListMode, ListOnLeave, ListRoute,
    ListScope, ListSource, ListSourceOrigin, ListSubscription, ListSyncStatus, MONITOR_TYPE_ADVANCED,
    MediaFacet, MonitorSelection, default_library_id_for_facet,
};
use scryer_parity::{Suite, corpus};
use scryer_plugin_sdk::{ListExternalId, ListMediaKind, ListPluginItem};
use serde_json::Value;

const SEASON_CASES: &[&str] = &["SER-REQUESTS-0073", "SER-REQUESTS-0074"];

#[test]
fn ser_requests_seasons() {
    let Some(mut suite) = Suite::open("scryer-application::parity_requests::ser_requests_seasons") else { return };
    let cases: Vec<Value> = corpus("corpus/reference/seerr/requests/source.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !SEASON_CASES.contains(&id) || !suite.wants(id) {
            continue;
        }
        let given = &case["input"]["given"];
        let monitor_specials = given["enableSpecialEpisodes"].as_bool().unwrap();
        let (monitor_type, raw, mut candidates): (&str, Option<MonitorSelection>, Vec<i32>) =
            match given["seasons"].as_array() {
                None => (
                    "allepisodes",
                    None,
                    given["tmdb_seasons"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s["season_number"].as_i64().unwrap() as i32)
                        .collect(),
                ),
                Some(list) => {
                    let seasons: Vec<i32> = list.iter().map(|n| n.as_i64().unwrap() as i32).collect();
                    // The candidates are the title's metadata seasons, never the requested
                    // list itself (that would echo the input): the case's tmdb_seasons, or
                    // with none the range 0..=max+1, which holds seasons that were not
                    // requested, so `should_monitor_season` must drop them.
                    let candidates = match given["tmdb_seasons"].as_array() {
                        Some(all) => all.iter().map(|s| s["season_number"].as_i64().unwrap() as i32).collect(),
                        None => (0..=seasons.iter().copied().max().unwrap_or(0) + 1).collect(),
                    };
                    (
                        MONITOR_TYPE_ADVANCED,
                        Some(MonitorSelection { seasons, series_movies: vec![] }),
                        candidates,
                    )
                }
            };
        candidates.sort_unstable();
        candidates.dedup();
        let selection = match normalize_requested_monitor_selection(Some(monitor_type), raw) {
            Ok(s) => s,
            Err(e) => {
                suite.record(id, false, e.to_string());
                continue;
            }
        };
        let actual: Vec<i64> = candidates
            .into_iter()
            .filter(|&n| should_monitor_season(monitor_type, n, monitor_specials, selection.as_ref()))
            .map(i64::from)
            .collect();
        let expected: Vec<i64> = case["expect"]["requested_seasons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap())
            .collect();
        suite.check(id, expected, actual);
    }
    suite.finish();
}

const SEASON_STATUS_CASES: &[&str] = &["SER-REQUESTS-0084", "SER-REQUESTS-0085"];

#[test]
fn ser_requests_season_status() {
    let Some(mut suite) = Suite::open("scryer-application::parity_requests::ser_requests_season_status") else { return };
    let cases: Vec<Value> = corpus("corpus/reference/seerr/requests/source.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !SEASON_STATUS_CASES.contains(&id) || !suite.wants(id) {
            continue;
        }
        let given = &case["input"]["given"];
        let permissions = super::strings(&given["by"]["permissions"]);
        let media_type = given["body"]["mediaType"].as_str().unwrap();
        match super::authorization(&permissions) {
            Ok(auth) => suite.check(
                id,
                case["expect"]["season_status"].as_str().unwrap(),
                super::scryer_status(&auth, media_type),
            ),
            Err(e) => suite.record(id, false, e),
        }
    }
    suite.finish();
}

const WATCHLIST_CASES: &[&str] = &["SER-REQUESTS-0047"];

fn at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap()
}

fn tmdb(id: i64) -> ExternalId {
    ExternalId { source: "tmdb".into(), kind: None, value: id.to_string() }
}

fn watchlist_subscription() -> ListSubscription {
    let kinds = vec![MediaFacet::Movie, MediaFacet::Series];
    let routes = kinds
        .iter()
        .map(|k| ListRoute {
            kind: k.clone(),
            library_id: default_library_id_for_facet(k),
            quality_profile_id: None,
            root_folder_id: None,
            monitor_type: "monitored".into(),
            min_availability: None,
            use_season_folders: None,
            release_numbering: None,
            tags: Vec::new(),
        })
        .collect();
    ListSubscription {
        id: "seerr-watchlist".into(),
        scope: ListScope::Personal,
        owner_user_id: "sync-user".into(),
        source: ListSource {
            provider: "plex".into(),
            source_type: "watchlist".into(),
            params: BTreeMap::new(),
            origin: ListSourceOrigin::ProviderFetch,
        },
        name: "Plex watchlist".into(),
        provider_url: None,
        kinds,
        enabled: true,
        mode: ListMode::Request,
        routes,
        filters: Vec::new(),
        max_per_sync: None,
        on_leave: ListOnLeave::Keep,
        interval_seconds: 6 * 3600,
        sync: ListSyncStatus::default(),
        counts: ListCounts::default(),
        credential_id: None,
        created_at: at(),
        updated_at: at(),
    }
}

/// The resolved items and exclusions a Seerr watchlist case translates to
/// (table above).
fn watchlist_items(given: &Value) -> (Vec<ResolvedItem>, Vec<ListExclusion>) {
    let media = given["media"].as_array().unwrap();
    let status_of = |media_type: &str, tmdb_id: i64| {
        media
            .iter()
            .find(|m| m["media_type"] == media_type && m["tmdb_id"].as_i64() == Some(tmdb_id))
            .and_then(|m| m["status"].as_str())
            .unwrap_or("UNKNOWN")
    };
    let mut items = Vec::new();
    let mut exclusions = Vec::new();
    for entry in given["watchlist"].as_array().unwrap() {
        let movie = entry["type"] == "movie";
        let media_type = if movie { "movie" } else { "tv" };
        let tmdb_id = entry["tmdbId"].as_i64().unwrap();
        let key = format!("{media_type}:{tmdb_id}");
        let (kind, hint) = if movie {
            (MediaFacet::Movie, ListMediaKind::Movie)
        } else {
            (MediaFacet::Series, ListMediaKind::Series)
        };
        let mut external_ids = vec![tmdb(tmdb_id)];
        let mut plugin_ids = vec![ListExternalId { source: "tmdb".into(), kind: None, id: tmdb_id.to_string() }];
        if let Some(tvdb) = entry["tvdbId"].as_i64() {
            external_ids.push(ExternalId { source: "tvdb".into(), kind: None, value: tvdb.to_string() });
            plugin_ids.push(ListExternalId { source: "tvdb".into(), kind: None, id: tvdb.to_string() });
        }
        let status = status_of(media_type, tmdb_id);
        if status == "BLOCKLISTED" {
            exclusions.push(ListExclusion {
                id: format!("excl-{key}"),
                kind: kind.clone(),
                external_ids: vec![tmdb(tmdb_id)],
                display_title: key.clone(),
                year: None,
                scope: ListExclusionScope::AllLists,
                created_by_user_id: None,
                created_at: at(),
            });
        }
        items.push(ResolvedItem {
            series_movie: None,
            resolution_reason: None,
            facts: None,
            item: ListPluginItem {
                item_key: key.clone(),
                kind_hint: Some(hint),
                external_ids: plugin_ids,
                ..Default::default()
            },
            kind: Some(kind),
            external_ids,
            resolved: true,
            smg_title_id: Some(tmdb_id),
            library_title_id: (status == "AVAILABLE").then(|| format!("title-{key}")),
        });
    }
    (items, exclusions)
}

#[test]
fn ser_requests_watchlist() {
    let Some(mut suite) = Suite::open("scryer-application::parity_requests::ser_requests_watchlist") else { return };
    let cases: Vec<Value> = corpus("corpus/reference/seerr/requests/watchlistsync.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !WATCHLIST_CASES.contains(&id) || !suite.wants(id) {
            continue;
        }
        let (items, exclusions) = watchlist_items(&case["input"]["given"]);
        let evaluated = evaluate(&watchlist_subscription(), items, &exclusions, &HashMap::new());
        let mut requested: Vec<String> = Vec::new();
        let mut not_requested: Vec<String> = Vec::new();
        for e in &evaluated {
            let key = e.item.item.item_key.clone();
            if e.decision == ItemDecision::Candidate {
                requested.push(key);
            } else {
                not_requested.push(key);
            }
        }
        requested.sort();
        not_requested.sort();
        let list = |v: &Value| {
            let mut s = super::strings(v);
            s.sort();
            s
        };
        let expect = &case["expect"];
        suite.check(
            id,
            (list(&expect["requested"]), list(&expect["not_requested"])),
            (requested, not_requested),
        );
    }
    suite.finish();
}

const WATCHLIST_RR_CASES: &[&str] = &["SER-REQUESTS-0048"];

#[test]
fn ser_requests_watchlist_rerequest() {
    let Some(mut suite) =
        Suite::open("scryer-application::parity_requests::ser_requests_watchlist_rerequest")
    else {
        return;
    };
    let cases: Vec<Value> = corpus("corpus/reference/seerr/requests/watchlistsync.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !WATCHLIST_RR_CASES.contains(&id) || !suite.wants(id) {
            continue;
        }
        let given = &case["input"]["given"];
        let (items, exclusions) = watchlist_items(given);
        let mut memberships = HashMap::new();
        let mut untranslated = None;
        for request in given["requests"].as_array().unwrap() {
            let key = request["media"].as_str().unwrap().to_string();
            if request["status"] != "COMPLETED" {
                untranslated = Some(format!("no translation for request status {}", request["status"]));
                break;
            }
            let (media_type, tmdb_id) = key.split_once(':').unwrap();
            let tmdb_id: i64 = tmdb_id.parse().unwrap();
            memberships.insert(
                key.clone(),
                ListMembership {
                    series_movie: None,
                    subscription_id: "seerr-watchlist".into(),
                    item_key: key.clone(),
                    rank: None,
                    season: None,
                    display_title: None,
                    year: None,
                    external_ids: vec![tmdb(tmdb_id)],
                    smg_title_id: Some(tmdb_id),
                    title_id: Some(format!("title-{key}")),
                    request_id: Some(format!("request-{key}")),
                    kind: if media_type == "movie" { MediaFacet::Movie } else { MediaFacet::Series },
                    state: ListMembershipState::InLibrary,
                    state_reason: None,
                    added_by_list: false,
                    first_seen_at: at(),
                    last_seen_at: at(),
                    left_at: None,
                    left_handled: false,
                },
            );
        }
        if let Some(e) = untranslated {
            suite.record(id, false, e);
            continue;
        }
        let evaluated = evaluate(&watchlist_subscription(), items, &exclusions, &memberships);
        let calls = evaluated
            .iter()
            .filter(|e| e.item.item.item_key == "movie:100" && e.decision == ItemDecision::Candidate)
            .count() as i64;
        suite.check(id, case["expect"]["request_calls_for_movie_100"].as_i64().unwrap(), calls);
    }
    suite.finish();
}
