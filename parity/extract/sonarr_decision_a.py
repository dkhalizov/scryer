"""Port Sonarr grab-decision fixtures A..M into the parity corpus and catalogue.

Hand-translated: the C# arrange/assert of each test is written below as data.
The script finds each test's line in the reference repo (so refs are exact),
checks every [TestCase] row against the parameters written here, keeps the
IDs already in the catalogue (key: ref path | method | row | params, stored as
a `# key:` comment above each entry) and writes:

  parity/corpus/reference/sonarr/decision/<Fixture>.json
  parity/catalogue/decision.sonarr-a.yaml

Units: *_mb is Sonarr's Megabytes() (MiB); quality definition min/max sizes
are MB per minute of runtime; offsets are hours relative to "now" unless named
otherwise. NBuilder defaults are written out where a test relies on them.

Run: python3 -I parity/extract/sonarr_decision_a.py
"""

import ast
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REF_REPO = Path.home() / "PycharmProjects/arr-reference/sonarr@v4.0.20.3014"
TAG = "Sonarr@v4.0.20.3014"
TEST_DIR = "src/NzbDrone.Core.Test/DecisionEngineTests/"
CATALOGUE = ROOT / "parity/catalogue/decision.sonarr-a.yaml"
CORPUS = ROOT / "parity/corpus/reference/sonarr/decision"
ID_PREFIX, ID_MAX = "SON-DECISION-", 999


def T(method, behaviour, given, expect, note=None):
    return dict(kind="test", method=method, behaviour=behaviour, given=given, expect=expect, note=note)


def R(method, row, params, behaviour, given, expect, note=None):
    return dict(kind="row", method=method, row=row, params=params, behaviour=behaviour,
                given=given, expect=expect, note=note)


def X(method, behaviour, why):
    return dict(kind="test", method=method, behaviour=behaviour, given=None, expect=None, why=why)


# --- AcceptableSizeSpecificationFixture -------------------------------------

SDTV_DEF = {"quality": "SDTV", "min_size": 2, "max_size": 10}
SEASON = {"season": 1, "count": 10, "runtime": 30, "ids": "1..10 (NBuilder)"}


def size_given(series_runtime, episodes, size, quality=None, definition=None, **extra):
    g = {
        "series": {"runtime": series_runtime},
        "quality_definition": definition or SDTV_DEF,
        "release": {"quality": quality or "SDTV", "revision": 2 if quality is None else 1, **size},
        "episodes": episodes,
        "season_episodes": SEASON,
    }
    for k, v in extra.items():
        g[k].update(v)
    return g


def ep(season=1, number=1, runtime=30, **kw):
    return {"season": season, "number": number, "runtime": runtime, **kw}


def eps(n, runtime, **kw):
    return [ep(number=i + 1, runtime=runtime, **kw) for i in range(n)]


def size_rows(method, n, rows, first_line_behaviour):
    out = []
    for i, (runtime, mb, ok) in enumerate(rows):
        if n == 1:
            episodes = [ep(runtime=runtime, position_in_season="middle (id 5)")]
        else:
            episodes = eps(n, runtime)
        out.append(R(method, i, [runtime, mb, ok],
                     f"{first_line_behaviour} of {mb} MB at {runtime} min/episode against SDTV 2-10 MB/min is "
                     f"{'accepted' if ok else 'rejected'}",
                     size_given(runtime, episodes, {"size_mb": mb}), {"accepted": ok}))
    return out


def season_edge_rows(method, position):
    out = []
    for i, (runtime, mb, ok) in enumerate([(30, 250, True), (30, 1000, False), (60, 250, True), (60, 2000, False)]):
        out.append(R(method, i, [runtime, mb, ok],
                     f"A single episode that is the {position} of its season (episode runtime 30, series runtime "
                     f"{runtime}) at {mb} MB against SDTV 2-10 MB/min is {'accepted' if ok else 'rejected'}",
                     size_given(runtime, [ep(runtime=30, position_in_season=position)], {"size_mb": mb}),
                     {"accepted": ok},
                     note="The test sets the episode runtime to the season's first episode runtime (30), not the row's runtime."))
    return out


