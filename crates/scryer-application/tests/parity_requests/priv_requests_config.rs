//! Parity net, area `requests`, stream `priv_requests_config` (Stage 8, private).
//!
//! Every config value is read at run time from the captured corpus
//! (`scryer_parity::corpus`); nothing captured is written in this file. Rows are
//! found through the catalogue entry's ref (`#id=`, `#taskName=`). The suites
//! select their own IDs from const tables before they ask `wants()` (D45).
//!
//! Suites and mapped IDs:
//! - `ser_requests_settings`: SER-REQUESTS-5001, 5007, 5018, 5030
//!   (corpus/captured/seerr/config/main.json, sonarr.json, jobs.json).
//! - `rad_requests_config`: RAD-REQUESTS-5002, 5004, 5005
//!   (corpus/captured/radarr/config/importlistconfig.json, importlistexclusion.json,
//!   system-task.json).
//!
//! Scryer functions under test:
//! - 5001: `parity_seams::normalize_default_library_grants` (storage
//!   normalisation of connection default grants), then
//!   `parity_seams::build_user_authorization` (the authorization a new auto-added
//!   user loads), then the root's `scryer_status` (`has_library_permission` +
//!   `legacy_outcome`). Mirrors security/external_accounts.rs, where a new user
//!   copies the connection's default grants.
//! - 5007, 5018: `normalize_requested_monitor_selection` then
//!   `should_monitor_season`, as in `requests::ser_requests_seasons`.
//! - 5030, RAD 5005: `JobKey::ListSync.interval_seconds()`.
//! - RAD 5002: `lists::leave::awaits_leave_action`.
//! - RAD 5004: `lists::evaluate::evaluate`.
//!
//! Translation table, reference -> Scryer:
//!
//! | Reference | Scryer |
//! |---|---|
//! | Seerr main.json defaultPermissionNames | media-server connection default library grants per library (root `authorization` table), default app permissions NONE |
//! | Seerr seasons "all" | monitor type "allepisodes", no selection |
//! | Seerr seasons [n..] | monitor type "advanced" with `MonitorSelection { seasons }`, normalised |
//! | enableSpecialEpisodes | `monitor_specials` argument |
//! | synthetic series | seasons 0..=3 (all with episodes) chosen by this harness, never captured |
//! | Seerr rule (MediaRequest.ts:383-394) | "all" = candidates except season 0; a list = the list; then without enableSpecialEpisodes only seasons above 0 |
//! | sonarr.json monitorNewItems | a season that appears after the add is monitored iff the value is "all" (RefreshSeriesService.cs:154, MediaRequestSubscriber.ts:772); probe: season 4 |
//! | jobs.json cronSchedule | only the form `0 */N * * * *` is translated, to N*60 seconds; anything else is an Err |
//! | Radarr listSyncLevel (CleanLibrary, ImportListSyncService.cs:183-226) | `ListOnLeave`: disabled -> Keep, logOnly -> Log, keepAndUnmonitor -> Unmonitor; removeAndKeep, removeAndDelete -> Err (no on-leave removal); any other value (Radarr has no keepAndTag case) -> Err (untranslated) |
//! | sync level owes an action on a departed item | `awaits_leave_action` on a departed, unhandled row added by the list (expected: level != "disabled"); control: the same row under Log must owe an action, so the row itself is checked |
//! | Radarr import-list exclusion | `ListExclusion` kind Movie, scope AllLists, on a synthetic tmdb id |
//! | system-task interval (minutes) | `JobKey::ListSync` sweep (seconds): expected = interval*60 |
//!
//! Ignored keys: main.json quotas, requestCount (5001); the "counts toward
//! availability" clause (5007, availability is not a Scryer concept); per-user
//! switches and the 20-item limit (5030, the item rules are SER-REQUESTS-0047/0048);
//! the exclusion count is not compared, only that every excluded item is excluded
//! and a control item is not (its ids are synthetic, the file holds only a count;
//! a count of 0 is recorded as a failure, since "all excluded" would be vacuous).
//!
//! Expected reds (conditional, by corpus value):
//! - 5007 is red when enableSpecialEpisodes is off: under "advanced" the selection
//!   replaces the monitor-specials rule (monitoring.rs:1347-1352), so an explicit
//!   season 0 is monitored (same cause as SER-REQUESTS-0074, waiver candidate).
//! - 5018 (fork P36c): the harness applies `monitor_selection_with_new_seasons` (known 1..=3)
//!   before `should_monitor_season`, so a new season 4 is monitored under "advanced".
//! - 5030 is red when the cron interval differs from the ListSync sweep
//!   (LIST_SYNC_INTERVAL_SECONDS, 15 minutes). The sweep is a lower bound: a list's
//!   own interval_seconds can only lengthen it (lists/sync.rs next_sync_at).
//! - RAD-REQUESTS-5005 is red when the task interval differs from that sweep.
//!
//! Unmapped (tests: []), with reasons:
//! - SER-REQUESTS-5006 (partialRequestsEnabled): UI-only in Seerr (the server only
//!   stores it) and Scryer has no switch, so no Scryer function takes the flag; any
//!   probe would be a fixed selection that the captured flag only picks the expected
//!   branch for (covered by requests::ser_requests_seasons).
//! - SER-REQUESTS-5002, 5003 (quotas): no quota setting. A limit is expressible
//!   only as a request rule over requester-history facts (request_rules/facts.rs:100-110).
//!   With no rule (Seerr quota 0 = unlimited), `evaluate_request_draft` short-circuits
//!   to `legacy_outcome` without reading a count (request_rules/evaluation.rs:231), so a
//!   probe would not exercise the count (vacuous).
//! - SER-REQUESTS-5008 (4K): concept absent, no 4K split.
//! - SER-REQUESTS-5009 (availability read from the media server): not a Scryer
//!   concept (MediaRequestStatus is Pending/Approved/Rejected/Canceled); the
//!   sign-in half would only echo the captured flag.
//! - SER-REQUESTS-5010 (metadata source): no source setting; metadata comes from
//!   the SMG gateway and no function names a source.
//! - SER-REQUESTS-5014, 5015, 5022 (default profile, root folder, tags, minimum
//!   availability): resolved in async `request_quality_profile_snapshot_for_submission`
//!   over repositories; `media_request_to_new_title` takes the resolved ids from its
//!   caller, so a call would echo the harness input. No per-server root or tags.
//! - SER-REQUESTS-5016 (series type): the facet is chosen by the client; no function
//!   classifies a title as anime from metadata keywords.
//! - SER-REQUESTS-5017 (season folders): per-library setting loaded async; passing the
//!   captured flag into the path builder would echo it (naming suites cover the effect).
//! - SER-REQUESTS-5019, 5023 (search on add): unconditional and inline in async
//!   `approve_media_request`; the check would be a mock interaction.
//! - SER-REQUESTS-5020, 5024 (requester tag): no per-requester tag. Titles get only
//!   request-rule policy tags, narrowed to registry labels (media_requests.rs:171); the
//!   requester stays on the request row and in lifecycle claims.
//! - SER-REQUESTS-5021, 5025, 5026, 5027, 5028, 5033 (availability sync jobs and
//!   scans of arr servers or Plex): concept absent, Scryer is the library.
//! - SER-REQUESTS-5040, 5041 (per-user watchlist on/off): the gate is read inline in
//!   async `lists::sync::sync_subscription`; a probe through `evaluate` would echo the
//!   per-kind flag through the route.
//! - RAD-REQUESTS-5001, SON-REQUESTS-5001: the fact is the absence of import lists;
//!   no production input. For SON-5001 the sync level and exclusions in the
//!   parenthesis are unused without a list, so a green on them would leave the main
//!   clause unchecked (Sonarr has no separate entries for them; Radarr's are 5002, 5004).
//! - RAD-REQUESTS-5003, 5006 (collections): concept absent, no TMDB collection that
//!   adds missing movies and no collection refresh job.
//! - MNT-REQUESTS-5001, 5002: maintenance actions (async `AppUseCase` code over
//!   repositories) read requesters for facts (maintenance_rules/action_execution.rs:1986)
//!   but never delete or edit a media_requests row; asserting that nothing is removed
//!   needs a repository mock-interaction check.

