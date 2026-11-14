"""Port Sonarr import fixtures (completed download handling, import decisions and
specifications, aggregation, file transfer, upgrade/delete, extras) into the
parity corpus and catalogue (stream sonarr-import, IDs SON-IMPORT-0001..0999).

Scope: only fixtures that exercise settings our Sonarr config uses
(corpus/captured/sonarr/config/*.json) or guard against data loss. Inside a
ported fixture every [Test] and [TestCase] row gets a catalogue entry; tests of
a setting value we do not use, Windows-only tests, and tests that assert only
mock calls or logging are entries with in_scope false and no corpus case (the
`why` below says which). Ruled-out fixtures are listed in RULED_OUT.

Hand-translated: each test's C# arrange/assert is written below as data. The
script parses the reference fixtures for every [Test] and [TestCase] row (so
refs carry exact lines), fails if any test has no data here or any data has no
test, checks every row's C# argument text against the `args` written here, and
writes:

  parity/corpus/reference/sonarr/import/<path under the test project>.json
  parity/catalogue/import.sonarr.yaml

IDs are stable: an existing entry's ref line is re-read at the pinned tag to
get its key (fixture path, method, row index, C# row arguments); keys found
again keep their ID, new keys get the next free ID. Status, decision and tests
of existing entries are kept as written.

Conventions for `given` (reference terms; a later phase maps them to Scryer):
- Fields a test does not arrange are left out and take the reference default:
  ints 0, bools false, enums their first member, mocks returning nothing.
  Mocked IConfigService returns defaults, not Sonarr's config defaults:
  episode_title_required Always, minimum_free_space_when_importing 0,
  download_propers_and_repacks PreferAndUpgrade.
- Paths wrapped in AsOsAgnostic() are written as on Linux ("C:\\a\\b" becomes
  "/C/a/b"); paths the test leaves raw are written raw.
- quality: {"quality": <Sonarr name>, "revision": <version>}.
- Sizes are bytes unless the key says _mib (Sonarr Megabytes() = MiB).
- NBuilder-generated values a test relies on are written out (ids 1..n,
  strings "<Property><n>", e.g. DownloadId1).
- expect keys name what the test asserts; "calls" maps a reference call
  (as written in the test's Verify) to its asserted count.

Run: python3 -I parity/extract/sonarr_import.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REF_REPO = Path.home() / "PycharmProjects/arr-reference/sonarr@v4.0.20.3014"
TAG = "Sonarr@v4.0.20.3014"
CATALOGUE = ROOT / "parity/catalogue/import.sonarr.yaml"
CORPUS = ROOT / "parity/corpus/reference/sonarr/import"
ID_PREFIX, ID_MIN, ID_MAX = "SON-IMPORT-", 1, 999

CORE = "src/NzbDrone.Core.Test/"
COMMON = "src/NzbDrone.Common.Test/"

DEFAULT_QUALITIES = ["Unknown", "SDTV", "WEBDL-480p", "DVD", "HDTV-720p", "HDTV-1080p", "HDTV-2160p", "Raw-HD",
                     "WEBDL-720p", "WEBDL-1080p", "WEBDL-2160p", "Bluray-720p", "Bluray-1080p", "Bluray-2160p"]
DEFAULT_PROFILE = {"qualities": DEFAULT_QUALITIES, "allowed": "all"}
MIB = 1024 * 1024
GIB = 1024 * MIB

# Out-of-scope reasons (config facts are from corpus/captured/sonarr/config/).
WINDOWS = "Windows-only test (WindowsOnly()); we run Sonarr's replacement on Linux"
MOCKS = "asserts only mock call counts or logging, no observable result"
ALWAYS = "episode title setting is 'always' (mediamanagement.json#episodeTitleRequired)"
PROPERS = "propers are 'preferAndUpgrade' (mediamanagement.json#downloadPropersAndRepacks)"
EXTRAS = "importExtraFiles is off (mediamanagement.json#importExtraFiles)"
STANDARD = "all 23 series are type standard (series-summary.json); daily/anime handling is unused"


def T(method, behaviour, given, expect, note=None):
    return dict(method=method, rows=None, behaviour=behaviour, given=given, expect=expect, note=note, why=None)


def X(method, behaviour, why):
    return dict(method=method, rows=None, behaviour=behaviour, given=None, expect=None, note=None, why=why)


def R(method, rows):
    """rows: [(args, behaviour, given, expect[, note])], args = the C# row text."""
    return dict(method=method, rows=[r if len(r) == 5 else (*r, None) for r in rows], why=None)


def osa(path):
    """AsOsAgnostic() on Linux."""
    if len(path) > 2 and path[1] == ":":
        path = "/" + path.replace(":", "")
    return path.replace("\\", "/")


def q(name, revision=1):
    return {"quality": name, "revision": revision}


# --- Download/CompletedDownloadServiceTests/ProcessFixture -------------------

CDH_ITEM = {"status": "Completed", "output_path": osa(r"C:\DropFolder\MyDownload"), "title": "Drone.S01E01.HDTV",
            "download_id": "DownloadId1", "category": "Category1"}
CDH_NOTE = ("Where present, download_id DownloadId1 and category Category1 are NBuilder defaults; "
            "other values (e.g. download_id 1234) are set by the test.")


def cdh_check(item=None, history=(), series_lookup=None):
    return {"tracked_download": {"state": "Downloading", "item": {**CDH_ITEM, **(item or {})},
                                 "remote_episode": {"series": "S", "episodes": [{"id": 1}]}},
            "download_client": {"id": 1, "name": "testClient"},
            "history_for_download_id": list(history),
            "series_lookup": series_lookup or {"Drone.S01E01.HDTV": "S"}}


BADLY_NAMED = dict(item={"download_id": "1234", "title": "Droned Pilot"},
                   series_lookup={"Droned S01E01": "S", "*": None})

CDH_PROCESS = [
    R("should_not_process_if_download_status_isnt_completed", [
        (f"DownloadItemStatus.{s}", f"A download with status {s} is not made ready to import",
         cdh_check(item={"status": s}), {"state_is_not": "ImportPending"})
        for s in ("Downloading", "Failed", "Queued", "Paused", "Warning")]),
    T("should_not_process_if_matching_history_is_not_found_and_no_category_specified",
      "A completed download with no grab history and no category is not made ready to import",
      cdh_check(item={"category": None}), {"state_is_not": "ImportPending"}),
    T("should_process_if_matching_history_is_not_found_but_category_specified",
      "A completed download with no grab history but a category, whose series matches, becomes ImportPending",
      cdh_check(item={"category": "tv"}, series_lookup={"*": "S"}), {"state": "ImportPending"}),
    T("should_not_process_if_output_path_is_empty", "A completed download with an empty output path is not made ready to import",
      cdh_check(item={"output_path": None}), {"state_is_not": "ImportPending"}),
    T("should_not_process_if_the_download_cannot_be_tracked_using_the_source_title_as_it_was_initiated_externally",
      "A badly named download whose title matches no series is not made ready to import",
      {**cdh_check(**BADLY_NAMED),
       "history_for_download_id": [{"source_title": "Droned S01E01", "event": "Grabbed"}],
       "process_path_results": [{"path": r"C:\TestPath\Droned.S01E01.mkv", "approved": True, "errors": []}]},
      {"state_is_not": "ImportPending"}),
    T("should_not_process_when_there_is_a_title_mismatch",
      "A completed download whose title maps to no series is not made ready to import",
      cdh_check(series_lookup={"Drone.S01E01.HDTV": None}), {"state_is_not": "ImportPending"}),
]

# --- Download/CompletedDownloadServiceTests/ImportFixture --------------------


def res(path, episodes=None, approved=True, errors=()):
    out = {"path": path, "approved": approved, "errors": list(errors)}
    if episodes is not None:
        out["episodes"] = episodes
    return out


E1, E2 = r"C:\TestPath\Droned.S01E01.mkv", r"C:\TestPath\Droned.S01E02.mkv"


def cdh_import(results, remote_episodes=({"id": 1, "season": 1, "number": 1},), history=(), already_imported=None,
               **extra):
    g = {"tracked_download": {"state": "Downloading", "item": dict(CDH_ITEM),
                              "remote_episode": {"series": "S", "episodes": list(remote_episodes)}},
         "download_client": {"id": 1, "name": "testClient"},
         "series_lookup": {"Drone.S01E01.HDTV": "S"},
         "most_recent_history_for_download_id": "empty EpisodeHistory",
         "history_for_download_id": list(history),
         "episodes_by_ids": [],
         "process_path_results": results}
    if already_imported is not None:
        g["tracked_download_already_imported"] = already_imported
    g.update(extra)
    return g


NOT_IMPORTED = {"state": "ImportBlocked", "events": {"DownloadCompletedEvent": 0}}
IMPORTED = {"state": "Imported", "events": {"DownloadCompletedEvent": 1},
            "calls": {"ProcessPath(output_path, Auto, series, item)": 1}}
EP = lambda i, n: {"id": i, "season": 1, "number": n}  # noqa: E731
BLANK3 = [{"id": 0}, {"id": 0}, {"id": 0}]
NB_HISTORY = ["NBuilder EpisodeHistory 1", "NBuilder EpisodeHistory 2"]

CDH_IMPORT = [
    T("should_not_mark_as_imported_if_all_files_were_rejected",
      "When every file of a completed download is rejected, the download is ImportBlocked and not completed",
      cdh_import([res(E1, [1], False, ["Test Failure"]), res(E2, [2], False, ["Test Failure"])]), NOT_IMPORTED),
    T("should_not_mark_as_imported_if_no_episodes_were_parsed",
      "A download whose grab maps to no episodes and whose files are rejected is ImportBlocked",
      cdh_import([res(E1, [1], False, ["Test Failure"]), res(E2, [2], False, ["Test Failure"])], remote_episodes=()),
      NOT_IMPORTED),
    T("should_not_mark_as_imported_if_all_files_were_skipped",
      "When every file of a completed download is skipped (approved but with an error), the download is ImportBlocked",
      cdh_import([res(E1, [1], True, ["Test Failure"]), res(E2, [2], True, ["Test Failure"])]), NOT_IMPORTED),
    T("should_not_mark_as_imported_if_some_of_episodes_were_not_imported",
      "When only one of three grabbed episodes is imported, the download is ImportBlocked",
      cdh_import([res(E1), res(E1, errors=["Test Failure"]), res(E1, errors=["Test Failure"])], remote_episodes=BLANK3),
      NOT_IMPORTED),
    T("should_not_mark_as_imported_if_some_of_episodes_were_not_imported_including_history",
      "When only one of three grabbed episodes is imported, the download stays ImportBlocked even if history says imported",
      cdh_import([res(E1), res(E1, errors=["Test Failure"]), res(E1, errors=["Test Failure"])], remote_episodes=BLANK3,
                 history=NB_HISTORY, already_imported=True),
      NOT_IMPORTED,
      "The already-imported check is mocked to return true only for this exact history list."),
    T("should_mark_as_imported_if_all_episodes_were_imported",
      "When every grabbed episode is imported, the download is Imported and DownloadCompletedEvent is published",
      cdh_import([res(E1, [1]), res(E2, [2])], remote_episodes=({"id": 1}, {"id": 2})), IMPORTED),
    T("should_mark_as_imported_if_all_episodes_were_imported_including_history",
      "A download is Imported when one episode imported now and history shows the rest already imported",
      cdh_import([res(E1, [1]), res(E2, [2], errors=["Test Failure"])], remote_episodes=({"id": 1}, {"id": 2}),
                 history=NB_HISTORY, already_imported=True),
      IMPORTED),
    T("should_mark_as_imported_if_double_episode_file_is_imported",
      "A download is Imported when one multi-episode file covers both grabbed episodes",
      cdh_import([res(r"C:\TestPath\Droned.S01E01-E02.mkv", [1, 2])], remote_episodes=({"id": 1}, {"id": 2})),
      IMPORTED),
    T("should_mark_as_imported_if_all_episodes_were_imported_but_extra_files_were_not",
      "A download is Imported when its episode imported even though another file was skipped",
      cdh_import([res(E1, [0]), res(E1, errors=["Test Failure"])], remote_episodes=({"id": 0},),
                 series_lookup={"*": "S"}),
      IMPORTED),
    T("should_mark_as_imported_if_the_download_can_be_tracked_using_the_source_seriesid",
      "A badly named download is Imported when its episodes import (series found by id)",
      cdh_import([res(E1, [1])], **{"tracked_download": {
          "state": "Downloading", "item": {**CDH_ITEM, "download_id": "1234", "title": "Droned Pilot"},
          "remote_episode": {"series": "S", "episodes": [EP(1, 1)]}},
          "series_lookup": {"Droned S01E01": "S", "*": None}, "series_by_id": "S",
          "most_recent_history_for_download_id": {"source_title": "Droned S01E01"}}),
      IMPORTED),
]

# --- Extras/ExtraServiceFixture ----------------------------------------------

EXTRA_FOLDER = osa(r"C:\Test\Unsorted TV\Series.Title.S01")


def extras(other, import_extra=False):
    return {"config": {"import_extra_files": import_extra, "extra_file_extensions": "nfo,srt"}, "is_read_only": True,
            "series_path": osa(r"C:\Test\TV\Series Title"),
            "episode_file": {"path": osa(r"C:\Test\TV\Series Title\Season 1\Series Title - S01E01.mkv")},
            "local_episode": {"path": EXTRA_FOLDER + "/Series.Title.S01E01.mkv"},
            "files_in_download_folder": [EXTRA_FOLDER + "/Series.Title.S01E01.mkv", EXTRA_FOLDER + "/" + other],
            "extra_services": [{"name": "subtitles", "order": 0, "accepts": [".srt"]},
                               {"name": "other", "order": 1, "accepts": "any"}]}


