"""Port Sonarr download-client and download-handling fixtures (area: torrent).

Scope: our single qBittorrent client (category tv-sonarr, remove completed and
failed on, seed goals from indexers) and the completed/failed download
handling around it. Hand-translated: the C# arrange/assert of each test is
written below as data. The script finds each test's line in the reference
repo (so refs are exact), checks every [TestCase] row against the parameters
written here, checks every [Test]/[TestCase] method of a fixture is ported,
keeps the IDs already in the catalogue (keyed by the "# key:" comment above
each entry: fixture path, method, row index and row arguments, so line shifts
and tag bumps do not renumber) and writes:

  parity/corpus/reference/sonarr/torrent/<Fixture>.json
  parity/catalogue/torrent.sonarr.yaml

Conventions in `expect`:
  {"not": v}          the test asserts the value is not v
  {"not_empty": true} the test asserts a non-null, non-empty value
  {"between": [a, b]} inclusive range
  null                the test asserts no value (NotHaveValue / BeNull)
Paths are the Unix form AsOsAgnostic() gives (C:\\X -> /C/X). Times are
minutes unless the key says otherwise. Floats written with an f suffix in C#
are single precision; they are copied as written.

Run: python3 -I parity/extract/sonarr_torrent.py
"""

import ast
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REF_REPO = Path.home() / "PycharmProjects/arr-reference/sonarr@v4.0.20.3014"
TAG = "Sonarr@v4.0.20.3014"
TEST_DIR = "src/NzbDrone.Core.Test/"
CATALOGUE = ROOT / "parity/catalogue/torrent.sonarr.yaml"
CORPUS = ROOT / "parity/corpus/reference/sonarr/torrent"
ID_PREFIX, ID_MAX = "SON-TORRENT-", 4999  # 5001+ belong to torrent.sonarr-config.yaml


def T(method, behaviour, op, given, expect, note=None):
    return dict(kind="test", method=method, behaviour=behaviour, op=op, given=given, expect=expect, note=note)


def R(method, row, params, behaviour, op, given, expect, note=None):
    return dict(kind="row", method=method, row=row, params=params, behaviour=behaviour, op=op,
                given=given, expect=expect, note=note)


def X(method, behaviour, why, rows=None):
    """Out of scope. rows: list of [TestCase] params when the method has rows."""
    if rows is None:
        return [dict(kind="test", method=method, behaviour=behaviour, op=None, given=None, expect=None, why=why)]
    return [dict(kind="row", method=method, row=i, params=p, behaviour=f"{behaviour} ({', '.join(map(str, p))})",
                 op=None, given=None, expect=None, why=why) for i, p in enumerate(rows)]


NBUILDER = ("Unset string fields come from NBuilder (<Property>1, e.g. DownloadId 'DownloadId1', "
            "Category 'Category1'); bools are false.")

# --- QBittorrentFixture -----------------------------------------------------

TITLE = "Droned.S01E01.Pilot.1080p.WEB-DL-DRONE"
DL_URL = "http://somewhere.com/Droned.S01E01.Pilot.1080p.WEB-DL-DRONE.ext"
SETTINGS = {"host": "127.0.0.1", "port": 2222, "tv_category": "tv"}
DHT_ON = {"dht_enabled": True}
# Setup: Subject.Definition = new DownloadClientDefinition() (Id 0, Name null).
DEFINITION = {"id": 0, "name": None}
# VerifyIdentifiable (DownloadClientFixtureBase.cs:77-84): client info matches the definition.
IDENT = {"client_info": {"protocol": "Torrent", **DEFINITION},
         "download_id": {"not_empty": True}, "title": {"not_empty": True}}


def torrent(state, progress=0.7, eta=8640000, **kw):
    return {"hash": "HASH", "name": TITLE, "size": 1000, "progress": progress, "eta": eta, "state": state,
            "label": "", "save_path": "", **kw}


def direct(t):
    """A torrent built with new QBittorrentTorrent{...}, not GivenCompletedTorrent: C# defaults made explicit."""
    return {"ratio_limit": -2, "seeding_time_limit": -2, "inactive_seeding_time_limit": -2,
            "last_activity_unix": 0, **t}


DIRECT_NOTE = ("Built directly, not via GivenCompletedTorrent: per-torrent limits keep their -2 default (use global) "
               "and LastActivity its C# default 0, i.e. the unix epoch (last_activity_unix), not 'now'. No seeding "
               "time is arranged: GivenTorrents mocks properties for the lower-case hash with SavePath only. Global "
               "seeding and inactive limits are off (-1), so neither is consulted.")


def items(t, preferences=None, **extra):
    return {"settings": SETTINGS, "definition": DEFINITION, "preferences": preferences or DHT_ON, "torrents": [t],
            **extra}


def one(**item):
    return {"item_count": 1, "item": {**IDENT, **item}}


def limits(max_ratio, max_seeding_time=-1, max_inactive=-1):
    """GivenGlobalSeedLimits: replaces the preferences (so dht_enabled is false)."""
    return {"max_ratio_action": "Pause", "max_ratio": max_ratio, "max_ratio_enabled": max_ratio >= 0,
            "max_seeding_time": max_seeding_time, "max_seeding_time_enabled": max_seeding_time >= 0,
            "max_inactive_seeding_time": max_inactive, "max_inactive_seeding_time_enabled": max_inactive >= 0}


def done(state="pausedUP", ratio=0.1, ratio_limit=-2, seeding_time=1, seeding_time_limit=-2,
         inactive_limit=-2, last_activity_min_ago=0):
    """GivenCompletedTorrent; seeding_time (minutes) is served by the properties call, in seconds."""
    t = torrent(state, progress=1.0, ratio=ratio, ratio_limit=ratio_limit, seeding_time_limit=seeding_time_limit,
                inactive_seeding_time_limit=inactive_limit, last_activity_minutes_ago=last_activity_min_ago)
    t["properties"] = {"seeding_time_s": seeding_time * 60}
    return t


def removable(yes):
    return {"item_count": 1, "item": {"can_be_removed": yes, "can_move_files": yes}}


PAUSED = ["pausedUP", "stoppedUP"]


def seed_rows(method, behaviour, pref, t_kw, yes, note=None):
    return [R(method, i, [s], f"{behaviour} ({s})", "qbittorrent.get_items",
              items(done(s, **t_kw), pref), removable(yes), note) for i, s in enumerate(PAUSED)]


def state_rows(method, states, behaviour, progress, expect):
    return [R(method, i, [s], f"{behaviour} ({s})", "qbittorrent.get_items",
              items(torrent(s, progress=progress)), expect) for i, s in enumerate(states)]


SINGLE_PREC = "Ratios with an f suffix are C# single-precision floats."
TORRENT_FILE = {"http_get": {"status": 200, "body": "empty"}, "torrent_file_hash": "CBC2F069FE8BB2F544EAE707D75BCD3DE9DCF951"}