ACCEPTABLE_SIZE = (
    size_rows("single_episode", 1, [(30, 50, False), (30, 250, True), (30, 500, False),
                                    (60, 100, False), (60, 500, True), (60, 1000, False)],
              "A single-episode release")
    + season_edge_rows("should_return_expected_result_for_first_episode_of_season", "first")
    + season_edge_rows("should_return_expected_result_for_last_episode_of_season", "last")
    + size_rows("multi_episode", 2, [(30, 100, False), (30, 500, True), (30, 1000, False),
                                     (60, 200, False), (60, 1000, True), (60, 2000, False)],
                "A two-episode release")
    + size_rows("multiset_episode", 6, [(30, 300, False), (30, 1500, True), (30, 3000, False),
                                        (60, 600, False), (60, 3000, True), (60, 6000, False)],
                "A six-episode release")
    + [
        T("should_return_true_if_size_is_zero", "A release with unknown size (0) skips the size check and is accepted",
          size_given(30, [ep()], {"size_bytes": 0}, definition={"quality": "SDTV", "min_size": 10, "max_size": 20}),
          {"accepted": True}),
        T("should_return_true_if_unlimited_30_minute", "A huge release is accepted when the quality has no maximum size (30 min)",
          size_given(30, [ep()], {"size_bytes": 18457280000}, definition={"quality": "SDTV", "min_size": 2, "max_size": None}),
          {"accepted": True}),
        T("should_return_true_if_unlimited_60_minute", "A huge release is accepted when the quality has no maximum size (60 min)",
          size_given(60, [ep(runtime=60)], {"size_bytes": 36857280000}, definition={"quality": "SDTV", "min_size": 2, "max_size": None}),
          {"accepted": True}),
        T("should_treat_daily_series_as_single_episode", "A daily-series release is sized as a single episode and accepted at 300 MB for 60 min",
          size_given(60, [ep(runtime=60)], {"size_mb": 300}, series={"series_type": "Daily"}),
          {"accepted": True}),
        T("should_return_true_if_RAWHD", "A Raw-HD release (no maximum size by default) of 8000 MB is accepted",
          size_given(45, [ep(runtime=30)], {"size_mb": 8000}, quality="Raw-HD",
                     definition={"quality": "Raw-HD", "min_size": 4, "max_size": None},
                     series={"series_type": "Daily"}),
          {"accepted": True},
          note="Raw-HD definition is the reference default (src/NzbDrone.Core/Qualities/Quality.cs:167); revision is the QualityModel default."),
        T("should_return_true_for_special", "A special skips the size check and is accepted",
          {"series": {}, "quality_definition": SDTV_DEF,
           "release": {"quality": "SDTV", "revision": 2, "special": True, "size_bytes": 0},
           "episodes": [ep()], "season_episodes": SEASON},
          {"accepted": True},
          note="The test does not set the release size; ReleaseInfo defaults it to 0, and series runtime is the NBuilder default."),
        T("should_return_false_if_series_runtime_is_zero_and_single_episode_is_not_from_first_season",
          "With unknown runtime, a single episode outside the first season is rejected",
          size_given(0, [ep(season=2, runtime=0, position_in_season="middle (id 5)")], {"size_mb": 200}),
          {"accepted": False}),
        T("should_return_false_if_series_runtime_is_zero_and_single_episode_aired_more_than_24_hours_after_first_aired_episode",
          "With unknown runtime, a first-season episode that aired 7 days after the season's first episode is rejected",
          size_given(0, [ep(number=2, runtime=0, position_in_season="middle (id 5)",
                            air_date_offset_hours_from_season_first_episode=168)], {"size_mb": 200}),
          {"accepted": False}),
        T("should_return_true_if_series_runtime_is_zero_and_single_episode_aired_less_than_24_hours_after_first_aired_episode",
          "With unknown runtime, a first-season episode that aired 1 hour after the season's first episode is accepted at 200 MB",
          size_given(0, [ep(number=2, runtime=0, position_in_season="middle (id 5)",
                            air_date_offset_hours_from_season_first_episode=1)], {"size_mb": 200}),
          {"accepted": True}),
        T("should_return_false_if_series_runtime_is_zero_and_multi_episode_is_not_from_first_season",
          "With unknown runtime, a two-episode release outside the first season is rejected",
          size_given(0, eps(2, 0, season=2), {"size_mb": 200}),
          {"accepted": False}),
        T("should_return_false_if_series_runtime_is_zero_and_multi_episode_aired_more_than_24_hours_after_first_aired_episode",
          "With unknown runtime, a two-episode release that aired 7 days after the season's first episode is rejected",
          size_given(0, eps(2, 0, air_date_offset_hours_from_season_first_episode=168), {"size_mb": 200}),
          {"accepted": False}),
        T("should_return_true_if_series_runtime_is_zero_and_multi_episode_aired_less_than_24_hours_after_first_aired_episode",
          "With unknown runtime, a two-episode release that aired 1 hour after the season's first episode is accepted at 200 MB",
          size_given(0, eps(2, 0, air_date_offset_hours_from_season_first_episode=1), {"size_mb": 200}),
          {"accepted": True}),
        T("should_use_series_runtime_if_episode_runtime_is_not_set",
          "An episode with runtime 0 falls back to the series runtime (30) and 200 MB is accepted",
          size_given(30, [ep(runtime=0, air_date_offset_hours_from_season_first_episode=1)], {"size_mb": 200}),
          {"accepted": True}),
    ]
)

# --- AlreadyImportedSpecificationFixture ------------------------------------

TITLE = "Series.Title.S01E01.720p.HDTV.x264-Sonarr"