NOT_OFFERED = {"calls": {"subtitles.CanImportFile(isReadOnly: true)": 0, "other.CanImportFile(isReadOnly: true)": 0}}

EXTRA = [
    T("should_not_pass_file_if_import_disabled", "With extra-file import off, an .nfo next to the episode is not imported",
      extras("Series.Title.S01E01.nfo"), NOT_OFFERED),
    R("should_not_pass_unwanted_file", [
        (f'"Series Title - S01E01.{e}"', f"With extra-file import off, a .{e} file next to the episode is not imported",
         extras(f"Series Title - S01E01.{e}"), NOT_OFFERED) for e in ("sub", "ass")]),
    X("should_pass_subtitle_file_to_subtitle_service", "An .srt next to the episode is handed to the subtitle service", EXTRAS),
    X("should_pass_nfo_file_to_other_service", "An .nfo next to the episode is handed to the other-extras service", EXTRAS),
    X("should_search_subtitles_when_importing_from_job_folder",
      "Importing from a job folder searches it recursively for extra files", EXTRAS),
    X("should_not_search_subtitles_when_not_importing_from_job_folder",
      "Importing a lone file searches only its folder for extra files", EXTRAS),
]

# --- MediaFiles/DownloadedEpisodesImportServiceFixture ------------------------

DROP = osa("c:\\drop\\")
SUB = osa("c:\\root\\foldername")
VIDEO = osa("c:\\root\\foldername\\30.rock.s01e01.ext")


def dei(call, path=DROP, series_found=None, decisions=None, import_results=None, **kw):
    g = {"call": call, "path": path, "subfolders": [SUB], "video_files": [VIDEO], "folder_exists": "any path"}
    if series_found is not None:
        g["series_found"] = series_found
    if decisions is not None:
        g["decisions"] = decisions
    if import_results is not None:
        g["import_results"] = import_results
    g.update(kw)
    return g


DEI_NOTE = ("ProcessRootFolder has no production caller at the tag (DownloadedEpisodesImportService.cs:57); "
            "it reaches the same ProcessFolder logic completed download handling uses via ProcessPath.")
TRACKED = {"download_id": "sab1", "status": "Downloading", "remote_episode_series": "S"}
FOLDER = r"C:\media\ba09030e-1234-1234-1234-123456789abc\[HorribleSubs] Maria the Virgin Witch - 09 [720p]"
RAR = VIDEO.replace(".ext", ".rar")

DOWNLOADED_IMPORT = [
    T("should_search_for_series_using_folder_name", "A download folder's series is looked up by the folder name",
      dei("ProcessRootFolder"), {"series_lookups": ["foldername"]}, DEI_NOTE),
    T("should_skip_if_file_is_in_use_by_another_process", "A locked video file is not imported",
      dei("ProcessRootFolder", series_found=True, file_locked=True), {"import_called": False}, DEI_NOTE),
    T("should_skip_if_no_series_found", "A folder that matches no series is neither evaluated nor imported",
      dei("ProcessRootFolder", series_found=False), {"decisions_made": False, "import_called": False}, DEI_NOTE),
    T("should_not_import_if_folder_is_a_series_path", "A download folder that is an existing series folder is not scanned",
      dei("ProcessRootFolder", series_found=True, folder_is_series_path=True, video_files=[]),
      {"calls": {"GetVideoFiles(any, true)": 0}}, DEI_NOTE),
    X("should_not_delete_folder_if_no_files_were_imported",
      "When nothing imports, the folder size is never checked",
      "asserts only that GetFolderSize is never called, which the code at the tag never calls; vacuous"),
    T("should_not_delete_folder_if_files_were_imported_and_video_files_remain",
      "After an import, the download folder is kept while a non-sample video file remains",
      dei("ProcessRootFolder", series_found=True, decisions=["approved"], import_results=["Imported"],
          sample_detection="Indeterminate (mock default)"),
      {"calls": {"DeleteFolder(any, true)": 0}}, DEI_NOTE),
    T("should_delete_folder_if_files_were_imported_and_only_sample_files_remain",
      "After an import, the download folder is deleted when only sample files remain",
      dei("ProcessRootFolder", series_found=True, decisions=["approved"], import_results=["Imported"],
          sample_detection="Sample"),
      {"calls": {"DeleteFolder(any, true)": 1}}, DEI_NOTE),
    R("should_remove_unpack_from_folder_name", [
        (f'"{p}"', f"A {p} prefix is stripped from the folder name before the series lookup",
         dei("ProcessRootFolder", subfolders=[osa(rf"C:\Test\Unsorted\{p}30.rock.s01e01.pilot.hdtv-lol")]),
         {"series_lookups": ["30.rock.s01e01.pilot.hdtv-lol"], "no_lookup_starting_with": p}, DEI_NOTE)
        for p in ("_UNPACK_", "_FAILED_")]),
    T("should_return_importresult_on_unknown_series",
      "Processing a lone file of an unknown series returns one Rejected result for that path",
      dei("ProcessPath", path=osa(r"C:\folder\file.mkv"), folder_exists=False, file_exists=True),
      {"results": [{"result": "Rejected", "path": osa(r"C:\folder\file.mkv")}]}),
    T("should_not_delete_if_there_is_large_rar_file",
      "After an import, the download folder is kept when a RAR file over 10 MiB remains",
      dei("ProcessRootFolder", series_found=True, decisions=["approved"], import_results=["Imported"],
          sample_detection="Sample", files_recursive=[RAR], file_size_mib=15),
      {"calls": {"DeleteFolder(any, true)": 0}}, DEI_NOTE),
    X("should_use_folder_if_folder_import",
      "Importing a folder passes the folder's parsed info (absolute episode 9) to the decision maker", STANDARD),
    T("should_not_use_folder_if_file_import", "Importing a lone file passes no folder info to the decision maker",
      dei("ProcessPath", path=osa(r"C:\media\ba09030e-1234-1234-1234-123456789abc\Torrents"
                                  r"\[HorribleSubs] Maria the Virgin Witch - 09 [720p].mkv"),
          series_found=True, folder_exists=False, file_exists=True),
      {"calls": {"GetImportDecisions(any, any, any, folder_info=null, true)": 1}}),
    T("should_not_process_if_file_and_folder_do_not_exist", "A path that is neither a folder nor a file yields no results",
      dei("ProcessPath", path=osa(FOLDER), folder_exists=False, file_exists=False),
      {"results": [], "series_lookups": []}),
    T("should_not_delete_if_no_files_were_imported", "The download folder is kept when no file was imported",
      dei("ProcessRootFolder", series_found=True, decisions=["approved"], import_results=[],
          sample_detection="Sample", file_size_mib=15),
      {"calls": {"DeleteFolder(any, true)": 0}}, DEI_NOTE),
    T("should_not_delete_folder_after_import",
      "In Auto mode, a download whose client cannot move files keeps its folder after import",
      dei("ProcessPath", series_found=True, decisions=["approved"], import_mode="Auto", tracked_download={**TRACKED, "can_move_files": False},
          import_results=["Imported"], video_files_after_import=[]),
      {"calls": {"DeleteFolder(any, true)": 0}}),
    T("should_delete_folder_if_importmode_move", "In Move mode, the download folder is deleted after import",
      dei("ProcessPath", series_found=True, decisions=["approved"], import_mode="Move", tracked_download={**TRACKED, "can_move_files": False},
          import_results=["Imported"], video_files_after_import=[]),
      {"calls": {"DeleteFolder(any, true)": 1}}),
    T("should_not_delete_folder_if_importmode_copy", "In Copy mode, the download folder is kept after import",
      dei("ProcessPath", series_found=True, decisions=["approved"], import_mode="Copy", tracked_download={**TRACKED, "can_move_files": True},
          import_results=["Imported"], video_files_after_import=[]),
      {"calls": {"DeleteFolder(any, true)": 0}}),
    T("should_return_rejection_if_nothing_imported_and_contains_rar_file",
      "A folder with nothing importable but a RAR file yields one Rejected result",
      dei("ProcessPath", path=osa(r"C:\Test\Unsorted\Series.Title.S01E01.abc-Sonarr"), series_found=True,
          decisions=[], import_results=[], files_recursive=[RAR]),
      {"result_count": 1, "results": [{"result": "Rejected"}]}),
    T("should_return_rejection_if_nothing_imported_and_contains_executable_file",
      "A folder with nothing importable but an .exe file yields one Rejected result",
      dei("ProcessPath", path=osa(r"C:\Test\Unsorted\Series.Title.S01E01.abc-Sonarr"), series_found=True,
          decisions=[], import_results=[], files_recursive=[VIDEO.replace(".ext", ".exe")]),
      {"result_count": 1, "results": [{"result": "Rejected"}]}),
]

# --- MediaFiles/EpisodeFileMovingServiceTests/MoveEpisodeFileFixture ----------


def move_given(series_folder_exists=False):
    return {"series_path": osa(r"C:\Test\TV\Series"), "episode_file": {"relative_path": r"Season 1\File.avi"},
            "built_file_path": osa(r"C:\Test\TV\Series\Season 01\File Name.avi"),
            "built_season_path": osa(r"C:\Test\TV\Series\Season 01"),
            "root_folder": osa("C:\\Test\\TV\\"), "root_folder_exists": True,
            "series_folder_exists": series_folder_exists, "season_folder_exists": False}


MOVE = [
    X("should_catch_UnauthorizedAccessException_during_folder_inheritance",
      "A permission error while inheriting folder permissions does not fail the move", WINDOWS),
    X("should_catch_InvalidOperationException_during_folder_inheritance",
      "An invalid-operation error while inheriting folder permissions does not fail the move", WINDOWS),
    T("should_notify_on_series_folder_creation", "Moving an episode into a new series folder publishes a series-folder-created event",
      move_given(), {"events": {"EpisodeFolderCreatedEvent with SeriesFolder": 1}}),
    T("should_notify_on_season_folder_creation", "Moving an episode into a new season folder publishes a season-folder-created event",
      move_given(), {"events": {"EpisodeFolderCreatedEvent with SeasonFolder": 1}}),
    T("should_not_notify_if_series_folder_already_exists",
      "No series-folder-created event is published when the series folder already exists",
      move_given(True), {"events": {"EpisodeFolderCreatedEvent with SeriesFolder": 0}}),
]

# --- MediaFiles/EpisodeImport/Aggregation/Aggregators -------------------------


def agg_eps(file_title, folder_title, path, other_video_files=False):
    g = {"file_title": file_title, "folder_title": folder_title, "path": osa(path)}
    if other_video_files:
        g["other_video_files"] = True
    return g


AGG_EPISODES = [
    T("should_not_use_folder_for_full_season", "A full-season folder name is not used to find the file's episodes",
      agg_eps("Series.Title.S01E01", "Series.Title.S01", r"C:\Test\Unsorted TV\Series.Title.S01\Series.Title.S01E01.mkv"),
      {"episodes_looked_up_from": "file"}),
    T("should_not_use_folder_when_it_contains_more_than_one_valid_video_file",
      "A folder holding other video files is not used to find the file's episodes",
      agg_eps("Series.Title.S01E01", "Series.Title.S01", r"C:\Test\Unsorted TV\Series.Title.S01\Series.Title.S01E01.mkv",
              True),
      {"episodes_looked_up_from": "file"}),
    T("should_not_use_folder_name_if_file_name_is_scene_name",
      "When the file name looks like a scene name, it is used instead of the folder name",
      agg_eps("Series.Title.S01E01", "Series.Title.S01E01",
              r"C:\Test\Unsorted TV\Series.Title.S01E01\Series.Title.S01E01.720p.HDTV-Sonarr.mkv"),
      {"episodes_looked_up_from": "file"}),
    T("should_use_folder_when_only_one_video_file",
      "A single-episode folder with one video file is used to find the episodes",
      agg_eps("Series.Title.S01E01", "Series.Title.S01E01", r"C:\Test\Unsorted TV\Series.Title.S01E01\Series.Title.S01E01.mkv"),
      {"episodes_looked_up_from": "folder"}),
    T("should_use_file_when_folder_is_absolute_and_file_is_not",
      "When the folder name has only an absolute number and the file has season/episode, the file is used",
      agg_eps("Series.Title.S01E01", "Series.Title.01", r"C:\Test\Unsorted TV\Series.Title.101\Series.Title.S01E01.mkv"),
      {"episodes_looked_up_from": "file"}),
    T("should_use_special_info_when_not_null",
      "When the file's episodes are not found, the special-episode parse of the title is used",
      {"file_title": "S00E01", "path": osa(r"C:\Test\TV\Series\Specials\S00E01.mkv"),
       "episodes_for_file_info": [], "special_parse": "returns a copy of the file info"},
      {"episodes_looked_up_from": "special"}),
]


def agq(augmenters, download_client_item=False):
    return {"augmenters": augmenters, "download_client_item": download_client_item}


EXT = {"name": "extension", "order": 1, "source": "Television", "source_confidence": "Fallback", "resolution": 720,
       "resolution_confidence": "Fallback", "revision": 1, "revision_confidence": "Fallback"}
NAME = {"name": "name", "order": 2, "source": "Television", "source_confidence": "Default", "resolution": 480,
        "resolution_confidence": "Default", "revision": 1, "revision_confidence": "Default"}