def download(url=DL_URL, preferences=None, **extra):
    return {"settings": SETTINGS, "preferences": preferences or DHT_ON,
            "release": {"title": TITLE, "download_url": url, "protocol": "Torrent"},
            **TORRENT_FILE, **extra}


QBT = [
    T("error_item_should_have_required_properties", "A torrent in state 'error' maps to a Warning queue item",
      "qbittorrent.get_items", items(torrent("error")), one(status="Warning")),
    *state_rows("paused_item_should_have_required_properties", ["pausedDL", "stoppedDL"],
                "A paused/stopped incomplete torrent maps to a Paused item with remaining size and no ETA", 0.7,
                one(status="Paused", remaining_size={"not": 0}, remaining_time=None)),
    *state_rows("completed_item_should_have_required_properties",
                ["pausedUP", "stoppedUP", "queuedUP", "uploading", "stalledUP", "forcedUP"],
                "A fully downloaded torrent in a seeding/stopped state maps to a Completed item with nothing remaining", 1.0,
                one(status="Completed", remaining_size=0, remaining_time=0)),
    *state_rows("queued_item_should_have_required_properties",
                ["queuedDL", "checkingDL", "checkingUP", "metaDL", "checkingResumeData"],
                "A queued/checking/metadata torrent maps to a Queued item with no ETA", 0.7,
                one(status="Queued", remaining_size={"not": 0}, remaining_time=None)),
    T("downloading_item_should_have_required_properties",
      "A downloading torrent maps to a Downloading item with a non-zero ETA",
      "qbittorrent.get_items", items(torrent("downloading", eta=60)),
      one(status="Downloading", remaining_size={"not": 0}, remaining_time={"not": 0})),
    T("stalledDL_item_should_have_required_properties", "A stalled download (stalledDL) maps to a Warning item with no ETA",
      "qbittorrent.get_items", items(torrent("stalledDL")), one(status="Warning", remaining_time=None)),
    T("missingFiles_item_should_have_required_properties", "A torrent with missing files maps to a Warning item with no ETA",
      "qbittorrent.get_items", items(torrent("missingFiles")), one(status="Warning", remaining_time=None)),
    T("single_file_torrent_outputpath_should_have_sanitised_name",
      "A single-file torrent's import path is save path + the file's name, not the torrent name",
      "qbittorrent.get_import_item",
      items(torrent("stalledDL", name="Droned.S01E01.Test\\'s.1080p.WEB-DL-DRONE.mkv", save_path="/C/Torrents"),
            files=["Droned.S01E01.Tests.1080p.WEB-DL-DRONE.mkv"], item={"download_id": "HASH"}),
      {"output_path": "/C/Torrents/Droned.S01E01.Tests.1080p.WEB-DL-DRONE.mkv"}),
    T("single_file_torrent_with_folder_should_only_have_first_subfolder",
      "A single-file torrent inside a folder imports from save path + the first folder only",
      "qbittorrent.get_import_item",
      items(torrent("stalledDL", name="Droned.S01E01.Test\\'s.1080p.WEB-DL-DRONE", save_path="/C/Torrents"),
            files=["Folder/Droned.S01E01.Tests.1080p.WEB-DL-DRONE.mkv"], item={"download_id": "HASH"}),
      {"output_path": "/C/Torrents/Folder"}),
    T("multi_file_torrent_outputpath_should_have_sanitised_name",
      "A multi-file torrent imports from save path + the files' common top folder, not the torrent name",
      "qbittorrent.get_import_item",
      items(torrent("stalledDL", name="Droned.S01.\\1/2", save_path="/C/Torrents"),
            files=["Droned.S01.12/E01.mkv", "Droned.S01.12/E02.mkv"], item={"download_id": "HASH"}),
      {"output_path": "/C/Torrents/Droned.S01.12"}),
    *[R("api_261_should_use_content_path", i, [s], f"With Web API 2.6.1+ the import path is the torrent's content path ({s})",
        "qbittorrent.get_import_item",
        items(torrent(s, name="Droned.S01.\\1/2", save_path="/C/Torrents", content_path="/C/Torrents/Droned.S01.12"),
              api_version="2.6.1", item={"download_id": "HASH", "output_path": "/C/Torrents/Droned.S01.12"}),
        {"output_path": "/C/Torrents/Droned.S01.12"},
        note="Torrent files are not set; GivenTorrents serves one file named after the torrent.")
      for i, s in enumerate(PAUSED)],
    T("Download_should_return_unique_id", "Adding a .torrent URL returns a download id",
      "qbittorrent.download", download(), {"download_id": {"not_empty": True}}),
    R("Download_should_get_hash_from_magnet_url", 0,
      ["magnet:?xt=urn:btih:ZPBPA2P6ROZPKRHK44D5OW6NHXU5Z6KR&tr=udp", "CBC2F069FE8BB2F544EAE707D75BCD3DE9DCF951"],
      "A base32 magnet info-hash is returned as the 40-char upper-case hex download id",
      "qbittorrent.download", download("magnet:?xt=urn:btih:ZPBPA2P6ROZPKRHK44D5OW6NHXU5Z6KR&tr=udp"),
      {"download_id": "CBC2F069FE8BB2F544EAE707D75BCD3DE9DCF951"}),
    T("Download_should_refuse_magnet_if_no_trackers_provided_and_dht_is_disabled",
      "A magnet with no trackers is refused when qBittorrent has DHT off",
      "qbittorrent.download", download("magnet:?xt=urn:btih:ZPBPA2P6ROZPKRHK44D5OW6NHXU5Z6KR", {"dht_enabled": False}),
      {"error": "ReleaseDownloadException"}),
    T("Download_should_accept_magnet_if_trackers_provided_and_dht_is_disabled",
      "A magnet with a tracker is added by URL even when DHT is off",
      "qbittorrent.download",
      download("magnet:?xt=urn:btih:ZPBPA2P6ROZPKRHK44D5OW6NHXU5Z6KR&tr=udp://abc", {"dht_enabled": False}),
      {"error": None, "add_torrent_from_url_calls": 1}),
    T("Download_should_set_top_priority", "With priority First the added torrent is moved to the top of the queue",
      "qbittorrent.download", download(settings={**SETTINGS, "recent_tv_priority": "First", "older_tv_priority": "First"}),
      {"move_to_top_calls": 1}),
    T("Download_should_not_fail_if_top_priority_not_available",
      "If moving to the top of the queue is refused (HTTP 403) the grab still succeeds with one warning",
      "qbittorrent.download",
      download(settings={**SETTINGS, "recent_tv_priority": "First", "older_tv_priority": "First"},
               move_to_top_response={"status": 403}),
      {"download_id": {"not_empty": True}, "warnings_logged": 1}),
    T("should_return_status_with_outputdirs", "Client status reports localhost and qBittorrent's default save path as the output root",
      "qbittorrent.get_status", {"settings": SETTINGS, "preferences": {"save_path": "/C/Downloads/Finished/QBittorrent"}},
      {"is_localhost": True, "output_root_folders": {"first": "/C/Downloads/Finished/QBittorrent"}}),
    T("should_correct_category_output_path",
      "With Web API 2.0+ the output root is the category's save path, '//server/x' read as a UNC path",
      "qbittorrent.get_status",
      {"settings": SETTINGS, "preferences": {"save_path": "/C/Downloads/Finished/QBittorrent"}, "api_version": "2.0",
       "categories": {"tv": {"name": "tv", "save_path": "//server/store/downloads"}}},
      {"is_localhost": True, "output_root_folders": {"first": "\\\\server\\store\\downloads"}},
      note="Expected value is a literal (not AsOsAgnostic) in the test."),
    T("Download_should_handle_http_redirect_to_magnet", "A .torrent URL that redirects (303) to a magnet is still added",
      "qbittorrent.download",
      download(http_get={"status": 303, "location": "magnet:?xt=urn:btih:ZPBPA2P6ROZPKRHK44D5OW6NHXU5Z6KR&tr=udp"}),
      {"download_id": {"not_empty": True}}),
    T("Download_should_handle_http_redirect_to_torrent", "A .torrent URL that redirects (302) to another .torrent is still added",
      "qbittorrent.download",
      download(http_get={"status": 302, "location": "http://test.sonarr.tv/not-a-real-torrent.torrent"}),
      {"download_id": {"not_empty": True}},
      note="Only the original URL is mocked to redirect; other GETs return 200 with an empty body."),
    T("should_not_be_removable_and_should_not_allow_move_files_if_max_ratio_not_reached",
      "A seeding torrent below the global ratio limit can be neither removed nor moved",
      "qbittorrent.get_items", items(direct(torrent("uploading", progress=1.0, ratio=0.5)), limits(1.0)),
      removable(False), note=DIRECT_NOTE),
    T("should_not_be_removable_and_should_not_allow_move_files_if_max_ratio_reached_and_not_paused",
      "A torrent at the ratio limit but still seeding (uploading) can be neither removed nor moved",
      "qbittorrent.get_items", items(done("uploading", ratio=1.0), limits(1.0)), removable(False)),
    *seed_rows("should_not_be_removable_and_should_not_allow_move_files_if_max_ratio_is_not_set",
               "A stopped torrent with no seed limit anywhere is never removable", limits(-1), {"ratio": 1.0}, False),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_max_ratio_reached_and_paused",
               "A stopped torrent at the global ratio limit is removable and movable", limits(1.0), {"ratio": 1.0}, True),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_max_ratio_reached_after_rounding_and_paused",
               "A stopped torrent at ratio 1.1006 against global 1.0 is removable", limits(1.0),
               {"ratio": 1.1006066990976857}, True, SINGLE_PREC),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_just_under_max_ratio_reached_after_rounding_and_paused",
               "A stopped torrent at ratio 0.9999 counts as reaching global 1.0 (rounding)", limits(1.0),
               {"ratio": 0.9999}, True, SINGLE_PREC),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_overridden_max_ratio_reached_and_paused",
               "A per-torrent ratio limit 0.8 overrides global 2.0: ratio 1.0 is removable", limits(2.0),
               {"ratio": 1.0, "ratio_limit": 0.8}, True, SINGLE_PREC),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_overridden_max_ratio_reached_after_rounding_and_paused",
               "Ratio 1.1006 reaches a per-torrent limit 1.1 (rounding) over global 2.0", limits(2.0),
               {"ratio": 1.1006066990976857, "ratio_limit": 1.1}, True, SINGLE_PREC),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_just_under_overridden_max_ratio_reached_after_rounding_and_paused",
               "Ratio 0.9999 reaches a per-torrent limit 1.0 (rounding) over global 2.0", limits(2.0),
               {"ratio": 0.9999, "ratio_limit": 1.0}, True, SINGLE_PREC),
    *seed_rows("should_not_be_removable_if_overridden_max_ratio_not_reached_and_paused",
               "A per-torrent ratio limit 0.8 overrides global 0.2: ratio 0.5 is not removable", limits(0.2),
               {"ratio": 0.5, "ratio_limit": 0.8}, False, SINGLE_PREC),
    T("should_not_be_removable_and_should_not_allow_move_files_if_max_seedingtime_reached_and_not_paused",
      "A torrent past the global seeding time but still uploading is not removable",
      "qbittorrent.get_items", items(done("uploading", ratio=2.0, seeding_time=30), limits(-1, 20)), removable(False)),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_max_seedingtime_reached_and_paused",
               "A stopped torrent that seeded the global 20 min is removable", limits(-1, 20),
               {"ratio": 2.0, "seeding_time": 20}, True),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_overridden_max_seedingtime_reached_and_paused",
               "A per-torrent seeding time 10 min overrides global 40: 20 min seeded is removable", limits(-1, 40),
               {"ratio": 2.0, "seeding_time": 20, "seeding_time_limit": 10}, True),
    *seed_rows("should_not_be_removable_if_overridden_max_seedingtime_not_reached_and_paused",
               "A per-torrent seeding time 40 min overrides global 20: 30 min seeded is not removable", limits(-1, 20),
               {"ratio": 2.0, "seeding_time": 30, "seeding_time_limit": 40}, False),
    T("should_not_be_removable_and_should_not_allow_move_files_if_max_inactive_seedingtime_reached_and_not_paused",
      "A torrent inactive past the global inactive limit but still uploading is not removable",
      "qbittorrent.get_items", items(done("uploading", ratio=2.0, last_activity_min_ago=25), limits(-1, max_inactive=20)),
      removable(False)),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_max_inactive_seedingtime_reached_and_paused",
               "A stopped torrent inactive 25 min against a global inactive limit 20 is removable",
               limits(-1, max_inactive=20), {"ratio": 2.0, "last_activity_min_ago": 25}, True),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_overridden_max_inactive_seedingtime_reached_and_paused",
               "A per-torrent inactive limit 10 overrides global 40: inactive 15 min is removable",
               limits(-1, max_inactive=40),
               {"ratio": 2.0, "seeding_time": 20, "inactive_limit": 10, "last_activity_min_ago": 15}, True),
    *seed_rows("should_not_be_removable_if_overridden_max_inactive_seedingtime_not_reached_and_paused",
               "A per-torrent inactive limit 40 overrides global 20: inactive 30 min is not removable",
               limits(-1, max_inactive=20),
               {"ratio": 2.0, "seeding_time": 30, "inactive_limit": 40, "last_activity_min_ago": 30}, False),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_max_seedingtime_reached_but_ratio_not_and_paused",
               "Seeding time reached (30 of 20 min) makes a stopped torrent removable even below the ratio limit",
               limits(2.0, 20), {"ratio": 1.0, "seeding_time": 30}, True),
    *seed_rows("should_be_removable_and_should_allow_move_files_if_max_inactive_seedingtime_reached_but_ratio_not_and_paused",
               "Inactive time reached (25 of 20 min) makes a stopped torrent removable even below the ratio limit",
               limits(2.0, max_inactive=20), {"ratio": 1.0, "last_activity_min_ago": 25}, True),
    *[R("should_not_fetch_details_twice", i, [s],
        f"Seeding time 20 of 30 min is not removable, and torrent properties are fetched once across two polls ({s})",
        "qbittorrent.get_items", {**items(done(s, ratio=2.0, seeding_time=20), limits(-1, 30)), "polls": 2},
        {**removable(False), "torrent_properties_fetches": 1},
        note="Removability is asserted on the first poll; the fetch count after the second.")
      for i, s in enumerate(PAUSED)],
    *[R("should_get_category_from_the_category_if_set", i, [s], f"The item's category comes from the torrent's category ({s})",
        "qbittorrent.get_items",
        items(direct({k: v for k, v in torrent(s, progress=1.0, category="tv-sonarr", ratio=1.0).items()
                      if k != "label"}), limits(1.0)),
        {"item_count": 1, "item": {"category": "tv-sonarr"}}, note=f"Label is not set (null). {DIRECT_NOTE}")
      for i, s in enumerate(PAUSED)],
    *[R("should_get_category_from_the_label_if_the_category_is_not_available", i, [s],
        f"The item's category falls back to the torrent's label when category is unset ({s})",
        "qbittorrent.get_items", items(direct(torrent(s, progress=1.0, label="tv-sonarr", ratio=1.0)), limits(1.0)),
        {"item_count": 1, "item": {"category": "tv-sonarr"}}, note=f"Category is not set (null). {DIRECT_NOTE}")
      for i, s in enumerate(PAUSED)],
    T("should_handle_eta_biginteger", "A torrent JSON with eta 18446744073709335000 (beyond u64) deserialises without loss",
      "qbittorrent.parse_torrent_json", {"json": "{ \"eta\": 18446744073709335000 }"},
      {"eta": "18446744073709335000"}),
    *X("Test_should_force_api_version_check", "The client's connection test forces a fresh Web API version check",
       "Mocks only: verifies the proxy selector is asked with force=true during the settings test; no data outcome."),
]