def imported_given(history=(), cdh=True, file_id=1, release=None):
    return {
        "config": {"enable_completed_download_handling": cdh},
        "episodes": [{"id": 1, "season": 12, "number": 3, "episode_file_id": file_id}],
        "release": release or {"quality": "HDTV-720p", "revision": 1},
        "history": [{"download_id": d, "source_title": TITLE, "quality": q, "revision": 1, "event": e}
                    for d, q, e in history],
    }


def torrent(info_hash):
    return {"quality": "HDTV-720p", "revision": 1, "protocol": "torrent", "info_hash": info_hash, "title": "Title1"}


GUID = "<GUID-A>"
GUID_NOTE = ("<GUID-A> stands for Guid.NewGuid().ToString().ToUpper(): an upper-case GUID; "
             "the spec compares InfoHash.ToUpper() with the history download id.")
GRAB_IMPORT_1080 = [(GUID, "HDTV-720p", "Grabbed"), (GUID, "HDTV-1080p", "DownloadFolderImported")]
NBUILDER_TITLE = "Release title is NBuilder's default (Title1)."

ALREADY_IMPORTED = [
    T("should_be_accepted_if_CDH_is_disabled", "With completed download handling off, the check accepts",
      imported_given(cdh=False), {"accepted": True}),
    T("should_be_accepted_if_episode_does_not_have_a_file", "An episode without a file is accepted",
      imported_given(file_id=0), {"accepted": True}),
    T("should_be_accepted_if_episode_does_not_have_grabbed_event", "An episode with no history is accepted",
      imported_given(), {"accepted": True}),
    T("should_be_accepted_if_episode_does_not_have_imported_event", "A grab without an import in history is accepted",
      imported_given([(GUID, "HDTV-720p", "Grabbed")]), {"accepted": True}),
    T("should_be_accepted_if_grabbed_and_imported_quality_is_the_same",
      "A grab imported at the same quality is accepted",
      imported_given([(GUID, "HDTV-720p", "Grabbed"), (GUID, "HDTV-720p", "DownloadFolderImported")]),
      {"accepted": True}),
    T("should_be_accepted_if_grabbed_download_id_and_release_torrent_hash_is_unknown",
      "A torrent release without an info hash is accepted even if a grab was imported at a different quality",
      imported_given(GRAB_IMPORT_1080, release=torrent(None)), {"accepted": True}, note=NBUILDER_TITLE),
    T("should_be_accepted_if_grabbed_download_does_not_have_an_id",
      "A grab without a download id does not match a torrent release by hash",
      imported_given([(None, "HDTV-720p", "Grabbed"), (GUID, "HDTV-1080p", "DownloadFolderImported")],
                     release=torrent(GUID)),
      {"accepted": True}, note=NBUILDER_TITLE),
    T("should_be_rejected_if_grabbed_download_id_matches_release_torrent_hash",
      "A torrent release whose info hash matches a grab that was imported at a different quality is rejected",
      imported_given(GRAB_IMPORT_1080, release=torrent(GUID)), {"accepted": False}, note=NBUILDER_TITLE),
    T("should_be_rejected_if_release_title_matches_grabbed_event_source_title",
      "A release matching an imported grab (by the arranged info hash) is rejected",
      imported_given(GRAB_IMPORT_1080, release=torrent(GUID)), {"accepted": False},
      note="Arrange is identical to the info-hash test above; the release title is NBuilder's default (Title1), "
           "not the history source title, so the title match the name describes is not what this test sets up."),
]
for _c in ALREADY_IMPORTED:
    if GUID in json.dumps(_c["given"]):
        _c["note"] = " ".join(x for x in (_c["note"], GUID_NOTE) if x)

# --- AnimeVersionUpgradeSpecificationFixture --------------------------------


def anime_given(series_type="Anime", file_group="DRONE2", release_group="DRONE", release_revision=2,
                file_id=1, propers="PreferAndUpgrade"):
    return {
        "config": {"download_propers_and_repacks": propers},
        "series": {"series_type": series_type},
        "existing_file": {"quality": "HDTV-720p", "revision": 1, "release_group": file_group},
        "episodes": [{"episode_file_id": file_id}],
        "release": {"quality": "HDTV-720p", "revision": release_revision, "release_group": release_group},
    }


ANIME = [
    T("should_be_true_when_no_existing_file", "An anime version upgrade with no existing file is accepted",
      anime_given(file_id=0), {"accepted": True}),
    T("should_be_true_if_series_is_not_anime", "The release-group rule applies only to anime series",
      anime_given(series_type="Standard"), {"accepted": True}),
    T("should_be_true_if_is_not_a_version_upgrade_for_existing_file",
      "A release that is not a version upgrade is accepted",
      anime_given(release_revision=1), {"accepted": True}),
    T("should_be_true_when_release_group_matches", "An anime version upgrade from the same release group is accepted",
      anime_given(file_group="DRONE"), {"accepted": True}),
    T("should_be_false_when_existing_file_doesnt_have_a_release_group",
      "An anime version upgrade is rejected when the existing file has no release group",
      anime_given(file_group=""), {"accepted": False}),
    T("should_should_be_false_when_release_doesnt_have_a_release_group",
      "An anime version upgrade is rejected when the release has no release group",
      anime_given(release_group=""), {"accepted": False}),
    T("should_be_false_when_release_group_does_not_match",
      "An anime version upgrade from a different release group is rejected",
      anime_given(), {"accepted": False}),
    T("should_return_true_when_repacks_are_not_preferred",
      "With propers/repacks set to do-not-prefer, the anime release-group rule accepts",
      anime_given(propers="DoNotPrefer"), {"accepted": True}),
]

