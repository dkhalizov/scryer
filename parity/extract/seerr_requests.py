"""Port Seerr request-lifecycle tests (area: requests), limited to what our config uses.

Our Seerr (corpus/captured/seerr/config/): Plex sign-in, new users get
[REQUEST, AUTO_APPROVE, AUTO_REQUEST], user 2 adds MANAGE_REQUESTS, user 1 is
ADMIN, quotas 0 (unlimited), partial season requests on, specials off, one
default non-4K Sonarr and Radarr (search on add, syncEnabled off), no override
rules, no 4K servers.

A "fixture" here is one innermost describe() block. Every it() of a ported
block gets an entry; rows that need something our config lacks are kept as
in_scope: false entries (reason after "out of scope:") with no corpus case.
Blocks ruled out whole are in RULED_OUT. Rules our config relies on that have
no reference test are ported from the implementing source line (SOURCE).

Hand-translated: each test's arrange/assert is written below as data, in
Seerr's own terms (MediaStatus, MediaRequestStatus, Permission names). The
seed users are admin (ADMIN) and demo (REQUEST), server/utils/seedTestDb.ts:54,70.
The script anchors every case to its it() / source line in the reference repo,
checks each ported block is covered exactly, keeps the IDs already in the
catalogue (keyed by ref + row within ref) and writes:

  parity/corpus/reference/seerr/requests/<file>.json
  parity/catalogue/requests.seerr.yaml

Run: python3 -I parity/extract/seerr_requests.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REF_REPO = Path.home() / "PycharmProjects/arr-reference/seerr@v3.5.2"
TAG = "Seerr@v3.5.2"
CATALOGUE = ROOT / "parity/catalogue/requests.seerr.yaml"
CORPUS = ROOT / "parity/corpus/reference/seerr/requests"
ID_PREFIX, ID_MAX = "SER-REQUESTS-", 4999  # 5001+ belong to requests.seerr-config.yaml

ADMIN = {"name": "admin", "permissions": ["ADMIN"]}
DEMO = {"name": "demo", "permissions": ["REQUEST"]}
SECOND = {"name": "second", "permissions": ["REQUEST"]}
DEMO_ADV = {"name": "demo", "permissions": ["REQUEST", "REQUEST_ADVANCED"]}

NO_4K = "we have no 4K Radarr or Sonarr server (radarr.json, sonarr.json; public.json movie4kEnabled/series4kEnabled off)"
QUOTA = "our quotas are 0 = unlimited (main.json defaultQuotas, users.json)"
ARR_SCAN = ("only the Radarr/Sonarr scanners opt into declineRequestsOnStatusReset or report abandoned entries "
            "(server/lib/scanners/radarr/index.ts:24, sonarr/index.ts:33) and syncEnabled is off on both")
ONE_SONARR = "we have one Sonarr server (sonarr.json)"
JELLYFIN = "we use Plex, not Jellyfin (main.json mediaServerType 1)"


def T(title, behaviour, op, given, expect, note=None):
    return dict(title=title, behaviour=behaviour, op=op, given=given, expect=expect, note=note)


def O(title, behaviour, why):
    return dict(title=title, behaviour=f"{behaviour}; out of scope: {why}", op=None)


def movie_req(status, by, tmdb=12345, media_status="UNKNOWN"):
    """seedRequest(): request.test.ts:196."""
    return {"media_type": "movie", "tmdb_id": tmdb, "media_status": media_status, "status": status,
            "requested_by": by, "is4k": False}


# --- server/entity/MediaRequest.test.ts ---------------------------------------

MEDIA_REQUEST = [
    O("rejects the second of two concurrent requests at the movie quota",
      "Of two concurrent movie requests from a user with a movie quota of 1, one fails with QuotaRestrictedError and one is saved",
      QUOTA),
    T("rejects a concurrent duplicate request for the same movie",
      "Two concurrent requests from one user for the same movie save one request; the other fails as a duplicate",
      "request.create", {"concurrent": True, "requests": [
          {"by": DEMO, "movie_quota_limit": 5, "media_type": "movie", "tmdb_id": 33333, "is4k": False},
          {"by": DEMO, "movie_quota_limit": 5, "media_type": "movie", "tmdb_id": 33333, "is4k": False}]},
      {"rejected": 1, "rejection": "DuplicateMediaRequestError", "requests_saved": 1},
      note="Drops the reference's externalApiGetMock.callCount() === 2 (MediaRequest.test.ts:108) on purpose: "
           "an internal TMDB-fetch count, not a request rule."),
    T("rejects a duplicate request that omits is4k",
      "A second request for the same movie that omits is4k is rejected as a duplicate of the first",
      "request.create", {"concurrent": False, "requests": [
          {"by": DEMO, "movie_quota_limit": 5, "media_type": "movie", "tmdb_id": 66666},
          {"by": DEMO, "movie_quota_limit": 5, "media_type": "movie", "tmdb_id": 66666}]},
      {"results": ["created", "DuplicateMediaRequestError"], "requests_saved": 1}),
    T("rejects a concurrent duplicate request from a different user",
      "Concurrent requests from two users for the same movie save one request; the other fails as a duplicate",
      "request.create", {"concurrent": True, "requests": [
          {"by": DEMO, "movie_quota_limit": 5, "media_type": "movie", "tmdb_id": 44444, "is4k": False},
          {"by": SECOND, "media_type": "movie", "tmdb_id": 44444, "is4k": False}]},
      {"rejected": 1, "rejection": "DuplicateMediaRequestError", "requests_saved": 1}),
    T("gives an overlapping season to only one of two concurrent users",
      "Concurrent series requests for seasons [1, 2] and [2, 3] both succeed and season 2 is requested only once (3 season requests in all)",
      "request.create", {"concurrent": True, "tmdb_seasons": [1, 2, 3], "requests": [
          {"by": DEMO, "movie_quota_limit": 5, "media_type": "tv", "tmdb_id": 55555, "seasons": [1, 2], "is4k": False},
          {"by": SECOND, "media_type": "tv", "tmdb_id": 55555, "seasons": [2, 3], "is4k": False}]},
      {"rejected": 0, "season_requests_for_season_2": 1, "season_requests": 3}),
    O("creates one media row for concurrent 4k and non-4k requests",
      "Concurrent 4K and non-4K requests for one movie create two requests and one media row", NO_4K),
]

# --- server/routes/request.test.ts --------------------------------------------

DELETE = [
    T("allows the owner to delete their own pending request",
      "The owner may delete their own pending request",
      "request.delete", {"request": movie_req("PENDING", DEMO), "actor": DEMO}, {"http_status": 204}),
    T("allows an admin to delete any pending request",
      "An admin may delete another user's pending request",
      "request.delete", {"request": movie_req("PENDING", DEMO), "actor": ADMIN}, {"http_status": 204}),
    T("prevents a non-owner non-admin from deleting a pending request",
      "A user who is neither the owner nor allowed to manage requests may not delete a pending request",
      "request.delete", {"request": movie_req("PENDING", ADMIN, tmdb=54321), "actor": DEMO}, {"http_status": 401}),
    T("prevents the owner from deleting an approved request",
      "The owner without MANAGE_REQUESTS may not delete their own request once it is approved",
      "request.delete", {"request": movie_req("APPROVED", DEMO), "actor": DEMO}, {"http_status": 401}),
    T("returns 404 for a non-existent request",
      "Deleting a request that does not exist answers not found",
      "request.delete", {"request": None, "actor": ADMIN}, {"http_status": 404}),
    T("deletes a request once when two deletes race",
      "Two racing deletes of one request: one succeeds, the other finds nothing, and the request is gone",
      "request.delete", {"request": movie_req("PENDING", DEMO), "actor": ADMIN, "concurrent_deletes": 2},
      {"http_statuses_sorted": [204, 404], "request_remaining": 0}),
]

EDIT_MOVIE = [
    T("persists server and root folder changes to the database",
      "An admin edit of a pending movie request saves the new server, profile and root folder",
      "request.edit", {"request": movie_req("PENDING", DEMO), "actor": ADMIN,
                       "body": {"mediaType": "movie", "serverId": 3, "profileId": 7, "rootFolder": "/updated/movies",
                                "tags": [1, 2]}},
      {"http_status": 200, "saved": {"serverId": 3, "profileId": 7, "rootFolder": "/updated/movies"}}),
    T("refuses to modify a request that is no longer pending",
      "Editing a request that is no longer pending is refused (conflict) and changes nothing",
      "request.edit", {"request": movie_req("APPROVED", DEMO), "actor": ADMIN,
                       "body": {"mediaType": "movie", "serverId": 3, "rootFolder": "/updated/movies"}},
      {"http_status": 409, "message_matches": "/pending/i", "saved": {"serverId": None, "rootFolder": None}}),
]


def tv_req(by, seasons, tmdb=67890, ignore_quota=False):
    """seedTvRequest(): request.test.ts:409; media status PENDING (seedTvMedia :378)."""
    return {"media_type": "tv", "tmdb_id": tmdb, "media_status": "PENDING", "status": "PENDING",
            "requested_by": by, "is4k": False, "seasons": seasons, "season_status": "PENDING",
            "ignore_quota": ignore_quota}


EDIT_TV = [
    T("does not add a season held by another request",
      "Editing a series request to add a season another request already holds leaves that season with the other request",
      "request.edit", {"requests": [tv_req(ADMIN, [1, 2]), tv_req(DEMO, [3])], "edits": [
          {"request": 0, "actor": ADMIN, "body": {"mediaType": "tv", "seasons": [1, 2, 3]}}]},
      {"http_statuses": [200], "seasons": [[1, 2], [3]]}),
    T("gives a season to only one of two concurrent edits",
      "Two concurrent edits that both add season 2 succeed, and season 2 ends up in exactly one request",
      "request.edit", {"concurrent": True, "requests": [tv_req(ADMIN, [1]), tv_req(DEMO, [3])], "edits": [
          {"request": 0, "actor": ADMIN, "body": {"mediaType": "tv", "seasons": [1, 2]}},
          {"request": 1, "actor": DEMO, "body": {"mediaType": "tv", "seasons": [3, 2]}}]},
      {"http_statuses": [200, 200], "holders_of_season_2": 1, "all_seasons_sorted": [1, 2, 3]}),
]

EDIT_AVAIL = [
    T("does not add a season the media already has",
      "Editing a series request does not add a season the media already has available",
      "request.edit", {"requests": [tv_req(DEMO, [1])], "media_seasons": [{"seasonNumber": 2, "status": "AVAILABLE"}],
                       "edits": [{"request": 0, "actor": DEMO, "body": {"mediaType": "tv", "seasons": [1, 2, 3]}}]},
      {"http_statuses": [200], "seasons": [[1, 3]]}),
    T("returns 202 when every requested season is already covered",
      "An edit whose every requested season is already available answers 202 (accepted, nothing to add)",
      "request.edit", {"requests": [tv_req(DEMO, [1])], "media_seasons": [{"seasonNumber": 2, "status": "AVAILABLE"}],
                       "edits": [{"request": 0, "actor": DEMO, "body": {"mediaType": "tv", "seasons": [2]}}]},
      {"http_statuses": [202]}),
    T("keeps the seasons it already holds once they are available",
      "An edit keeps seasons the request already holds even after they became available or processing",
      "request.edit", {"requests": [tv_req(DEMO_ADV, [1, 2])],
                       "media_seasons": [{"seasonNumber": 1, "status": "AVAILABLE"},
                                         {"seasonNumber": 2, "status": "PROCESSING"}],
                       "edits": [{"request": 0, "actor": DEMO_ADV,
                                  "body": {"mediaType": "tv", "seasons": [1, 2], "serverId": 3}}]},
      {"http_statuses": [200], "seasons": [[1, 2]], "saved": {"serverId": 3}}),
    O("does not charge quota for a season the media already has",
      "With a series quota of 1, adding a season the media already has is not charged and not added", QUOTA),
]

TV_OPTIONS = {"serverId": 2, "profileId": 9, "rootFolder": "/chosen", "tags": [3], "languageProfileId": 4}
MOVIE_OPTIONS = {"serverId": 2, "profileId": 9, "rootFolder": "/chosen", "tags": [3]}
NO_OPTIONS = {"serverId": None, "profileId": None, "rootFolder": None, "languageProfileId": None, "tags": None}

EDIT_ADVANCED = [
    T("keeps the stored options when a regular requester edits their seasons",
      "A regular requester's season edit ignores the server, profile, root folder, language profile and tags it sends and keeps the stored ones",
      "request.edit", {"requests": [dict(tv_req(DEMO, [1]), stored_options={
          "serverId": 1, "profileId": 5, "rootFolder": "/stored", "languageProfileId": 2, "tags": [6]})],
          "edits": [{"request": 0, "actor": DEMO, "body": dict({"mediaType": "tv", "seasons": [1, 2]}, **TV_OPTIONS)}]},
      {"http_statuses": [200], "seasons": [[1, 2]], "saved_options": {
          "serverId": 1, "profileId": 5, "rootFolder": "/stored", "languageProfileId": 2, "tags": [6]}}),
    T("rejects a regular requester editing their movie request as a series",
      "A regular requester may not edit their movie request into a series request",
      "request.edit", {"requests": [movie_req("PENDING", DEMO)],
                       "edits": [{"request": 0, "actor": DEMO,
                                  "body": dict({"mediaType": "tv", "seasons": [1]}, **TV_OPTIONS)}]},
      {"http_statuses": [403]}),
]

STATUS = [
    T("transitions to ${action}d and records the acting user",
      "Approving a pending request makes it APPROVED and records the approver as modifiedBy",
      "request.set_status", {"request": movie_req("PENDING", DEMO), "actor": ADMIN, "action": "approve"},
      {"http_status": 200, "status": "APPROVED", "modified_by": "admin", "updated_at_advances": True}),
    T("transitions to ${action}d and records the acting user",
      "Declining a pending request makes it DECLINED and records the decliner as modifiedBy",
      "request.set_status", {"request": movie_req("PENDING", DEMO), "actor": ADMIN, "action": "decline"},
      {"http_status": 200, "status": "DECLINED", "modified_by": "admin", "updated_at_advances": True}),
    T("rejects a status the route does not define",
      "An unknown status action is rejected and the request stays PENDING with no modifiedBy",
      "request.set_status", {"request": movie_req("PENDING", DEMO), "actor": ADMIN, "action": "frobnicate"},
      {"http_status": 400, "message_matches": "/approve/i", "status": "PENDING", "modified_by": None}),
    T("refuses to act on a request that is no longer pending",
      "Declining an approved request is refused (conflict) and it stays APPROVED",
      "request.set_status", {"request": movie_req("APPROVED", DEMO), "actor": ADMIN, "action": "decline"},
      {"http_status": 409, "message_matches": "/pending/i", "status": "APPROVED"}),
    T("applies only one of a concurrent approve and decline",
      "Of a concurrent approve and decline only one applies; the request ends in that one's status",
      "request.set_status", {"request": movie_req("PENDING", DEMO), "actor": ADMIN,
                             "concurrent_actions": ["approve", "decline"]},
      {"http_statuses_sorted": [200, 409], "status_if_approve_won": "APPROVED",
       "status_if_decline_won": "DECLINED"}),
    T("rejects the removed pending verb even on a non-pending request",
      "The 'pending' status action no longer exists: it is rejected and a declined request stays DECLINED",
      "request.set_status", {"request": movie_req("DECLINED", DEMO), "actor": ADMIN, "action": "pending"},
      {"http_status": 400, "message_matches": "/approve/i", "status": "DECLINED"}),
]

RETRY = [
    T("re-approves a failed request and records the acting user",
      "Retrying a FAILED request makes it APPROVED again and records the retrier as modifiedBy",
      "request.retry", {"request": movie_req("FAILED", DEMO), "actor": ADMIN},
      {"http_status": 200, "status": "APPROVED", "modified_by": "admin", "updated_at_advances": True}),
    T("refuses to retry a request that has not failed",
      "Retrying a request that has not failed is refused (conflict) and it stays PENDING",
      "request.retry", {"request": movie_req("PENDING", DEMO), "actor": ADMIN},
      {"http_status": 409, "message_matches": "/failed/i", "status": "PENDING"}),
    T("sends a concurrently retried request to *arr once",
      "Two concurrent retries of a failed movie request send it to Radarr once and leave it APPROVED",
      "request.retry", {"request": movie_req("FAILED", DEMO), "actor": ADMIN, "concurrent_retries": 2},
      {"http_statuses_sorted": [200, 409], "sent_to_radarr": 1, "status": "APPROVED"}),
]

# seedDeletedMediaScenario(): request.test.ts:1070
DELETED_SCENARIO = {"media": {"media_type": "movie", "tmdb_id": 99001, "status_before_rerequest": "DELETED",
                              "status": "PENDING", "status4k": "UNKNOWN"},
                    "requests": [
                        {"name": "stale", "status": "COMPLETED", "requested_by": ADMIN, "is4k": False,
                         "is_auto_request": True},
                        {"name": "new", "status": "APPROVED", "requested_by": ADMIN, "is4k": False}]}

RESTORE = [
    T("restores media status to DELETED when the re-request is deleted and a stale completed request remains",
      "Deleting the re-request of a DELETED movie, while its old completed auto-request remains, puts the media back to DELETED",
      "request.delete", dict(DELETED_SCENARIO, actor=ADMIN, delete=["new"]),
      {"http_statuses": [204], "media_status": "DELETED"}),
    O("restores media status4k to DELETED when the re-request is deleted and a stale completed request remains",
      "The same restoration for the 4K status", NO_4K),
    T("resets media status to UNKNOWN when the stale completed request is also deleted",
      "Deleting both the re-request and the old completed request of a DELETED movie resets the media to UNKNOWN with no requests left",
      "request.delete", dict(DELETED_SCENARIO, actor=ADMIN, delete=["new", "stale"]),
      {"last_http_status": 204, "media_status": "UNKNOWN", "requests_remaining": 0}),
    O("resets media status4k to UNKNOWN when the stale completed 4K request is also deleted",
      "The same reset for the 4K status", NO_4K),
    T("does not reset media status when other active requests still exist",
      "Deleting one of two pending requests for a PENDING movie leaves the media PENDING",
      "request.delete", {"media": {"media_type": "movie", "tmdb_id": 99002, "status": "PENDING", "status4k": "UNKNOWN"},
                         "requests": [{"name": "one", "status": "PENDING", "requested_by": ADMIN, "is4k": False},
                                      {"name": "two", "status": "PENDING", "requested_by": ADMIN, "is4k": False}],
                         "actor": ADMIN, "delete": ["one"]},
      {"http_statuses": [204], "media_status": "PENDING"}),
    T("does not reset media status when status is PARTIALLY_AVAILABLE and only completed requests remain",
      "Deleting the only (completed) request of a PARTIALLY_AVAILABLE movie leaves it PARTIALLY_AVAILABLE",
      "request.delete", {"media": {"media_type": "movie", "tmdb_id": 99005, "status": "PARTIALLY_AVAILABLE",
                                   "status4k": "UNKNOWN"},
                         "requests": [{"name": "done", "status": "COMPLETED", "requested_by": ADMIN, "is4k": False}],
                         "actor": ADMIN, "delete": ["done"]},
      {"http_statuses": [204], "media_status": "PARTIALLY_AVAILABLE"}),
]

ADVANCED_CREATE = [
    T("ignores advanced options on a movie request from a regular requester",
      "A regular requester's movie request ignores the server, profile, root folder and tags it sends (all stored empty)",
      "request.create", {"radarr": [{"id": 1, "isDefault": True, "is4k": False}, {"id": 2, "isDefault": False, "is4k": False}],
                         "sonarr": [], "by": DEMO,
                         "body": dict({"mediaType": "movie", "mediaId": 88020}, **MOVIE_OPTIONS)},
      {"http_status": 201, "stored_options": NO_OPTIONS}),
    T("ignores advanced options on a series request from a regular requester",
      "A regular requester's series request ignores the server, profile, root folder, language profile and tags it sends",
      "request.create", {"sonarr": [{"id": 1, "isDefault": True, "is4k": False}, {"id": 2, "isDefault": False, "is4k": False}],
                         "radarr": [], "by": DEMO,
                         "body": dict({"mediaType": "tv", "mediaId": 88021, "seasons": [1]}, **TV_OPTIONS)},
      {"http_status": 201, "stored_options": NO_OPTIONS}),
    O("ignores a 4K server and its options on a non-4K request",
      "A non-4K request naming a 4K server falls back to the default server and stores no options", NO_4K),
    T("keeps advanced options from an advanced requester",
      "A requester with REQUEST_ADVANCED keeps the server, profile, root folder, language profile and tags it sends",
      "request.create", {"sonarr": [{"id": 1, "isDefault": True, "is4k": False}, {"id": 2, "isDefault": False, "is4k": False}],
                         "radarr": [], "by": DEMO_ADV,
                         "body": dict({"mediaType": "tv", "mediaId": 88023, "seasons": [1]}, **TV_OPTIONS)},
      {"http_status": 201, "stored_options": TV_OPTIONS}),
    T("keeps advanced options when the serverId is null",
      "A REQUEST_ADVANCED movie request with serverId null keeps its profile, root folder and tags",
      "request.create", {"radarr": [{"id": 1, "isDefault": True, "is4k": False}], "sonarr": [], "by": DEMO_ADV,
                         "body": dict({"mediaType": "movie", "mediaId": 88024}, **dict(MOVIE_OPTIONS, serverId=None))},
      {"http_status": 201, "stored_options": dict(MOVIE_OPTIONS, serverId=None, languageProfileId=None)}),
]


def tv_show(tmdb, seasons):
    """seedTvShow(): request.test.ts:1889; media PROCESSING, approved admin requests (:1906)."""
    return {"media_type": "tv", "tmdb_id": tmdb, "status": "PROCESSING", "status4k": "UNKNOWN", "seasons": seasons}


ORPHAN = [
    T("resets a request-covered PROCESSING season to UNKNOWN so it can be re-requested",
      "Deleting the approved request that covers a PROCESSING season resets the season to UNKNOWN, and the season can be requested again",
      "request.delete", {"media": tv_show(99101, [{"seasonNumber": 1, "status": "PROCESSING"}]),
                         "requests": [{"name": "r1", "status": "APPROVED", "requested_by": ADMIN, "seasons": [1],
                                       "season_status": "APPROVED", "is4k": False}],
                         "actor": ADMIN, "delete": ["r1"],
                         "then_request": {"by": DEMO, "body": {"mediaType": "tv", "mediaId": 99101, "seasons": [1]}}},
      {"http_statuses": [204], "season_status": {"1": "UNKNOWN"}, "then_request_http_status": 201}),
    T("does not touch PROCESSING seasons the deleted request did not cover",
      "Deleting a request resets only the seasons it covered; another PROCESSING season stays PROCESSING",
      "request.delete", {"media": tv_show(99102, [{"seasonNumber": 1, "status": "PROCESSING"},
                                                  {"seasonNumber": 2, "status": "PROCESSING"}]),
                         "requests": [{"name": "r1", "status": "APPROVED", "requested_by": ADMIN, "seasons": [1],
                                       "season_status": "APPROVED", "is4k": False}],
                         "actor": ADMIN, "delete": ["r1"]},
      {"http_statuses": [204], "season_status": {"1": "UNKNOWN", "2": "PROCESSING"}}),
    T("keeps a season PROCESSING while another active request still covers it",
      "A PROCESSING season stays PROCESSING when another approved request still covers it",
      "request.delete", {"media": tv_show(99103, [{"seasonNumber": 1, "status": "PROCESSING"}]),
                         "requests": [{"name": "r1", "status": "APPROVED", "requested_by": ADMIN, "seasons": [1],
                                       "season_status": "APPROVED", "is4k": False},
                                      {"name": "r2", "status": "APPROVED", "requested_by": ADMIN, "seasons": [1],
                                       "season_status": "APPROVED", "is4k": False}],
                         "actor": ADMIN, "delete": ["r1"]},
      {"http_statuses": [204], "season_status": {"1": "PROCESSING"}}),
    O("leaves season status4k untouched when deleting a non-4K request",
      "Deleting a non-4K request resets the season status but leaves its 4K status PROCESSING", NO_4K),
]

# --- server/lib/watchlistsync.test.ts ----------------------------------------

SYNC_USER = {"permissions": ["AUTO_REQUEST"], "plex_token": True, "watchlistSyncMovies": True, "watchlistSyncTv": True}

WATCHLIST = [
    T("re-requests DELETED watchlist items and skips non-requestable ones",
      "Watchlist sync requests each watchlisted movie that is UNKNOWN or DELETED and each show that is DELETED, once, and skips AVAILABLE and BLOCKLISTED media",
      "watchlist.sync", {"user": SYNC_USER, "media": [
          {"media_type": "movie", "tmdb_id": 100, "status": "DELETED"},
          {"media_type": "movie", "tmdb_id": 101, "status": "UNKNOWN"},
          {"media_type": "movie", "tmdb_id": 102, "status": "AVAILABLE"},
          {"media_type": "movie", "tmdb_id": 103, "status": "BLOCKLISTED"},
          {"media_type": "tv", "tmdb_id": 200, "status": "DELETED"},
          {"media_type": "tv", "tmdb_id": 201, "status": "AVAILABLE"}],
          "watchlist": [{"type": "movie", "tmdbId": t} for t in (100, 101, 102, 103)]
          + [{"type": "show", "tmdbId": t, "tvdbId": t * 1000} for t in (200, 201)]},
      {"each_requested_once": True, "requested": ["movie:100", "movie:101", "tv:200"],
       "not_requested": ["movie:102", "movie:103", "tv:201"]}),
    T("re-requests DELETED watchlist items even when a stale auto-request exists",
      "Watchlist sync re-requests a DELETED movie even though the user's earlier completed auto-request for it exists",
      "watchlist.sync", {"user": SYNC_USER,
                         "media": [{"media_type": "movie", "tmdb_id": 100, "status": "DELETED"}],
                         "requests": [{"media": "movie:100", "status": "COMPLETED", "is4k": False,
                                       "is_auto_request": True, "requested_by": "sync user"}],
                         "watchlist": [{"type": "movie", "tmdbId": 100}]},
      {"request_calls_for_movie_100": 1}),
]

# --- server/lib/scanners/plex/plex.test.ts ------------------------------------

PLEX_SCAN = [
    T("leaves an approved request alone when the show has no episodes yet",
      "A Plex full scan that finds a requested show with no episodes yet leaves its approved request APPROVED",
      "plex.full_scan", {"libraries": [{"name": "TV Shows", "type": "show", "enabled": True}],
                         "media": {"media_type": "tv", "tmdb_id": 6100, "status": "PROCESSING", "status4k": "UNKNOWN",
                                   "seasons": [{"seasonNumber": 1, "status": "PROCESSING", "status4k": "UNKNOWN"}]},
                         "request": {"status": "APPROVED", "is4k": False, "seasons": [1], "season_status": "APPROVED"},
                         "plex": {"show_seasons": [1], "season_episodes": {"1": 0}}},
      {"request_status": "APPROVED"}),
]

# --- server/lib/scanners/baseScanner.test.ts ----------------------------------

IN_FLIGHT_SHOW = {"media": {"media_type": "tv", "status": "PROCESSING", "status4k": "UNKNOWN",
                            "seasons": [{"seasonNumber": 1, "status": "PROCESSING", "status4k": "UNKNOWN"}]},
                  "request": {"status": "APPROVED", "is4k": False, "seasons": [1], "season_status": "APPROVED"}}
NO_EPISODES = [{"seasonNumber": 1, "episodes": 0, "episodes4k": 0, "totalEpisodes": 10}]

BASE_SCANNER = [
    T("leaves the request alone for a scanner that has not opted in",
      "A media-server scan (no request declining) that finds no episodes for a requested season leaves the approved request APPROVED",
      "scanner.process_show", {**IN_FLIGHT_SHOW, "decline_requests_on_status_reset": False,
                               "scanned_seasons": NO_EPISODES, "resolve": True},
      {"request_status": "APPROVED"}),
    O("declines the request for a scanner that has opted in",
      "An *arr scan that finds no episodes for a requested season declines the request", ARR_SCAN),
    T("resets a season in the loop for a scanner that has not opted in",
      "A media-server scan that finds no episodes resets the PROCESSING season and the show to UNKNOWN at once and leaves the request APPROVED",
      "scanner.process_show", {**IN_FLIGHT_SHOW, "decline_requests_on_status_reset": False,
                               "scanned_seasons": NO_EPISODES, "resolve": False},
      {"season_status": {"1": "UNKNOWN"}, "media_status": "UNKNOWN", "request_status": "APPROVED"}),
    O("leaves a show alone at resolve time once something else has set it processing",
      "An *arr scan's deferred reset is dropped when the show was set PROCESSING again before resolve", ARR_SCAN),
    O("resets a movie in the loop for a scanner that has not opted in",
      "An abandoned movie entry (not processing, no file) resets a PROCESSING movie to UNKNOWN and leaves the request APPROVED",
      ARR_SCAN),
    O("defers a movie reset to the resolve pass for a scanner that has opted in",
      "An *arr scan defers an abandoned movie's reset to resolve, then resets it to UNKNOWN and declines the request", ARR_SCAN),
]

# --- server/lib/availabilitySync.test.ts --------------------------------------

TMDB_4 = [{"season_number": n, "episode_count": 10} for n in (1, 2, 3, 4)]  # beforeEach, availabilitySync.test.ts:448
# configureSonarr([{ syncEnabled: true }]): availabilitySync.test.ts:235 (defaults), :961 etc.
# Our syncEnabled is off; availabilitySync.ts never reads it.
ONE_SONARR_CFG = [{"is4k": False, "syncEnabled": True}]


def sonarr_series(sid, seasons_total, files):
    """fakeSonarrSeasons(): availabilitySync.test.ts:414 (10 episodes per season)."""
    return {"id": sid, "seasons": [{"seasonNumber": n, "episodeFileCount": files.get(n, 0), "totalEpisodeCount": 10}
                                   for n in range(1, seasons_total + 1)]}


AVAIL_PLEX_TV = [
    T("should mark deleted seasons when Plex returns empty season metadata entries",
      "Availability sync marks every season with no episodes in Plex and no files in Sonarr DELETED, keeps the one with episodes AVAILABLE, and makes the show PARTIALLY_AVAILABLE",
      "availability.sync", {"media_server": "PLEX", "sonarr": ONE_SONARR_CFG, "tmdb_seasons": TMDB_4,
                            "media": {"media_type": "tv", "tmdb_id": 2000, "status": "AVAILABLE", "externalServiceId": 200,
                                      "seasons": [{"seasonNumber": n, "status": "AVAILABLE", "status4k": "UNKNOWN"}
                                                  for n in range(1, 9)]},
                            "plex": {"show_seasons": list(range(1, 9)),
                                     "season_episodes": {str(n): (21 if n == 6 else 0) for n in range(1, 9)}},
                            "sonarr_series": sonarr_series(200, 8, {6: 21})},
      {"season_status": {str(n): ("AVAILABLE" if n == 6 else "DELETED") for n in range(1, 9)},
       "media_status": "PARTIALLY_AVAILABLE"}),
    O("should mark a deleted show as DELETED when a second standard Sonarr instance has a colliding externalServiceId",
      "A show gone from its Sonarr and Plex is DELETED even if a second Sonarr has another series at the same id", ONE_SONARR),
    T("should assume season exists when getChildrenMetadata fails for episodes (safe fallback)",
      "When Plex fails to list a season's episodes, availability sync keeps the season and the show AVAILABLE",
      "availability.sync", {"media_server": "PLEX", "sonarr": ONE_SONARR_CFG, "tmdb_seasons": TMDB_4,
                            "media": {"media_type": "tv", "tmdb_id": 2001, "status": "AVAILABLE", "externalServiceId": 201,
                                      "seasons": [{"seasonNumber": 1, "status": "AVAILABLE", "status4k": "UNKNOWN"}]},
                            "plex": {"show_seasons": [1], "season_episodes": {"1": "error: Connection refused"}},
                            "sonarr_series": sonarr_series(201, 1, {1: 10})},
      {"season_status": {"1": "AVAILABLE"}, "media_status": "AVAILABLE"}),
    T("should mark deleted seasons when only some seasons have episodes in Plex (no Sonarr link)",
      "With no Sonarr link, availability sync marks seasons without episodes in Plex DELETED, keeps the others AVAILABLE, and makes the show PARTIALLY_AVAILABLE",
      "availability.sync", {"media_server": "PLEX", "sonarr": ONE_SONARR_CFG, "tmdb_seasons": TMDB_4,
                            "media": {"media_type": "tv", "tmdb_id": 2002, "status": "AVAILABLE", "externalServiceId": None,
                                      "seasons": [{"seasonNumber": n, "status": "AVAILABLE", "status4k": "UNKNOWN"}
                                                  for n in range(1, 5)]},
                            "plex": {"show_seasons": [1, 2, 3, 4], "season_episodes": {"1": 0, "2": 10, "3": 0, "4": 10}},
                            "sonarr_series": "lookup fails (404)"},
      {"season_status": {"1": "DELETED", "2": "AVAILABLE", "3": "DELETED", "4": "AVAILABLE"},
       "media_status": "PARTIALLY_AVAILABLE"}),
]

AVAIL_MERGED = [
    O("should remove a stale 4K season status when the shared item has no 4K episodes for that season",
      "With a 4K Sonarr, a season's 4K status is removed when the shared Plex item has no 4K episodes for it", NO_4K),
    O("should remove a stale non-4K season status when the shared item only has 4K episodes for that season",
      "With a 4K Sonarr, a season's non-4K status is removed when the shared Plex item only has 4K episodes for it", NO_4K),
    T("should keep a non-4K season with only 4K episodes when no 4K Sonarr server is configured",
      "Without a 4K Sonarr server, a season whose only episodes in Plex are 4K keeps its non-4K AVAILABLE status (and the show stays AVAILABLE)",
      "availability.sync", {"media_server": "PLEX", "sonarr": ONE_SONARR_CFG, "tmdb_seasons": TMDB_4,
                            "media": {"media_type": "tv", "tmdb_id": 687173, "status": "AVAILABLE", "externalServiceId": None,
                                      "seasons": [{"seasonNumber": 1, "status": "AVAILABLE", "status4k": "UNKNOWN"}]},
                            "plex": {"show_seasons": [1],
                                     "season_episodes": {"1": [{"versions": [{"width": 3840, "videoResolution": "4k"}]}]}},
                            "sonarr_series": "lookup fails (404)"},
      {"season_status": {"1": "AVAILABLE"}, "media_status": "AVAILABLE"},
      note="externalServiceId unset and the Sonarr lookup are the beforeEach defaults (availabilitySync.test.ts:442)."),
    O("should not remove the 4K show status when episode metadata cannot be fetched for any season",
      "With a 4K Sonarr, the show's 4K status is kept when no season's episodes can be fetched", NO_4K),
]

AVAIL_SPECIALS = [
    O("should not demote an available show when only the specials season is missing (Jellyfin)",
      "An AVAILABLE show stays AVAILABLE when only its specials season is missing from Jellyfin", JELLYFIN),
    T("should not demote an available show when only the specials season is missing (Plex)",
      "An AVAILABLE show stays AVAILABLE when only its specials season (season 0) is missing from Plex",
      "availability.sync", {"media_server": "PLEX", "sonarr": ONE_SONARR_CFG,
                            "tmdb_seasons": [{"season_number": 0, "episode_count": 3},
                                             {"season_number": 1, "episode_count": 10}],
                            "media": {"media_type": "tv", "tmdb_id": 13863, "status": "AVAILABLE", "externalServiceId": 301,
                                      "seasons": [{"seasonNumber": 0, "status": "UNKNOWN", "status4k": "UNKNOWN"},
                                                  {"seasonNumber": 1, "status": "AVAILABLE", "status4k": "UNKNOWN"}]},
                            "plex": {"show_seasons": [1], "season_episodes": {"1": 10}},
                            "sonarr_series": sonarr_series(301, 1, {1: 10})},
      {"media_status": "AVAILABLE"}),
    O("should mark a removed specials season as DELETED without demoting the show (Jellyfin)",
      "A specials season removed from Jellyfin becomes DELETED and the show stays AVAILABLE", JELLYFIN),
]

# --- server/api/servarr/radarr.test.ts ----------------------------------------

ADD_OPTIONS = {"title": "Test Movie", "qualityProfileId": 1, "profileId": 1, "minimumAvailability": "Released",
               "tags": [], "year": 2026, "rootFolderPath": "/movies", "tmdbId": 7, "searchNow": True}

RADARR_ADD = [
    T("does not trigger a search for an existing, monitored movie that is not yet available",
      "Adding (search on) a movie Radarr already has, monitored and not yet available, does not start a search",
      "radarr.add_movie", {"options": ADD_OPTIONS,
                           "existing": {"id": 7, "monitored": True, "hasFile": False, "isAvailable": False}},
      {"search_calls": 0}),
    T("triggers a search for an existing, monitored movie that is already available",
      "Adding (search on) a movie Radarr already has, monitored, available and without a file, starts one search",
      "radarr.add_movie", {"options": ADD_OPTIONS,
                           "existing": {"id": 7, "monitored": True, "hasFile": False, "isAvailable": True}},
      {"search_calls": 1}),
    T("does not trigger a search for an existing, previously unmonitored movie that is not yet available",
      "Adding (search on) a movie Radarr has unmonitored updates it to monitored once and does not search when it is not yet available",
      "radarr.add_movie", {"options": dict(ADD_OPTIONS, monitored=True),
                           "existing": {"id": 7, "monitored": False, "hasFile": False, "tags": []},
                           "update_returns": {"id": 7, "monitored": True, "isAvailable": False}},
      {"update_calls": 1, "search_calls": 0}),
    T("triggers a search for an existing, previously unmonitored movie that is already available",
      "Adding (search on) a movie Radarr has unmonitored updates it to monitored once and searches for movie 7 when it is available",
      "radarr.add_movie", {"options": dict(ADD_OPTIONS, monitored=True),
                           "existing": {"id": 7, "monitored": False, "hasFile": False, "tags": []},
                           "update_returns": {"id": 7, "monitored": True, "isAvailable": True}},
      {"update_calls": 1, "search_calls": 1, "search_movie_id": 7}),
]

# (file under server/, corpus name, [(describe line, cases)])
FIXTURES = [
    ("entity/MediaRequest.test.ts", "MediaRequest", [(84, MEDIA_REQUEST)]),
    ("routes/request.test.ts", "request_route", [
        (231, DELETE), (316, EDIT_MOVIE), (434, EDIT_TV), (514, EDIT_AVAIL), (844, EDIT_ADVANCED),
        (912, STATUS), (1009, RETRY), (1069, RESTORE), (1766, ADVANCED_CREATE), (1883, ORPHAN)]),
    ("lib/watchlistsync.test.ts", "watchlistsync", [(97, WATCHLIST)]),
    ("lib/scanners/plex/plex.test.ts", "plex_scanner", [(241, PLEX_SCAN)]),
    ("lib/scanners/baseScanner.test.ts", "base_scanner", [(156, BASE_SCANNER)]),
    ("lib/availabilitySync.test.ts", "availability_sync",
     [(958, AVAIL_PLEX_TV), (1727, AVAIL_MERGED), (2037, AVAIL_SPECIALS)]),
    ("api/servarr/radarr.test.ts", "radarr_api", [(122, RADARR_ADD)]),
]

# --- rules with no reference test, from the implementing source line ------------
# (file under server/, exact stripped source line, occurrence, total occurrences, case)

SRC_NOTE = "No reference test; expectation read from the implementing source line."


def S(path, line, nth, total, behaviour, op, given, expect):
    return dict(path=path, line=line, nth=nth, total=total,
                case=T(None, f"{behaviour} (no reference test; from the implementing source)", op, given, expect, SRC_NOTE))


SOURCE = [
    S("entity/MediaRequest.ts", "status: user.hasPermission(", 0, 3,
      "A movie request from a user with AUTO_APPROVE (or AUTO_APPROVE_MOVIE or MANAGE_REQUESTS) is created APPROVED",
      "request.create", {"by": {"permissions": ["REQUEST", "AUTO_APPROVE", "AUTO_REQUEST"]},
                         "body": {"mediaType": "movie", "is4k": False}},
      {"status": "APPROVED"}),
    S("entity/MediaRequest.ts", "status: user.hasPermission(", 0, 3,
      "A movie request from a user with REQUEST only is created PENDING",
      "request.create", {"by": {"permissions": ["REQUEST"]}, "body": {"mediaType": "movie", "is4k": False}},
      {"status": "PENDING"}),
    S("entity/MediaRequest.ts", "status: user.hasPermission(", 1, 3,
      "A series request from a user with AUTO_APPROVE (or AUTO_APPROVE_TV or MANAGE_REQUESTS) is created APPROVED",
      "request.create", {"by": {"permissions": ["REQUEST", "AUTO_APPROVE", "AUTO_REQUEST"]},
                         "body": {"mediaType": "tv", "is4k": False, "seasons": [1]}},
      {"status": "APPROVED"}),
    S("entity/MediaRequest.ts", "status: user.hasPermission(", 1, 3,
      "A series request from a user with REQUEST only is created PENDING",
      "request.create", {"by": {"permissions": ["REQUEST"]}, "body": {"mediaType": "tv", "is4k": False, "seasons": [1]}},
      {"status": "PENDING"}),
    S("entity/MediaRequest.ts", "status: user.hasPermission(", 2, 3,
      "A series request from a user with AUTO_APPROVE (or AUTO_APPROVE_TV or MANAGE_REQUESTS) creates every season request APPROVED",
      "request.create", {"by": {"permissions": ["REQUEST", "AUTO_APPROVE", "AUTO_REQUEST"]},
                         "body": {"mediaType": "tv", "is4k": False, "seasons": [1]}},
      {"season_status": "APPROVED"}),
    S("entity/MediaRequest.ts", "status: user.hasPermission(", 2, 3,
      "A series request from a user with REQUEST only creates every season request PENDING",
      "request.create", {"by": {"permissions": ["REQUEST"]}, "body": {"mediaType": "tv", "is4k": False, "seasons": [1]}},
      {"season_status": "PENDING"}),
    S("entity/MediaRequest.ts", "requestBody.seasons === 'all'", 0, 1,
      "A series request for 'all' seasons covers every TMDB season except season 0 and seasons with no episodes",
      "request.requested_seasons", {"seasons": "all", "enableSpecialEpisodes": False,
                                    "tmdb_seasons": [{"season_number": 0, "episode_count": 3},
                                                     {"season_number": 1, "episode_count": 10},
                                                     {"season_number": 2, "episode_count": 10},
                                                     {"season_number": 3, "episode_count": 0}]},
      {"requested_seasons": [1, 2]}),
    S("entity/MediaRequest.ts", "if (!settings.main.enableSpecialEpisodes) {", 0, 1,
      "With specials disabled, season 0 is dropped from an explicit season list",
      "request.requested_seasons", {"seasons": [0, 1], "enableSpecialEpisodes": False},
      {"requested_seasons": [1]}),
    S("entity/MediaRequest.ts", "const finalSeasons = requestedSeasons.filter(", 0, 1,
      "A new series request drops seasons held by another active (not DECLINED or COMPLETED) request of the same 4K kind and seasons whose media status is other than UNKNOWN or DELETED",
      "request.requested_seasons", {"seasons": [1, 2, 3, 4], "is4k": False,
                                    "other_requests": [{"status": "APPROVED", "is4k": False, "seasons": [1]},
                                                       {"status": "DECLINED", "is4k": False, "seasons": [3]}],
                                    "media_seasons": [{"seasonNumber": 2, "status": "AVAILABLE"},
                                                      {"seasonNumber": 4, "status": "DELETED"}]},
      {"requested_seasons": [3, 4]}),
    S("entity/MediaRequest.ts", "throw new NoSeasonsAvailableError('No seasons available to request');", 0, 1,
      "A new series request with no seasons left after those drops fails with NoSeasonsAvailableError",
      "request.create", {"body": {"mediaType": "tv", "seasons": [1]},
                         "media_seasons": [{"seasonNumber": 1, "status": "AVAILABLE"}]},
      {"error": "NoSeasonsAvailableError"}),
    S("subscriber/MediaRequestSubscriber.ts", "searchNow: !radarrSettings.preventSearch,", 0, 1,
      "An approved movie is sent to Radarr with search on add unless the server has preventSearch",
      "request.send_to_radarr", {"radarr": {"preventSearch": False}}, {"searchNow": True}),
    S("subscriber/MediaRequestSubscriber.ts", "searchNow: !sonarrSettings.preventSearch,", 0, 1,
      "An approved series is sent to Sonarr with search on add unless the server has preventSearch",
      "request.send_to_sonarr", {"sonarr": {"preventSearch": False}}, {"searchNow": True}),
    S("routes/auth.ts", "permissions: settings.main.defaultPermissions,", 0, 3,
      "A Plex user signing in for the first time with new Plex sign-in on is created as a PLEX user with the default permissions",
      "auth.plex_sign_in", {"newPlexLogin": True, "defaultPermissions": 8388768, "existing_user": False,
                             "first_user": False, "has_server_access": True},
      {"user_created": True, "userType": "PLEX", "permissions": 8388768}),  # main.json defaultPermissions
    S("lib/watchlistsync.ts", "Permission.AUTO_REQUEST,", 0, 1,
      "Watchlist sync skips a user without AUTO_REQUEST, AUTO_REQUEST_MOVIE or AUTO_REQUEST_TV",
      "watchlist.sync", {"user": {"permissions": ["REQUEST", "AUTO_APPROVE"], "plex_token": True,
                                  "watchlistSyncMovies": True, "watchlistSyncTv": True},
                         "watchlist": [{"type": "movie", "tmdbId": 100}]},
      {"requested": []}),
    S("lib/watchlistsync.ts", "seasons: mediaItem.type === 'show' ? 'all' : undefined,", 0, 1,
      "Watchlist sync requests a show for 'all' seasons, non-4K, as an auto-request by the watchlist's owner",
      "watchlist.sync", {"user": SYNC_USER, "watchlist": [{"type": "show", "tmdbId": 200, "tvdbId": 200000}]},
      {"request": {"mediaType": "tv", "mediaId": 200, "seasons": "all", "is4k": False, "isAutoRequest": True}}),
]

RULED_OUT = {
    "routes/request.test.ts:612 PUT (quota)": QUOTA + "; quota enforcement never triggers (the quota rows of "
        "other ported blocks, MediaRequest.test.ts:85 and request.test.ts:586, are catalogued in_scope: false).",
    "routes/request.test.ts:891 GET /request/:id": "Which user fields Seerr's REST response omits (notification settings); "
        "API serialization, not a request rule.",
    "routes/request.test.ts:1339, :1516, :1699 (override rules)": "No override rules (overriderule.json is empty).",
    "routes/auth.test.ts (all)": "Jellyfin quick connect, local sign-in, password reset, /auth/me shape; we sign in "
        "with Plex. The default-permissions rule of :502 is ported from routes/auth.ts (Plex path) instead.",
    "lib/availabilitySync.test.ts:488 (Jellyfin), :1308 (two Radarrs), :1358, :1595, :1681 (4K Radarr)":
        f"{JELLYFIN}; we have one Radarr; {NO_4K}.",
    "lib/scanners/sonarr/*.test.ts, lib/scanners/radarr/*.test.ts": "syncEnabled is off on both servers.",
    "lib/scanners/jellyfin/*.test.ts, api/jellyfin.test.ts": JELLYFIN + ".",
    "api/servarr/radarr.test.ts:17, :96; api/servarr/sonarr.test.ts (all)": "removeMovie/removeSeries and "
        "lookup error handling of Seerr's *arr HTTP client; in Scryer requests and library are one app.",
    "api/plextv.test.ts": "Plex Discover HTTP client caching, concurrency and error handling under watchlist sync.",
    "routes/blocklist.test.ts": "No blocklist in use (blocklist keyword tags empty).",
    "routes/mediaInfo.test.ts, routes/issue.test.ts, routes/user/*.test.ts": "Response privacy, issues and user "
        "management API; not request rules.",
    "routes/imageproxy.test.ts, lib/imageproxy.test.ts, utils/*.test.ts": "Image proxy and utilities.",
    "src/utils/refreshIntervalHelper.test.ts": "Frontend (src/) polling-interval helper for the UI; no request rule.",
    "cypress/e2e/**": "UI only (pages load, settings fields shown or hidden by permission); no request rule.",
}


# --- reference anchoring ----------------------------------------------------

IT = re.compile(r"""\bit\((['"`])(.*?)\1""")


def block_its(lines, start):
    """[(line_no, title)] of it() calls inside the describe() block opening at `start` (1-based)."""
    assert re.search(r"\bdescribe\(", lines[start - 1]), f"line {start}: not a describe"
    depth, out = 0, []
    for n in range(start, len(lines) + 1):
        line = lines[n - 1]
        if n > start and (m := IT.search(line)):
            out.append((n, m.group(2)))
        depth += line.count("{") - line.count("}")
        if n > start and depth <= 0:
            return out
    raise AssertionError(f"line {start}: describe never closes")


def anchor_source(rel, text, nth, total):
    lines = (REF_REPO / "server" / rel).read_text(encoding="utf-8").splitlines()
    hits = [i + 1 for i, l in enumerate(lines) if l.strip() == text]
    assert len(hits) == total, f"{rel}: '{text}' found {len(hits)}x, expected {total}"
    return hits[nth]


def existing_ids():
    """{(ref, row): id}; row counts entries sharing one ref, in file order."""
    if not CATALOGUE.exists():
        return {}
    seen, out = {}, {}
    for i, r in re.findall(r'- id: (\S+)\n(?:  .*\n)*?  ref: "(.*)"', CATALOGUE.read_text()):
        row = seen.get(r, 0)
        seen[r] = row + 1
        out[(r, row)] = i
    return out


def collect():
    """[(sort key, ref, case, corpus file)] for every entry, in deterministic order."""
    items = []
    for fi, (rel, name, blocks) in enumerate(FIXTURES):
        path = f"server/{rel}"
        lines = (REF_REPO / path).read_text(encoding="utf-8").splitlines()
        for start, cases in blocks:
            declared = block_its(lines, start)
            # a template-literal title is an it() inside a loop: one row per loop value
            titles = sorted(t for _, t in declared if "${" not in t)
            covered = sorted(c["title"] for c in cases if "${" not in c["title"])
            assert titles == covered and {t for _, t in declared} == {c["title"] for c in cases}, \
                f"{path}:{start}: uncovered {set(titles) - set(covered)}, unknown {set(covered) - set(titles)}"
            for c in cases:
                line = [n for n, t in declared if t == c["title"]]
                assert len(set(line)) == 1, f"{path}: '{c['title']}' at {line}"
                items.append(((fi, line[0]), f"{TAG} {path}:{line[0]}", c, name))
    for s in SOURCE:
        line = anchor_source(s["path"], s["line"], s["nth"], s["total"])
        items.append(((len(FIXTURES), s["path"], line), f"{TAG} server/{s['path']}:{line}", s["case"], "source"))
    items.sort(key=lambda x: x[0])  # stable: rows sharing a line keep their written order
    return items


def kept_lines():
    """{id: [status/decision/tests lines as written]} from the current catalogue (ratchet-owned, never regenerated)."""
    if not CATALOGUE.exists():
        return {}
    return {m[0]: re.findall(r"(?m)^  ((?:status|decision|tests):.*(?:\n    .*)*)", m[1])
            for m in re.findall(r"(?m)^- id: (\S+)\n((?:  .*\n?|    .*\n?)*)", CATALOGUE.read_text())}


def main():
    ids = existing_ids()
    kept = kept_lines()
    next_n = max((int(i.rsplit("-", 1)[1]) for i in ids.values()), default=0) + 1
    rows, entries, corpus = {}, [], {}
    for _, ref, case, name in collect():
        row = rows.get(ref, 0)
        rows[ref] = row + 1
        if (ref, row) not in ids:
            assert next_n <= ID_MAX, "ID range exhausted"
            ids[(ref, row)] = f"{ID_PREFIX}{next_n:04d}"
            next_n += 1
        cid, in_scope = ids[(ref, row)], case["op"] is not None
        entries.append((cid, case["behaviour"], ref, in_scope))
        if in_scope:
            c = {"id": cid, "ref": ref, "input": {"op": case["op"], "given": case["given"]}, "expect": case["expect"]}
            if case.get("note"):
                c["note"] = case["note"]
            corpus.setdefault(name, []).append(c)

    CORPUS.mkdir(parents=True, exist_ok=True)
    for name, cases in corpus.items():
        (CORPUS / f"{name}.json").write_text(json.dumps(cases, indent=1, ensure_ascii=False) + "\n")

    out = ["# Seerr request-lifecycle tests (and untested rules, from source), ported by parity/extract/seerr_requests.py.",
           "# Corpus: corpus/reference/seerr/requests/. in_scope: false rows say why after \"out of scope:\".", ""]
    for cid, behaviour, ref, in_scope in entries:
        out += [f"- id: {cid}", f"  behaviour: {json.dumps(behaviour, ensure_ascii=False)}", "  source: reference",
                f"  ref: {json.dumps(ref)}", f"  in_scope: {'true' if in_scope else 'false'}",
                *[f"  {k}" for k in kept.get(cid, ["status: unmapped", "tests: []"])]]
    CATALOGUE.write_text("\n".join(out) + "\n")
    n_in = sum(e[3] for e in entries)
    print(f"entries {len(entries)} (in scope {n_in}, out {len(entries) - n_in}), corpus cases "
          f"{sum(map(len, corpus.values()))}, id range {entries[0][0]}..{max(e[0] for e in entries)}")
    for what, why in RULED_OUT.items():
        print(f"ruled out: {what}: {why}")


if __name__ == "__main__":
    sys.exit(main())
