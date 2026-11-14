//! Parity net, area `requests` (notes/11-parity-net-design.md, parity/README.md).
//!
//! What is checked: the status a new request is created with, from the
//! requester's permissions. Scryer path: the requester's library grants
//! (`UserAuthorization::has_library_permission`, the only check
//! `authorization::effective_library_permission` applies to Request and
//! Auto-Approve, since the app-level override excludes both) decide whether
//! the request may be filed at all (`require_request_submission_permission`,
//! media_requests.rs:451, non-held admission) and then, with no request rule
//! covering the library, the verdict is `legacy_outcome(permission)`
//! (request_rules/evaluation.rs: `consulted.is_empty()` branch).
//!
//! Translation table, Seerr permission -> Scryer grant (per library):
//!
//! | Seerr | Scryer |
//! |---|---|
//! | ADMIN | `UserAuthorization::full_admin()` (every grant, every library) |
//! | MANAGE_REQUESTS | `LibraryPermission::ManageTitles` (the grant `can_manage_media_requests` checks) on both libraries |
//! | REQUEST | `Request` on the movie and the series default library |
//! | REQUEST_MOVIE / REQUEST_TV | `Request` on the movie / series default library |
//! | AUTO_APPROVE | `AutoApproveRequests` on both libraries |
//! | AUTO_APPROVE_MOVIE / AUTO_APPROVE_TV | `AutoApproveRequests` on the movie / series library |
//! | AUTO_REQUEST, AUTO_REQUEST_MOVIE, AUTO_REQUEST_TV | ignored: watchlist auto-requests, no effect on a created request's status (MediaRequest.ts:340, :455) |
//!
//! Seerr media type `movie` -> `MediaFacet::Movie`, `tv` -> `MediaFacet::Series`
//! (default library ids from `default_library_id_for_facet`).
//! Outcome -> Seerr status: AutoApprove -> APPROVED, ManualReview -> PENDING,
//! Deny -> DECLINED; a submission Scryer refuses for lack of Request -> REFUSED
//! (no Seerr status; it never matches).
//!
//! Suites and mapped IDs:
//! - `ser_requests`: SER-REQUESTS-0071, 0072, 0077, 0078 (source.json, `request.create`
//!   with `expect.status`). Ignored keys: `body.is4k` (no 4K split in Scryer),
//!   `body.seasons` (the status is the request's; the requested-season set is
//!   checked in `requests::ser_requests_seasons`).
//! - `ser_requests_config`: SER-REQUESTS-5004, 5005 (users.json, by the `#id=` in
//!   each entry's ref). Expected status per media type is derived from the
//!   user's Seerr permissions with Seerr's own rule (MediaRequest.ts:340, :455):
//!   ADMIN, MANAGE_REQUESTS or AUTO_APPROVE[_MOVIE|_TV] -> APPROVED, else
//!   PENDING, and REFUSED without ADMIN or REQUEST[_MOVIE|_TV]
//!   (the source-derived cases SER-REQUESTS-0071/0077 state the same rule).
//!   Ignored keys: quotas, requestCount, userType.
//! - `priv_requests_config::{ser_requests_settings, rad_requests_config}`:
//!   SER-REQUESTS-5001, 5007, 5018, 5030, RAD-REQUESTS-5002, 5004, 5005:
//!   see `parity_requests/priv_requests_config.rs`.
//! - `requests::ser_requests_seasons` (0073, 0074),
//!   `requests::ser_requests_season_status` (0084, 0085),
//!   `requests::ser_requests_watchlist` (0047),
//!   `requests::ser_requests_watchlist_rerequest` (0048): see
//!   `parity_requests/requests.rs`.
//!
//! Unmapped (tests: []), with reasons:
//! - SER-REQUESTS-0002..0005, 0007..0045 (request_route.json, MediaRequest.json):
//!   request lifecycle (duplicates, delete/edit/approve/decline/retry, season
//!   bookkeeping) runs through async `AppUseCase` methods over repositories.
//!   Re-checked: `AppServices::builder` is public but needs 10 repository
//!   objects and a stateful `MediaRequestRepository` (the SQL store in
//!   scryer-infrastructure-library or the cfg(test) doubles); more than a seam.
//!   Per-group reasons: `parity_requests/requests.rs`.
//! - SER-REQUESTS-0075, 0076: no per-season exclusion; the title-level
//!   refusal is async `ensure_request_subject_is_not_in_library`.
//! - SER-REQUESTS-0079, 0080 (watchlist sync), 0049, 0050,
//!   0052, 0056..0065 (media-server scan, availability sync), 0067..0070, 0082,
//!   0083 (Radarr/Sonarr add and search), 0081 (Plex sign-in): async
//!   integrations (lists sync, library scan, acquisition, auth) with no pure
//!   seam; reasons in `parity_requests/requests.rs`.
//! - SER-REQUESTS-5001, 5007, 5018, 5030 (default permissions,
//!   specials, new seasons, watchlist cadence) are mapped in
//!   `priv_requests_config::ser_requests_settings`.
//! - SER-REQUESTS-5002, 5003 (quotas): no quota setting; with no request rule the
//!   evaluation reads no requester count (vacuous). Reasons in
//!   `parity_requests/priv_requests_config.rs`.
//! - SER-REQUESTS-5006 (partial requests): UI-only setting, no Scryer switch; no function takes
//!   the flag. Reason in `parity_requests/priv_requests_config.rs`.
//! - SER-REQUESTS-5008 (4K): concept absent (no 4K split).
//! - SER-REQUESTS-5009, 5010, 5014..5017, 5019..5028, 5033, 5040, 5041 (the other config
//!   entries): reasons in `parity_requests/priv_requests_config.rs`.
//! - SER-REQUESTS-6001..6194 (captured requests, corpus/captured/seerr/data/cases/
//!   requests.json; no suite fn, D30), per field group:
//!   (1) expect.status/statusName is the lifecycle outcome of approval plus Plex
//!   availability sync; `MediaRequestStatus` has no COMPLETED (scryer-domain
//!   lib.rs:218) and no scan or availability path writes a request (as 0049..0065).
//!   (2) The creation verdict is the requester permission rule, already checked per
//!   user by `ser_requests_config` (5004/5005, red until P8); a per-request copy
//!   would duplicate it.
//!   (3) expect.media.* is media-server availability; the input has no library facts.
//!   (4) expect.profileId/rootFolder/serverId/languageProfileId/tags are stored null
//!   unless an advanced override or override rule applied (MediaRequest.ts:289-308,
//!   :370-371); the override is not in the input. Scryer has a per-request profile
//!   (`requested/approved_quality_profile_id`) resolved by async
//!   `request_quality_profile_snapshot_for_submission` over repositories (more than
//!   a seam), and per-request `policy_tags` emitted only by request rules (none in
//!   the input, so [] vs null would be vacuous). It has no per-request root folder,
//!   server or language profile.
//!   (5) expect.seasons: the raw request row has one `seasons` field, copied into
//!   both input.seasons and expect.seasons, so expected == input and the
//!   "advanced" monitor path reduces to `seasons.contains`, an echo (empty for a
//!   movie). The season-selection path is covered by `requests::ser_requests_seasons`
//!   (0073/0074); Seerr's pre-store season filtering (MediaRequest.ts:400-436) is
//!   0075/0076.
//! - SER-REQUESTS-6195..6204 (cases/media.json, media rows with no request):
//!   status/status4k is written by the Plex scan and availability sync; the input
//!   holds only mediaType, external ids and a request count. Scryer has no media
//!   or season availability status (as 0049..0065); 4K: concept absent.
//! - RAD-REQUESTS-5002, 5004, 5005 are mapped in `priv_requests_config`;
//!   RAD-REQUESTS-5001, SON-REQUESTS-5001 (no import lists), RAD-REQUESTS-5003,
//!   5006 (collections) and MNT-REQUESTS-5001, 5002 (maintenance never edits a
//!   request; needs a mock-interaction check) stay unmapped, reasons in
//!   `parity_requests/priv_requests_config.rs`.