# --- BlockedIndexerSpecificationFixture -------------------------------------

BLOCKED = [
    T("should_return_true_if_no_blocked_indexer", "A release from an indexer that is not blocked is accepted",
      {"release": {"indexer_id": 1}, "blocked_indexers": []}, {"accepted": True}),
    T("should_return_false_if_blocked_indexer", "A release from a blocked indexer is rejected temporarily",
      {"release": {"indexer_id": 1}, "blocked_indexers": [{"provider_id": 1, "disabled_till_offset_hours": 0}]},
      {"accepted": False, "permanent": False}),
]

# --- CustomFormatAllowedByProfileSpecificationFixture -----------------------

AWESOME_ONLY = [{"format": "Cool Format", "score": -2}, {"format": "Awesome Format", "score": 1}]
BOTH = [{"format": "Awesome Format", "score": 1}, {"format": "Cool Format", "score": 2}]
CF_NOTE = "Scores come from CustomFormatsTestHelpers.GetSampleFormatItems (src/NzbDrone.Core.Test/CustomFormats/CustomFormatsTestHelpers.cs:19)."


def cf_given(formats, items, min_score=1):
    return {
        "profile": {"cutoff": "Bluray-1080p", "min_format_score": min_score, "format_items": items},
        "release": {"quality": "DVD", "revision": 2, "custom_formats": formats},
    }


CUSTOM_FORMAT = [
    T("should_allow_if_format_score_greater_than_min", "A release whose format score (1) reaches the minimum (1) is accepted",
      cf_given(["Awesome Format"], AWESOME_ONLY), {"accepted": True}, note=CF_NOTE),
    T("should_deny_if_format_score_not_greater_than_min", "A release whose format score (-2) is below the minimum (1) is rejected",
      cf_given(["Cool Format"], AWESOME_ONLY), {"accepted": False}, note=CF_NOTE),
    T("should_deny_if_format_score_not_greater_than_min_2", "A release whose summed format score (-1) is below the minimum (1) is rejected",
      cf_given(["Cool Format", "Awesome Format"], AWESOME_ONLY), {"accepted": False}, note=CF_NOTE),
    T("should_allow_if_all_format_is_defined_in_profile", "A release whose summed format score (3) reaches the minimum (1) is accepted",
      cf_given(["Cool Format", "Awesome Format"], BOTH), {"accepted": True}, note=CF_NOTE),
    T("should_deny_if_no_format_was_parsed_and_min_score_positive", "A release with no formats (score 0) is rejected when the minimum is 1",
      cf_given([], BOTH), {"accepted": False}, note=CF_NOTE),
    T("should_allow_if_no_format_was_parsed_min_score_is_zero", "A release with no formats (score 0) is accepted when the minimum is 0",
      cf_given([], BOTH, min_score=0), {"accepted": True}, note=CF_NOTE),
]

# --- DownloadDecisionMakerFixture -------------------------------------------

SPEC = {
    "pass1": {"name": "pass1", "result": "accept"},
    "pass2": {"name": "pass2", "result": "accept"},
    "pass3": {"name": "pass3", "result": "accept"},
    "fail1": {"name": "fail1", "result": "reject", "reason": "Unknown", "message": "fail1"},
    "fail2": {"name": "fail2", "result": "reject", "reason": "Unknown", "message": "fail2"},
    "fail3": {"name": "fail3", "result": "reject", "reason": "Unknown", "message": "fail3"},
}
OFFICE = "The.Office.S03E115.DVDRip.XviD-OSiTV"


def ddm_given(specs, title=OFFICE, series_known=True, episodes=1, search="rss", **extra):
    return {"search": search, "specs": [SPEC[s] for s in specs], "releases": [{"title": title}],
            "mapped": {"series_known": series_known, "episodes": episodes}, **extra}