MEDIAINFO = {"name": "mediainfo", "order": 4, "resolution": 1080, "resolution_confidence": "MediaInfo"}
RELEASE = {"name": "release_name", "order": 5, "source": "Web", "source_confidence": "MediaInfo"}
NULL = {"name": "null", "returns": None}
AGQ_NOTE = "new Revision() is version 1; ResolutionOnly/SourceOnly results leave the other parts unset."


def a(base, **kw):
    return {**base, **kw}


AGG_QUALITY = [
    T("should_return_HDTV720_from_extension_when_other_augments_are_null",
      "With only the file extension known, the import quality is HDTV-720p from the extension",
      agq([EXT, NULL]), {"quality": "HDTV-720p", "source_detection": "Extension", "resolution_detection": "Extension"},
      AGQ_NOTE),
    T("should_return_SDTV_when_HDTV720_came_from_extension",
      "A name-derived TV 480p beats an extension guess and yields SDTV",
      agq([EXT, NAME]), {"quality": "SDTV", "source_detection": "Name", "resolution_detection": "Name"}, AGQ_NOTE),
    T("should_return_HDTV1080p_when_HDTV720_came_from_extension_and_mediainfo_indicates_1080",
      "A media-info 1080 resolution with an extension TV source yields HDTV-1080p",
      agq([EXT, MEDIAINFO]), {"quality": "HDTV-1080p", "source_detection": "Extension", "resolution_detection": "MediaInfo"},
      AGQ_NOTE),
    T("should_return_HDTV1080p_when_SDTV_came_from_name_and_mediainfo_indicates_1080",
      "A media-info 1080 resolution with a name TV source yields HDTV-1080p",
      agq([NAME, MEDIAINFO]), {"quality": "HDTV-1080p", "source_detection": "Name", "resolution_detection": "MediaInfo"},
      AGQ_NOTE),
    T("should_return_WEBDL480p_when_file_name_has_HDTV480p_but_release_name_indicates_webdl_source",
      "A WEB source from the grabbed release name turns a name-derived TV 480p into WEBDL-480p",
      agq([NAME, RELEASE], True), {"quality": "WEBDL-480p", "source_detection": "Name", "resolution_detection": "Name"},
      AGQ_NOTE),
    T("should_return_version_1_when_no_version_specified", "Without a version tag the import revision is 1",
      agq([NAME, RELEASE], True), {"revision": 1, "revision_detection": "Unknown"}, AGQ_NOTE),
    T("should_return_version_2_when_name_indicates_proper", "A proper tag in the name gives revision 2",
      agq([a(NAME, revision=2, revision_confidence="Tag"), RELEASE], True), {"revision": 2, "revision_detection": "Name"},
      AGQ_NOTE),
    T("should_return_version_0_when_file_name_indicates_v0", "A v0 tag in the file name gives revision 0",
      agq([a(NAME, revision=0, revision_confidence="Tag"), RELEASE], True), {"revision": 0, "revision_detection": "Name"},
      AGQ_NOTE),
    T("should_return_version_2_when_file_name_indicates_v0_and_release_name_indicates_v2",
      "A v2 tag in the release name beats a v0 tag in the file name",
      agq([a(NAME, revision=0, revision_confidence="Tag"),
           {"name": "release_name", "order": 5, "source": "Television", "source_confidence": "Default",
            "resolution": 480, "resolution_confidence": "Default", "revision": 2, "revision_confidence": "Tag"}], True),
      {"revision": 2, "revision_detection": "Name"}, AGQ_NOTE),
    T("should_return_Bluray576p_when_Bluray_came_from_name_and_mediainfo_indicates_576p",
      "A name Bluray source with a media-info 576 resolution yields Bluray-576p",
      agq([a(NAME, source="Bluray", revision=0, revision_confidence="Tag"),
           a(MEDIAINFO, resolution=576)]),
      {"quality": "Bluray-576p", "source_detection": "Name", "resolution_detection": "MediaInfo"}, AGQ_NOTE),
    T("should_return_SDTV_when_HDTV_came_from_name_and_mediainfo_indicates_576p",
      "A name TV source with a media-info 576 resolution yields SDTV",
      agq([a(NAME, revision=0, revision_confidence="Tag"), a(MEDIAINFO, resolution=576)]),
      {"quality": "SDTV", "source_detection": "Name", "resolution_detection": "MediaInfo"}, AGQ_NOTE),
]

RG_PATH = osa(r"C:\Test\Unsorted TV\Series.Title.S01\Series.Title.S01E01.WEB-DL.mkv")
AGG_RELEASE_GROUP = [
    T("should_not_use_downloadclient_for_full_season", "A full-season download name does not set the file's release group",
      {"file_title": "Series.Title.S01E01.WEB-DL-Wizzy", "download_client_title": "Series.Title.S01.WEB-DL-Viva",
       "path": RG_PATH}, {"release_group": "Wizzy"}),
    T("should_not_use_folder_for_full_season", "A full-season folder name does not set the file's release group",
      {"file_title": "Series.Title.S01E01.WEB-DL-Wizzy", "folder_title": "Series.Title.S01.WEB-DL-Drone",
       "path": RG_PATH}, {"release_group": "Wizzy"}),
    T("should_prefer_downloadclient", "The download name's release group wins over folder and file",
      {"file_title": "Series.Title.S01E01.WEB-DL-Wizzy", "folder_title": "Series.Title.S01E01.WEB-DL-Drone",
       "download_client_title": "Series.Title.S01E01.WEB-DL-Viva", "path": RG_PATH}, {"release_group": "Viva"}),
    T("should_prefer_folder", "Without a group in the download name, the folder's release group wins over the file",
      {"file_title": "Series.Title.S01E01.WEB-DL-Wizzy", "folder_title": "Series.Title.S01E01.WEB-DL-Drone",
       "download_client_title": "Series.Title.S01E01.WEB-DL", "path": RG_PATH}, {"release_group": "Drone"}),
    T("should_fallback_to_file", "Without a group in download or folder name, the file's release group is used",
      {"file_title": "Series.Title.S01E01.WEB-DL-Wizzy", "folder_title": "Series.Title.S01E01.WEB-DL",
       "download_client_title": "Series.Title.S01E01.WEB-DL",
       "path": osa(r"C:\Test\Unsorted TV\Series.Title.S01\Series.Title.S01E01.mkv")}, {"release_group": "Wizzy"}),
]


def mi(width, height=None, title=None, media_info=True):
    if not media_info:
        return {"media_info": None}
    # NBuilder CreateNew() fills unset int/string properties with 1 / "<Property>1".
    return {"media_info": {"width": width, "height": 1 if height is None else height,
                           "title": "Title1" if title is None else title}}


MI_ROWS = [(4096, 1, 2160), (4000, 1, 2160), (3840, 1, 2160), (3200, 1, 2160), (2000, 1, 1080), (1920, 1, 1080),
           (1440, 1080, 1080), (1800, 1, 1080), (1490, 1, 720), (1280, 1, 720), (1200, 1, 720), (1000, 1, 576),
           (720, 576, 576), (800, 1, 480), (720, 1, 480), (600, 1, 480), (100, 1, 480)]

AUGMENT_MEDIAINFO = [
    T("should_return_null_if_media_info_is_null", "Without media info, no quality is derived from it",
      mi(0, media_info=False), {"result": None}),
    T("should_return_null_if_media_info_width_is_zero", "Media info with width 0 yields no quality",
      mi(0), {"result": None}, "Height 1 and title Title1 are NBuilder defaults."),
    R("should_return_closest_resolution", [
        (f"{w}, {h}, {r}", f"Media info {w}x{h} maps to resolution {r} with unknown source", mi(w, h),
         {"resolution": r, "source": "Unknown"}, "Title Title1 is the NBuilder default.") for w, h, r in MI_ROWS]),
    T("should_include_source_if_extracted_from_title", "A WEB tag in the media-info title sets the source to Web",
      mi(1920, 1080, "Series.Title.S01E05.WEB.x264-Sonarr"), {"resolution": 1080, "source": "Web"}),
    T("should_have_unknown_source_if_no_source_extracted_from_title",
      "A media-info title without a source tag leaves the source unknown",
      mi(1920, 1080, "Series.Title.S01E05.x264-Sonarr"), {"resolution": 1080, "source": "Unknown"}),
    T("should_include_source_for_576_if_extracted_from_title", "A Bluray tag in the media-info title sets the source at 576",
      mi(1024, 576, "Series.Title.S01E05.Bluray.x264-Sonarr"), {"resolution": 576, "source": "Bluray"}),
]

AUGMENT_RELEASE_NAME = [
    T("should_return_null_if_download_client_item_is_null", "Without a download, no quality is derived from the release name",
      {"download_client_item": None}, {"result": None}),
    T("should_return_null_if_no_grabbed_history", "Without a grab in download history, no quality comes from the release name",
      {"download_client_item": "NBuilder", "latest_grab": None}, {"result": None}),
    R("should_return_augmented_quality", [
        (f'"{t}", QualitySource.{s}, Confidence.{sc}, {r}, Confidence.{rc}',
         f"The grabbed title {t} gives source {s} ({sc}) and resolution {r} ({rc})",
         {"download_client_item": "NBuilder", "latest_grab": {"source_title": t}},
         {"source": s, "source_confidence": sc, "resolution": r, "resolution_confidence": rc})
        for t, s, sc, r, rc in [("Series.Title.S01E01.1080p.WEB.x264", "Web", "Tag", 1080, "Tag"),
                                ("Series.Title.S01E01.WEB.x264", "Web", "Tag", 480, "Fallback"),
                                ("Series.Title.S01E01.720p.x264", "Television", "Fallback", 720, "Tag")]]),
]

# --- MediaFiles/EpisodeImport/DetectSampleFixture -----------------------------

DS_PATH = r"C:\Test\30 Rock\30.rock.s01e01.avi"


def ds(call="series", series_runtime=30, season=1, path=DS_PATH, runtime_s=None, episode_runtime=None,
       series_type="Standard", runtime_unreadable=False):
    g = {"call": call, "series": {"type": series_type, "runtime": series_runtime},
         "episodes": [{"season": season}], "path": path, "quality": "HDTV-720p"}
    if episode_runtime is not None:
        g["episodes"][0]["runtime"] = episode_runtime
    if runtime_s is not None:
        g["file_runtime_seconds"] = runtime_s
        g["media_info"] = {"runtime_seconds": runtime_s}
    if runtime_unreadable:
        g["file_runtime_seconds"] = None
    return g


DS_CALL = "call 'series' is IsSample(series, path, isSpecial) with isSpecial from the episodes' season 0; " \
          "'local_episode' is IsSample(localEpisode)."

DETECT_SAMPLE = [
    T("should_return_false_if_season_zero", "A special (season 0) is never a sample", ds(season=0),
      {"result": "NotSample"}, DS_CALL),
    T("should_return_false_for_flv", "An .flv file is never a sample and its runtime is not read",
      ds(path=r"C:\Test\some.show.s01e01.flv"), {"result": "NotSample", "calls": {"GetRunTime": 0}}, DS_CALL),
    T("should_return_false_for_strm", "A .strm file is never a sample and its runtime is not read",
      ds(path=r"C:\Test\some.show.s01e01.strm"), {"result": "NotSample", "calls": {"GetRunTime": 0}}, DS_CALL),
    X("should_use_runtime", "The file's runtime is read to decide sample status", MOCKS),
    T("should_return_true_if_runtime_is_less_than_minimum", "A 60 s file of a 30-minute series is a sample",
      ds(runtime_s=60), {"result": "Sample"}, DS_CALL),
    T("should_return_false_if_runtime_greater_than_minimum", "A 600 s file of a 30-minute series is not a sample",
      ds(runtime_s=600), {"result": "NotSample"}, DS_CALL),
    T("should_return_false_if_runtime_greater_than_webisode_minimum",
      "A 299 s file of a 6-minute series is not a sample", ds(series_runtime=6, runtime_s=299),
      {"result": "NotSample"}, DS_CALL),
    T("should_return_false_if_runtime_greater_than_anime_short_minimum",
      "A 60 s file of a 2-minute series is not a sample", ds(series_runtime=2, runtime_s=60),
      {"result": "NotSample"}, DS_CALL),
    T("should_return_true_if_runtime_less_than_anime_short_minimum", "A 10 s file of a 2-minute series is a sample",
      ds(series_runtime=2, runtime_s=10), {"result": "Sample"}, DS_CALL),
    T("should_return_indeterminate_if_mediainfo_result_is_null", "An unreadable runtime makes the result Indeterminate",
      ds(runtime_unreadable=True), {"result": "Indeterminate"}, DS_CALL),
    X("should_not_treat_daily_episode_a_special", "A daily series' season-0 episode is still checked by runtime", STANDARD),
    X("should_return_false_for_anime_special", "An anime special is never a sample", STANDARD),
    T("should_use_runtime_from_media_info",
      "With media info present, its runtime (120 s) is used and the file is not re-read; a 30-minute episode is a sample",
      ds(call="local_episode", runtime_s=120, episode_runtime=30), {"result": "Sample", "calls": {"GetRunTime": 0}},
      DS_CALL),
    T("should_use_runtime_from_episode_over_series",
      "The episode runtime (30) is used over the series runtime (5); a 120 s file is a sample",
      ds(call="local_episode", series_runtime=5, runtime_s=120, episode_runtime=30), {"result": "Sample"}, DS_CALL),
    T("should_default_to_45_minutes_if_runtime_is_zero",
      "With no series or episode runtime, 45 minutes is assumed and a 120 s file is a sample",
      ds(call="local_episode", series_runtime=0, runtime_s=120, episode_runtime=0), {"result": "Sample"}, DS_CALL),
]