use std::collections::{BTreeMap, HashMap};

use chrono::{TimeZone, Utc};
use scryer_application::JobKey;
use scryer_application::lists::evaluate::{ItemDecision, evaluate};
use scryer_application::lists::leave::awaits_leave_action;
use scryer_application::lists::resolve::ResolvedItem;
use scryer_application::parity_seams::{
    build_user_authorization, normalize_default_library_grants,
    normalize_requested_monitor_selection, should_monitor_season,
};
use scryer_domain::{
    AppPermissionMask, ExternalId, LibraryGrant, ListCounts, ListExclusion, ListExclusionScope,
    ListMembership, ListMembershipState, ListMode, ListOnLeave, ListRoute, ListScope, ListSource,
    ListSourceOrigin, ListSubscription, ListSyncStatus, MONITOR_TYPE_ADVANCED, MediaFacet,
    MediaServerDefaultLibraryGrant, MonitorSelection, User, UserAccountKind,
    default_library_id_for_facet,
};
use scryer_parity::{Catalogue, Suite, corpus};
use scryer_plugin_sdk::{ListExternalId, ListMediaKind, ListPluginItem};
use serde_json::{Value, json};

type Check = fn(&str) -> Result<(Value, Value), String>;

fn at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap()
}

/// The part of the catalogue entry's ref after `marker`.
fn ref_key(id: &str, marker: &str) -> String {
    let catalogue = Catalogue::load();
    let reference = &catalogue.entries.iter().find(|(_, e)| e.id == id).unwrap().1.reference;
    reference.rsplit_once(marker).unwrap().1.to_string()
}