MOCK_ONLY = "mock call verification only; no decision result is asserted"
DDM = [
    X("should_call_all_specifications", "Every specification is evaluated once even when some reject", MOCK_ONLY),
    X("should_call_delayed_specifications_if_non_delayed_passed",
      "Disk-priority (delayed) specifications run when the default ones pass", MOCK_ONLY),
    X("should_not_call_delayed_specifications_if_non_delayed_failed",
      "Disk-priority (delayed) specifications are skipped when a default one rejects", MOCK_ONLY),
    T("should_return_rejected_if_single_specs_fail", "A release rejected by its only specification is not approved",
      ddm_given(["fail1"]), {"accepted": False}),
    T("should_return_rejected_if_one_of_specs_fail", "A release rejected by one of several specifications is not approved",
      ddm_given(["pass1", "fail1", "pass2", "pass3"]), {"accepted": False}),
    T("should_return_pass_if_all_specs_pass", "A release accepted by every specification is approved",
      ddm_given(["pass1", "pass2", "pass3"]), {"accepted": True}),
    T("should_have_same_number_of_rejections_as_specs_that_failed",
      "A decision carries one rejection per rejecting specification",
      ddm_given(["pass1", "pass2", "pass3", "fail1", "fail2", "fail3"]), {"rejection_count": 3}),
    X("should_not_attempt_to_map_episode_if_not_parsable",
      "An unparsable release title is not mapped to episodes or evaluated", MOCK_ONLY),
    T("should_not_attempt_to_map_episode_if_series_title_is_blank",
      "In RSS sync a title that parses without a series title yields no decision",
      ddm_given(["pass1", "pass2", "pass3"], title="1937 - Snow White and the Seven Dwarves"),
      {"decision_count": 0}),
    X("should_return_rejected_result_for_unparsable_search",
      "In a search an unparsable title is not mapped or evaluated", MOCK_ONLY),
    X("should_not_attempt_to_make_decision_if_series_is_unknown",
      "A release for an unknown series is not evaluated by specifications", MOCK_ONLY),
    X("broken_report_shouldnt_blowup_the_process",
      "A mapping exception on one release does not stop the others", "exception injected through a mock; asserts mock calls and logged errors only"),
    T("should_return_unknown_series_rejection_if_series_is_unknown",
      "A release for an unknown series still yields one decision",
      ddm_given(["pass1", "pass2", "pass3"], series_known=False), {"decision_count": 1}),
    T("should_only_include_reports_for_requested_episodes",
      "A season search approves only releases for the requested episodes (scene numbering)",
      {"search": "season", "specs": [{"name": "EpisodeRequestedSpecification", "result": "real"}],
       "series": {"title": "Title1"},
       "series_episodes": [{"season": 1, "number": 1, "scene_season": 2, "scene_number": 1},
                           {"season": 1, "number": 2, "scene_season": 2, "scene_number": 2}],
       "criteria": {"season": 1, "episodes": [{"season": 1, "number": 1, "scene_season": 2, "scene_number": 1}]},
       "releases": [{"title": "Title1.S02E01.720p.WEB-DL-DRONE"}, {"title": "Title1.S02E02.720p.WEB-DL-DRONE"}],
       "mapping": "a release maps to the series episode whose scene number equals the parsed episode number"},
      {"approved_count": 1},
      note="Series title and episode/scene numbers are NBuilder defaults (Title1; 1, 2); titles are built by the test from them."),
    T("should_not_allow_download_if_series_is_unknown", "A release for an unknown series is not allowed to download",
      ddm_given(["pass1", "pass2", "pass3"], series_known=False), {"decision_count": 1, "download_allowed": False}),
    T("should_not_allow_download_if_no_episodes_found", "A release that maps to no episodes is not allowed to download",
      ddm_given(["pass1", "pass2", "pass3"], episodes=0), {"decision_count": 1, "download_allowed": False}),
    T("should_return_a_decision_when_exception_is_caught",
      "A mapping exception still yields a decision for the release",
      {"search": "rss", "specs": [SPEC["pass1"]], "releases": [{"title": OFFICE}], "mapped": "throws"},
      {"decision_count": 1}, note="The test also asserts one logged error (ExpectedErrors(1))."),
    T("should_return_unknown_series_rejection_if_series_title_is_an_alias_for_another_series",
      "An unknown-series rejection names the TVDB id the title is a scene alias for",
      ddm_given(["pass1", "pass2", "pass3"], series_known=False, scene_mapping_tvdb_id=12345),
      {"decision_count": 1, "rejection_contains": "12345"}),
]

# --- FreeSpaceSpecificationFixture ------------------------------------------


def free_given(min_free, available, size, skip=None, root_missing=False):
    g = {"series": {"path": "C:\\Test\\TV\\Series"},
         "config": {"minimum_free_space_when_importing_mb": min_free},
         "disk": {"available_mb": available} if not root_missing else {"root_folder_missing": True},
         "release": {"size_mb": size}}
    if skip is not None:
        g["config"]["skip_free_space_check_when_importing"] = skip
    return g