# --- MediaFiles/EpisodeImport/GetSceneNameFixture ------------------------------

SEASON_NAME = "series.title.s02.dvdrip.x264-ingot"
EPISODE_NAME = "series.title.s02e23.dvdrip.x264-ingot"
UNSORTED = osa(r"C:\Test\Unsorted TV")
SN_DEFAULT_PATH = osa(r"C:\Test\TV\Series Title") + "/Series Title - S02E23 - Episode Title.mkv"


def sn(path=SN_DEFAULT_PATH, dci=None, folder=None, other=False):
    g = {"path": path, "quality": "Bluray-720p", "release_group": "DRONE"}
    if dci is not None:
        g["download_client_info"] = dci
    if folder is not None:
        g["folder_info"] = folder
    if other:
        g["other_video_files"] = True
    return g


def info(title, full_season=False):
    return {"release_title": title, "full_season": full_season}


AAAAA = f"{UNSORTED}/{EPISODE_NAME}/aaaaa.mkv"
BATCH = "[HorribleSubs] Series Title (01-62) [1080p] (Batch)"

SCENE_NAME = [
    T("should_use_download_client_item_title_as_scene_name", "The download's release title is used as the scene name",
      sn(dci=info(EPISODE_NAME)), {"scene_name": EPISODE_NAME}),
    T("should_not_use_download_client_item_title_as_scene_name_if_full_season",
      "A full-season download title is not used as the scene name",
      sn(path=f"{UNSORTED}/{SEASON_NAME}/{EPISODE_NAME}", dci=info(SEASON_NAME, True)), {"scene_name": None}),
    T("should_not_use_download_client_item_title_as_scene_name_if_there_are_other_video_files",
      "A download title is not used as the scene name when the download has other video files",
      sn(path=f"{UNSORTED}/{SEASON_NAME}/{EPISODE_NAME}", dci=info(SEASON_NAME), other=True), {"scene_name": None}),
    T("should_use_file_name_as_scenename_only_if_it_looks_like_scenename",
      "A file name that looks like a scene name is used as the scene name",
      sn(path=f"{UNSORTED}/{SEASON_NAME}/{EPISODE_NAME}.mkv"), {"scene_name": EPISODE_NAME}),
    T("should_not_use_file_name_as_scenename_if_it_doesnt_look_like_scenename",
      "A file name that does not look like a scene name is not used", sn(path=AAAAA), {"scene_name": None}),
    T("should_use_folder_name_as_scenename_only_if_it_looks_like_scenename",
      "A folder name that looks like a scene name is used as the scene name",
      sn(folder={"release_title": EPISODE_NAME}), {"scene_name": EPISODE_NAME}),
    T("should_not_use_folder_name_as_scenename_if_it_doesnt_look_like_scenename",
      "A folder name that does not look like a scene name is not used",
      sn(path=AAAAA, folder={"release_title": "aaaaa"}), {"scene_name": None}),
    T("should_not_use_folder_name_as_scenename_if_it_is_for_a_full_season",
      "A full-season folder name is not used as the scene name",
      sn(path=AAAAA, folder=info(SEASON_NAME, True)), {"scene_name": None}),
    T("should_not_use_folder_name_as_scenename_if_it_is_for_batch", "A batch download or folder name is not used as the scene name",
      sn(path=f"{UNSORTED}/{BATCH}/[HorribleSubs] Series Title - 14 [1080p].mkv", dci=info(BATCH),
         folder=info(SEASON_NAME), other=True), {"scene_name": None}),
    T("should_not_use_folder_name_as_scenename_if_there_are_other_video_files",
      "A folder name is not used as the scene name when the folder has other video files",
      sn(path=AAAAA, folder=info(SEASON_NAME), other=True), {"scene_name": None}),
    R("should_remove_extension_from_nzb_title_for_scene_name", [
        (f'"{e}"', f"A {e} extension on the download title is dropped from the scene name",
         sn(dci={"release_title": EPISODE_NAME + e}), {"scene_name": EPISODE_NAME}) for e in (".mkv", ".par2", ".nzb")]),
]

# --- MediaFiles/EpisodeImport/ImportApprovedEpisodesFixture --------------------

IA_SERIES = osa(r"C:\Test\TV\30 Rock")
IA_OUTPUT = osa(r"C:\Test\Unsorted\TV\30.Rock.S01E01")
IA_APPROVED = {"count": 5, "episodes": "one NBuilder episode each (ids 1..5)",
               "path": IA_SERIES + "/30 Rock - S01E01 - Pilot.avi", "quality": "Bluray-720p", "release_group": "DRONE",
               "file_episode_info": "empty ParsedEpisodeInfo"}
IA_REJECTED = {"count": 3, "rejection": "Rejected!", "file_episode_info": "empty ParsedEpisodeInfo"}
NAME_ = "Series.Title.S01E01.720p.HDTV.x264-Sonarr"
UNS = osa("C:\\Test\\Unsorted\\TV\\")  # trailing separator kept


def ia(decisions, new_download, dci=None, import_mode=None, existing_same_path=None, **kw):
    g = {"series": {"path": IA_SERIES, "profile": DEFAULT_PROFILE}, "decisions": decisions,
         "new_download": new_download, "history_for_download_id": []}
    if dci is not None:
        g["download_client_item"] = dci
    if import_mode is not None:
        g["import_mode"] = import_mode
    if existing_same_path is not None:
        g["existing_files_with_same_relative_path"] = existing_same_path
    g.update(kw)
    return g


def first(**kw):
    return [{**IA_APPROVED, "count": 1, **kw}]


def ofp(path, dci_output=None, folder_title=None, dci=True):
    d = None
    if dci:
        d = {"output_path": dci_output}
    g = ia(first(path=path, **({"folder_info": {"release_title": folder_title}} if folder_title else {})), True, d)
    return g


IMPORT_APPROVED = [
    T("should_not_import_any_if_there_are_no_approved_decisions", "With only rejected decisions nothing is imported",
      ia([IA_REJECTED], False), {"imported_count": 0, "calls": {"MediaFileService.Add": 0}}),
    T("should_import_each_approved", "Each of five approved decisions yields a result",
      ia([IA_APPROVED], False, existing_same_path=0), {"result_count": 5}),
    T("should_only_import_approved", "Of rejected and approved decisions, only the approved ones are imported",
      ia([IA_REJECTED, IA_APPROVED], False, existing_same_path=0), {"result_count": 8, "imported_count": 5}),
    T("should_only_import_each_episode_once", "A second decision for an already imported episode is not imported",
      ia([IA_APPROVED, {**IA_APPROVED, "count": 1, "same_local_episode_as": "first approved"}], False,
         existing_same_path=0),
      {"imported_count": 5}),
    T("should_move_new_downloads", "A new download is moved, not copied, into the series folder",
      ia(first(), True), {"calls": {"UpgradeEpisodeFile(copyOnly: false)": 1}}),
    T("should_publish_EpisodeImportedEvent_for_new_downloads", "Importing a new download publishes EpisodeImportedEvent",
      ia(first(), True), {"events": {"EpisodeImportedEvent": 1}}),
    T("should_not_move_existing_files", "A file that is not a new download is not moved",
      ia(first(), False, existing_same_path=0), {"calls": {"UpgradeEpisodeFile(copyOnly: false)": 0}}),
    T("should_import_larger_files_first", "Of two files for one episode, only the larger one is imported",
      ia([{**IA_APPROVED, "count": 1, "label": "file", "size": GIB},
          {"count": 1, "label": "sample", "episodes": "the same episode as 'file'",
           "path": osa(r"C:\Test\TV\30 Rock\30 Rock - S01E01 - Pilot.avi"), "quality": "Bluray-720p",
           "size": 80 * MIB}], False,
         existing_same_path=0),
      {"result_count": 2, "imported": ["file"]}),
    T("should_copy_when_cannot_move_files_downloads", "A download whose client cannot move files is copied",
      ia(first(path=IA_OUTPUT + "/30 Rock - S01E01 - Pilot.avi"), True,
         {"output_path": IA_OUTPUT, "title": "30.Rock.S01E01", "can_move_files": False}),
      {"calls": {"UpgradeEpisodeFile(copyOnly: true)": 1}}),
    T("should_use_override_importmode", "An explicit Move import mode moves even when the client cannot move files",
      ia(first(path=IA_OUTPUT + "/30 Rock - S01E01 - Pilot.avi"), True,
         {"output_path": IA_OUTPUT, "title": "30.Rock.S01E01", "can_move_files": False}, import_mode="Move"),
      {"calls": {"UpgradeEpisodeFile(copyOnly: false)": 1}}),
    T("should_use_file_name_only_for_download_client_item_without_a_job_folder",
      "A single-file download records only the file name as the original path",
      ofp(UNS + NAME_ + ".mkv", UNS + NAME_ + ".mkv"), {"original_file_path": NAME_ + ".mkv"}),
    T("should_use_folder_and_file_name_only_for_download_client_item_with_a_job_folder",
      "A download in a job folder records folder/file as the original path",
      ofp(UNS + NAME_ + "/" + NAME_ + ".mkv", UNS + NAME_), {"original_file_path": f"{NAME_}/{NAME_}.mkv"}),
    T("should_include_intermediate_folders_for_download_client_item_with_a_job_folder",
      "Intermediate folders below the job folder are kept in the original path",
      ofp(UNS + NAME_ + "/subfolder/" + NAME_ + ".mkv", UNS + NAME_),
      {"original_file_path": f"{NAME_}/subfolder/{NAME_}.mkv"}),
    T("should_use_folder_info_release_title_to_find_relative_path",
      "Without a download item, the folder's release title anchors the original path",
      ofp(UNS + NAME_ + "/subfolder/" + NAME_ + ".mkv", folder_title=NAME_, dci=False),
      {"original_file_path": f"{NAME_}/subfolder/{NAME_}.mkv"}),
    X("should_get_relative_path_when_there_is_no_grandparent_windows",
      "A file directly under a drive root records just its file name as the original path", WINDOWS),
    T("should_get_relative_path_when_there_is_no_grandparent_mono",
      "A file directly under / records just its file name as the original path",
      ofp("/" + NAME_ + ".mkv", folder_title=NAME_, dci=False), {"original_file_path": NAME_ + ".mkv"}),
    X("should_get_relative_path_when_there_is_no_grandparent_for_UNC_path",
      "A file directly under a UNC share records just its file name as the original path", WINDOWS),
    T("should_use_folder_info_release_title_to_find_relative_path_when_file_is_not_in_download_client_item_output_directory",
      "When the file is outside the download's output folder, the folder's release title anchors the original path",
      ofp(UNS + NAME_ + "/subfolder/" + NAME_ + ".mkv", osa("C:\\Test\\Unsorted\\TV-Other\\") + NAME_, NAME_),
      {"original_file_path": f"{NAME_}/subfolder/{NAME_}.mkv"}),
    T("should_delete_existing_metadata_files_with_the_same_path",
      "An existing episode file record with the same relative path is deleted (ManualOverride) before import",
      ia(first(), False, existing_same_path=1), {"calls": {"MediaFileService.Delete(ManualOverride)": 1}}),
    T("should_use_folder_info_release_title_to_find_relative_path_when_download_client_item_has_an_empty_output_path",
      "When the download has no output path, the folder's release title anchors the original path",
      ofp(UNS + NAME_ + "/subfolder/" + NAME_ + ".mkv", None, NAME_),
      {"original_file_path": f"{NAME_}/subfolder/{NAME_}.mkv"}),
    T("should_include_scene_name_with_new_downloads", "The scene name is stored on the imported episode file",
      ia(first(scene_name="Series.Title.S01E01.dvdrip-DRONE"), True),
      {"calls": {"UpgradeEpisodeFile(copyOnly: false) with SceneName Series.Title.S01E01.dvdrip-DRONE": 1}}),
]

# --- MediaFiles/EpisodeImport/ImportDecisionMakerFixture -----------------------

SPEC = {"pass1": "accept", "pass2": "accept", "pass3": "accept",
        "fail1": "reject", "fail2": "reject", "fail3": "reject"}


def idm(specs, augmentation="not arranged", files=None):
    return {"series": {"path": osa(r"C:\Test\Series"), "profile": DEFAULT_PROFILE},
            "video_files": files or [osa(r"C:\Test\Unsorted\The.Office.S03E115.DVDRip.Spanish.XviD-OSiTV.avi")],
            "specs": [{"name": s, "result": SPEC[s]} for s in specs], "augmentation": augmentation,
            **({"augmented_episodes": [{}]} if augmentation == "success" else {})}


IDM = [
    X("should_call_all_specifications", "Every import specification is evaluated once even when some reject", MOCKS),
    T("should_return_rejected_if_single_specs_fail", "A file rejected by its only specification is not approved",
      idm(["fail1"]), {"decision_count": 1, "approved": False}),
    T("should_return_rejected_if_one_of_specs_fail", "A file rejected by one of several specifications is not approved",
      idm(["pass1", "fail1", "pass2", "pass3"]), {"decision_count": 1, "approved": False}),
    T("should_return_approved_if_all_specs_pass", "A file accepted by every specification is approved",
      idm(["pass1", "pass2", "pass3"], "success"), {"decision_count": 1, "approved": True}),
    T("should_have_same_number_of_rejections_as_specs_that_failed",
      "A decision carries one rejection per rejecting specification",
      idm(["pass1", "pass2", "pass3", "fail1", "fail2", "fail3"], "success"), {"decision_count": 1, "rejection_count": 3}),
    X("should_not_blowup_the_process_due_to_failed_parse",
      "An aggregation error on one file does not stop the others", MOCKS),
    T("should_return_a_decision_when_exception_is_caught", "An aggregation error still yields a decision for the file",
      idm([], "throws", ["The.Office.S03E115.DVDRip.XviD-OSiTV"]), {"decision_count": 1},
      "No specifications are arranged (empty list)."),
]