# --- CompletedDownloadServiceTests/ProcessFixture ---------------------------

CDS_ITEM = {"status": "Completed", "output_path": "/C/DropFolder/MyDownload", "title": "Drone.S01E01.HDTV",
            "download_id": "DownloadId1", "category": "Category1"}
CDS_SERIES = {"Drone.S01E01.HDTV": "series"}


def cds(item=None, history=(), series_by_title=None, **extra):
    return {"tracked_state": "Downloading", "download_item": {**CDS_ITEM, **(item or {})},
            "remote_episode": {"series": "series", "episode_ids": [1]},
            "client": {"id": 1, "name": "testClient"}, "history_by_download_id": list(history),
            "series_by_title": CDS_SERIES if series_by_title is None else series_by_title, **extra}


NOT_PENDING = {"state": {"not": "ImportPending"}}
BADLY_NAMED = {"item": {"download_id": "1234", "title": "Droned Pilot"},
               "history": [{"source_title": "Droned S01E01", "event_type": "Grabbed"}],
               "series_by_title": {"Droned S01E01": "series"}}

CDS_PROCESS = [
    *[R("should_not_process_if_download_status_isnt_completed", i, [f"DownloadItemStatus.{s}"],
        f"A download whose client status is {s} is not queued for import", "completed_download.check",
        cds({"status": s}), NOT_PENDING, NBUILDER)
      for i, s in enumerate(["Downloading", "Failed", "Queued", "Paused", "Warning"])],
    T("should_not_process_if_matching_history_is_not_found_and_no_category_specified",
      "A completed download with no grab history and no category is not imported",
      "completed_download.check", cds({"category": None}), NOT_PENDING),
    T("should_process_if_matching_history_is_not_found_but_category_specified",
      "A completed download with no grab history but a category is queued for import",
      "completed_download.check", cds({"category": "tv"}, series_by_title={"*": "series"}),
      {"state": "ImportPending"}),
    T("should_not_process_if_output_path_is_empty", "A completed download with no output path is not imported",
      "completed_download.check", cds({"output_path": None}), NOT_PENDING, NBUILDER),
    T("should_not_process_if_the_download_cannot_be_tracked_using_the_source_title_as_it_was_initiated_externally",
      "A badly named download is not imported even if its grab history's source title matches a series",
      "completed_download.check",
      cds(BADLY_NAMED["item"], BADLY_NAMED["history"], BADLY_NAMED["series_by_title"],
          import_results=[{"path": "C:\\TestPath\\Droned.S01E01.mkv", "errors": [], "rejected": False}]),
      NOT_PENDING,
      note="History series id is unset (0) and no series lookup by id is arranged; any other title maps to no series."),
    T("should_not_process_when_there_is_a_title_mismatch", "A download whose title maps to no series is not imported",
      "completed_download.check", cds(series_by_title={}), NOT_PENDING, NBUILDER),
]