FREE_SPACE = [
    T("should_return_true_when_available_space_is_more_than_size", "A release smaller than the free space is accepted",
      free_given(0, 200, 100), {"accepted": True}),
    T("should_return_true_when_available_space_minus_size_is_more_than_minimum_free_space",
      "A release that leaves more than the minimum free space is accepted", free_given(50, 200, 100), {"accepted": True}),
    T("should_return_false_available_space_is_less_than_size", "A release larger than the free space is rejected",
      free_given(0, 200, 1000), {"accepted": False}),
    T("should_return_false_when_available_space_minus_size_is_less_than_minimum_free_space",
      "A release that would leave less than the minimum free space is rejected", free_given(150, 200, 100), {"accepted": False}),
    T("should_return_true_if_skip_free_space_check_is_true", "With the free-space check skipped, the release is accepted",
      free_given(150, 200, 100, skip=True), {"accepted": True}),
    T("should_return_true_if_root_folder_is_not_available", "When the root folder is missing, the free-space check accepts",
      free_given(150, None, 100, root_missing=True), {"accepted": True},
      note="Free-space lookup throws DirectoryNotFoundException."),
]
for _c in FREE_SPACE:
    _c["note"] = " ".join(x for x in (_c["note"], "Series path is passed through AsOsAgnostic() in the test.") if x)

# --- FullSeasonSpecificationFixture -----------------------------------------


def season_given(last_offset, full_season=True):
    episodes = [{"air_date_offset_hours": -192} for _ in range(3)]
    episodes[-1] = {"air_date_offset_hours": last_offset}
    return {"release": {"title": "Series.Title.S01.720p.BluRay.X264-RlsGrp", "full_season": full_season},
            "episodes": episodes}


FULL_SEASON = [
    T("should_return_true_if_is_not_a_full_season", "A non-season-pack release is accepted even if an episode has not aired",
      season_given(48, full_season=False), {"accepted": True}),
    T("should_return_true_if_all_episodes_have_aired", "A season pack whose episodes have all aired is accepted",
      season_given(-192), {"accepted": True}),
    T("should_return_true_if_all_episodes_will_have_aired_in_the_next_24_hours",
      "A season pack whose last episode airs within 24 hours is accepted", season_given(23), {"accepted": True}),
    T("should_return_false_if_one_episode_has_not_aired", "A season pack with an episode airing in 2 days is rejected",
      season_given(48), {"accepted": False}),
    T("should_return_false_if_an_episode_does_not_have_an_air_date",
      "A season pack with an episode without an air date is rejected", season_given(None), {"accepted": False}),
]

# --- MaximumSizeSpecificationFixture ----------------------------------------


def max_given(limit, size):
    return {"config": {"maximum_size_mb": limit}, "release": {"size_mb": size}}


MAXIMUM_SIZE = [
    T("should_return_true_when_maximum_size_is_set_to_zero", "A maximum size of 0 means no limit",
      max_given(0, 1000), {"accepted": True}),
    T("should_return_true_when_size_is_smaller_than_maximum_size", "A release below the maximum size is accepted",
      max_given(2000, 1999), {"accepted": True}),
    T("should_return_true_when_size_is_equals_to_maximum_size", "A release exactly at the maximum size is accepted",
      max_given(2000, 2000), {"accepted": True}),
    T("should_return_false_when_size_is_bigger_than_maximum_size", "A release above the maximum size is rejected",
      max_given(2000, 2001), {"accepted": False}),
    T("should_return_true_when_size_is_zero", "A release of unknown size (0) passes the maximum size check",
      max_given(2000, 0), {"accepted": True}),
]

# --- MinimumAgeSpecificationFixture -----------------------------------------


def age_given(min_age, age):
    return {"config": {"minimum_age_minutes": min_age}, "release": {"protocol": "usenet", "age_minutes": age}}


MINIMUM_AGE = [
    T("should_return_true_when_minimum_age_is_set_to_zero", "A minimum age of 0 accepts any usenet release",
      age_given(0, 100), {"accepted": True}),
    T("should_return_true_when_age_is_greater_than_minimum_age", "A usenet release older than the minimum age is accepted",
      age_given(30, 100), {"accepted": True}),
    T("should_return_false_when_age_is_less_than_minimum_age", "A usenet release younger than the minimum age is rejected",
      age_given(30, 10), {"accepted": False}),
]

# --- MonitoredEpisodeSpecificationFixture -----------------------------------


def mon_given(episodes, series=True, criteria=None, **release):
    g = {"series": {"monitored": series}, "episodes": [{"monitored": m} for m in episodes], "search_criteria": criteria}
    if release:
        g["release"] = release
    return g