# --- MediaFiles/EpisodeImport/Specifications ----------------------------------

AI_TITLE_PATH = osa(r"C:\Test\Unsorted\30 Rock\30.rock.s01e01.avi")


def ai(history=None, file_id=1, dci=True):
    g = {"episodes": [{"id": 1, "season": 1, "episode_file_id": file_id}], "path": AI_TITLE_PATH,
         "download_client_item": {"download_id": "DownloadId1"} if dci else None}
    if history is not None:
        g["history"] = history
    return g


def h(event, download_id, days=None):
    out = {"episode_id": 1, "event": event, "download_id": download_id}
    if days is not None:
        out["date_offset_days"] = days
    return out


AI_NOTE = "download_id values are NBuilder defaults (DownloadId1, DownloadId2, ...); episode_file_id 1 is NBuilder's."

ALREADY_IMPORTED = [
    T("should_accepted_if_download_client_item_is_null", "Without a download item the already-imported check accepts",
      ai(dci=False), {"accepted": True}),
    T("should_accept_if_episode_does_not_have_file", "An episode without a file is accepted", ai(file_id=0),
      {"accepted": True}, AI_NOTE),
    T("should_accept_if_episode_has_not_been_imported", "An episode with a grab but no import in history is accepted",
      ai([h("Grabbed", "DownloadId1")]), {"accepted": True}, AI_NOTE),
    T("should_accept_if_episode_was_grabbed_after_being_imported", "An episode grabbed again after an import is accepted",
      ai([h("Grabbed", "DownloadId1", 0), h("DownloadFolderImported", "DownloadId2", -1), h("Grabbed", "DownloadId3", -2)]),
      {"accepted": True}, AI_NOTE),
    T("should_reject_if_episode_imported_after_being_grabbed",
      "An episode whose download was already imported is rejected",
      ai([h("DownloadFolderImported", "DownloadId1", -1), h("Grabbed", "DownloadId2", -2)]), {"accepted": False}, AI_NOTE),
]


def et(title="Title1", air_offset_hours=0, existing=False):
    g = {"config": {"episode_title_required": "Always"}, "naming_requires_episode_title": True,
         "series": {"type": "Standard", "path": osa(r"C:\Test\TV\30 Rock")},
         "episodes": [{"season": 1, "title": title, "air_date_offset_hours": air_offset_hours}],
         "path": AI_TITLE_PATH}
    if existing:
        g["existing_file"] = True
    return g


ET_NOTE = "Title1 is NBuilder's default episode title."

EPISODE_TITLE = [
    T("should_reject_when_title_is_null", "A just-aired episode without a title is not imported", et(None), {"accepted": False}),
    T("should_reject_when_title_is_TBA", "A just-aired episode titled TBA is not imported", et("TBA"), {"accepted": False}),
    T("should_accept_when_file_is_in_series_folder", "A file already in the series folder skips the title check",
      et("TBA", existing=True), {"accepted": True}),
    T("should_accept_when_did_not_air_recently_but_title_is_TBA",
      "An episode that aired 7 days ago is imported even if titled TBA", et("TBA", -168), {"accepted": True}),
    X("should_accept_when_episode_title_is_not_required",
      "When the naming format has no episode title, a TBA title is accepted",
      "our standard episode format contains {Episode Title} (naming.json#standardEpisodeFormat)"),
    X("should_accept_when_episode_title_is_never_required", "With the setting 'never', the title check accepts", ALWAYS),
    X("should_accept_if_episode_title_is_required_for_bulk_season_releases_and_not_bulk_season",
      "With 'bulk season releases', a non-bulk release is accepted", ALWAYS),
    X("should_accept_if_episode_title_is_required_for_bulk_season_releases",
      "With 'bulk season releases', a season whose episodes did not all air the same day is accepted", ALWAYS),
    X("should_reject_if_episode_title_is_required_for_bulk_season_releases_and_it_is_missing",
      "With 'bulk season releases', a TBA title in a same-day season drop is rejected", ALWAYS),
    X("should_reject_if_episode_title_is_required_for_bulk_season_releases_and_some_episodes_do_not_have_air_date",
      "With 'bulk season releases', a TBA title is rejected when some season episodes lack an air date", ALWAYS),
]


def fs(size=None, free=None, min_free=0, existing=False, free_throws=False, free_unset=False):
    g = {"config": {"minimum_free_space_when_importing_mib": min_free},
         "root_folder": osa(r"C:\Test\TV"), "series_path": osa(r"C:\Test\TV") + "/30 Rock", "path": AI_TITLE_PATH}
    if size is not None:
        g["size"] = size
    if free_throws:
        g["available_space"] = "throws"
    elif not free_unset:
        g["available_space"] = free
    if existing:
        g["existing_file"] = True
    return g


FREE_SPACE = [
    T("should_reject_when_there_isnt_enough_disk_space", "A 100 MiB file is rejected with 80 MiB free",
      fs(100 * MIB, 80 * MIB), {"accepted": False}),
    T("should_reject_when_there_isnt_enough_space_for_file_plus_min_free_space",
      "A 100 MiB file is rejected with 150 MiB free when 100 MiB must stay free",
      fs(100 * MIB, 150 * MIB, 100), {"accepted": False}),
    T("should_accept_when_there_is_enough_disk_space", "A 100 MiB file is accepted with 1 GiB free",
      fs(100 * MIB, GIB), {"accepted": True}),
    T("should_use_series_paths_parent_for_free_space_check", "Free space is measured on the series folder's parent",
      fs(100 * MIB, GIB), {"accepted": True, "calls": {"GetAvailableSpace(" + osa(r"C:\Test\TV") + ")": 1}}),
    T("should_pass_if_free_space_is_null", "Unknown free space accepts the file", fs(100 * MIB, None), {"accepted": True}),
    T("should_pass_if_exception_is_thrown", "An error reading free space accepts the file",
      fs(100 * MIB, free_throws=True), {"accepted": True}),
    T("should_skip_check_for_files_under_series_folder", "A file already in the series folder skips the free-space check",
      fs(existing=True, free_unset=True), {"accepted": True, "calls": {"GetAvailableSpace(any)": 0}}),
    T("should_return_true_if_free_space_is_null", "Unknown free space accepts a file of unset size",
      fs(free=None), {"accepted": True}),
    X("should_return_true_when_skip_check_is_enabled", "With the free-space check skipped, the file is accepted",
      "skipFreeSpaceCheckWhenImporting is off (mediamanagement.json)"),
]

FULL_SEASON = [
    T("should_return_true_if_no_fileinfo_available", "A file whose name did not parse passes the full-season check",
      {"file_info": None}, {"accepted": True}),
    T("should_return_false_when_file_contains_the_full_season", "A single file named as a full season is rejected",
      {"file_info": {"full_season": True}}, {"accepted": False}),
    T("should_return_true_when_file_does_not_contain_the_full_season", "A file named as an episode passes",
      {"file_info": {"full_season": False}}, {"accepted": True}),
]

HAS_AUDIO = [
    T("should_accept_if_media_info_is_null", "A file without media info passes the audio-track check",
      {"media_info": None}, {"accepted": True}),
    T("should_reject_if_audio_stream_count_is_0", "A file with no audio streams is rejected",
      {"media_info": {"audio_stream_count": 0}}, {"accepted": False}),
    T("should_accept_if_audio_stream_count_is_0", "A file with one audio stream is accepted",
      {"media_info": {"audio_stream_count": 1}}, {"accepted": True}),
]

MF_PATH = r"C:\Test\Unsorted\Series.Title.S01E01.720p.HDTV-Sonarr\S01E05.mkv"


def pi(season=1, episodes=(5,), full=False, **kw):
    return {"season": season, "episodes": list(episodes), "full_season": full, **kw}


def eps_for(season, numbers):
    return [{"id": season * 10 + n, "season": season, "number": n} for n in numbers]


def mf(path=MF_PATH, file=None, folder=None, file_eps=None, folder_eps=None, existing=False, raw_path=False):
    g = {"path": path if raw_path else osa(path), "file_info": file if file != "default" else pi(),
         "folder_info": folder if folder != "default" else pi(episodes=(1,))}
    if file_eps is not None:
        g["episodes_for_file_info"] = file_eps
    if folder_eps is not None:
        g["episodes_for_folder_info"] = folder_eps
    if existing:
        g["existing_file"] = True
    return g


MF_NOTE = "Episode lookups return episodes with id season*10+number, as the test's GivenEpisodes builds them."
P = r"C:\Test\Unsorted\Series.Title.S01.720p.HDTV-Sonarr" "\\"

MATCHES_FOLDER = [
    T("should_be_accepted_for_existing_file", "A file already in the series folder passes the folder check",
      mf(file="default", folder="default", existing=True), {"accepted": True}),
    T("should_be_accepted_if_folder_name_is_not_parseable", "A file in an unparseable folder passes",
      mf(r"C:\Test\Unsorted\Series.Title\S01E01.mkv", file="default", folder=None), {"accepted": True}),
    T("should_be_accepted_if_file_name_is_not_parseable", "An unparseable file name passes the folder check",
      mf(r"C:\Test\Unsorted\Series.Title.S01E01\AFDAFD.mkv", file=None, folder="default"), {"accepted": True}),
    T("should_should_be_accepted_for_full_season", "A file inside a full-season folder passes",
      mf(r"C:\Test\Unsorted\Series.Title.S01\S01E01.mkv", pi(), pi(episodes=(), full=True),
         eps_for(1, [5]), eps_for(1, [1, 2, 3, 4, 5])), {"accepted": True}, MF_NOTE),
    T("should_be_accepted_if_file_and_folder_have_the_same_episode", "File and folder naming the same episode pass",
      mf(r"C:\Test\Unsorted\Series.Title.S01E01.720p.HDTV-Sonarr\S01E01.mkv", pi(episodes=(1,)), pi(episodes=(1,)),
         eps_for(1, [1]), eps_for(1, [1])), {"accepted": True}, MF_NOTE),
    T("should_be_accepted_if_file_is_one_episode_in_folder", "A file for one episode of a multi-episode folder passes",
      mf(r"C:\Test\Unsorted\Series.Title.S01E01E02.720p.HDTV-Sonarr\S01E01.mkv", pi(episodes=(1,)), pi(episodes=(1,)),
         eps_for(1, [1]), eps_for(1, [1])), {"accepted": True},
      MF_NOTE + " The folder info's episodes are set to [1] although the folder name says E01E02."),
    T("should_disregard_subfolder", "A file for episodes 5-6 in a folder for episodes 1-2 is rejected",
      mf(r"C:\Test\Unsorted\Series.Title.S01E01E02.720p.HDTV-Sonarr\S01E05E06.mkv", pi(episodes=(5, 6)),
         pi(episodes=(1, 2)), eps_for(1, [5, 6]), eps_for(1, [1, 2])), {"accepted": False}, MF_NOTE),
    T("should_be_rejected_if_file_and_folder_do_not_have_same_episode", "A file for episode 5 in a folder for episode 1 is rejected",
      mf(MF_PATH, pi(), pi(episodes=(1,)), eps_for(1, [5]), eps_for(1, [1])), {"accepted": False}, MF_NOTE),
    T("should_be_rejected_if_file_and_folder_do_not_have_the_same_episodes",
      "A two-episode file whose episodes differ from the folder's is rejected",
      mf(r"C:\Test\Unsorted\Series.Title.S01E01E02.720p.HDTV-Sonarr\S01E05E06.mkv", pi(episodes=(5, 6)),
         pi(episodes=(1, 2)), eps_for(1, [5, 6]), eps_for(1, [1, 2])), {"accepted": False}, MF_NOTE),
    T("should_be_rejected_if_file_and_folder_do_not_have_episodes_from_the_same_season",
      "A season-2 file inside a season-1 full-season folder is rejected",
      mf(P + "S02E01.mkv", pi(2, (1,)), pi(1, (1, 2), True), eps_for(2, [1]), eps_for(1, [1, 2])),
      {"accepted": False}, MF_NOTE),
    T("should_be_rejected_if_file_and_folder_do_not_have_episodes_from_the_same_partial_season",
      "A season-2 file inside a folder for season-1 episodes is rejected",
      mf(P + "S02E01.mkv", pi(2, (1,)), pi(1, (1, 2)), eps_for(2, [1]), eps_for(1, [1, 2])),
      {"accepted": False}, MF_NOTE),
    T("should_be_accepted_if_file_and_folder_have_episodes_from_the_same_season",
      "A season-1 file inside a season-1 full-season folder passes",
      mf(P + "S01E01.mkv", pi(1, (1,)), pi(1, (1, 2), True), eps_for(1, [1]), eps_for(1, [1, 2])),
      {"accepted": True}, MF_NOTE),
    T("should_be_accepted_if_both_file_and_folder_info_map_to_same_special",
      "File and folder names that both map to the same special pass",
      {"series": {"id": 1, "title": "Some Special"},
       "folder_title": "Some.Special.S12E00.WEB-DL.1080p-GoodNightTV",
       "file_title": "Some.Special.S12E00.WEB-DL.1080p-GoodNightTV.mkv",
       "special_title_parse_maps_both_to": "Some.Special.S0E100.WEB-DL.1080p-GoodNightTV.mkv",
       "episode_lookup": "episodes of the mapped title's parse, id season*10+number"},
      {"accepted": True}, "File and folder infos come from the reference parser on the titles above."),
    T("should_be_accepted_if_file_has_absolute_episode_number_and_folder_uses_standard",
      "A file with an absolute number in a standard-numbered folder for the same episode passes",
      mf(P + "S02E01.mkv", pi(1, (5,), absolute_episodes=[1]), pi(1, (1, 2)), eps_for(1, [1]), eps_for(1, [1, 2])),
      {"accepted": True}, MF_NOTE),
    T("should_be_accepted_if_scene_season_number_matches_but_season_number_does_not",
      "A file whose episode's scene season matches a full-season folder passes",
      mf(r"C:\Test\Unsorted\Series.Title.S01\S01E01.mkv", pi(), pi(episodes=(), full=True),
         [{"id": 15, "season": 5, "scene_season": 1, "number": 5}], eps_for(1, [1, 2, 3, 4, 5])),
      {"accepted": True}, MF_NOTE),
]