# --- CompletedDownloadServiceTests/ImportFixture ----------------------------

IMPORTED = {"state": "Imported", "download_completed_events": 1,
            "process_path": {"calls": 1, "path": "/C/DropFolder/MyDownload", "mode": "Auto"}}
NOT_IMPORTED = {"state": "ImportBlocked", "download_completed_events": 0}
RESULT_NOTE = ("Each import result's 'result' follows ImportResult.Result (src/NzbDrone.Core/MediaFiles/EpisodeImport/"
               "ImportResult.cs): errors + approved = Skipped, errors + rejected = Rejected, no errors = Imported.")


def res(path, episodes, errors=(), rejected=False):
    errors = list(errors)
    result = "Imported" if not errors else ("Rejected" if rejected else "Skipped")
    r = {"path": path, "episode_ids": episodes, "rejected": rejected, "errors": errors, "result": result}
    return r


def imp(results, remote_episodes, history_count=0, already_imported=None, item=None, **extra):
    g = {"tracked_state": "Downloading", "download_item": {**CDS_ITEM, **(item or {})},
         "remote_episode": {"series": "series", "episode_ids": remote_episodes},
         "client": {"id": 1, "name": "testClient"}, "series_by_title": CDS_SERIES,
         "history_by_download_id_count": history_count, "import_results": results, **extra}
    if already_imported is not None:
        g["already_imported"] = already_imported
    return g


S01E01, S01E02 = "C:\\TestPath\\Droned.S01E01.mkv", "C:\\TestPath\\Droned.S01E02.mkv"
THREE_NEW = "The three remote episodes are new Episode() objects: ids all 0."