MONITORED = [
    T("setup_should_return_monitored_episode_should_return_true",
      "Monitored single- and two-episode releases of a monitored series are accepted",
      {"series": {"monitored": True}, "search_criteria": None,
       "subjects": [{"episodes": [{"monitored": True}]}, {"episodes": [{"monitored": True}, {"monitored": True}]}]},
      {"accepted_each": [True, True]}, note="One test, two assertions: one subject each."),
    T("not_monitored_series_should_be_skipped", "A release for an unmonitored series is rejected in RSS sync",
      mon_given([True, True], series=False), {"accepted": False}),
    T("only_episode_not_monitored_should_return_false", "A release whose only episode is unmonitored is rejected",
      mon_given([False]), {"accepted": False}),
    T("both_episodes_not_monitored_should_return_false", "A release whose episodes are all unmonitored is rejected",
      mon_given([False, False]), {"accepted": False}),
    T("only_first_episode_not_monitored_should_return_false",
      "A two-episode release with the first episode unmonitored is rejected", mon_given([False, True]), {"accepted": False}),
    T("only_second_episode_not_monitored_should_return_false",
      "A two-episode release with the second episode unmonitored is rejected", mon_given([True, False]), {"accepted": False}),
    T("should_return_true_for_single_episode_search", "A single-episode search ignores an unmonitored series",
      mon_given([True], series=False, criteria={"kind": "single_episode"}), {"accepted": True}),
    T("should_return_true_if_episode_is_monitored_for_season_search", "A season search accepts a monitored episode",
      mon_given([True], criteria={"kind": "season"}), {"accepted": True}),
    T("should_return_false_if_episode_is_not_monitored_for_season_search",
      "A season search for monitored episodes only rejects an unmonitored episode",
      mon_given([False], criteria={"kind": "season", "monitored_episodes_only": True}), {"accepted": False}),
    T("should_return_true_if_episode_is_not_monitored_and_monitoredEpisodesOnly_flag_is_false",
      "A single-episode search not limited to monitored episodes accepts an unmonitored episode",
      mon_given([False], criteria={"kind": "single_episode", "monitored_episodes_only": False}), {"accepted": True}),
    T("should_return_false_if_episode_is_not_monitored_and_monitoredEpisodesOnly_flag_is_true",
      "A single-episode search limited to monitored episodes rejects an unmonitored episode",
      mon_given([False], criteria={"kind": "single_episode", "monitored_episodes_only": True}), {"accepted": False}),
    T("should_return_false_if_all_episodes_are_not_monitored_for_season_pack_release",
      "A season pack with any unmonitored episode is rejected in RSS sync",
      mon_given([True, False], full_season=True), {"accepted": False}),
]

# --- MultiSeasonSpecificationFixture ----------------------------------------

MULTI_SEASON = [
    T("should_return_true_if_is_not_a_multi_season_release", "A single-season pack is accepted",
      {"release": {"title": "Series.Title.S01-05.720p.BluRay.X264-RlsGrp", "full_season": True, "multi_season": False},
       "episodes": [{}, {}, {"air_date_offset_hours": 48}]},
      {"accepted": True}, note="Other episodes' air dates are NBuilder defaults."),
    T("should_return_false_if_is_a_multi_season_release", "A multi-season pack is rejected",
      {"release": {"title": "Series.Title.S01-05.720p.BluRay.X264-RlsGrp", "full_season": True, "multi_season": True},
       "episodes": [{}, {}, {}]},
      {"accepted": False}),
]

FIXTURES = [
    ("AcceptableSizeSpecificationFixture", "AcceptableSizeSpecification", ACCEPTABLE_SIZE),
    ("AlreadyImportedSpecificationFixture", "AlreadyImportedSpecification", ALREADY_IMPORTED),
    ("AnimeVersionUpgradeSpecificationFixture", "AnimeVersionUpgradeSpecification", ANIME),
    ("BlockedIndexerSpecificationFixture", "BlockedIndexerSpecification", BLOCKED),
    ("CustomFormatAllowedByProfileSpecificationFixture", "CustomFormatAllowedbyProfileSpecification", CUSTOM_FORMAT),
    ("DownloadDecisionMakerFixture", "DownloadDecisionMaker", DDM),
    ("FreeSpaceSpecificationFixture", "FreeSpaceSpecification", FREE_SPACE),
    ("FullSeasonSpecificationFixture", "FullSeasonSpecification", FULL_SEASON),
    ("MaximumSizeSpecificationFixture", "MaximumSizeSpecification", MAXIMUM_SIZE),
    ("MinimumAgeSpecificationFixture", "MinimumAgeSpecification", MINIMUM_AGE),
    ("MonitoredEpisodeSpecificationFixture", "MonitoredEpisodeSpecification", MONITORED),
    ("MultiSeasonSpecificationFixture", "MultiSeasonSpecification", MULTI_SEASON),
]


# --- reference anchoring ----------------------------------------------------

def cs_value(node):
    """Evaluates a [TestCase] argument: literals, true/false/null, unary minus, *."""
    if isinstance(node, ast.Constant):
        return node.value
    if isinstance(node, ast.Name) and node.id in ("true", "false", "null"):
        return {"true": True, "false": False, "null": None}[node.id]
    if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.USub):
        return -cs_value(node.operand)
    if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Mult):
        return cs_value(node.left) * cs_value(node.right)
    raise ValueError(ast.dump(node))


def attributes(lines, method):
    """Returns [(line_no, text)] of the attribute lines directly above `void method(`."""
    pat = re.compile(rf"\bvoid {re.escape(method)}\(")
    hits = [i for i, l in enumerate(lines) if pat.search(l)]
    assert len(hits) == 1, f"{method}: {len(hits)} declarations"
    i, attrs = hits[0] - 1, []
    while i >= 0 and lines[i].strip().startswith("["):
        attrs.append((i + 1, lines[i].strip()))
        i -= 1
    return attrs[::-1]


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
    """Stable ID key: ref path | method | row index | [TestCase] params. No line numbers, so a
    reference bump that shifts lines keeps every ID."""
    if case["kind"] == "test":
        return f"{rel}|{case['method']}|-|-"
    return f"{rel}|{case['method']}|{case['row']}|{json.dumps(case['params'], ensure_ascii=False)}"