MG_PATH = osa(r"C:\Test\Unsorted\Series.Title.S01E01.720p.HDTV-Sonarr\S01E05.mkv")


def mg(grabbed=None, dci=True, existing=False):
    g = {"path": MG_PATH, "episodes": [1], "download_client_item": "NBuilder" if dci else None,
         "grabbed_release_episode_ids": grabbed}
    if existing:
        g["existing_file"] = True
    return g


MATCHES_GRAB = [
    T("should_be_accepted_for_existing_file", "A file already in the series folder passes the grab check",
      mg(dci=False, existing=True), {"accepted": True}),
    T("should_be_accepted_if_no_download_client_item", "Without a download item the grab check accepts",
      mg(dci=False), {"accepted": True}),
    T("should_be_accepted_if_no_grabbed_release_info", "Without grab info the check accepts", mg(), {"accepted": True}),
    T("should_be_accepted_if_file_episode_matches_single_grabbed_release_info",
      "A file for the grabbed episode passes", mg([1]), {"accepted": True}),
    T("should_be_accepted_if_file_episode_is_in_multi_episode_grabbed_release_info",
      "A file for one of the grabbed episodes passes", mg([1, 2]), {"accepted": True}),
    T("should_be_rejected_if_file_episode_does_not_match_single_grabbed_release_info",
      "A file for an episode that was not grabbed is rejected", mg([2]), {"accepted": False}),
    T("should_be_rejected_if_file_episode_is_not_in_multi_episode_grabbed_release_info",
      "A file for an episode outside a multi-episode grab is rejected", mg([2, 3]), {"accepted": False}),
]

NOT_SAMPLE = [
    T("should_return_true_for_existing_file", "A file already in the series folder is never treated as a sample",
      {"existing_file": True, "series": {"type": "Standard"}, "episodes": [{"season": 1}],
       "path": r"C:\Test\30 Rock\30.rock.s01e01.avi", "quality": "HDTV-720p"}, {"accepted": True}),
]


def nu(path=r"C:\Test\Unsorted TV\30.rock\30.rock.s01e01.avi", last_write_hours=None):
    g = {"config": {"download_client_working_folders": "_UNPACK_|_FAILED_"}, "path": osa(path), "size": 100}
    if last_write_hours is not None:
        g["last_write_offset_hours"] = last_write_hours
    return g


UNPACK = r"C:\Test\Unsorted TV\_UNPACK_30.rock\someSubFolder\30.rock.s01e01.avi"

NOT_UNPACKING = [
    T("should_return_true_if_not_in_working_folder", "A file outside a working folder is not unpacking", nu(),
      {"accepted": True}),
    X("should_return_true_when_in_old_working_folder", "A file in a working folder untouched for an hour is accepted", WINDOWS),
    T("should_return_false_if_in_working_folder_and_last_write_time_was_recent",
      "A file in an _UNPACK_ folder written just now is still unpacking", nu(UNPACK, 0), {"accepted": False}),
    T("should_return_false_if_unopacking_on_linux",
      "On Linux a file in an _UNPACK_ folder is still unpacking even if last written 5 days ago",
      nu(UNPACK, -120), {"accepted": False}),
]


def ep_file(quality, revision=1, languages=None, file_id=1):
    f = {"quality": quality, "revision": revision}
    if languages is not None:
        f["languages"] = languages
    return {"episode_file_id": file_id, "file": f}


NO_FILE = {"episode_file_id": 0, "file": None}


def up(episodes, local=q("HDTV-720p"), languages=("Spanish",), propers=None, cf=None):
    g = {"series": {"type": "Standard", "profile": dict(DEFAULT_PROFILE)},
         "local_episode": {**local, "languages": list(languages), "path": r"C:\Test\30 Rock\30.rock.s01e01.avi"},
         "episodes": episodes}
    if propers is not None:
        g["config"] = {"download_propers_and_repacks": propers}
    if cf is not None:
        g.update(cf)
    return g


EMPTY_CF = {"existing_file_custom_formats": [], "local_episode_custom_formats": []}


def cf_case(score):
    return {"profile_format_items": [{"format": "Name1", "score": score}],
            "existing_file_custom_formats": ["Name1"],
            "local_episode_custom_formats": ["Name1"], "local_episode_custom_format_score": 20}


def up_cf(file_quality, score):
    g = up([ep_file(file_quality)], q("Bluray-1080p"), propers="DoNotPrefer")
    c = cf_case(score)
    g["series"]["profile"]["format_items"] = c.pop("profile_format_items")
    g.update(c)
    return g


UP_PROPERS = "Propers are DoNotPrefer here; the outcome does not depend on it (revisions are equal or qualities differ)."
CF_NOTE = "Name1 is NBuilder's custom format name."

UPGRADE = [
    T("should_return_true_if_no_existing_episodeFile", "An episode without a file accepts any import",
      up([NO_FILE]), {"accepted": True}),
    T("should_return_true_if_no_existing_episodeFile_for_multi_episodes", "Episodes without files accept any import",
      up([NO_FILE, NO_FILE]), {"accepted": True}),
    T("should_return_true_if_upgrade_for_existing_episodeFile", "HDTV-720p replaces an SDTV file",
      up([ep_file("SDTV", languages=["Spanish"])]), {"accepted": True}),
    T("should_return_true_if_language_upgrade_for_existing_episodeFile_and_quality_is_same",
      "Same quality with a different language is accepted", up([ep_file("HDTV-720p", languages=["English"])]),
      {"accepted": True}),
    T("should_return_false_if_language_upgrade_for_existing_episodeFile_and_quality_is_worse",
      "A lower quality is rejected even with a different language", up([ep_file("Bluray-1080p", languages=["English"])]),
      {"accepted": False}),
    T("should_return_true_if_upgrade_for_existing_episodeFile_for_multi_episodes",
      "HDTV-720p replaces an SDTV file shared by two episodes", up([ep_file("SDTV")] * 2), {"accepted": True}),
    T("should_return_true_if_language_upgrade_for_existing_episodeFile_for_multi_episodes_and_quality_is_same",
      "Same quality with a different language is accepted for two episodes",
      up([ep_file("HDTV-720p", languages=["English"])] * 2), {"accepted": True}),
    T("should_return_false_if_language_upgrade_for_existing_episodeFile_for_multi_episodes_and_quality_is_worse",
      "A lower quality is rejected for two episodes even with a different language",
      up([ep_file("Bluray-1080p", languages=["English"])] * 2), {"accepted": False}),
    T("should_return_false_if_not_an_upgrade_for_existing_episodeFile", "HDTV-720p does not replace Bluray-720p",
      up([ep_file("Bluray-720p")]), {"accepted": False}),
    T("should_return_false_if_not_an_upgrade_for_existing_episodeFile_for_multi_episodes",
      "HDTV-720p does not replace a Bluray-720p file shared by two episodes", up([ep_file("Bluray-720p")] * 2),
      {"accepted": False}),
    T("should_return_false_if_not_an_upgrade_for_one_existing_episodeFile_for_multi_episode",
      "A two-episode import is rejected when it is not an upgrade for one of the episodes' files",
      up([ep_file("SDTV", file_id=1), ep_file("Bluray-720p", file_id=2)]), {"accepted": False}),
    T("should_return_false_if_not_a_revision_upgrade_and_prefers_propers",
      "With propers preferred, a v1 file does not replace a v2 file of the same quality",
      up([ep_file("HDTV-720p", 2, ["Spanish"])], propers="PreferAndUpgrade"), {"accepted": False}),
    T("should_return_true_if_it_is_a_preferred_word_downgrade_and_language_downgrade_and_a_quality_upgrade",
      "Bluray-2160p replaces Bluray-1080p regardless of formats and language",
      up([ep_file("Bluray-1080p", languages=["French"])], q("Bluray-2160p"), propers="DoNotPrefer",
         cf={**EMPTY_CF, "local_episode_file_info": "NBuilder"}), {"accepted": True}, UP_PROPERS),
    T("should_return_true_if_it_is_a_preferred_word_downgrade_but_a_language_upgrade",
      "Same quality and equal format scores with a different language is accepted",
      up([ep_file("Bluray-1080p", languages=["English"])], q("Bluray-1080p"), propers="DoNotPrefer",
         cf={**EMPTY_CF, "local_episode_file_info": "NBuilder"}), {"accepted": True}, UP_PROPERS),
    X("should_return_true_if_not_a_revision_upgrade_and_does_not_prefer_propers",
      "With propers not preferred, a v1 file may replace a v2 file of the same quality", PROPERS),
    X("should_return_true_when_comparing_to_a_lower_quality_proper",
      "With propers not preferred, a v1 file may replace a v2 file of the same quality (Bluray-1080p)", PROPERS),
    T("should_return_true_if_it_is_a_preferred_word_upgrade", "Same quality with equal (empty) format scores is accepted",
      up([ep_file("Bluray-1080p")], q("Bluray-1080p"), propers="DoNotPrefer",
         cf={**EMPTY_CF, "local_episode_file_info": "NBuilder"}), {"accepted": True}, UP_PROPERS),
    T("should_return_true_if_it_has_an_equal_preferred_word_score", "Same quality with an equal format score is accepted",
      up([ep_file("Bluray-1080p")], q("Bluray-1080p"), propers="DoNotPrefer",
         cf={**EMPTY_CF, "local_episode_file_info": "NBuilder"}), {"accepted": True}, UP_PROPERS),
    T("should_return_true_if_episode_file_is_null", "Episodes whose file record is missing accept the import",
      up([{"episode_file_id": 1, "file": None}] * 2), {"accepted": True}),
    T("should_return_true_if_upgrade_to_custom_format_score",
      "Same quality with a higher format score (20 over 10) is accepted", up_cf("Bluray-1080p", 10), {"accepted": True},
      CF_NOTE + " " + UP_PROPERS),
    T("should_return_true_if_not_upgrade_to_custom_format_score_but_is_upgrade_to_quality",
      "A quality upgrade is accepted even with a lower format score (20 under 50)", up_cf("Bluray-720p", 50),
      {"accepted": True}, CF_NOTE + " " + UP_PROPERS),
    T("should_return_false_if_not_upgrade_to_custom_format_score",
      "Same quality with a lower format score (20 under 50) is rejected", up_cf("Bluray-1080p", 50), {"accepted": False},
      CF_NOTE + " " + UP_PROPERS),
]

# --- MediaFiles/MediaFileDeletionService/DeleteEpisodeFileFixture --------------


def de(root_exists=False, root_has_folders=False, series_folder_exists=False, file_exists=False, delete_throws=False):
    g = {"root_folder": r"C:\Test\TV", "series_folder": "Series Title", "episode_file": {"relative_path": "Series Title - S01E01"},
         "root_folder_exists": root_exists, "root_folder_has_folders": root_has_folders,
         "series_folder_exists": series_folder_exists, "file_exists": file_exists}
    if delete_throws:
        g["delete_file_throws"] = "IOException"
    return g


DELETE = [
    T("should_throw_if_root_folder_does_not_exist", "Deleting a file fails when the root folder is missing",
      de(), {"throws": "NzbDroneClientException"}),
    T("should_should_throw_if_root_folder_is_empty", "Deleting a file fails when the root folder is empty (unmounted)",
      de(True), {"throws": "NzbDroneClientException"}),
    T("should_delete_from_db_if_series_folder_does_not_exist",
      "With the series folder missing, only the database record is removed",
      de(True, True), {"calls": {"MediaFileService.Delete(Manual)": 1, "RecycleBin.DeleteFile(file)": 0}}),
    T("should_delete_from_db_if_episode_file_does_not_exist", "With the file missing, only the database record is removed",
      de(True, True, True), {"calls": {"MediaFileService.Delete(Manual)": 1, "RecycleBin.DeleteFile(file)": 0}}),
    T("should_delete_from_disk_and_db_if_episode_file_exists", "An existing file is deleted from disk and database",
      de(True, True, True, True),
      {"calls": {"RecycleBin.DeleteFile(file, 'Series Title')": 1, "MediaFileService.Delete(Manual)": 1}}),
    T("should_handle_error_deleting_episode_file", "A disk error while deleting keeps the database record and fails",
      de(True, True, True, True, True),
      {"throws": "NzbDroneClientException",
       "calls": {"RecycleBin.DeleteFile(file, 'Series Title')": 1, "MediaFileService.Delete(Manual)": 0}}),
]

# --- MediaFiles/MediaFileServiceTests/FilterFixture ---------------------------

F = [osa("C:\\file1.avi"), osa("C:\\file2.avi"), osa("C:\\file3.avi")]