CDS_IMPORT = [
    T("should_not_mark_as_imported_if_all_files_were_rejected", "If every file is rejected the download is import-blocked",
      "completed_download.import",
      imp([res(S01E01, [1], ["Test Failure"], True), res(S01E02, [2], ["Test Failure"], True)], [1]),
      NOT_IMPORTED, RESULT_NOTE),
    T("should_not_mark_as_imported_if_no_episodes_were_parsed",
      "If the download maps to no episodes and every file is rejected it is import-blocked",
      "completed_download.import",
      imp([res(S01E01, [1], ["Test Failure"], True), res(S01E02, [2], ["Test Failure"], True)], []),
      NOT_IMPORTED, RESULT_NOTE),
    T("should_not_mark_as_imported_if_all_files_were_skipped", "If every file is skipped the download is import-blocked",
      "completed_download.import",
      imp([res(S01E01, [1], ["Test Failure"]), res(S01E02, [2], ["Test Failure"])], [1]),
      NOT_IMPORTED, RESULT_NOTE),
    T("should_not_mark_as_imported_if_some_of_episodes_were_not_imported",
      "If only one of three files imports (no history) the download is import-blocked",
      "completed_download.import",
      imp([res(S01E01, []), res(S01E01, [], ["Test Failure"]), res(S01E01, [], ["Test Failure"])], [0, 0, 0]),
      NOT_IMPORTED, f"{THREE_NEW} {RESULT_NOTE}"),
    T("should_not_mark_as_imported_if_some_of_episodes_were_not_imported_including_history",
      "If only one of three files imports the download is import-blocked, even when history says it was imported",
      "completed_download.import",
      imp([res(S01E01, []), res(S01E01, [], ["Test Failure"]), res(S01E01, [], ["Test Failure"])], [0, 0, 0],
          history_count=2, already_imported=True),
      NOT_IMPORTED,
      f"{THREE_NEW} The already-imported check is mocked true only for the exact history list. {RESULT_NOTE}"),
    T("should_mark_as_imported_if_all_episodes_were_imported", "If every episode's file imports the download is Imported",
      "completed_download.import", imp([res(S01E01, [1]), res(S01E02, [2])], [1, 2]), IMPORTED, RESULT_NOTE),
    T("should_mark_as_imported_if_all_episodes_were_imported_including_history",
      "If one file imports and history shows the rest already imported, the download is Imported",
      "completed_download.import",
      imp([res(S01E01, [1]), res(S01E02, [2], ["Test Failure"])], [1, 2], history_count=2, already_imported=True),
      IMPORTED, RESULT_NOTE),
    T("should_mark_as_imported_if_double_episode_file_is_imported",
      "A single two-episode file covering both episodes marks the download Imported",
      "completed_download.import", imp([res("C:\\TestPath\\Droned.S01E01-E02.mkv", [1, 2])], [1, 2]), IMPORTED, RESULT_NOTE),
    T("should_mark_as_imported_if_all_episodes_were_imported_but_extra_files_were_not",
      "An extra file that is skipped does not stop a download whose episodes all imported",
      "completed_download.import",
      imp([res(S01E01, [0]), res(S01E01, [], ["Test Failure"])], [0], series_by_title={"*": "series"}),
      IMPORTED, f"The one remote episode is new Episode() (id 0). {RESULT_NOTE}"),
    T("should_mark_as_imported_if_the_download_can_be_tracked_using_the_source_seriesid",
      "A badly named download imports when the grab history's series id resolves the series",
      "completed_download.import",
      imp([res(S01E01, [1])], [1], item={"download_id": "1234", "title": "Droned Pilot"},
          series_by_title={"Droned S01E01": "series"}, most_recent_history={"source_title": "Droned S01E01"},
          series_by_id={"*": "series"}),
      IMPORTED, f"Titles other than 'Droned S01E01' map to no series. {RESULT_NOTE}"),
]

# --- FailedDownloadServiceTests ---------------------------------------------


def fds(state, item=None, grab_history_count=2):
    return {"tracked_state": state,
            "download_item": {"status": "Completed", "output_path": "/C/DropFolder/MyDownload",
                              "title": "Drone.S01E01.HDTV", "download_id": "DownloadId1", **(item or {})},
            "remote_episode": {"episode_ids": [1]}, "grabbed_history_count": grab_history_count}


FAILED = {"state": "Failed", "download_failed_events": 1}

FDS_PROCESS_FAILED = [
    *X("should_mark_failed_if_encrypted", "A pending-failed download that is encrypted is marked Failed",
       "Encrypted downloads are reported only by usenet/aria2 clients (IsEncrypted is set in Sabnzbd, NzbVortex, "
       "Aria2); our qBittorrent client never sets it."),
    T("should_mark_failed_if_download_item_is_failed", "A pending-failed download whose client item failed is marked Failed",
      "failed_download.process_failed", fds("FailedPending", {"status": "Failed"}), FAILED, NBUILDER),
    T("should_include_tracked_download_in_message", "The download-failed event carries the tracked download",
      "failed_download.process_failed", fds("FailedPending", {"status": "Failed"}),
      {**FAILED, "event_has_tracked_download": True}, NBUILDER),
]

FDS_PROCESS = [
    T("should_not_fail_if_matching_history_is_not_found", "A completed download with no grab history is not failed",
      "failed_download.check", fds("Downloading", grab_history_count=0),
      {"state": {"not": "Failed"}, "download_failed_events": 0}),
    T("should_warn_if_matching_history_is_not_found", "A failed download with no grab history gets a warning message",
      "failed_download.check", fds("Downloading", {"status": "Failed"}, 0), {"status_messages": {"not_empty": True}}),
    T("should_not_warn_if_matching_history_is_not_found_and_not_failed",
      "A failed download with no grab history gets a warning message (test name says otherwise)",
      "failed_download.check", fds("Downloading", {"status": "Failed"}, 0), {"status_messages": {"not_empty": True}},
      note="The test's name contradicts its body: it sets status Failed and asserts messages are not empty."),
]

# --- TrackedDownloads -------------------------------------------------------


def eps_hist(n, history):
    """GivenEpisodes(n) gives NBuilder ids 1..n; history: [(episode index, [event types])]."""
    return {"episode_ids": list(range(1, n + 1)),
            "history": [{"episode_id": i + 1, "event_type": e} for i, evs in history for e in evs]}