use std::collections::HashMap;

use scryer_application::request_rules::legacy_outcome;
use scryer_domain::{
    LibraryPermission, LibraryPermissionMask, MediaFacet, RequestDecisionOutcome,
    UserAuthorization, default_library_id_for_facet,
};
use scryer_parity::{Catalogue, Suite, corpus};
use serde_json::Value;

#[path = "parity_requests/requests.rs"]
mod requests;

#[path = "parity_requests/priv_requests_config.rs"]
mod priv_requests_config;

/// The Scryer authorization a Seerr permission list translates to (table above).
fn authorization(permissions: &[String]) -> Result<UserAuthorization, String> {
    if permissions.iter().any(|p| p == "ADMIN") {
        return Ok(UserAuthorization::full_admin());
    }
    let movie = default_library_id_for_facet(&MediaFacet::Movie);
    let series = default_library_id_for_facet(&MediaFacet::Series);
    let mut movie_grants = Vec::new();
    let mut series_grants = Vec::new();
    for permission in permissions {
        let (on_movie, on_series, grant) = match permission.as_str() {
            "MANAGE_REQUESTS" => (true, true, LibraryPermission::ManageTitles),
            "REQUEST" => (true, true, LibraryPermission::Request),
            "REQUEST_MOVIE" => (true, false, LibraryPermission::Request),
            "REQUEST_TV" => (false, true, LibraryPermission::Request),
            "AUTO_APPROVE" => (true, true, LibraryPermission::AutoApproveRequests),
            "AUTO_APPROVE_MOVIE" => (true, false, LibraryPermission::AutoApproveRequests),
            "AUTO_APPROVE_TV" => (false, true, LibraryPermission::AutoApproveRequests),
            "AUTO_REQUEST" | "AUTO_REQUEST_MOVIE" | "AUTO_REQUEST_TV" => continue,
            other => return Err(format!("no translation for Seerr permission {other}")),
        };
        if on_movie {
            movie_grants.push(grant);
        }
        if on_series {
            series_grants.push(grant);
        }
    }
    let libraries = HashMap::from([
        (movie, LibraryPermissionMask::from_permissions(movie_grants)),
        (
            series,
            LibraryPermissionMask::from_permissions(series_grants),
        ),
    ]);
    Ok(UserAuthorization {
        libraries,
        loaded: true,
        ..UserAuthorization::default()
    })
}