fn run(name: &str, table: &[(&str, Check)]) {
    let Some(mut suite) = Suite::open(name) else { return };
    for &(id, check) in table {
        if !suite.wants(id) {
            continue;
        }
        match check(id) {
            Ok((expected, actual)) => suite.check(id, expected, actual),
            Err(e) => suite.record(id, false, e),
        }
    }
    suite.finish();
}

// ---- Seerr settings ----

fn main_config() -> Value {
    corpus("corpus/captured/seerr/config/main.json")
}

fn new_user_status(_id: &str) -> Result<(Value, Value), String> {
    let perms = super::strings(&main_config()["defaultPermissionNames"]);
    if perms.iter().any(|p| p == "ADMIN") {
        return Err("ADMIN is an app permission, no default-grant translation".into());
    }
    let grants: Vec<MediaServerDefaultLibraryGrant> = super::authorization(&perms)?
        .libraries
        .into_iter()
        .map(|(library_id, permissions)| MediaServerDefaultLibraryGrant { library_id, permissions })
        .collect();
    let grants: Vec<LibraryGrant> = normalize_default_library_grants(grants)
        .into_iter()
        .map(|g| LibraryGrant {
            user_id: "new-user".into(),
            library_id: g.library_id,
            permissions: g.permissions,
        })
        .collect();
    let user = User {
        id: "new-user".into(),
        username: "new-user".into(),
        password_hash: None,
        password_change_required: false,
        account_kind: UserAccountKind::ExternalAutoProvisioned,
        authorization: Default::default(),
    };
    let auth = build_user_authorization(&user, AppPermissionMask::NONE, grants);
    let expected: Vec<(&str, &str)> =
        ["movie", "tv"].iter().map(|m| (*m, super::seerr_status(&perms, m))).collect();
    let actual: Vec<(&str, &str)> =
        ["movie", "tv"].iter().map(|m| (*m, super::scryer_status(&auth, m))).collect();
    Ok((json!(expected), json!(actual)))
}

/// Seerr's requested seasons (MediaRequest.ts:383-394) over the synthetic seasons 0..=3.
fn seerr_seasons(requested: Option<&[i32]>, specials: bool) -> Vec<i32> {
    (0..=3)
        .filter(|&n| match requested {
            None => n != 0,
            Some(list) => list.contains(&n),
        })
        .filter(|&n| specials || n > 0)
        .collect()
}

/// Seasons of `candidates` Scryer monitors for a request (`None` = all seasons).
fn scryer_seasons(
    requested: Option<&[i32]>,
    specials: bool,
    candidates: std::ops::RangeInclusive<i32>,
) -> Result<Vec<i32>, String> {
    let (monitor_type, raw) = match requested {
        None => ("allepisodes", None),
        Some(list) => (
            MONITOR_TYPE_ADVANCED,
            Some(MonitorSelection { seasons: list.to_vec(), series_movies: vec![] }),
        ),
    };
    let selection =
        normalize_requested_monitor_selection(Some(monitor_type), raw).map_err(|e| e.to_string())?;
    // fork (P36c): a refresh adding seasons above the known 1..=3 extends an advanced selection.
    let selection = if monitor_type == MONITOR_TYPE_ADVANCED {
        let known = (1..=3).collect();
        scryer_application::parity_seams::monitor_selection_with_new_seasons(
            selection.as_ref(),
            &known,
            candidates.clone(),
        )
        .or(selection)
    } else {
        selection
    };
    Ok(candidates
        .filter(|&n| should_monitor_season(monitor_type, n, specials, selection.as_ref()))
        .collect())
}