ALREADY = [
    T("should_return_false_if_there_is_no_history", "With no history a download is not already imported",
      "tracked_download.is_imported", eps_hist(1, []), {"imported": False}),
    T("should_return_false_if_single_episode_download_is_not_imported",
      "A single-episode download with only a Grabbed event is not imported",
      "tracked_download.is_imported", eps_hist(1, [(0, ["Grabbed"])]), {"imported": False}),
    T("should_return_false_if_no_episode_in_multi_episode_download_is_imported",
      "A two-episode download with only Grabbed events is not imported",
      "tracked_download.is_imported", eps_hist(2, [(0, ["Grabbed"]), (1, ["Grabbed"])]), {"imported": False}),
    T("should_should_return_false_if_only_one_episode_in_multi_episode_download_is_imported",
      "A two-episode download with only one episode imported is not imported",
      "tracked_download.is_imported", eps_hist(2, [(0, ["DownloadFolderImported", "Grabbed"]), (1, ["Grabbed"])]),
      {"imported": False}),
    T("should_return_true_if_single_episode_download_is_imported",
      "A single-episode download with a DownloadFolderImported event is imported",
      "tracked_download.is_imported", eps_hist(1, [(0, ["DownloadFolderImported", "Grabbed"])]), {"imported": True}),
    T("should_return_true_if_multi_episode_download_is_imported",
      "A two-episode download with every episode imported is imported",
      "tracked_download.is_imported",
      eps_hist(2, [(0, ["DownloadFolderImported", "Grabbed"]), (1, ["DownloadFolderImported", "Grabbed"])]),
      {"imported": True}),
]

CLIENT = {"id": 1, "protocol": "Torrent"}
HIST = {"download_id": "35238", "source_title": "TV Series S01", "series_id": 5, "episode_id": 4}
BLACKHOLE_ITEM = {"title": "TV Series - S01E01", "download_id": "12345",
                  "client_info": {"id": 1, "type": "Blackhole", "name": "Blackhole Client", "protocol": "Torrent"}}
REMOTE = {"series_id": 5, "episode_ids": [4], "parsed": {"series_title": "TV Series", "season": 1}, "mapped_season": 1}
REMOTE_E1 = {"series_id": 5, "episode_ids": [4], "parsed": {"series_title": "TV Series", "season": 1, "episodes": [1]},
             "mapped_season": 0}
UNMAPPED = {"tracked_count_before": 1, "tracked_count": 1, "first": {"remote_episode": None}}

TRACKED = [
    T("should_track_downloads_using_the_source_title_if_it_cannot_be_found_using_the_download_title",
      "A download whose title does not parse is mapped via its grab history's source title",
      "tracked_download.track",
      {"client": CLIENT, "item": {"title": "The torrent release folder", "download_id": "35238"},
       "history_by_download_id": {"35238": [HIST]},
       "map": [{"when": {"season": 1, "series_title": "TV Series"}, "returns": REMOTE}]},
      {"remote_episode": {"series_id": 5, "first_episode_id": 4, "parsed_season": 1, "mapped_season": 1}}),
    T("should_set_indexer", "A tracked download takes its release's indexer name from the grab history",
      "tracked_download.track",
      {"client": CLIENT, "item": {"title": "TV.Series.S01.MULTi.1080p.WEB.H265-RlsGroup", "download_id": "35238"},
       "history_by_download_id": {"35238": [{**HIST, "event_type": "Grabbed", "data": {"indexer": "MyIndexer (Prowlarr)"}}]},
       "indexers": [{"id": 1, "name": "MyIndexer (Prowlarr)", "multi_languages": ["Original", "French"]}],
       "map": [{"when": "any", "returns": REMOTE}]},
      {"remote_episode": {"release": {"indexer": "MyIndexer (Prowlarr)"}}}),
    T("should_parse_as_special_when_source_title_parsing_fails",
      "When the grab source title only parses as a special, the download maps to that special",
      "tracked_download.track",
      {"client": CLIENT, "item": {"title": "The torrent release folder", "download_id": "35238"},
       "history_by_download_id": {"35238": [{**HIST, "source_title": "TV Series Special"}]},
       "special_episode_title_parse": {"series_title": "TV Series", "season": 0, "episodes": [1]},
       "map": [{"when": {"season": 0, "series_title": "TV Series"},
                "returns": {"series_id": 5, "episode_ids": [4],
                            "parsed": {"series_title": "TV Series", "season": 0, "episodes": [1]}, "mapped_season": 0}}]},
      {"remote_episode": {"series_id": 5, "first_episode_id": 4, "parsed_season": 0, "mapped_season": 0}}),
    T("should_unmap_tracked_download_if_episode_deleted",
      "When a tracked download's episode is deleted on refresh, it stays tracked but loses its mapping",
      "tracked_download.track_then_event",
      {"client": CLIENT, "item": BLACKHOLE_ITEM, "history_by_download_id": {"*": []},
       "map": [{"when": "any", "returns": REMOTE_E1}],
       "then": {"remap_returns": None,
                "event": {"type": "EpisodeInfoRefreshed", "series_id": 5, "added": [], "updated": [], "removed": [4]}}},
      UNMAPPED),
    T("should_not_throw_when_processing_deleted_episodes",
      "An unmapped tracked download survives an episode-deleted refresh without error",
      "tracked_download.track_then_event",
      {"client": CLIENT, "item": BLACKHOLE_ITEM, "history_by_download_id": {"*": []},
       "map": [{"when": "any", "returns": None}],
       "then": {"remap_returns": None,
                "event": {"type": "EpisodeInfoRefreshed", "series_id": 5, "added": [], "updated": [], "removed": [4]}}},
      UNMAPPED),
    T("should_not_throw_when_processing_deleted_series",
      "An unmapped tracked download survives its series being deleted without error",
      "tracked_download.track_then_event",
      {"client": CLIENT, "item": BLACKHOLE_ITEM, "history_by_download_id": {"*": []},
       "map": [{"when": "any", "returns": None}],
       "then": {"remap_returns": None,
                "event": {"type": "SeriesDeleted", "series_ids": [5], "delete_files": True,
                          "add_import_list_exclusion": True}}},
      UNMAPPED),
]

# --- DownloadServiceFixture -------------------------------------------------

USENET_NOTE = "The reference uses a usenet client; the grab pipeline under test does not depend on protocol."


def dl(clients=("Usenet",), protocol="Usenet", client_throws=None, **extra):
    return {"clients": list(clients),
            "remote_episode": {"series": "NBuilder", "episode_ids": [12, 99], "series_id": 5,
                               "release": {"protocol": protocol, "download_url": "http://test.site/download1.ext"}},
            "client_throws": client_throws, **extra}


