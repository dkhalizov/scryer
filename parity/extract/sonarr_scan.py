"""Port Sonarr disk-scan fixtures (series folder scan and path filtering, DB cleanup of
missing files, media info refresh, ffprobe reading and HDR detection, media-info
formatting) into the parity corpus and catalogue (ledger D25, IDs SON-SCAN-0001..0999).

Scope: the MediaFiles fixtures not already ported by parity/extract/sonarr_import.py
(grep of parity/catalogue/import.sonarr.yaml refs), limited to settings our Sonarr
config uses (corpus/captured/sonarr/config/mediamanagement.json, rootfolder.json).
Inside a ported fixture every [Test] and [TestCase] row gets a catalogue entry; tests
of a setting value we do not use are entries with in_scope false and no corpus case.
Ruled-out fixtures are listed in RULED_OUT with the config fact. Scope also covers
ProviderTests/DiskScanProviderTests (GetVideoFiles/FilterPaths), which sits outside MediaFiles/.

Language rulings: the episode-file API also reports each file's languages, but language
aggregation (AggregateLanguageFixture, AugmentLanguageFromMediaInfoFixture) stays ruled out as
in the import stream, because no custom format reads a language, so nothing we decide depends on
it. Media-info formatting is kept because the API reports the strings themselves. If Phase 3
compares API languages, revisit both streams together.

Media-info formatting is in scope although our naming uses only {Quality Full}
(naming.json) and no notification exists (notification.json is empty): the episode
file API (Sonarr.Api.V3/EpisodeFiles/MediaInfoResource.cs:39-48) reports these
formatted values, and Phase 3 captures each file's media info from that API.

Hand-translated, same machinery as sonarr_import.py: the script parses the reference
fixtures for every [Test] and [TestCase] row (refs carry exact lines), fails if any
test has no data here or any data has no test, checks every row's C# argument text
against the `args` written here, and writes:

  parity/corpus/reference/sonarr/scan/<path under the test project>.json
  parity/catalogue/scan.sonarr.yaml

IDs are stable: an existing entry's ref line is re-read at the pinned tag to get its
key (fixture path, method, row index, C# row arguments); keys found again keep their
ID, new keys get the next free ID. Status, decision and tests of existing entries are
kept as written.

Conventions for `given` (reference terms; a later phase maps them to Scryer):
- Fields a test does not arrange take the reference default (mocked IConfigService
  returns false/0: create_empty_series_folders false, delete_empty_folders false,
  which match our config).
- Paths wrapped in AsOsAgnostic() are written as on Linux ("C:\\a\\b" becomes "/C/a/b").
- NBuilder values a test relies on are written out (ids 1..n, strings "<Property><n>").
- expect "calls" maps a reference call (as written in the test's Verify, shortened)
  to its asserted count; "warnings" is ExpectedWarns.

Run: python3 -I parity/extract/sonarr_scan.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REF_REPO = Path.home() / "PycharmProjects/arr-reference/sonarr@v4.0.20.3014"
TAG = "Sonarr@v4.0.20.3014"
CATALOGUE = ROOT / "parity/catalogue/scan.sonarr.yaml"
CORPUS = ROOT / "parity/corpus/reference/sonarr/scan"
ID_PREFIX, ID_MIN, ID_MAX = "SON-SCAN-", 1, 999

CORE = "src/NzbDrone.Core.Test/"
MF = CORE + "MediaFiles/"

CREATE_EMPTY = "createEmptySeriesFolders is off (mediamanagement.json#createEmptySeriesFolders)"
MEDIAINFO_ON = "enableMediaInfo is on (mediamanagement.json#enableMediaInfo)"


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


# --- MediaFiles/DiskScanServiceTests/ScanFixture -------------------------------

ROOT_FOLDER = osa(r"C:\Test\TV")
OTHER = osa(r"C:\Test\TV\OtherSeries")
SERIES = osa(r"C:\Test\TV\Series")
SCAN_NOTE = ("Root folder for the series is /C/Test/TV (GetBestRootFolderPath); folders not listed as existing "
             "do not exist; the series has no episode files in the DB. GetFiles returns files_on_disk for every "
             "call (video and non-video listing). GetImportDecisions(l => l.Count == n, ...) means it was called "
             "with a list of n files.")


def scan(subfolders=None, files=None, series_path=SERIES, **config):
    g = {"series_path": series_path, "root_folder": ROOT_FOLDER}
    if subfolders is None:
        g["root_folder_exists"] = False
    else:
        g.update(root_folder_exists=True, root_folder_subfolders=subfolders)
    if files is not None:
        g["files_on_disk"] = [f"{series_path}/{f}" for f in files]
    if config:
        g["config"] = config
    return g


def decisions(n):
    return {"calls": {f"GetImportDecisions(l => l.Count == {n}, series, false)": 1}}


NOT_SCANNED = {"warnings": 1, "calls": {"GetFiles(series.Path, true)": 0, "CreateFolder(series.Path)": 0,
                                        "MediaFileTableCleanupService.Clean": 0}}
TIGOLE = "Series Title S01E01 (1080p BluRay x265 10bit Tigole).mkv"

SCAN = [
    T("should_not_scan_if_root_folder_does_not_exist",
      "A series whose folder and root folder are both missing is not scanned, created or cleaned, with a warning",
      scan(), NOT_SCANNED),
    T("should_not_scan_if_series_root_folder_is_empty",
      "A series whose folder is missing under an empty root folder is not scanned, created, cleaned or imported",
      scan(subfolders=[]),
      {"warnings": 1, "calls": {**NOT_SCANNED["calls"], "GetImportDecisions(any, series, false)": 0}}),
    X("should_create_if_series_folder_does_not_exist_but_create_folder_enabled",
      "A missing series folder is created when create-empty-series-folders is on", CREATE_EMPTY),
    T("should_not_create_if_series_folder_does_not_exist_and_create_folder_disabled",
      "A missing series folder is not created when create-empty-series-folders is off",
      scan(subfolders=[OTHER], create_empty_series_folders=False), {"calls": {"CreateFolder(series.Path)": 0}}),
    T("should_clean_but_not_import_if_series_folder_does_not_exist",
      "A missing series folder under a non-empty root folder cleans the DB of the series' files and imports nothing",
      scan(subfolders=[OTHER]),
      {"calls": {"FolderExists(series.Path)": 1, "MediaFileTableCleanupService.Clean": 1,
                 "GetImportDecisions(any, series, false)": 0}}),
    T("should_not_scan_various_extras_subfolders",
      "Video files under Behind the Scenes, Deleted Scenes, Featurettes, Interviews, Samples, Scenes, Shorts, "
      "Trailers and Other subfolders are not scanned",
      scan([SERIES], [f"{d}/file{i}.mkv" for i, d in enumerate(
          ["Behind the Scenes", "Deleted Scenes", "Featurettes", "Interviews", "Samples", "Scenes", "Shorts",
           "Trailers", "Other"], 1)] + [TIGOLE]), decisions(1)),
    T("should_not_scan_featurettes_subfolders", "Video files in a Featurettes subfolder are not scanned",
      scan([SERIES], ["Featurettes/An Epic Reborn.mkv", "Featurettes/Deleted & Alternate Scenes.mkv",
                      "Featurettes/En Garde - Multi-Angle Dailies.mkv",
                      "Featurettes/Layer-By-Layer - Sound Design - Multiple Audio.mkv", TIGOLE]), decisions(1)),
    X("should_clean_but_not_import_if_series_folder_does_not_exist_and_create_folder_enabled",
      "With create-empty-series-folders on, a missing series folder still cleans the DB and imports nothing",
      CREATE_EMPTY),
    T("should_find_files_at_root_of_series_folder", "Video files at the root of the series folder are scanned",
      scan([SERIES], ["file1.mkv", "s01e01.mkv"]), decisions(2)),
    T("should_not_scan_extras_subfolder",
      "An Extras subfolder is skipped in any letter case; the folder is listed once for video and once for "
      "other files",
      scan([SERIES], ["EXTRAS/file1.mkv", "Extras/file2.mkv", "EXTRAs/file3.mkv", "ExTrAs/file4.mkv",
                      "Season 1/s01e01.mkv"]),
      {"calls": {"GetFiles(any, any)": 2, "GetImportDecisions(l => l.Count == 1, series, false)": 1}}),
    T("should_not_scan_AppleDouble_subfolder", "A .AppleDouble subfolder is skipped in any letter case",
      scan([SERIES], [".AppleDouble/file1.mkv", ".appledouble/file2.mkv", "Season 1/s01e01.mkv"]), decisions(1)),
    T("should_scan_extras_series_and_subfolders",
      "A series folder named Extras is scanned; its own Extras and .AppleDouble subfolders are still skipped",
      scan([osa(r"C:\Test\TV\Extras")],
           ["Extras/file1.mkv", ".AppleDouble/file2.mkv", "Season 1/s01e01.mkv", "Season 1/s01e02.mkv",
            "Season 2/s02e01.mkv", "Season 2/s02e02.mkv"], series_path=osa(r"C:\Test\TV\Extras")), decisions(4)),
    T("should_not_scan_subfolders_that_start_with_period",
      "Subfolders starting with a period (.@__thumb in any case, .hidden) are skipped",
      scan([SERIES], [".@__thumb/file1.mkv", ".@__THUMB/file2.mkv", ".hidden/file2.mkv", "Season 1/s01e01.mkv"]),
      decisions(1)),
    T("should_scan_files_that_start_with_period", "A video file whose name starts with a period is scanned",
      scan([SERIES], ["Season 1/.s01e01.mkv"]), decisions(1)),
    T("should_not_scan_subfolder_of_season_folder_that_starts_with_a_period",
      "Period-prefixed subfolders inside a season folder are skipped",
      scan([SERIES], ["Season 1/.@__thumb/file1.mkv", "Season 1/.@__THUMB/file2.mkv", "Season 1/.hidden/file2.mkv",
                      "Season 1/.AppleDouble/s01e01.mkv", "Season 1/s01e01.mkv"]), decisions(1)),
    T("should_not_scan_Synology_eaDir", "A Synology @eaDir subfolder is skipped",
      scan([SERIES], ["@eaDir/file1.mkv", "Season 1/s01e01.mkv"]), decisions(1)),
    T("should_not_scan_thumb_folder", "A .@__thumb subfolder is skipped",
      scan([SERIES], [".@__thumb/file1.mkv", "Season 1/s01e01.mkv"]), decisions(1)),
    T("should_scan_dotHack_folder", "A series folder whose own name starts with a period (.hack) is scanned",
      scan([osa(r"C:\Test\TV\.hack")], ["Season 1/file1.mkv", "Season 1/s01e01.mkv"],
           series_path=osa(r"C:\Test\TV\.hack")), decisions(2)),
    T("should_exclude_inline_extra_files", "Files named with an extra suffix (-deleted, -other) are not scanned",
      scan([SERIES], ["Series Title S01E01.mkv", "Deleted Scenes-deleted.mkv", "The World of Pandora-other.mkv"]),
      decisions(1)),
    T("should_exclude_osx_metadata_files", "macOS resource-fork files (._ prefix) are not scanned",
      scan([SERIES], ["._24 The Status Quo Combustion.mp4", "24 The Status Quo Combustion.mkv"]), decisions(1)),
    T("should_not_scan_excluded_files",
      "Excluded files (.DS_Store, .unmanic, .unmanic.part) are neither scanned nor offered as possible extra files",
      scan([SERIES], [".DS_Store", ".unmanic", ".unmanic.part", "24 The Status Quo Combustion.mkv"]),
      {**decisions(1), "events": {"SeriesScannedEvent": {"count": 1, "possible_extra_files": 0}}}),
]

# --- MediaFiles/MediaFileTableCleanupServiceFixture ----------------------------

DELETED_PATH = "ANY FILE WITH THIS PATH IS CONSIDERED DELETED!"
LINKED = [{"id": i, "episode_file_id": i} for i in range(1, 11)]
UNLINKED = [{"id": i, "episode_file_id": 0} for i in range(1, 11)]


def db_files(rel=None):
    return [{"id": i, "relative_path": rel or f"RelativePath{i}"} for i in range(1, 11)]


def deleted_two():
    return [f if f["id"] < 9 else {**f, "relative_path": DELETED_PATH} for f in db_files()]


def on_disk(files):
    return [f"{SERIES}/{f['relative_path']}" for f in files]


def cleanup(files, episodes=LINKED, disk=None, **kw):
    return {"series_path": SERIES, "db_files": files, "episodes": episodes,
            "files_on_disk": on_disk(files) if disk is None else disk, **kw}


CLEANUP = [
    T("should_skip_files_that_exist_in_disk", "DB files that are on disk and linked to episodes leave episodes untouched",
      cleanup(db_files()), {"calls": {"UpdateEpisode(any)": 0}}),
    T("should_delete_non_existent_files", "DB files missing from disk are deleted with reason MissingFromDisk",
      cleanup(deleted_two(), disk=on_disk(deleted_two()[:8])),
      {"calls": {"Delete(e.RelativePath == DELETED_PATH, MissingFromDisk)": 2}},
      "The test picks which 2 of the 10 files get RelativePath = DELETED_PATH at random (NBuilder Random(2)); "
      "ids 9 and 10 here are one representative choice. files_on_disk is the other 8 (Clean is passed only "
      "those); FileExists is false for any path containing DELETED_PATH."),
    T("should_delete_files_that_dont_belong_to_any_episodes",
      "DB files on disk that no episode links to are deleted with reason NoLinkedEpisodes",
      cleanup(db_files("ExistingPath"), UNLINKED), {"calls": {"Delete(any, NoLinkedEpisodes)": 10}}),
    T("should_unlink_episode_when_episodeFile_does_not_exist",
      "Episodes linked to a file id absent from the DB are unlinked (episode file id set to 0)",
      cleanup([], disk=[]), {"calls": {"UpdateEpisode(e.EpisodeFileId == 0)": 10}}),
    T("should_not_update_episode_when_episodeFile_exists",
      "Episodes whose linked file is in the DB and on disk are not updated",
      cleanup(db_files("ExistingPath")), {"calls": {"UpdateEpisode(any)": 0}}),
]
CLEANUP_NOTE = "Episodes and episode files are NBuilder lists of 10 (ids 1..10; episode i links file i unless unlinked)."

# --- MediaFiles/MediaInfo/UpdateMediaInfoServiceFixture ------------------------

MI_SERIES = osa(r"C:\series")


def mi_files(n, first=None, rest=None, path=None):
    """NBuilder list: ids 1..n; path None or NBuilder "Path<i>"; first file overrides."""
    out = []
    for i in range(1, n + 1):
        f = {"id": i, "path": path(i) if path else None, "relative_path": "media.mkv", "media_info": None}
        if i == 1 and first:
            f.update(first)
        out.append(f)
    return out


def nb_path(i):
    return f"Path{i}"


def handle(files, reader="returns media info for every path", **kw):
    return {"op": "Handle", "series": {"id": 1, "path": MI_SERIES}, "config": {"enable_media_info": True}, "files": files,
            "file_exists": True, "reader": reader, **kw}


def update(file, reader="returns media info for every path", **kw):
    return {"op": "Update", "series": {"id": 1, "path": MI_SERIES}, "config": {"enable_media_info": True}, "file": file,
            "file_exists": True, "reader": reader, **kw}


MEDIA = f"{MI_SERIES}/media.mkv"
UMI_NOTE = ("Handle is Handle(SeriesScannedEvent); Update is Update(episodeFile, series). A file's path is "
            "Path if set, else series path + relative path. MINIMUM_MEDIA_INFO_SCHEMA_REVISION is 8, CURRENT is 11 "
            "(NzbDrone.Core/MediaFiles/MediaInfo/VideoFileInfoReader.cs:24).")

UPDATE_MEDIA_INFO = [
    T("should_skip_up_to_date_media_info",
      "On scan, files with current media info (schema 11) are skipped; files without media info are re-read and saved",
      handle(mi_files(3, {"media_info": {"schema_revision": 11}})),
      {"calls": {f"GetMediaInfo({MEDIA})": 2, "MediaFileService.Update(any)": 2}}),
    T("should_skip_not_yet_date_media_info",
      "On scan, files with media info at the minimum schema (8) are skipped; files without media info are re-read",
      handle(mi_files(3, {"media_info": {"schema_revision": 8}})),
      {"calls": {f"GetMediaInfo({MEDIA})": 2, "MediaFileService.Update(any)": 2}}),
    T("should_update_outdated_media_info",
      "On scan, files with media info below the minimum schema (0) are re-read and saved",
      handle(mi_files(3, {"media_info": {"schema_revision": 0}})),
      {"calls": {f"GetMediaInfo({MEDIA})": 3, "MediaFileService.Update(any)": 3}}),
    T("should_ignore_missing_files", "On scan, files that do not exist on disk are not read or saved",
      handle(mi_files(2, path=nb_path), file_exists=False),
      {"calls": {"GetMediaInfo(media.mkv)": 0, "MediaFileService.Update(any)": 0}},
      "Paths Path1, Path2 are NBuilder defaults; FileExists is not arranged and returns false."),
    T("should_continue_after_failure", "On scan, a file whose media info cannot be read does not stop the others",
      handle(mi_files(2, {"relative_path": "media2.mkv"}),
             reader=f"returns null for {MI_SERIES}/media2.mkv, media info for every other path"),
      {"calls": {f"GetMediaInfo({MEDIA})": 1, "MediaFileService.Update(any)": 1}}),
    X("should_not_update_files_if_media_info_disabled", "On scan, media info is not read when media info is disabled",
      MEDIAINFO_ON),
    X("should_not_update_if_media_info_disabled", "Update reads no media info when media info is disabled",
      MEDIAINFO_ON),
    T("should_update_media_info", "Updating a file reads its media info once and saves the file",
      update({"id": 1, "path": None, "relative_path": "media.mkv", "media_info": {"schema_revision": 3}}),
      {"calls": {f"GetMediaInfo({MEDIA})": 1, "MediaFileService.Update(episodeFile)": 1}}),
    T("should_not_update_media_info_if_new_info_is_null",
      "When media info cannot be read, the file keeps its old media info",
      update({"id": 1, "path": "Path1", "relative_path": "media.mkv", "media_info": {"schema_revision": 3}},
             reader="returns null for every path"),
      {"file_media_info_is_null": False},
      "Path Path1 is the NBuilder default, so the read is of Path1; the test mocks null for /C/series/media.mkv "
      "and the loose mock returns null for any other path too."),
    T("should_not_save_episode_file_if_new_info_is_null", "When media info cannot be read, the file is not saved",
      update({"id": 1, "path": "Path1", "relative_path": "media.mkv", "media_info": {"schema_revision": 3}},
             reader="returns null for every path"),
      {"calls": {"MediaFileService.Update(episodeFile)": 0}},
      "Path Path1 is the NBuilder default (see should_not_update_media_info_if_new_info_is_null)."),
    R("should_not_update_media_info_if_file_does_not_support_media_info", [
        (f'"{ext}"', f"A {ext} file whose media info read returns nothing is read once and not saved",
         update({"id": 1, "path": f"{MI_SERIES}/media{ext}", "relative_path": "RelativePath1", "media_info": None},
                reader=f"returns null for {MI_SERIES}/media{ext}"),
         {"calls": {f"GetMediaInfo({MI_SERIES}/media{ext})": 1, "MediaFileService.Update(episodeFile)": 0}})
        for ext in (".iso", ".m3u", ".strm")]),
]

# --- MediaFiles/MediaInfo/VideoFileInfoReaderFixture ---------------------------

SAMPLE = CORE + "Files/Media/H264_sample.mp4"
INFO = {"video_format": "h264", "video_codec_id": "avc1", "video_profile": "Constrained Baseline",
        "audio_format": "aac", "audio_codec_id": "mp4a", "audio_profile": "LC", "audio_bitrate": 125509,
        "audio_channels": 2, "audio_channel_positions": "stereo", "audio_languages": ["eng"], "height": 320,
        "run_time_seconds": 10, "scan_type": "Progressive", "subtitles": [], "video_bitrate": 193694,
        "video_fps": 24, "width": 480, "video_bit_depth": 8, "video_colour_primaries": "smpte170m",
        "video_transfer_characteristics": "bt709", "title": "Sample Title"}
INFO_UNICODE = {k: v for k, v in INFO.items() if k != "video_bit_depth"}
RUNTIME_NOTE = "run_time_seconds is TimeSpan.Seconds (the seconds component), as the test asserts."


def hdr(bit_depth, primaries, transfer, side, dovi, expected):
    args = f'{bit_depth}, "{primaries}", "{transfer}", "{side}", {"null" if dovi is None else dovi}, HdrFormat.{expected}'
    g = {"op": "GetHdrFormat", "bit_depth": bit_depth, "colour_primaries": primaries, "transfer_characteristics": transfer,
         "side_data": [s.removeprefix("FFMpegCore.") for s in side.split(",") if s]}
    if dovi is not None:
        g["dv_bl_signal_compatibility_id"] = dovi
    what = ", ".join(g["side_data"]) or "no side data"
    return (args, f"{bit_depth}-bit {primaries or 'unset'}/{transfer or 'unset'} with {what}"
                  f"{'' if dovi is None else f' (DV compatibility id {dovi})'} is HDR format {expected}",
            g, {"hdr_format": expected})


DOVI, PLUS = "FFMpegCore.DoviConfigurationRecordSideData", "FFMpegCore.HdrDynamicMetadataSpmte2094"
HDR_ROWS = [
    (8, "", "", "", None, "None"), (10, "", "", "", None, "None"), (10, "bt709", "bt709", "", None, "None"),
    (8, "bt2020", "smpte2084", "", None, "None"), (10, "bt2020", "bt2020-10", "", None, "None"),
    (10, "bt2020", "arib-std-b67", "", None, "Hlg10"), (10, "bt2020", "smpte2084", "", None, "Pq10"),
    (10, "bt2020", "smpte2084", "FFMpegCore.SideData", None, "Pq10"),
    (10, "bt2020", "smpte2084", "FFMpegCore.MasteringDisplayMetadata", None, "Hdr10"),
    (10, "bt2020", "smpte2084", "FFMpegCore.ContentLightLevelMetadata", None, "Hdr10"),
    (10, "bt2020", "smpte2084", PLUS, None, "Hdr10Plus"), (10, "bt2020", "smpte2084", DOVI, None, "DolbyVision"),
    (10, "bt2020", "smpte2084", DOVI, 1, "DolbyVisionHdr10"),
    (10, "bt2020", "smpte2084", f"{DOVI},{PLUS}", 1, "DolbyVisionHdr10Plus"),
    (10, "bt2020", "smpte2084", f"{DOVI},{PLUS}", 6, "DolbyVisionHdr10Plus"),
    (10, "bt2020", "smpte2084", DOVI, 2, "DolbyVisionSdr"), (10, "bt2020", "smpte2084", DOVI, 4, "DolbyVisionHlg"),
]

VIDEO_FILE_INFO = [
    T("get_runtime", "The sample H.264 file's runtime is read as 10 seconds",
      {"op": "GetRunTime", "file": SAMPLE}, {"run_time_seconds": 10}, RUNTIME_NOTE),
    T("get_info", "The sample H.264 file's streams are read with codec, profile, bitrates, size, fps and colour",
      {"op": "GetMediaInfo", "file": SAMPLE}, INFO, RUNTIME_NOTE),
    T("get_info_unicode", "Media info is read from a file whose name has non-ASCII characters",
      {"op": "GetMediaInfo", "file": SAMPLE, "copied_to": "<temp dir>/H264_Pok\u00e9mon.mkv"}, INFO_UNICODE,
      RUNTIME_NOTE),
    R("should_detect_hdr_correctly", [hdr(*r) for r in HDR_ROWS]),
]
VFI_NOTE = ("file is a path in the reference repo at the tag; the fixture is [DiskAccessTest] and reads it with "
            "ffprobe. HDR rows call the static GetHdrFormat(bitDepth, primaries, transfer, sideData); side data "
            "names are FFMpegCore types.")

# --- MediaFiles/MediaInfo/MediaInfoFormatterTests ------------------------------

AUDIO_ROWS = [
    ("mp2, ,  ", "droned.s01e03.swedish.720p.hdtv.x264-prince", "MP2"), ("vorbis, ,  ", "DB Super HDTV", "Vorbis"),
    ("pcm_s16le, ,  ", "DW DVDRip XviD-idTV, ", "PCM"), ("truehd, ,  ", "", "TrueHD"),
    ("truehd, ,  ", "TrueHD", "TrueHD"), ("truehd, thd+,  ", "Atmos", "TrueHD Atmos"),
    ("truehd, thd+,  ", "TrueHD.Atmos.7.1", "TrueHD Atmos"), ("truehd, thd+,  ", "", "TrueHD Atmos"),
    ("wmav1, , ", "Droned.wmv", "WMA"), ("wmav2, , ", "B.N.S04E18.720p.WEB-DL", "WMA"),
    ("opus, ,  ", "Roadkill Ep3x11 - YouTube.webm", "Opus"), ("mp3, ,  ", "climbing.mp4", "MP3"),
    ("dts, , DTS-HD MA", "DTS-HD.MA", "DTS-HD MA"), ("dts, , DTS:X", "DTS-X", "DTS-X"),
    ("dts, , DTS-HD MA", "DTS-HD.MA", "DTS-HD MA"), ("dts, , DTS-ES", "DTS-ES", "DTS-ES"),
    ("dts, , DTS-ES", "DTS", "DTS-ES"), ("dts, , DTS-HD HRA", "DTSHD-HRA", "DTS-HD HRA"), ("dts, , DTS", "DTS", "DTS"),
    ("eac3, ec+3,  ", "EAC3.Atmos", "EAC3 Atmos"), ("eac3, ,  ", "DDP5.1", "EAC3"), ("ac3, ,  ", "DD5.1", "AC3"),
    ("adpcm_ima_qt, ,  ", "Custom?", "PCM"), ("adpcm_ms, ,  ", "Custom", "PCM"),
]


def audio(pack, scene, expected):
    fmt, codec_id, profile = pack.split(", ")
    return (f'"{pack}", "{scene}", "{expected}"', f"Audio {fmt}/{codec_id or '-'}/{profile.strip() or '-'} "
            f"with scene name '{scene}' formats as '{expected}'",
            {"op": "FormatAudioCodec", "media_info": {"audio_format": fmt, "audio_codec_id": codec_id,
                                                      "audio_profile": profile}, "scene_name": scene},
            {"result": expected})


AUDIO_FORMAT = [
    R("should_format_audio_format", [audio(*r) for r in AUDIO_ROWS]),
    T("should_return_AudioFormat_by_default", "An unknown audio format is reported as its raw format name",
      {"op": "FormatAudioCodec", "media_info": {"audio_format": "Other Audio Format",
                                                "audio_codec_id": "Other Audio Codec"},
       "scene_name": "My.Series.S01E01-Sonarr"}, {"result": "Other Audio Format"}),
]

VIDEO_ROWS = [
    ("mpeg2video, ", "Droned.S01E02.1080i.HDTV.DD5.1.MPEG2-NTb", "MPEG2"), ("mpeg2video, ", "", "MPEG2"),
    ("mpeg1video, ", "The.Simpsons.S13E04.INTERNAL-ANiVCD.mpg", "MPEG"), ("vc1, WVC1", "B.N.S04E18.720p.WEB-DL", "VC1"),
    ("vc1, V_MS/VFW/FOURCC/WVC1", "", "VC1"),
    ("vc1, WMV3", "It's Always Sunny S07E13 The Gang's RevengeHDTV.XviD-2HD.avi", "VC1"),
    ("h264, V.MPEG4/ISO/AVC", "pd.2015.S03E08.720p.iP.WEBRip.AAC2.0.H264-BTW", "h264"),
    ("h264, V_MPEG4/ISO/AVC", "Resistance.2019.S01E03.1080p.RTE.WEB-DL.AAC2.0.x264-RTN", "x264"),
    ("wmv1, WMV1", "Droned.wmv", "WMV"), ("wmv2, WMV2", "Droned.wmv", "WMV"), ("mpeg4, XVID", "", "XviD"),
    ("mpeg4, DIVX", "", "DivX"), ("mpeg4, divx", "", "DivX"), ("mpeg4, DIV3", "spsm.dvdrip.divx.avi'.", "DivX"),
    ("msmpeg4, DIV3", "Exit the Dragon, Enter the Tiger (1976) 360p MPEG Audio.avi", "DivX"),
    ("msmpeg4v2, DIV3", "Exit the Dragon, Enter the Tiger (1976) 360p MPEG Audio.avi", "DivX"),
    ("msmpeg4v3, DIV3", "Exit the Dragon, Enter the Tiger (1976) 360p MPEG Audio.avi", "DivX"),
    ("vp6f, 4", "Top Gear - S12E01 - Lorries - SD TV.flv", "VP6"), ("vp6, 4", "Top Gear - S12E01 - Lorries - SD TV.flv", "VP6"),
    ("vp7, VP70", "Sweet Seymour.avi", "VP7"), ("vp8, V_VP8", "Dick.mkv", "VP8"),
    ("vp9, V_VP9", "Roadkill Ep3x11 - YouTube.webm", "VP9"),
    ("h264, x264", "Ghost Advent - S04E05 - Stanley Hotel SDTV.avi", "x264"),
    ("hevc, V_MPEGH/ISO/HEVC", "The BBT S11E12 The Matrimonial Metric 1080p 10bit AMZN WEB-DL", "h265"),
    ("mpeg4, mp4v-20", "", ""), ("mpeg4, XVID", "American.Chopper.S06E07.Mountain.Creek.Bike.DSR.XviD-KRS", "XviD"),
    ("rpza, V_QUICKTIME", "Custom", ""), ("mpeg4, FMP4", "", ""), ("mpeg4, MP42", "", ""),
    ("mpeg4, mp43", "Bubble.Guppies.S01E13.480p.WEB-DL.H.264-BTN-Custom", ""),
]
VIDEO_FALLBACK_ROWS = [
    ("h264, x264", "Some.Video.S01E01.h264", "x264"), ("hevc, x265", "Some.Video.S01E01.h265", "x265"),
    ("h264, ", "Some.Video.S01E01.x264", "x264"), ("hevc, ", "Some.Video.S01E01.x265", "x265"),
    ("h264, ", "Some.Video.S01E01", "h264"), ("hevc, ", "Some.Video.S01E01", "h265"),
]


def video(pack, scene, expected):
    fmt, codec_id = pack.split(", ")
    return (f'"{pack}", "{scene}", "{expected}"', f"Video {fmt}/{codec_id or '-'} with scene name '{scene}' "
            f"formats as '{expected}'",
            {"op": "FormatVideoCodec", "media_info": {"video_format": fmt, "video_codec_id": codec_id},
             "scene_name": scene}, {"result": expected})


VIDEO_FORMAT = [
    R("should_format_video_format", [video(*r) for r in VIDEO_ROWS]),
    R("should_format_video_format_fallbacks", [video(*r) for r in VIDEO_FALLBACK_ROWS]),
    T("should_return_VideoFormat_by_default", "An unknown video format is reported as its raw format name",
      {"op": "FormatVideoCodec", "media_info": {"video_format": "VideoCodec"}, "scene_name": None},
      {"result": "VideoCodec"}),
]

HDR_FORMATS = ["None", "Hlg10", "Pq10", "Hdr10", "Hdr10Plus", "DolbyVision"]
DYNAMIC_RANGE = [R("should_format_video_dynamic_range", [
    (f'HdrFormat.{f}, "{e}"', f"HDR format {f} formats as dynamic range '{e}'",
     {"op": "FormatVideoDynamicRange", "media_info": {"video_hdr_format": f, "schema_revision": 8}}, {"result": e})
    for f, e in zip(HDR_FORMATS, ["", "HDR", "HDR", "HDR", "HDR", "HDR"])])]
DYNAMIC_RANGE_TYPE = [R("should_format_video_dynamic_range_type", [
    (f'HdrFormat.{f}, "{e}"', f"HDR format {f} formats as dynamic range type '{e}'",
     {"op": "FormatVideoDynamicRangeType", "media_info": {"video_hdr_format": f, "schema_revision": 9}},
     {"result": e})
    for f, e in [("None", ""), ("Hlg10", "HLG"), ("Pq10", "PQ"), ("Hdr10", "HDR10"), ("Hdr10Plus", "HDR10Plus"),
                 ("DolbyVision", "DV"), ("DolbyVisionHdr10", "DV HDR10"), ("DolbyVisionHdr10Plus", "DV HDR10Plus"),
                 ("DolbyVisionHlg", "DV HLG"), ("DolbyVisionSdr", "DV SDR")]])]
FORMATTER_NOTE = "MediaInfoFormatter is static; media_info lists only the MediaInfoModel fields the test sets."

# --- ProviderTests/DiskScanProviderTests/GetVideoFilesFixture ------------------

GVF_NAMES = ["30 Rock1.mkv", "30 Rock2.avi", "30 Rock3.MP4", "30 Rock4.wMv", "movie.exe", "movie"]
GVF_NOTE = ("Test path is C:\\Test\\ (not AsOsAgnostic, written as in the test). IDiskProvider.GetFiles is mocked "
            "to return file_names joined onto the path (Path.Combine, with special_subfolder between when set) "
            "whatever the call. Video means the extension is in MediaFileExtensions.Extensions "
            "(case-insensitive: .MP4 and .wMv count).")
GVF_PATH = "C:\\Test\\"


def gvf_listing(files_recursive, files_top):
    return {"calls": {f"GetFiles(path, true)": files_recursive, "GetFiles(path, false)": files_top}}


GET_VIDEO_FILES = [
    T("should_check_all_directories", "GetVideoFiles with no flag lists the path recursively, once, and never top level only",
      {"op": "GetVideoFiles", "path": GVF_PATH}, gvf_listing(1, 0)),
    T("should_check_all_directories_when_allDirectories_is_true", "GetVideoFiles(path, true) lists recursively, once",
      {"op": "GetVideoFiles", "path": GVF_PATH, "all_directories": True}, gvf_listing(1, 0)),
    T("should_check_top_level_directory_only_when_allDirectories_is_false",
      "GetVideoFiles(path, false) lists the top level only, once",
      {"op": "GetVideoFiles", "path": GVF_PATH, "all_directories": False}, gvf_listing(0, 1)),
    T("should_return_video_files_only", "Of 6 listed files only the 4 with a video extension are returned",
      {"op": "GetVideoFiles", "path": GVF_PATH, "file_names": GVF_NAMES}, {"count": 4}),
    R("should_filter_certain_sub_folders", [
        (f'"{f}"', f"FilterPaths drops files under a '{f}' subfolder and keeps the files directly in the path",
         {"op": "FilterPaths", "path": GVF_PATH, "file_names": GVF_NAMES, "special_subfolder": f},
         {"excludes_special_subfolder_files": True, "kept_count_greater_than": 0})
        for f in ["Extras", "@eadir", "extrafanart", "Plex Versions", ".secret", ".hidden", ".unwanted"]]),
]

# --- fixture table -------------------------------------------------------------

FIXTURES = [
    # (path relative to the repo, op (None: each case's given carries it), data, file note)
    (MF + "DiskScanServiceTests/ScanFixture.cs", "DiskScanService.Scan", SCAN, SCAN_NOTE),
    (MF + "MediaFileTableCleanupServiceFixture.cs", "MediaFileTableCleanupService.Clean", CLEANUP, CLEANUP_NOTE),
    (MF + "MediaInfo/UpdateMediaInfoServiceFixture.cs", "UpdateMediaInfoService", UPDATE_MEDIA_INFO, UMI_NOTE),
    (MF + "MediaInfo/VideoFileInfoReaderFixture.cs", "VideoFileInfoReader", VIDEO_FILE_INFO, VFI_NOTE),
    (MF + "MediaInfo/MediaInfoFormatterTests/FormatAudioCodecFixture.cs", None, AUDIO_FORMAT, FORMATTER_NOTE),
    (MF + "MediaInfo/MediaInfoFormatterTests/FormatVideoCodecFixture.cs", None, VIDEO_FORMAT, FORMATTER_NOTE),
    (MF + "MediaInfo/MediaInfoFormatterTests/FormatVideoDynamicRangeFixture.cs", None, DYNAMIC_RANGE, FORMATTER_NOTE),
    (MF + "MediaInfo/MediaInfoFormatterTests/FormatVideoDynamicRangeTypeFixture.cs", None, DYNAMIC_RANGE_TYPE,
     FORMATTER_NOTE),
    (CORE + "ProviderTests/DiskScanProviderTests/GetVideoFilesFixture.cs", "DiskScanService", GET_VIDEO_FILES,
     GVF_NOTE),
]

# MediaFiles fixtures neither ported here nor in import.sonarr.yaml. Each was ruled out by the import stream
# (RULED_OUT in sonarr_import.py) for a config fact that holds for scan too, since scan runs the same import
# decision and aggregation path (DiskScanService.cs:140).
NO_LANG_CF = "no language custom formats: all CFs are ReleaseTitle/ReleaseGroup specs (customformat.json)"
RULED_OUT = {
    MF + "EpisodeImport/Aggregation/Aggregators/AggregateLanguageFixture.cs": NO_LANG_CF,
    MF + "EpisodeImport/Aggregation/Aggregators/Augmenters/Language/AugmentLanguageFromMediaInfoFixture.cs": NO_LANG_CF,
    MF + "EpisodeImport/Aggregation/Aggregators/AggregateReleaseHashFixture.cs":
        "release hash is only used by the {Release Hash} naming token, absent from naming.json",
    MF + "EpisodeImport/Aggregation/Aggregators/AggregateSubtitleInfoFixture.cs":
        "importExtraFiles is off (mediamanagement.json#importExtraFiles)",
    MF + "EpisodeImport/Specifications/AbsoluteEpisodeNumberSpecificationFixture.cs":
        "all series are type standard (series-summary.json); anime absolute numbering is unused",
    MF + "DownloadedEpisodesCommandServiceFixture.cs":
        "manual DownloadedEpisodesScan command; useScriptImport is off (mediamanagement.json) and imports run "
        "through completed download handling",
    MF + "RenameEpisodeFileServiceFixture.cs": "user-triggered rename command, not part of scan or import",
    MF + "MediaFileRepositoryFixture.cs": "database repository internals, no behaviour of ours",
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
    return path[len(CORE):].replace("/", ".")[:-3] + ".json"


def check_scope(problems):
    """Every MediaFiles fixture is ported here, in import.sonarr.yaml, or ruled out; and none twice."""
    imported = set(re.findall(r"(src/NzbDrone\.Core\.Test/MediaFiles/[^:\"]+\.cs)",
                              (ROOT / "parity/catalogue/import.sonarr.yaml").read_text()))
    ours = {f[0] for f in FIXTURES}
    for p in sorted(ours & imported):
        problems.append(f"{p}: already ported by import.sonarr.yaml")
    scanned = list((REF_REPO / MF).rglob("*.cs")) + list((REF_REPO / CORE / "ProviderTests/DiskScanProviderTests").rglob("*.cs"))
    for f in sorted(scanned):
        p = str(f.relative_to(REF_REPO))
        if "[Test" in f.read_text(encoding="utf-8-sig") and p not in ours | imported | set(RULED_OUT):
            problems.append(f"{p}: MediaFiles fixture neither ported, imported nor ruled out")


def main():
    problems = []
    check_scope(problems)
    parsed = {path: parse_fixture((REF_REPO / path).read_text(encoding="utf-8-sig").splitlines())
              for path, _, _, _ in FIXTURES}

    old = read_catalogue()
    key_to_id = {}
    for eid, e in old.items():
        path, line = e["ref"][len(TAG) + 1:].rsplit(":", 1)
        hits = [(path, m, r, a) for (ln, m, r, a) in parsed.get(path, []) if ln == int(line)]
        if hits:
            key_to_id[hits[0]] = eid
    used = {int(i[len(ID_PREFIX):]) for i in old}
    next_n = max([n for n in used if ID_MIN <= n <= ID_MAX], default=ID_MIN - 1) + 1

    entries, skipped = [], []
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
            given = dict(given)
            case_op = given.pop("op", None)
            full_op = f"{op}.{case_op}" if op and case_op else (op or case_op)
            case = {"id": eid, "ref": ref, "input": {"op": full_op, "given": given}, "expect": expect}
            if note:
                case["note"] = note
            cases.append(case)
        out = CORPUS / corpus_name(path)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(cases, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")

    if problems:
        sys.exit("\n".join(problems))

    lines = ["# Sonarr disk-scan fixtures (series folder scan, DB cleanup, media info refresh, ffprobe reading,",
             "# HDR detection, media-info formatting), ported by parity/extract/sonarr_scan.py (ledger D25).",
             "# Corpus: corpus/reference/sonarr/scan/. Ruled-out fixtures: RULED_OUT in the script.", ""]
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
    for p, why in RULED_OUT.items():
        print(f"ruled out: {p}: {why}")


if __name__ == "__main__":
    main()