def flt(files, existing):
    return {"series": {"id": 10, "path": osa("C:\\")}, "files": files, "existing_relative_paths": existing}


FILTER = [
    T("filter_should_return_all_files_if_no_existing_files", "With no known files, every candidate is new",
      flt(F, []), {"result_equivalent_to": F}),
    T("filter_should_return_none_if_all_files_exist", "When every candidate is already known, none is new",
      flt(F, ["file1.avi", "file2.avi", "file3.avi"]), {"result": []}),
    T("filter_should_return_none_existing_files", "A known file is filtered out of the candidates",
      flt(F, ["file2.avi"]), {"count": 2, "excludes": [osa("C:\\file2.avi")]}),
    X("filter_should_return_none_existing_files_ignoring_case", "Known files match candidates case-insensitively", WINDOWS),
    T("filter_should_return_none_existing_files_not_ignoring_case",
      "On Linux, a known file does not match a candidate that differs only in case",
      flt([F[0], osa("C:\\FILE2.avi"), F[2]], ["file2.avi"]), {"count": 3}),
    T("filter_should_not_change_casing", "Filtering keeps the candidate's casing",
      flt([osa("C:\\FILE1.avi")], []), {"count": 1, "contains": [osa("C:\\FILE1.avi")],
                                        "excludes": [osa("C:\\FILE1.avi").lower()]}),
]

# --- MediaFiles/UpgradeMediaFileServiceFixture --------------------------------

ONE = [{"episode_file_id": 1, "file": {"id": 1, "relative_path": r"Season 01\30.rock.s01e01.avi"}}]
TWO_SAME = ONE * 2
TWO_DIFF = [{"episode_file_id": 1, "file": {"id": 1, "relative_path": r"Season 01\30.rock.s01e01.avi"}},
            {"episode_file_id": 2, "file": {"id": 2, "relative_path": r"Season 01\30.rock.s01e02.avi"}}]


def um(episodes, files_exist=True, root_exists=True):
    return {"series_path": osa(r"C:\Test\TV\Series"), "root_folder_exists": root_exists, "files_exist": files_exist,
            "episodes": episodes, "new_episode_file": "NBuilder"}


UM_NOTE = ("Old files go through the recycle-bin provider's DeleteFile; with no recycle bin configured "
           "(mediamanagement.json#recycleBin) that is a permanent delete.")

UPGRADE_MEDIA_FILE = [
    T("should_delete_single_episode_file_once", "Upgrading one episode deletes its old file once",
      um(ONE), {"calls": {"RecycleBin.DeleteFile": 1}}, UM_NOTE),
    T("should_delete_the_same_episode_file_only_once", "An old file shared by two episodes is deleted once",
      um(TWO_SAME), {"calls": {"RecycleBin.DeleteFile": 1}}, UM_NOTE),
    T("should_delete_multiple_different_episode_files", "Two different old files are each deleted",
      um(TWO_DIFF), {"calls": {"RecycleBin.DeleteFile": 2}}, UM_NOTE),
    T("should_delete_episode_file_from_database", "The replaced file's record is deleted with reason Upgrade",
      um(ONE), {"calls": {"MediaFileService.Delete(Upgrade)": 1}}),
    T("should_delete_existing_file_fromdb_if_file_doesnt_exist",
      "A replaced file missing on disk still has its record deleted (Upgrade)",
      um(ONE, files_exist=False), {"calls": {"MediaFileService.Delete(old file, Upgrade)": 1}}),
    T("should_not_try_to_recyclebin_existing_file_if_file_doesnt_exist", "A replaced file missing on disk is not deleted again",
      um(ONE, files_exist=False), {"calls": {"RecycleBin.DeleteFile": 0}}),
    T("should_return_old_episode_file_in_oldFiles", "The replaced file is returned in OldFiles", um(ONE),
      {"old_files_count": 1}),
    T("should_return_old_episode_files_in_oldFiles", "Both replaced files are returned in OldFiles", um(TWO_DIFF),
      {"old_files_count": 2}),
    T("should_throw_if_there_are_existing_episode_files_and_the_root_folder_is_missing",
      "With the root folder missing, the upgrade fails and no record is deleted",
      um(ONE, root_exists=False), {"throws": "RootFolderNotFoundException",
                                   "calls": {"MediaFileService.Delete(old file, Upgrade)": 0}}),
    T("should_import_if_existing_file_doesnt_exist_in_db", "An episode whose file record is missing imports without deleting",
      um([{"episode_file_id": 1, "file": None}]), {"calls": {"MediaFileService.Delete(any)": 0}}),
]

# --- NzbDrone.Common.Test/DiskTests/DiskTransferServiceFixture ----------------

SRC = osa(r"C:\source\my.video.mkv")
DST = osa(r"C:\target\my.video.mkv")
UPPER = osa(r"C:\source") + "/MY.VIDEO.MKV"
BACKUP = SRC + ".backup~"


def tf(mode, target=DST, overwrite=None, faults=None, mounts=None, existing=None, on_call=None):
    g = {"source": SRC, "target": target, "mode": mode,
         "mounts": mounts or {"source": {"root": osa(r"C:\source")}, "target": {"root": osa(r"C:\target")}},
         "existing_files": {SRC: 1000, **(existing or {})}}
    if overwrite is not None:
        g["overwrite"] = overwrite
    if faults:
        g["faults"] = faults
    if on_call:
        g["on_call"] = on_call
    return g


TF_NOTE = ("Emulated disk: Copy/Move/Delete update existence; files are 1000 bytes unless stated; "
           "verification compares source and target sizes.")
RENAME = {f"MoveFile({SRC}, {BACKUP}, true)": "source becomes backup"}
PARTIAL = "target appears with 900 bytes"

TRANSFER_FILE = [
    T("should_throw_if_path_is_the_same", "Transferring a file onto itself fails without touching it",
      tf("HardLink", SRC), {"throws": "IOException", "calls": {f"TryCreateHardLink({SRC}, {SRC})": 0}}, TF_NOTE),
    T("should_throw_if_different_casing_unless_moving", "Hardlinking to the same path in different case fails",
      tf("HardLink", UPPER), {"throws": "IOException"}, TF_NOTE),
    T("should_rename_via_temp_if_different_casing", "A case-only rename moves through a temporary backup name",
      tf("Move", UPPER, on_call=RENAME), {"calls": {f"MoveFile({BACKUP}, {UPPER}, false)": 1}}, TF_NOTE),
    T("should_rollback_rename_via_temp_on_exception", "A failed case-only rename restores the original file",
      tf("Move", UPPER, on_call=RENAME, faults={f"MoveFile({BACKUP}, {UPPER}, false)": "IOException"}),
      {"throws": "IOException", "calls": {f"MoveFile({BACKUP}, {SRC}, false)": 1}}, TF_NOTE),
    T("should_log_error_if_rollback_move_fails", "A failed case-only rename whose rollback also fails still raises",
      tf("Move", UPPER, on_call=RENAME, faults={f"MoveFile({BACKUP}, {UPPER}, false)": "IOException",
                                                 f"MoveFile({BACKUP}, {SRC}, false)": "IOException"}),
      {"throws": "IOException"}, TF_NOTE + " The test also expects one logged error."),
    T("should_throw_if_destination_is_child_of_source", "Moving a file into a path below itself fails",
      tf("Move", SRC + "/child"), {"throws": "IOException"}, TF_NOTE),
    T("should_hardlink_only", "A successful hardlink reports mode HardLink", tf("HardLink", on_call={
        f"TryCreateHardLink({SRC}, {DST})": "succeeds, target appears"}), {"result": "HardLink"}, TF_NOTE),
    T("should_throw_if_hardlink_only_failed", "HardLink mode fails when the hardlink cannot be made",
      tf("HardLink", faults={"TryCreateHardLink(any, any)": "returns false"}), {"throws": "IOException"}, TF_NOTE),
    T("should_use_copy_delete_on_cifs", "Moving onto a CIFS mount copies then deletes the source",
      tf("Move", mounts={"source": {"root": osa(r"C:\source"), "drive_format": "ext4"},
                         "target": {"root": osa(r"C:\target"), "drive_format": "cifs"}}),
      {"calls": {f"MoveFile({SRC}, {DST}, false)": 0, f"CopyFile({SRC}, {DST}, false)": 1, f"DeleteFile({SRC})": 1}},
      TF_NOTE),
    T("should_use_move_on_cifs_if_same_mount", "Moving within one CIFS mount is a plain move",
      tf("Move", mounts={"source": {"root": osa(r"C:\source"), "drive_format": "cifs"}, "target": "same as source"}),
      {"calls": {f"MoveFile({SRC}, {DST}, false)": 1, f"CopyFile({SRC}, {DST}, false)": 0, f"DeleteFile({SRC})": 0}},
      TF_NOTE),
    T("should_not_verify_copy", "Copy mode copies the file", tf("Copy"), {"calls": {f"CopyFile({SRC}, {DST}, false)": 1}},
      TF_NOTE),
    T("should_not_verify_move", "Move mode moves the file", tf("Move"), {"calls": {f"MoveFile({SRC}, {DST}, false)": 1}},
      TF_NOTE),
    T("should_delete_existing_target_when_overwriting", "With overwrite, an existing target is deleted before the move",
      tf("Move", overwrite=True, existing={DST: 1000}),
      {"calls": {f"DeleteFile({DST})": 1, f"MoveFile({SRC}, {DST}, false)": 1}}, TF_NOTE),
    T("should_throw_if_existing_target_when_not_overwriting", "Without overwrite, an existing target stops the move untouched",
      tf("Move", overwrite=False, existing={DST: 1000}),
      {"throws": "DestinationAlreadyExistsException",
       "calls": {f"DeleteFile({DST})": 0, f"MoveFile({SRC}, {DST}, false)": 0}}, TF_NOTE),
    T("should_verify_copy", "A copy is verified by comparing source and target sizes",
      tf("Copy"), {"calls": {f"GetFileSize({SRC})": 1, f"GetFileSize({DST})": 1}}, TF_NOTE),
    T("should_rollback_copy_on_partial_and_throw", "A partial copy is deleted and the transfer fails",
      tf("Copy", on_call={f"CopyFile({SRC}, {DST}, false)": PARTIAL}),
      {"throws": "IOException", "calls": {f"DeleteFile({DST})": 1}}, TF_NOTE),
    T("should_log_error_if_rollback_copy_fails", "A partial copy whose cleanup fails still raises",
      tf("Copy", on_call={f"CopyFile({SRC}, {DST}, false)": PARTIAL}, faults={f"DeleteFile({DST})": "IOException"}),
      {"throws": "IOException"}, TF_NOTE + " The test also expects one logged error."),
    T("should_verify_move", "A move is verified by comparing source and target sizes",
      tf("Move"), {"calls": {f"GetFileSize({SRC})": 1, f"GetFileSize({DST})": 1}}, TF_NOTE),
    T("should_not_rollback_move_on_partial_and_throw_if_source_gone",
      "A partial move whose source is gone keeps the partial target and fails",
      tf("Move", on_call={f"MoveFile({SRC}, {DST}, false)": "source disappears, " + PARTIAL}),
      {"throws": "IOException", "calls": {f"DeleteFile({DST})": 0}}, TF_NOTE + " The test also expects one logged error."),
    T("should_rollback_move_on_partial_if_source_remains", "A partial move whose source remains deletes the partial target",
      tf("Move", on_call={f"MoveFile({SRC}, {DST}, false)": PARTIAL}),
      {"throws": "IOException", "calls": {f"DeleteFile({DST})": 1}}, TF_NOTE),
    T("should_not_rollback_move_on_partial_if_destination_already_exists",
      "A move that fails because the target already exists does not delete the target",
      tf("Move", faults={f"MoveFile({SRC}, {DST}, false)": "FileAlreadyExistsException"}),
      {"throws": "FileAlreadyExistsException", "calls": {f"DeleteFile({DST})": 0}},
      TF_NOTE + " The test first arranges a partial-target callback, then replaces that setup with the throw."),
    T("should_log_error_if_rollback_partialmove_fails", "A partial move whose cleanup fails still raises",
      tf("Move", on_call={f"MoveFile({SRC}, {DST}, false)": PARTIAL}, faults={f"DeleteFile({DST})": "IOException"}),
      {"throws": "IOException"}, TF_NOTE + " The test also expects one logged error."),
    T("should_throw_if_destination_is_readonly", "A copy onto a read-only destination fails",
      tf("Copy", faults={f"CopyFile(any, any, false)": "IOException"}), {"throws": "IOException"}, TF_NOTE),
]

FOLDER_OPS = ("folder transfer (Copy/Move/Mirror/TransferFolder) is not on the import path: imports transfer single "
              "files (EpisodeFileMovingService.cs:151); folder ops serve series-path moves, the recycle bin (none "
              "configured) and app updates")


def folder_test(method):
    return X(method, "Folder transfer: " + method.replace("_", " "), FOLDER_OPS)


# --- fixture table -------------------------------------------------------------