fn specials(_id: &str) -> Result<(Value, Value), String> {
    let specials = main_config()["enableSpecialEpisodes"].as_bool().unwrap();
    let probes: [Option<&[i32]>; 2] = [None, Some(&[0, 1])];
    let mut expected = Vec::new();
    let mut actual = Vec::new();
    for probe in probes {
        expected.push(seerr_seasons(probe, specials));
        actual.push(scryer_seasons(probe, specials, 0..=3)?);
    }
    Ok((json!(expected), json!(actual)))
}

fn new_seasons(id: &str) -> Result<(Value, Value), String> {
    let key = ref_key(id, "#id=");
    let server_id: i64 = key.split('/').next().unwrap().parse().unwrap();
    let servers: Vec<Value> = corpus("corpus/captured/seerr/config/sonarr.json");
    let server = servers.iter().find(|s| s["id"].as_i64() == Some(server_id)).unwrap();
    let monitored = match server["monitorNewItems"].as_str().unwrap() {
        "all" => true,
        "none" => false,
        other => return Err(format!("no translation for monitorNewItems {other}")),
    };
    let specials = main_config()["enableSpecialEpisodes"].as_bool().unwrap();
    let probes: [Option<&[i32]>; 2] = [None, Some(&[1])];
    let mut actual = Vec::new();
    for probe in probes {
        actual.push(!scryer_seasons(probe, specials, 4..=4)?.is_empty());
    }
    Ok((json!([monitored, monitored]), json!(actual)))
}

fn watchlist_cadence(id: &str) -> Result<(Value, Value), String> {
    let key = ref_key(id, "#id=");
    let jobs: Vec<Value> = corpus("corpus/captured/seerr/config/jobs.json");
    let job = jobs.iter().find(|j| j["id"].as_str() == Some(key.as_str())).unwrap();
    let cron = job["cronSchedule"].as_str().unwrap();
    let fields: Vec<&str> = cron.split_whitespace().collect();
    let minutes: i64 = match fields.as_slice() {
        ["0", every, "*", "*", "*", "*"] => every
            .strip_prefix("*/")
            .and_then(|n| n.parse().ok())
            .ok_or("untranslated cron")?,
        _ => return Err("untranslated cron".into()),
    };
    Ok((json!(minutes * 60), json!(JobKey::ListSync.interval_seconds().unwrap())))
}

const SETTINGS: &[(&str, Check)] = &[
    ("SER-REQUESTS-5001", new_user_status),
    ("SER-REQUESTS-5007", specials),
    ("SER-REQUESTS-5018", new_seasons),
    ("SER-REQUESTS-5030", watchlist_cadence),
];

#[test]
fn ser_requests_settings() {
    run("scryer-application::parity_requests::ser_requests_settings", SETTINGS);
}

// ---- import lists (Radarr) ----

/// Radarr's list sync level (CleanLibrary has no keepAndTag case).
fn on_leave(level: &str) -> Result<ListOnLeave, String> {
    match level {
        "disabled" => Ok(ListOnLeave::Keep),
        "logOnly" => Ok(ListOnLeave::Log),
        "keepAndUnmonitor" => Ok(ListOnLeave::Unmonitor),
        "removeAndKeep" | "removeAndDelete" => Err("Scryer has no on-leave removal".into()),
        _ => Err("untranslated sync level".into()),
    }
}

fn subscription(facet: MediaFacet, on_leave: ListOnLeave) -> ListSubscription {
    ListSubscription {
        id: "probe-list".into(),
        scope: ListScope::Public,
        owner_user_id: "probe-owner".into(),
        source: ListSource {
            provider: "probe".into(),
            source_type: "probe".into(),
            params: BTreeMap::new(),
            origin: ListSourceOrigin::ProviderFetch,
        },
        name: "probe".into(),
        provider_url: None,
        kinds: vec![facet.clone()],
        enabled: true,
        mode: ListMode::Add,
        routes: vec![ListRoute {
            kind: facet.clone(),
            library_id: default_library_id_for_facet(&facet),
            quality_profile_id: None,
            root_folder_id: None,
            monitor_type: "monitored".into(),
            min_availability: None,
            use_season_folders: None,
            release_numbering: None,
            tags: Vec::new(),
        }],
        filters: Vec::new(),
        max_per_sync: None,
        on_leave,
        interval_seconds: 6 * 3600,
        sync: ListSyncStatus::default(),
        counts: ListCounts::default(),
        credential_id: None,
        created_at: at(),
        updated_at: at(),
    }
}