DOWNLOAD = [
    T("Download_report_should_publish_on_grab_event", "A successful grab publishes an episode-grabbed event",
      "download.download_report", dl(), {"grabbed_events": 1}, USENET_NOTE),
    T("Download_report_should_grab_using_client", "A grab is handed to the matching download client once",
      "download.download_report", dl(), {"client_download_calls": {"Usenet": 1}}, USENET_NOTE),
    T("Download_report_should_not_publish_on_failed_grab_event",
      "A grab whose client call fails raises and publishes no grabbed event",
      "download.download_report", dl(client_throws={"type": "WebException"}),
      {"error": "WebException", "grabbed_events": 0}, USENET_NOTE),
    T("Download_report_should_trigger_indexer_backoff_on_indexer_error",
      "A failure fetching the release from the indexer records an indexer failure",
      "download.download_report", dl(client_throws={"type": "ReleaseDownloadException", "inner": "WebException"}),
      {"error": "ReleaseDownloadException", "indexer_failures": 1}, USENET_NOTE),
    T("Download_report_should_trigger_indexer_backoff_on_http429_with_long_time",
      "An indexer HTTP 429 with Retry-After '300' backs the indexer off 5 minutes",
      "download.download_report",
      dl(client_throws={"type": "ReleaseDownloadException",
                        "inner": {"type": "TooManyRequestsException", "status": 429, "retry_after": "300"}}),
      {"error": "ReleaseDownloadException", "indexer_failures": 1, "indexer_backoff_min": 5.0}, USENET_NOTE),
    T("Download_report_should_trigger_indexer_backoff_on_http429_based_on_date",
      "An indexer HTTP 429 with Retry-After as a date 300 s ahead backs the indexer off about 5 minutes",
      "download.download_report",
      dl(client_throws={"type": "ReleaseDownloadException",
                        "inner": {"type": "TooManyRequestsException", "status": 429,
                                  "retry_after_date_offset_s": 300, "retry_after_format": "RFC1123 ('r')"}}),
      {"error": "ReleaseDownloadException", "indexer_failures": 1, "indexer_backoff_min": {"between": [4.9, 5.1]}},
      USENET_NOTE),
    T("Download_report_should_not_trigger_indexer_backoff_on_downloadclient_error",
      "A download-client error does not back off the indexer",
      "download.download_report", dl(client_throws={"type": "DownloadClientException"}),
      {"error": "DownloadClientException", "indexer_failures": 0}, USENET_NOTE),
    T("Download_report_should_not_trigger_indexer_backoff_on_indexer_404_error",
      "A release gone from the indexer (unavailable) does not back off the indexer",
      "download.download_report", dl(client_throws={"type": "ReleaseUnavailableException", "inner": "WebException"}),
      {"error": "ReleaseUnavailableException", "indexer_failures": 0}, USENET_NOTE),
    T("should_not_attempt_download_if_client_isnt_configured",
      "With no download client for the protocol the grab fails and nothing is published",
      "download.download_report", dl(clients=()),
      {"error": "DownloadClientUnavailableException", "client_download_calls": {}, "grabbed_events": 0}),
    T("should_attempt_download_even_if_client_is_disabled",
      "A grab still goes to a client that is temporarily disabled by backoff",
      "download.download_report", dl(blocked_clients=[{"client": "Usenet", "disabled_till_offset_h": 3}]),
      {"client_download_calls": {"Usenet": 1}, "grabbed_events": 1, "blocked_provider_lookups": 0}, USENET_NOTE),
    *X("should_send_download_to_correct_usenet_client", "A usenet release goes to the usenet client, not the torrent one",
       "We run no usenet client or indexer (downloadclient.json, indexer.json)."),
    T("should_send_download_to_correct_torrent_client", "A torrent release goes to the torrent client, not the usenet one",
      "download.download_report", dl(clients=("Torrent", "Usenet"), protocol="Torrent"),
      {"client_download_calls": {"Torrent": 1, "Usenet": 0}}),
]

# --- SeedConfigProviderFixture ----------------------------------------------

SEED = [
    T("should_not_return_config_for_non_existent_indexer", "A torrent release from an unknown indexer (id 0) gets no seed config",
      "seed_config.get", {"release": {"protocol": "Torrent", "indexer_id": 0}, "indexer_settings": None},
      {"seed_config": None}),
    T("should_not_return_config_for_invalid_indexer", "A season pack from an indexer with no cached settings gets no seed config",
      "seed_config.get",
      {"release": {"protocol": "Torrent", "indexer_id": 1}, "parsed": {"full_season": True}, "indexer_settings": None},
      {"seed_config": None}),
    T("should_return_season_time_for_season_packs", "A season pack gets the indexer's season-pack seed time (10 min)",
      "seed_config.get",
      {"release": {"protocol": "Torrent", "indexer_id": 1}, "parsed": {"full_season": True},
       "indexer_settings": {"fail_downloads": ["Executables"], "seed_criteria": {"season_pack_seed_time": 10}}},
      {"seed_config": {"not": None}, "seed_time_min": 10},
      note="Other seed criteria are TorznabSettings defaults (unset)."),
]

FIXTURES = [
    ("Download/DownloadClientTests/QBittorrentTests/QBittorrentFixture", "QBittorrentFixture", QBT),
    ("Download/CompletedDownloadServiceTests/ProcessFixture", "CompletedDownloadService.ProcessFixture", CDS_PROCESS),
    ("Download/CompletedDownloadServiceTests/ImportFixture", "CompletedDownloadService.ImportFixture", CDS_IMPORT),
    ("Download/FailedDownloadServiceTests/ProcessFailedFixture", "FailedDownloadService.ProcessFailedFixture",
     FDS_PROCESS_FAILED),
    ("Download/FailedDownloadServiceTests/ProcessFixture", "FailedDownloadService.ProcessFixture", FDS_PROCESS),
    ("Download/TrackedDownloads/TrackedDownloadAlreadyImportedFixture", "TrackedDownloadAlreadyImportedFixture", ALREADY),
    ("Download/TrackedDownloads/TrackedDownloadServiceFixture", "TrackedDownloadServiceFixture", TRACKED),
    ("Download/DownloadServiceFixture", "DownloadServiceFixture", DOWNLOAD),
    ("IndexerTests/SeedConfigProviderFixture", "SeedConfigProviderFixture", SEED),
]

# Whole fixtures ruled out, with the config fact that rules them out.
RULED_OUT = {
    "Download/DownloadClientTests/{Blackhole,DelugeTests,DownloadStationTests,FreeboxDownloadTests,HadoukenTests,"
    "NzbVortexTests,NzbgetTests,PneumaticProviderFixture,RTorrentTests,SabnzbdTests,TransmissionTests,UTorrentTests,"
    "VuzeTests}": "Our only download client is qBittorrent (downloadclient.json#id=1).",
    "Download/NzbValidationServiceFixture.cs": "Usenet only; no usenet indexers or clients (indexer.json, downloadclient.json).",
    "Download/DownloadClientProviderFixture.cs": "Round-robin, tags and priority across several clients; we have one "
    "client with no tags (downloadclient.json).",
    "Download/DownloadClientStatusServiceFixture.cs": "Client backoff escalation timing; not driven by any setting we use.",
    "Download/DownloadApprovedReportsTests/DownloadApprovedFixture.cs": "Grab selection from approved decisions; belongs "
    "to the decision area, not client handling.",
    "Download/Pending/PendingReleaseServiceTests/*": "Delay-profile pending releases; decision area (delayprofile.json).",
    "Download/Aggregation/Aggregators/AggregateLanguagesFixture.cs": "Release language aggregation; parse area.",
    "Download/DownloadHistoryTests/DownloadHistoryRepositoryFixture.cs": "Database repository delete; storage internals.",
}