FIXTURES = [
    # (path relative to the repo, op, data, file note)
    (CORE + "Download/CompletedDownloadServiceTests/ImportFixture.cs", "CompletedDownloadService.Import", CDH_IMPORT, CDH_NOTE),
    (CORE + "Download/CompletedDownloadServiceTests/ProcessFixture.cs", "CompletedDownloadService.Check", CDH_PROCESS, CDH_NOTE),
    (CORE + "Extras/ExtraServiceFixture.cs", "ExtraService.ImportEpisode", EXTRA, None),
    (CORE + "MediaFiles/DownloadedEpisodesImportServiceFixture.cs", "DownloadedEpisodesImportService", DOWNLOADED_IMPORT,
     None),
    (CORE + "MediaFiles/EpisodeFileMovingServiceTests/MoveEpisodeFileFixture.cs", "EpisodeFileMovingService.MoveEpisodeFile",
     MOVE, None),
    (CORE + "MediaFiles/EpisodeImport/Aggregation/Aggregators/AggregateEpisodesFixture.cs", "AggregateEpisodes",
     AGG_EPISODES, "File and folder infos come from the reference parser on the titles given."),
    (CORE + "MediaFiles/EpisodeImport/Aggregation/Aggregators/AggregateQualityFixture.cs", "AggregateQuality",
     AGG_QUALITY, None),
    (CORE + "MediaFiles/EpisodeImport/Aggregation/Aggregators/AggregateReleaseGroupFixture.cs", "AggregateReleaseGroup",
     AGG_RELEASE_GROUP, "Parsed infos come from the reference parser on the titles given."),
    (CORE + "MediaFiles/EpisodeImport/Aggregation/Aggregators/Augmenters/Quality/AugmentQualityFromMediaInfoFixture.cs",
     "AugmentQualityFromMediaInfo", AUGMENT_MEDIAINFO, None),
    (CORE + "MediaFiles/EpisodeImport/Aggregation/Aggregators/Augmenters/Quality/AugmentQualityFromReleaseNameFixture.cs",
     "AugmentQualityFromReleaseName", AUGMENT_RELEASE_NAME, None),
    (CORE + "MediaFiles/EpisodeImport/DetectSampleFixture.cs", "DetectSample.IsSample", DETECT_SAMPLE, None),
    (CORE + "MediaFiles/EpisodeImport/GetSceneNameFixture.cs", "SceneNameCalculator.GetSceneName", SCENE_NAME, None),
    (CORE + "MediaFiles/EpisodeImport/ImportApprovedEpisodesFixture.cs", "ImportApprovedEpisodes.Import", IMPORT_APPROVED,
     None),
    (CORE + "MediaFiles/EpisodeImport/ImportDecisionMakerFixture.cs", "ImportDecisionMaker.GetImportDecisions", IDM, None),
    (CORE + "MediaFiles/EpisodeImport/Specifications/AlreadyImportedSpecificationFixture.cs",
     "AlreadyImportedSpecification", ALREADY_IMPORTED, None),
    (CORE + "MediaFiles/EpisodeImport/Specifications/EpisodeTitleSpecificationFixture.cs", "EpisodeTitleSpecification",
     EPISODE_TITLE, ET_NOTE),
    (CORE + "MediaFiles/EpisodeImport/Specifications/FreeSpaceSpecificationFixture.cs", "FreeSpaceSpecification",
     FREE_SPACE, None),
    (CORE + "MediaFiles/EpisodeImport/Specifications/FullSeasonSpecificationFixture.cs", "FullSeasonSpecification",
     FULL_SEASON, None),
    (CORE + "MediaFiles/EpisodeImport/Specifications/HasAudioTrackSpecificationFixture.cs", "HasAudioTrackSpecification",
     HAS_AUDIO, None),
    (CORE + "MediaFiles/EpisodeImport/Specifications/MatchesFolderSpecificationFixture.cs", "MatchesFolderSpecification",
     MATCHES_FOLDER, None),
    (CORE + "MediaFiles/EpisodeImport/Specifications/MatchesGrabSpecificationFixture.cs", "MatchesGrabSpecification",
     MATCHES_GRAB, None),
    (CORE + "MediaFiles/EpisodeImport/Specifications/NotSampleSpecificationFixture.cs", "NotSampleSpecification",
     NOT_SAMPLE, None),
    (CORE + "MediaFiles/EpisodeImport/Specifications/NotUnpackingSpecificationFixture.cs", "NotUnpackingSpecification",
     NOT_UNPACKING, None),
    (CORE + "MediaFiles/EpisodeImport/Specifications/UpgradeSpecificationFixture.cs", "UpgradeSpecification (import)",
     UPGRADE, "Profile qualities are QualityFixture.GetDefaultQualities() (src/NzbDrone.Core.Test/Qualities/QualityFixture.cs:66)."),
    (CORE + "MediaFiles/MediaFileDeletionService/DeleteEpisodeFileFixture.cs", "MediaFileDeletionService.DeleteEpisodeFile",
     DELETE, None),
    (CORE + "MediaFiles/MediaFileServiceTests/FilterFixture.cs", "MediaFileService.FilterExistingFiles", FILTER, None),
    (CORE + "MediaFiles/UpgradeMediaFileServiceFixture.cs", "UpgradeMediaFileService.UpgradeEpisodeFile",
     UPGRADE_MEDIA_FILE, None),
    (COMMON + "DiskTests/DiskTransferServiceFixture.cs", "DiskTransferService.TransferFile", TRANSFER_FILE, None),
]

# Fixtures in the import area that are not ported, with the config fact that rules them out.
RULED_OUT = {
    CORE + "MediaFiles/EpisodeImport/Specifications/AbsoluteEpisodeNumberSpecificationFixture.cs": STANDARD,
    CORE + "MediaFiles/EpisodeImport/Aggregation/Aggregators/AggregateLanguageFixture.cs":
        "no language custom formats: all 24 CFs are ReleaseTitle/ReleaseGroup specs (customformat.json)",
    CORE + "MediaFiles/EpisodeImport/Aggregation/Aggregators/Augmenters/Language/AugmentLanguageFromMediaInfoFixture.cs":
        "no language custom formats: all 24 CFs are ReleaseTitle/ReleaseGroup specs (customformat.json)",
    CORE + "MediaFiles/EpisodeImport/Aggregation/Aggregators/AggregateReleaseHashFixture.cs":
        "release hash is only used by the {Release Hash} naming token, absent from naming.json",
    CORE + "MediaFiles/EpisodeImport/Aggregation/Aggregators/AggregateSubtitleInfoFixture.cs": EXTRAS,
    CORE + "Extras/Others/OtherExtraServiceFixture.cs": EXTRAS,
    CORE + "Extras/Subtitles/SubtitleServiceFixture.cs": EXTRAS,
    CORE + "Extras/Metadata/Consumers/": "every metadata consumer is disabled (metadata.json)",
    CORE + "MediaFiles/DownloadedEpisodesCommandServiceFixture.cs":
        "manual DownloadedEpisodesScan command (API/script trigger); our imports run through completed download "
        "handling and useScriptImport is off (mediamanagement.json)",
    CORE + "MediaFiles/RenameEpisodeFileServiceFixture.cs": "user-triggered rename command, not part of import",
    CORE + "MediaFiles/MediaFileRepositoryFixture.cs": "database repository internals, no behaviour of ours",
    CORE + "MediaFiles/DiskScanServiceTests/ScanFixture.cs": "scan area (scan stream)",
    CORE + "MediaFiles/MediaFileTableCleanupServiceFixture.cs": "scan area (scan stream)",
    CORE + "MediaFiles/MediaInfo/": "scan area (scan stream)",
}

RE_METHOD = re.compile(r"^\s*public\s+void\s+(\w+)\s*\(")
RE_CASE = re.compile(r"^\s*\[TestCase\((.*)\)\]\s*(//.*)?$")
RE_TEST = re.compile(r"^\s*\[Test\]\s*$")


def parse_fixture(lines):
    """[(line, method, row_index or None, args or None)] in file order (1-based lines)."""
    out, cases, test_line = [], [], None
    for i, l in enumerate(lines):
        if m := RE_CASE.match(l):
            cases.append((i + 1, m.group(1)))
        elif RE_TEST.match(l):
            test_line = i + 1
        elif m := RE_METHOD.match(l):
            name = m.group(1)
            if cases:
                out += [(ln, name, k, a) for k, (ln, a) in enumerate(cases)]
            elif test_line:
                out.append((test_line, name, None, None))
            cases, test_line = [], None
    return out


def yaml_str(s):
    return json.dumps(s, ensure_ascii=False)


def read_catalogue():
    """{id: {"ref": str, "raw": {key: text}}} from the existing catalogue (blocks as written by this script)."""
    if not CATALOGUE.exists():
        return {}
    out, cur, key = {}, None, None
    for line in CATALOGUE.read_text().splitlines():
        if line.startswith("- id: "):
            cur = {"raw": {}}
            out[line[6:].strip()] = cur
            key = None
        elif cur is not None and (m := re.match(r"^  (\w+):(.*)$", line)):
            key = m.group(1)
            cur["raw"][key] = line
            if key == "ref":
                cur["ref"] = json.loads(m.group(2).strip())
        elif cur is not None and key and line.startswith("  "):
            cur["raw"][key] += "\n" + line
    return out


def corpus_name(path):
    rel = path[len(CORE):] if path.startswith(CORE) else "Common." + path[len(COMMON):]
    return rel.replace("/", ".")[:-3] + ".json"


def main():
    parsed = {}
    for path, _, _, _ in FIXTURES:
        parsed[path] = parse_fixture((REF_REPO / path).read_text(encoding="utf-8-sig").splitlines())

    # Folder-transfer tests of DiskTransferServiceFixture are catalogued out of scope.
    disk = next(f for f in FIXTURES if f[0].endswith("DiskTransferServiceFixture.cs"))
    covered = {d["method"] for d in disk[2]}
    disk[2].extend(folder_test(m) for _, m, _, _ in parsed[disk[0]] if m not in covered)

    old = read_catalogue()
    key_to_id = {}
    for eid, e in old.items():
        path, line = e["ref"][len(TAG) + 1:].rsplit(":", 1)
        hits = [(path, m, r, a) for (ln, m, r, a) in parsed.get(path, []) if ln == int(line)]
        if hits:
            key_to_id[hits[0]] = eid
    used = {int(i[len(ID_PREFIX):]) for i in old}
    next_n = max([n for n in used if ID_MIN <= n <= ID_MAX], default=ID_MIN - 1) + 1

    entries, problems, skipped = [], [], []
    for path, op, data, file_note in FIXTURES:
        by_method = {d["method"]: d for d in data}
        if len(by_method) != len(data):
            problems.append(f"{path}: duplicate method in data")
        tests = parsed[path]
        for m in sorted(set(by_method) - {t[1] for t in tests}):
            problems.append(f"{path}: data for {m} but no such test")
        cases = []
        for ln, method, row, args in tests:
            d = by_method.get(method)
            if d is None:
                problems.append(f"{path}:{ln}: no data for {method}")
                continue
            if row is None:
                if d["rows"] is not None:
                    problems.append(f"{path}:{ln}: {method} is a [Test] but data has rows")
                    continue
                behaviour, given, expect, note, why = d["behaviour"], d["given"], d["expect"], d["note"], d["why"]
            else:
                if d["rows"] is None or row >= len(d["rows"]):
                    problems.append(f"{path}:{ln}: {method} row {row} has no data")
                    continue
                want, behaviour, given, expect, note = d["rows"][row]
                why = None
                if want != args:
                    problems.append(f"{path}:{ln}: row args {args!r} != data {want!r}")
                    continue
                if row == 0 and len(d["rows"]) != sum(1 for t in tests if t[1] == method):
                    problems.append(f"{path}: {method} row count differs")
            key = (path, method, row, args)
            eid = key_to_id.get(key)
            if eid is None:
                assert next_n <= ID_MAX, "ID range exhausted"
                eid = f"{ID_PREFIX}{next_n:04d}"
                next_n += 1
            ref = f"{TAG} {path}:{ln}"
            in_scope = why is None
            entries.append((eid, behaviour, ref, in_scope))
            if not in_scope:
                skipped.append(f"{eid} {path.rsplit('/', 1)[1][:-3]}.{method}: {why}")
                continue
            note = " ".join(n for n in (file_note, note) if n) or None
            case = {"id": eid, "ref": ref, "input": {"op": op, "given": given}, "expect": expect}
            if note:
                case["note"] = note
            cases.append(case)
        out = CORPUS / corpus_name(path)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(cases, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")

    if problems:
        sys.exit("\n".join(problems))

    lines = ["# Sonarr import fixtures (completed download handling, import decisions and specifications,",
             "# aggregation, file transfer, upgrade and delete, extras), ported by parity/extract/sonarr_import.py.",
             "# Corpus: corpus/reference/sonarr/import/. Ruled-out fixtures: RULED_OUT in the script.", ""]
    seen = set()
    for eid, behaviour, ref, in_scope in entries:
        seen.add(eid)
        raw = old.get(eid, {}).get("raw", {})
        lines += [f"- id: {eid}", f"  behaviour: {yaml_str(behaviour)}", "  source: reference",
                  f"  ref: {yaml_str(ref)}", f"  in_scope: {'true' if in_scope else 'false'}",
                  raw.get("status", "  status: unmapped")]
        if "decision" in raw:
            lines.append(raw["decision"])
        lines.append(raw.get("tests", "  tests: []"))
    stale = [i for i in old if i not in seen]
    for eid in stale:  # IDs are never reused: keep entries whose test vanished, as written.
        print(f"warning: {eid} no longer matches a reference test; kept as is", file=sys.stderr)
        lines.append(f"- id: {eid}")
        lines += list(old[eid]["raw"].values())
    CATALOGUE.write_text("\n".join(lines) + "\n", encoding="utf-8")
    n_in = sum(1 for e in entries if e[3])
    print(f"{len(entries)} entries ({n_in} in scope, {len(entries) - n_in} out), {len(stale)} stale, "
          f"ids {min(e[0] for e in entries)}..{max(e[0] for e in entries)}")
    for s in skipped:
        print("skipped:", s)


if __name__ == "__main__":
    main()