def existing_ids():
    """Returns (ids by key, ids by ref, next free number). Ref is the fallback for catalogues
    written before keys were stored."""
    if not CATALOGUE.exists():
        return {}, {}, 1
    text = CATALOGUE.read_text()
    by_key = dict(re.findall(r"^# key: (.+)\n- id: (\S+)$", text, re.M))
    by_ref = dict((r, i) for i, r in re.findall(r'- id: (\S+)\n(?:  .*\n)*?  ref: "(.*)"', text))
    m = re.search(r"^# next-id: (\d+)$", text, re.M)  # IDs are never reused, even after an entry is dropped
    used = [int(i.rsplit("-", 1)[1]) for i in by_ref.values()]
    return by_key, by_ref, max([int(m.group(1)) if m else 1] + [u + 1 for u in used])


def yaml_str(s):
    return json.dumps(s, ensure_ascii=False)


def kept_lines():
    """{id: [status/decision/tests lines as written]} from the current catalogue (ratchet-owned, never regenerated)."""
    if not CATALOGUE.exists():
        return {}
    return {m[0]: re.findall(r"(?m)^  ((?:status|decision|tests):.*(?:\n    .*)*)", m[1])
            for m in re.findall(r"(?m)^- id: (\S+)\n((?:  .*\n?|    .*\n?)*)", CATALOGUE.read_text())}


def main():
    by_key, by_ref, next_n = existing_ids()
    kept = kept_lines()
    entries, skipped, assigned = [], [], set()
    for fixture, spec, cases in FIXTURES:
        rel = f"{TEST_DIR}{fixture}.cs"
        lines = (REF_REPO / rel).read_text(encoding="utf-8-sig").splitlines()
        declared = {m for l in lines for m in re.findall(r"public void (\w+)\(", l)}
        covered = {c["method"] for c in cases}
        assert declared - {"Setup"} <= covered, f"{fixture}: uncovered tests {sorted(declared - covered - {'Setup'})}"
        for m in covered:
            rows = [t for _, t in attributes(lines, m) if t.startswith("[TestCase(")]
            mine = [c for c in cases if c["method"] == m and c["kind"] == "row"]
            assert len(rows) == len(mine), f"{fixture}.{m}: {len(rows)} rows, {len(mine)} ported"
        corpus, seen_rows = [], {}
        for line, case in sorted(((anchor(rel, lines, c), c) for c in cases), key=lambda x: x[0]):
            ref = f"{TAG} {rel}:{line}"
            assert ref not in seen_rows, f"duplicate ref {ref}"
            seen_rows[ref] = True
            key = case_key(rel, case)
            cid = by_key.get(key) or by_ref.get(ref)
            if cid is None:
                assert next_n <= ID_MAX, "ID range exhausted"
                cid = f"{ID_PREFIX}{next_n:04d}"
                next_n += 1
            assert cid not in assigned, f"{cid} assigned twice"
            assigned.add(cid)
            in_scope = case["given"] is not None
            entries.append((key, cid, case["behaviour"], ref, in_scope))
            if not in_scope:
                skipped.append(f"{cid} {fixture}.{case['method']}: {case['why']}")
                continue
            item = {"id": cid, "ref": ref, "input": {"spec": spec, "given": case["given"]}, "expect": case["expect"]}
            if case.get("note"):
                item["note"] = case["note"]
            corpus.append(item)
        CORPUS.mkdir(parents=True, exist_ok=True)
        (CORPUS / f"{fixture}.json").write_text(json.dumps(corpus, indent=1, ensure_ascii=False) + "\n")

    out = ["# Sonarr grab-decision fixtures A..M (DecisionEngineTests), ported by",
           "# parity/extract/sonarr_decision_a.py. Corpus: corpus/reference/sonarr/decision/.",
           "# Re-run the script; do not hand-edit IDs or keys (status/tests may be edited).",
           f"# next-id: {next_n}", ""]
    for key, cid, behaviour, ref, in_scope in entries:
        out += [f"# key: {key}", f"- id: {cid}", f"  behaviour: {yaml_str(behaviour)}", "  source: reference",
                f"  ref: {yaml_str(ref)}", f"  in_scope: {'true' if in_scope else 'false'}",
                *[f"  {k}" for k in kept.get(cid, ["status: unmapped", "tests: []"])]]
    CATALOGUE.write_text("\n".join(out) + "\n")
    print(f"entries {len(entries)}, corpus cases {len(entries) - len(skipped)}, skipped {len(skipped)}")
    print(f"id range {min(assigned)}..{max(assigned)}")
    for s in skipped:
        print("skipped:", s)


if __name__ == "__main__":
    sys.exit(main())