fn facet(media_type: &str) -> MediaFacet {
    match media_type {
        "movie" => MediaFacet::Movie,
        "tv" => MediaFacet::Series,
        other => panic!("unknown Seerr media type {other}"),
    }
}

/// The Seerr status Scryer creates the request with.
fn scryer_status(auth: &UserAuthorization, media_type: &str) -> &'static str {
    let library = default_library_id_for_facet(&facet(media_type));
    if !auth.has_library_permission(&library, LibraryPermission::Request) {
        return "REFUSED";
    }
    let permission = auth.has_library_permission(&library, LibraryPermission::AutoApproveRequests);
    match legacy_outcome(permission) {
        RequestDecisionOutcome::AutoApprove => "APPROVED",
        RequestDecisionOutcome::ManualReview => "PENDING",
        RequestDecisionOutcome::Deny => "DECLINED",
    }
}

/// Seerr's own rule for a new request's status (MediaRequest.ts:340, :455).
fn seerr_status(permissions: &[String], media_type: &str) -> &'static str {
    let has = |names: &[&str]| permissions.iter().any(|p| names.contains(&p.as_str()));
    let (request, auto) = match media_type {
        "movie" => ("REQUEST_MOVIE", "AUTO_APPROVE_MOVIE"),
        _ => ("REQUEST_TV", "AUTO_APPROVE_TV"),
    };
    if !has(&["ADMIN", "REQUEST", request]) {
        "REFUSED"
    } else if has(&["ADMIN", "MANAGE_REQUESTS", "AUTO_APPROVE", auto]) {
        "APPROVED"
    } else {
        "PENDING"
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

const SOURCE_STATUS_CASES: &[&str] = &[
    "SER-REQUESTS-0071",
    "SER-REQUESTS-0072",
    "SER-REQUESTS-0077",
    "SER-REQUESTS-0078",
];

#[test]
fn ser_requests() {
    let Some(mut suite) = Suite::open("scryer-application::parity_requests::ser_requests") else { return };
    let cases: Vec<Value> = corpus("corpus/reference/seerr/requests/source.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !SOURCE_STATUS_CASES.contains(&id) || !suite.wants(id) {
            continue;
        }
        let given = &case["input"]["given"];
        let permissions = strings(&given["by"]["permissions"]);
        let media_type = given["body"]["mediaType"].as_str().unwrap();
        let expected = case["expect"]["status"].as_str().unwrap();
        match authorization(&permissions) {
            Ok(auth) => suite.check(id, expected, scryer_status(&auth, media_type)),
            Err(e) => suite.record(id, false, e),
        }
    }
    suite.finish();
}

const CONFIG_USER_CASES: &[&str] = &["SER-REQUESTS-5004", "SER-REQUESTS-5005"];

#[test]
fn ser_requests_config() {
    let Some(mut suite) = Suite::open("scryer-application::parity_requests::ser_requests_config") else { return };
    let catalogue = Catalogue::load();
    let users: Vec<Value> = corpus("corpus/captured/seerr/config/users.json");
    for &id in CONFIG_USER_CASES {
        if !suite.wants(id) {
            continue;
        }
        let reference = &catalogue
            .entries
            .iter()
            .find(|(_, e)| e.id == id)
            .unwrap()
            .1
            .reference;
        let user_id: i64 = reference.rsplit_once("#id=").unwrap().1.parse().unwrap();
        let user = users
            .iter()
            .find(|u| u["id"].as_i64() == Some(user_id))
            .unwrap();
        let permissions = strings(&user["permissionNames"]);
        let expected: Vec<(&str, &str)> = ["movie", "tv"]
            .iter()
            .map(|m| (*m, seerr_status(&permissions, m)))
            .collect();
        match authorization(&permissions) {
            Ok(auth) => {
                let actual: Vec<(&str, &str)> = ["movie", "tv"]
                    .iter()
                    .map(|m| (*m, scryer_status(&auth, m)))
                    .collect();
                suite.check(id, expected, actual);
            }
            Err(e) => suite.record(id, false, e),
        }
    }
    suite.finish();
}