# --- reference anchoring ----------------------------------------------------

def cs_value(node):
    """Evaluates a [TestCase] argument: literals, true/false/null, unary minus, Enum.Member."""
    if isinstance(node, ast.Constant):
        return node.value
    if isinstance(node, ast.Name) and node.id in ("true", "false", "null"):
        return {"true": True, "false": False, "null": None}[node.id]
    if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.USub):
        return -cs_value(node.operand)
    if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name):
        return f"{node.value.id}.{node.attr}"
    raise ValueError(ast.dump(node))


DECL = re.compile(r"\bpublic (?:async )?(?:void|Task) (\w+)\(")


def attributes(lines, method):
    """Returns [(line_no, text)] of the attribute lines directly above the method declaration."""
    hits = [i for i, l in enumerate(lines) if (m := DECL.search(l)) and m.group(1) == method]
    assert len(hits) == 1, f"{method}: {len(hits)} declarations"
    i, attrs = hits[0] - 1, []
    while i >= 0 and lines[i].strip().startswith("["):
        attrs.append((i + 1, lines[i].strip()))
        i -= 1
    return attrs[::-1]


def test_methods(lines):
    out = set()
    for l in lines:
        m = DECL.search(l)
        if m and any(t == "[Test]" or t.startswith("[TestCase(") for _, t in attributes(lines, m.group(1))):
            out.add(m.group(1))
    return out


def anchor(path, lines, case):
    attrs = attributes(lines, case["method"])
    if case["kind"] == "test":
        tests = [n for n, t in attrs if t == "[Test]"]
        assert len(tests) == 1, f"{path}:{case['method']}: no [Test]"
        return tests[0]
    rows = [(n, t) for n, t in attrs if t.startswith("[TestCase(")]
    n, text = rows[case["row"]]
    args = ast.parse("f" + text[len("[TestCase"):-1], mode="eval").body.args
    actual = [cs_value(a) for a in args]
    assert actual == case["params"], f"{path}:{n}: row {actual} != {case['params']}"
    return n


def case_key(rel, case):
    """Stable ID key: fixture path (no tag), method, row index and row arguments; survives line shifts and tag bumps."""
    row = "-" if case["kind"] == "test" else f"{case['row']}|{json.dumps(case['params'], ensure_ascii=False)}"
    return f"{rel}|{case['method']}|{row}"


def existing_ids():
    """({key: id}, {ref: id}) from the catalogue; the ref map only adopts entries written before keys existed."""
    if not CATALOGUE.exists():
        return {}, {}
    text = CATALOGUE.read_text()
    by_key = {k: i for k, i in re.findall(r"^# key: (.*)\n- id: (\S+)$", text, re.M)}
    by_ref = {r: i for i, r in re.findall(r'- id: (\S+)\n(?:  .*\n)*?  ref: "(.*)"', text)}
    return by_key, ({} if by_key else by_ref)


def yaml_str(s):
    return json.dumps(s, ensure_ascii=False)


def kept_lines():
    """{id: [status/decision/tests lines as written]} from the current catalogue (ratchet-owned, never regenerated)."""
    if not CATALOGUE.exists():
        return {}
    return {m[0]: re.findall(r"(?m)^  ((?:status|decision|tests):.*(?:\n    .*)*)", m[1])
            for m in re.findall(r"(?m)^- id: (\S+)\n((?:  .*\n?|    .*\n?)*)", CATALOGUE.read_text())}


def main():
    by_key, by_ref = existing_ids()
    kept = kept_lines()
    used = {int(i.rsplit("-", 1)[1]) for i in [*by_key.values(), *by_ref.values()]}
    next_n = max(used, default=0) + 1
    assigned = set()
    entries, skipped = [], []
    for stem, out_name, cases in FIXTURES:
        rel = f"{TEST_DIR}{stem}.cs"
        lines = (REF_REPO / rel).read_text(encoding="utf-8-sig").splitlines()
        declared, covered = test_methods(lines), {c["method"] for c in cases}
        assert declared == covered, f"{stem}: uncovered {sorted(declared - covered)}, unknown {sorted(covered - declared)}"
        for m in covered:
            rows = [t for _, t in attributes(lines, m) if t.startswith("[TestCase(")]
            mine = [c for c in cases if c["method"] == m and c["kind"] == "row"]
            assert len(rows) == len(mine), f"{stem}.{m}: {len(rows)} rows, {len(mine)} ported"
        corpus, seen = [], set()
        for line, case in sorted(((anchor(rel, lines, c), c) for c in cases), key=lambda x: x[0]):
            ref = f"{TAG} {rel}:{line}"
            assert ref not in seen, f"duplicate ref {ref}"
            seen.add(ref)
            key = case_key(rel, case)
            cid = by_key.get(key) or by_ref.get(ref)
            if cid is None:
                assert next_n <= ID_MAX, "ID range exhausted"
                cid = f"{ID_PREFIX}{next_n:04d}"
                next_n += 1
            assert cid not in assigned, f"{cid} assigned twice ({key})"
            assigned.add(cid)
            in_scope = case["given"] is not None
            entries.append((cid, key, case["behaviour"], ref, in_scope))
            if not in_scope:
                skipped.append(f"{cid} {stem.rsplit('/', 1)[1]}.{case['method']}: {case['why']}")
                continue
            item = {"id": cid, "ref": ref, "input": {"op": case["op"], "given": case["given"]}, "expect": case["expect"]}
            if case.get("note"):
                item["note"] = case["note"]
            corpus.append(item)
        CORPUS.mkdir(parents=True, exist_ok=True)
        (CORPUS / f"{out_name}.json").write_text(json.dumps(corpus, indent=1, ensure_ascii=False) + "\n")

    out = ["# Sonarr download-client (qBittorrent) and download-handling fixtures, ported by",
           "# parity/extract/sonarr_torrent.py. Corpus: corpus/reference/sonarr/torrent/.", ""]
    for cid, key, behaviour, ref, in_scope in entries:
        out += [f"# key: {key}", f"- id: {cid}", f"  behaviour: {yaml_str(behaviour)}", "  source: reference",
                f"  ref: {yaml_str(ref)}", f"  in_scope: {'true' if in_scope else 'false'}",
                *[f"  {k}" for k in kept.get(cid, ["status: unmapped", "tests: []"])]]
    CATALOGUE.parent.mkdir(parents=True, exist_ok=True)
    CATALOGUE.write_text("\n".join(out) + "\n")
    print(f"entries {len(entries)}, corpus cases {len(entries) - len(skipped)}, in_scope false {len(skipped)}, "
          f"ruled-out fixtures {len(RULED_OUT)}")
    print(f"id range {min(assigned)}..{max(assigned)}")
    for s in skipped:
        print("skipped:", s)
    for what, why in RULED_OUT.items():
        print(f"ruled out: {what}: {why}")


if __name__ == "__main__":
    sys.exit(main())