/// Whether a departed item's on-leave action is still owed under `on_leave`.
fn leave_owed(facet: MediaFacet, on_leave: ListOnLeave) -> bool {
    let row = ListMembership {
        series_movie: None,
        subscription_id: "probe-list".into(),
        item_key: "probe-item".into(),
        rank: None,
        season: None,
        display_title: None,
        year: None,
        external_ids: Vec::new(),
        smg_title_id: Some(1),
        title_id: Some("probe-title".into()),
        request_id: None,
        kind: facet.clone(),
        state: ListMembershipState::Added,
        state_reason: None,
        added_by_list: true,
        first_seen_at: at(),
        last_seen_at: at(),
        left_at: Some(at()),
        left_handled: false,
    };
    awaits_leave_action(&subscription(facet, on_leave), &row)
}

/// (every excluded item is excluded, a control item is a candidate), with
/// `n` exclusions on synthetic `source` ids.
fn exclusion_probe(facet: MediaFacet, source: &str, n: i64) -> (bool, bool) {
    let ext = |i: i64| ExternalId { source: source.into(), kind: None, value: i.to_string() };
    let exclusions: Vec<ListExclusion> = (1..=n)
        .map(|i| ListExclusion {
            id: format!("excl-{i}"),
            kind: facet.clone(),
            external_ids: vec![ext(i)],
            display_title: format!("excluded {i}"),
            year: None,
            scope: ListExclusionScope::AllLists,
            created_by_user_id: None,
            created_at: at(),
        })
        .collect();
    let hint = if facet == MediaFacet::Movie { ListMediaKind::Movie } else { ListMediaKind::Series };
    let items: Vec<ResolvedItem> = (1..=n + 1)
        .map(|i| ResolvedItem {
            series_movie: None,
            resolution_reason: None,
            facts: None,
            item: ListPluginItem {
                item_key: format!("item-{i}"),
                kind_hint: Some(hint.clone()),
                external_ids: vec![ListExternalId { source: source.into(), kind: None, id: i.to_string() }],
                ..Default::default()
            },
            kind: Some(facet.clone()),
            external_ids: vec![ext(i)],
            resolved: true,
            smg_title_id: Some(i),
            library_title_id: None,
        })
        .collect();
    let decisions = evaluate(&subscription(facet, ListOnLeave::Keep), items, &exclusions, &HashMap::new());
    let all_excluded = decisions[..n as usize].iter().all(|d| d.decision == ItemDecision::Excluded);
    (all_excluded, decisions[n as usize].decision == ItemDecision::Candidate)
}

fn rad_leave(_id: &str) -> Result<(Value, Value), String> {
    let config: Value = corpus("corpus/captured/radarr/config/importlistconfig.json");
    let level = config["listSyncLevel"].as_str().unwrap();
    // Control: the same departed row under Log, so a wrong row cannot pass under Keep.
    Ok((
        json!({"action_owed": level != "disabled", "control_owed": true}),
        json!({
            "action_owed": leave_owed(MediaFacet::Movie, on_leave(level)?),
            "control_owed": leave_owed(MediaFacet::Movie, ListOnLeave::Log),
        }),
    ))
}

fn rad_exclusions(_id: &str) -> Result<(Value, Value), String> {
    let v: Value = corpus("corpus/captured/radarr/config/importlistexclusion.json");
    let n = v["count"].as_i64().unwrap();
    if n == 0 {
        return Err("no exclusions captured".into());
    }
    let (all, control) = exclusion_probe(MediaFacet::Movie, "tmdb", n);
    Ok((
        json!({"all_excluded": true, "control_candidate": true}),
        json!({"all_excluded": all, "control_candidate": control}),
    ))
}

fn rad_task(id: &str) -> Result<(Value, Value), String> {
    let name = ref_key(id, "#taskName=");
    let tasks: Vec<Value> = corpus("corpus/captured/radarr/config/system-task.json");
    let task = tasks.iter().find(|t| t["taskName"].as_str() == Some(name.as_str())).unwrap();
    let minutes = task["interval"].as_i64().unwrap();
    Ok((json!(minutes * 60), json!(JobKey::ListSync.interval_seconds().unwrap())))
}

const RADARR: &[(&str, Check)] = &[
    ("RAD-REQUESTS-5002", rad_leave),
    ("RAD-REQUESTS-5004", rad_exclusions),
    ("RAD-REQUESTS-5005", rad_task),
];

#[test]
fn rad_requests_config() {
    run("scryer-application::parity_requests::rad_requests_config", RADARR);
}
