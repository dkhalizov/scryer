"""Port Radarr's import suites (MediaFiles/ and Extras/ExtraServiceFixture) to
parity/corpus/reference/radarr/import and parity/catalogue/import.radarr.yaml
(notes/11-parity-net-design.md, phase 2), limited to fixtures our config uses.

Run from anywhere:  python3 -I parity/extract/radarr_import.py
Same shape as radarr_quality.py: every ref is located in the reference source
by anchor (a wrong anchor fails loudly), [TestCase] rows are read from the
source, and hand-transcribed values are checked against the test body with
`must`. IDs are keyed by (path, test method, row index, row source text) and
preserved on re-run, with their status/decision/tests lines; an old entry's key
is read from the clone named by its ref's tag, so a tag bump keeps IDs as long
as the old clone is still on disk. Entries whose test vanished are kept as is.
Every *Fixture.cs under MediaFiles/ and Extras/ is either ported or listed in
RULED_OUT with the config fact that rules it out (checked at run time).

Paths: tests wrap Windows paths in AsOsAgnostic; we run on POSIX, so they are
given as AsOsAgnostic returns them there ('C:\\X\\y' -> '/C/X/y',
NzbDrone.Test.Common/StringExtensions.cs:8). Paths a test leaves raw stay raw.
"""
import json
import re
from pathlib import Path

TAG = "Radarr@v6.4.4.10685"
SRC = Path.home() / "PycharmProjects/arr-reference/radarr@v6.4.4.10685"
OUT = Path(__file__).resolve().parents[1]
CATALOGUE = OUT / "catalogue/import.radarr.yaml"
CORPUS = OUT / "corpus/reference/radarr/import"
PREFIX = "RAD-IMPORT-"

T = "src/NzbDrone.Core.Test/"
C = "src/NzbDrone.Core/"
_lines = {}


def lines(rel, src=SRC):
    if (src, rel) not in _lines:
        _lines[src, rel] = (src / rel).read_text(encoding="utf-8-sig").splitlines()
    return _lines[src, rel]


def method_line(rel, method):
    hits = [i for i, l in enumerate(lines(rel)) if re.search(rf"\bvoid {method}\(", l)]
    assert len(hits) == 1, (rel, method, hits)
    return hits[0]


def body(rel, method):
    """Text of the method, from its signature to its closing brace."""
    i = method_line(rel, method)
    j = i
    while lines(rel)[j] != "        }":
        j += 1
    return "\n".join(lines(rel)[i:j + 1])


def must(rel, method, *snippets):
    """Guard hand-transcribed values: each snippet occurs in the test body."""
    b = body(rel, method)
    for s in snippets:
        assert s in b, (rel, method, s)


def must_file(rel, *snippets):
    text = "\n".join(lines(rel))
    for s in snippets:
        assert s in text, (rel, s)


def test_ref(rel, method):
    """Ref of the [Test] attribute above `method`."""
    i = method_line(rel, method)
    while not lines(rel)[i].strip().startswith("[Test]"):
        i -= 1
        assert i >= 0, (rel, method)
    return f"{TAG} {rel}:{i + 1}"


def split_args(s):
    return [a.strip() for a in re.findall(r'\s*("(?:[^"\\]|\\.)*"|[^,]+)', s)]


def testcase_rows(rel, method):
    """[(ref, [args])] of the single-line [TestCase(...)] rows above `method`."""
    i = method_line(rel, method) - 1
    rows = []
    while (s := lines(rel)[i].strip()).startswith("[TestCase(") or s in ("", "[Test]"):
        if s.startswith("[TestCase("):
            rows.append((f"{TAG} {rel}:{i + 1}", split_args(re.fullmatch(r"\[TestCase\((.*)\)\]\s*(//.*)?", s)[1])))
        i -= 1
    assert rows, (rel, method)
    return rows[::-1]


def multiline_rows(rel, method):
    """[(ref, [verbatim strings])] of [TestCase(new object[] { @"..", .. })] rows above `method`."""
    i = method_line(rel, method) - 1
    starts = []
    while lines(rel)[i].strip() != "}":
        if lines(rel)[i].strip() == "[TestCase(new object[]":
            starts.append(i)
        i -= 1
    rows = []
    for s in sorted(starts):
        vals, j = [], s + 1
        while lines(rel)[j].strip() != "})]":
            vals += re.findall(r'@"([^"]*)"', lines(rel)[j])
            j += 1
        rows.append((f"{TAG} {rel}:{s + 1}", vals))
    assert rows, (rel, method)
    return rows


def line_ref(rel, pattern):
    hits = [i for i, l in enumerate(lines(rel)) if re.search(pattern, l)]
    assert len(hits) == 1, (rel, pattern, hits)
    return hits[0]


def src_at(rel, pattern):
    """'path:line' (no tag) of the one line matching pattern, for notes."""
    return f"{rel}:{line_ref(rel, pattern) + 1}"


def P(win):
    """AsOsAgnostic on POSIX."""
    if len(win) > 2 and win[1] == ":":
        win = "/" + win.replace(":", "")
    return win.replace("\\", "/")


def pj(*parts):
    """Path.Combine on POSIX (an absolute later part would reset; none here)."""
    out = parts[0]
    for p in parts[1:]:
        out = out + p if out.endswith("/") else out + "/" + p
    return out


cases = []  # dicts: file, ref, behaviour, in_scope, input?, expect?, note?


def add(file, ref, behaviour, input=None, expect=None, note=None, in_scope=True):
    assert (input is None) == (not in_scope), ref
    cases.append(dict(file=file, ref=ref, behaviour=behaviour, in_scope=in_scope,
                      input=input, expect=expect, note=note))


def skip(rel, method, behaviour):
    add(None, test_ref(rel, method), behaviour, in_scope=False)


# Quality names as Radarr declares them (Quality.cs), keyed by C# member name.
QNAME = {}
for l in lines(C + "Qualities/Quality.cs"):
    m = re.search(r"public static Quality (\w+) => new Quality\(\d+, \"([^\"]+)\"", l)
    if m and not l.strip().startswith("//"):
        QNAME[m[1]] = m[2]


def q(expr):
    return QNAME[expr.removeprefix("Quality.").removeprefix("Core.Qualities.Quality.").strip()]


QX = T + "Qualities/QualityFixture.cs"
gd = line_ref(QX, r"public static List<QualityProfileQualityItem> GetDefaultQualities")
DEFAULT_Q = []
for l in lines(QX)[gd + 4:]:
    if "};" in l:
        break
    DEFAULT_Q.append(q(re.fullmatch(r"\s*(Quality\.\w+),?", l)[1]))
assert len(DEFAULT_Q) == 22, DEFAULT_Q
DEFAULT_PROFILE = [{"quality": x, "allowed": True} for x in DEFAULT_Q]
DEFAULT_PROFILE_NOTE = f"Profile items from QualityFixture.GetDefaultQualities() ({QX}:{gd + 1}): all 22 allowed, in that order."

NB_NOTE = ("NBuilder fills unset scalars sequentially: ints get the object's index (so a built Movie has Id 1 and "
           "MovieFileId 1, i.e. HasFile), strings get '<Property><index>' (DownloadClientItem.DownloadId 'DownloadId1', "
           "history rows 'DownloadId1'..'DownloadIdN').")
MOQ_NOTE = "Unmocked IConfigService members return their default (0 / false / first enum value)."

# --- MediaFiles/MovieImport/Specifications ----------------------------------------
S = T + "MediaFiles/MovieImport/Specifications/"
SPEC = "specifications.json"

# NotUnpackingSpecification
NU = S + "NotUnpackingSpecificationFixture.cs"
must_file(NU, '.Returns("_UNPACK_|_FAILED_")', r'@"C:\Test\Unsorted TV\30.rock\30.rock.s01e01.avi"',
          r'@"C:\Test\Unsorted TV\_UNPACK_30.rock\someSubFolder\30.rock.s01e01.avi"')
NU_BASE = P(r"C:\Test\Unsorted TV\30.rock\30.rock.s01e01.avi")
NU_WORK = P(r"C:\Test\Unsorted TV\_UNPACK_30.rock\someSubFolder\30.rock.s01e01.avi")
NU_NOTE = f"On non-Windows any file under a working folder is rejected regardless of age ({src_at(C + 'MediaFiles/MovieImport/Specifications/NotUnpackingSpecification.cs', r'if \(OsInfo.IsNotWindows\)')})."
add(SPEC, test_ref(NU, "should_return_true_if_not_in_working_folder"),
    "A file not under a '_UNPACK_|_FAILED_' working folder passes the unpacking check",
    {"op": "import_spec", "given": {"spec": "NotUnpacking", "working_folders": "_UNPACK_|_FAILED_", "path": NU_BASE}},
    {"accepted": True})
skip(NU, "should_return_true_when_in_old_working_folder",
     "On Windows a file in a working folder last written over 5 minutes ago passes the unpacking check (WindowsOnly; we run on Linux)")
must(NU, "should_return_false_if_in_working_folder_and_last_write_time_was_recent", "DateTime.UtcNow);")
add(SPEC, test_ref(NU, "should_return_false_if_in_working_folder_and_last_write_time_was_recent"),
    "A file under an _UNPACK_ folder last written just now is rejected as still unpacking",
    {"op": "import_spec", "given": {"spec": "NotUnpacking", "working_folders": "_UNPACK_|_FAILED_", "path": NU_WORK,
                                    "last_write_ago_minutes": 0}},
    {"accepted": False}, NU_NOTE)
must(NU, "should_return_false_if_unopacking_on_linux", "PosixOnly();", "AddDays(-5)")
add(SPEC, test_ref(NU, "should_return_false_if_unopacking_on_linux"),
    "On Linux a file under an _UNPACK_ folder is rejected as still unpacking even when last written 5 days ago",
    {"op": "import_spec", "given": {"spec": "NotUnpacking", "working_folders": "_UNPACK_|_FAILED_", "path": NU_WORK,
                                    "last_write_ago_minutes": 5 * 24 * 60, "os": "posix"}},
    {"accepted": False}, NU_NOTE)

# FreeSpaceSpecification
FS = S + "FreeSpaceSpecificationFixture.cs"
must_file(FS, r'_rootFolder = @"C:\Test\TV".AsOsAgnostic();', 'Path.Combine(_rootFolder, "30 Rock")')
FS_MOVIE = P(r"C:\Test\TV") + "/30 Rock"
FS_NOTE = (MOQ_NOTE + " So MinimumFreeSpaceWhenImporting is 0 MB and SkipFreeSpaceCheckWhenImporting false unless the test sets them; "
           "LocalMovie.Size defaults to 0. Free space is read for the movie folder's parent "
           f"({src_at(C + 'MediaFiles/MovieImport/Specifications/FreeSpaceSpecification.cs', r'Directory.GetParent\(localMovie.Movie.Path\)')}).")


def fs_given(**kw):
    g = {"spec": "FreeSpace", "movie_path": FS_MOVIE, "existing_file": False, "minimum_free_space_mb": 0,
         "skip_free_space_check": False}
    g.update(kw)
    return {"op": "import_spec", "given": g}


for method, given, expect, beh, snips in [
    ("should_reject_when_there_isnt_enough_disk_space", dict(file_size_mb=100, free_space_mb=80), {"accepted": False, "warnings": 1},
     "A 100 MB import is rejected when only 80 MB is free", ["GivenFileSize(100.Megabytes());", "GivenFreeSpace(80.Megabytes());", "ExpectedWarns(1)"]),
    ("should_reject_when_there_isnt_enough_space_for_file_plus_min_free_space",
     dict(file_size_mb=100, free_space_mb=150, minimum_free_space_mb=100), {"accepted": False, "warnings": 1},
     "A 100 MB import is rejected when 150 MB is free and 100 MB must stay free", [".Returns(100);", "GivenFreeSpace(150.Megabytes());", "ExpectedWarns(1)"]),
    ("should_accept_when_there_is_enough_disk_space", dict(file_size_mb=100, free_space_mb=1024), {"accepted": True},
     "A 100 MB import is accepted when 1 GB is free", ["GivenFreeSpace(1.Gigabytes());"]),
    ("should_use_series_paths_parent_for_free_space_check", dict(file_size_mb=100, free_space_mb=1024),
     {"accepted": True, "free_space_read_for": P(r"C:\Test\TV")},
     "Free space is checked on the parent of the movie folder (the root folder)", ["GetAvailableSpace(_rootFolder), Times.Once()"]),
    ("should_pass_if_free_space_is_null", dict(file_size_mb=100, free_space_mb=None), {"accepted": True},
     "An import is accepted when free space cannot be determined", ["GivenFreeSpace(null);"]),
    ("should_pass_if_exception_is_thrown", dict(file_size_mb=100, free_space_mb="error"), {"accepted": True, "errors_logged": 1},
     "An import is accepted when reading free space fails", [".Throws(new TestException());", "ExpectedErrors(1)"]),
    ("should_skip_check_for_files_under_series_folder", dict(existing_file=True), {"accepted": True, "free_space_read": False},
     "A file already in the movie folder skips the free space check", ["_localMovie.ExistingFile = true;", "Times.Never()"]),
    ("should_return_true_if_free_space_is_null", dict(free_space_mb=None), {"accepted": True},
     "A zero-size import is accepted when free space cannot be determined", ["long? freeSpace = null;"]),
    ("should_return_true_when_skip_check_is_enabled", dict(skip_free_space_check=True), {"accepted": True},
     "With the free space check disabled every import passes it", ["s.SkipFreeSpaceCheckWhenImporting", ".Returns(true);"]),
]:
    must(FS, method, *snips)
    add(SPEC, test_ref(FS, method), beh, fs_given(**given), expect, FS_NOTE)

# NotSampleSpecification
NS = S + "NotSampleSpecificationFixture.cs"
must(NS, "should_return_true_for_existing_file", "ExistingFile = true;")
add(SPEC, test_ref(NS, "should_return_true_for_existing_file"),
    "A file already in the movie folder passes the sample check",
    {"op": "import_spec", "given": {"spec": "NotSample", "existing_file": True}}, {"accepted": True})

# AlreadyImportedSpecification
AI = S + "AlreadyImportedSpecificationFixture.cs"
AI_NOTE = NB_NOTE + (" The download client item is 'DownloadId1'; history only counts rows with the same DownloadId "
                     f"({src_at(C + 'MediaFiles/MovieImport/Specifications/AlreadyImportedSpecification.cs', r'h.DownloadId == downloadClientItem.DownloadId &&$')}), "
                     "so the given download_ids decide the outcome as much as the dates.")


def hist(event, did, days_ago=None):
    h = {"event": event, "download_id": did}
    if days_ago is not None:
        h["days_ago"] = days_ago
    return h


for method, given, expect, beh, snips in [
    ("should_accepted_if_download_client_item_is_null", {"download_client_item": None, "movie_has_file": True}, {"accepted": True},
     "Without a download client item the already-imported check passes", ["IsSatisfiedBy(_localMovie, null)"]),
    ("should_accept_if_episode_does_not_have_file", {"download_client_item": {"download_id": "DownloadId1"}, "movie_has_file": False},
     {"accepted": True}, "A movie with no file passes the already-imported check", ["MovieFileId = 0;"]),
    ("should_accept_if_episode_has_not_been_imported",
     {"download_client_item": {"download_id": "DownloadId1"}, "movie_has_file": True, "history": [hist("Grabbed", "DownloadId1")]},
     {"accepted": True}, "A download with only a grab in history passes the already-imported check", ["CreateListOfSize(1)"]),
    ("should_accept_if_episode_was_grabbed_after_being_imported",
     {"download_client_item": {"download_id": "DownloadId1"}, "movie_has_file": True,
      "history": [hist("Grabbed", "DownloadId1", 0), hist("DownloadFolderImported", "DownloadId2", 1), hist("Grabbed", "DownloadId3", 2)]},
     {"accepted": True}, "A download grabbed again after an import passes the already-imported check",
     ["CreateListOfSize(3)", "AddDays(-1)", "AddDays(-2)"]),
    ("should_reject_if_episode_imported_after_being_grabbed",
     {"download_client_item": {"download_id": "DownloadId1"}, "movie_has_file": True,
      "history": [hist("DownloadFolderImported", "DownloadId1", 1), hist("Grabbed", "DownloadId2", 2)]},
     {"accepted": False}, "A download already imported from this download client item is rejected",
     ["CreateListOfSize(2)", "AddDays(-1)", "AddDays(-2)"]),
]:
    must(AI, method, *snips)
    add(SPEC, test_ref(AI, method), beh, {"op": "import_spec", "given": {"spec": "AlreadyImported", **given}}, expect, AI_NOTE)

# GrabbedReleaseQualitySpecification
GR = S + "GrabbedReleaseQualityFixture.cs"
must_file(GR, "new QualityModel(Quality.Bluray720p)")
for method, given, beh, snips in [
    ("should_be_accepted_when_downloadClientItem_is_null", {"download_client_item": False},
     "Without a download client item the grabbed-quality check passes", ["IsSatisfiedBy(_localMovie, null)"]),
    ("should_be_accepted_if_no_history_for_downloadId", {"download_client_item": True, "history": []},
     "With no history for the download the grabbed-quality check passes", ["new List<MovieHistory>()"]),
    ("should_be_accepted_if_no_grabbed_history_for_downloadId", {"download_client_item": True, "history": [{"event": "Unknown"}]},
     "With no grab in history for the download the grabbed-quality check passes", ["MovieHistoryEventType.Unknown"]),
    ("should_be_accepted_if_grabbed_history_quality_is_unknown",
     {"download_client_item": True, "history": [{"event": "Grabbed", "quality": "Unknown"}]},
     "A grab of unknown quality passes the grabbed-quality check", ["new QualityModel(Quality.Unknown)"]),
    ("should_be_accepted_if_grabbed_history_quality_matches",
     {"download_client_item": True, "history": [{"event": "Grabbed", "quality": "Bluray-720p"}]},
     "A grab of the same quality as the file passes the grabbed-quality check", ["h.Quality = _localMovie.Quality"]),
    ("should_be_accepted_if_grabbed_history_quality_does_not_match_log_only",
     {"download_client_item": True, "history": [{"event": "Grabbed", "quality": "HDTV-720p"}]},
     "A grab of a different quality than the file still passes the grabbed-quality check (log only)",
     ["new QualityModel(Quality.HDTV720p)"]),
]:
    must(GR, method, *snips)
    add(SPEC, test_ref(GR, method), beh,
        {"op": "import_spec", "given": {"spec": "GrabbedReleaseQuality", "file_quality": "Bluray-720p", **given}}, {"accepted": True},
        f"The check only logs a mismatch ({src_at(C + 'MediaFiles/MovieImport/Specifications/GrabbedReleaseQualitySpecification.cs', r'Log only for info')}).")

# HasAudioTrackSpecification
HA = S + "HasAudioTrackSpecificationFixture.cs"
for method, mi, acc, beh, snip in [
    ("should_accept_if_media_info_is_null", None, True, "A file without media info passes the audio track check", "MediaInfo = null;"),
    ("should_reject_if_audio_stream_count_is_0", {"audio_stream_count": 0}, False, "A file with no audio streams is rejected", "AudioStreamCount = 0"),
    ("should_accept_if_audio_stream_count_is_0", {"audio_stream_count": 1}, True, "A file with one audio stream passes the audio track check", "AudioStreamCount = 1"),
]:
    must(HA, method, snip)
    add(SPEC, test_ref(HA, method), beh, {"op": "import_spec", "given": {"spec": "HasAudioTrack", "media_info": mi}}, {"accepted": acc})

# MatchesGrabSpecification
MG = S + "MatchesGrabSpecificationFixture.cs"
for method, given, acc, beh, snips in [
    ("should_be_accepted_for_existing_file", {"existing_file": True, "download_client_item": False, "grabbed_movie_ids": None}, True,
     "A file already in the movie folder passes the matches-grab check", ["ExistingFile = true;"]),
    ("should_be_accepted_if_no_download_client_item", {"existing_file": False, "download_client_item": False, "grabbed_movie_ids": None}, True,
     "Without grab information the matches-grab check passes", ["IsSatisfiedBy(_localMovie, null)"]),
    ("should_be_accepted_if_no_grab_release_info", {"existing_file": False, "download_client_item": True, "grabbed_movie_ids": None}, True,
     "A download with no grabbed release info passes the matches-grab check", ["GivenHistoryForMovies();"]),
    ("should_be_accepted_if_file_episode_matches_single_grab_release_info",
     {"existing_file": False, "download_client_item": True, "grabbed_movie_ids": [1]}, True,
     "A file for the movie that was grabbed passes the matches-grab check", ["GivenHistoryForMovies(_movie1);"]),
    ("should_be_rejected_if_file_episode_does_not_match_single_grab_release_info",
     {"existing_file": False, "download_client_item": True, "grabbed_movie_ids": [2]}, False,
     "A file for movie 1 from a release grabbed for movie 2 is rejected", ["GivenHistoryForMovies(_movie2);"]),
]:
    must(MG, method, *snips)
    add(SPEC, test_ref(MG, method), beh, {"op": "import_spec", "given": {"spec": "MatchesGrab", "movie_id": 1, **given}}, {"accepted": acc},
        "grabbed_movie_ids null: LocalMovie.Release is null (GivenHistoryForMovies returns early with no movies).")

# NotMultiPartSpecification
MP = T + "MediaFiles/MovieImport/Specifications/NotMultiPartSpecificationFixture.cs"
for method, acc, beh in [
    ("should_be_accepted_for_legitimate_files", True, "is not treated as multi-part"),
    ("should_be_rejected_for_multi_part_files", False, "is rejected as a multi-part movie"),
    ("should_not_reject_if_multi_part_schemes_mixed", True, "is not rejected when the folder mixes part schemes"),
    ("should_not_reject_if_file_names_mixed", True, "is not rejected when the other part has a different name"),
]:
    for ref, paths in multiline_rows(MP, method):
        files = [P(p) for p in paths]
        add(SPEC, ref, f"'{files[0].rsplit('/', 1)[1]}' alongside {len(files) - 1} other file(s) {beh}",
            {"op": "import_spec", "given": {"spec": "NotMultiPart", "path": files[0], "files_in_folder": files}},
            {"accepted": acc}, "The file is the first path of the row; the folder listing is the whole row.")

# UpgradeSpecification (import side)
UP = S + "UpgradeSpecificationFixture.cs"
must_file(UP, "Items = Qualities.QualityFixture.GetDefaultQualities()",
          "Quality = new QualityModel(Quality.HDTV720p, new Revision(version: 1))")
UP_NOTE = (DEFAULT_PROFILE_NOTE + " " + MOQ_NOTE + " So propers default to PreferAndUpgrade (ProperDownloadTypes.cs: first member). "
           "Existing custom format score: one format, scored in the profile and parsed from the existing file, so the score is that format's weight.")


def rev(v=1):
    return {"version": v, "real": 0}


for method, given, acc, beh, snips in [
    ("should_return_true_if_no_existing_episodeFile", {"movie_file_id": 0, "existing_file": None}, True,
     "A movie with no file accepts any import", ["MovieFileId = 0;"]),
    ("should_return_true_if_upgrade_for_existing_episodeFile",
     {"movie_file_id": 1, "existing_file": {"quality": "SDTV", "revision": rev()}}, True,
     "HDTV-720p replaces an existing SDTV file", ["new QualityModel(Quality.SDTV, new Revision(version: 1))"]),
    ("should_return_false_if_not_an_upgrade_for_existing_episodeFile",
     {"movie_file_id": 1, "existing_file": {"quality": "Bluray-720p", "revision": rev()}}, False,
     "HDTV-720p does not replace an existing Bluray-720p file", ["new QualityModel(Quality.Bluray720p, new Revision(version: 1))"]),
    ("should_return_false_if_not_a_revision_upgrade_and_prefers_propers",
     {"propers": "PreferAndUpgrade", "movie_file_id": 1, "existing_file": {"quality": "HDTV-720p", "revision": rev(2)}}, False,
     "With propers preferred, HDTV-720p v1 does not replace an existing HDTV-720p v2 file",
     ["ProperDownloadTypes.PreferAndUpgrade", "new QualityModel(Quality.HDTV720p, new Revision(version: 2))"]),
    ("should_return_true_if_not_a_revision_upgrade_and_does_not_prefer_propers",
     {"propers": "DoNotPrefer", "movie_file_id": 1, "existing_file": {"quality": "HDTV-720p", "revision": rev(2)}}, True,
     "With propers not preferred, HDTV-720p v1 may replace an existing HDTV-720p v2 file",
     ["ProperDownloadTypes.DoNotPrefer", "new QualityModel(Quality.HDTV720p, new Revision(version: 2))"]),
    ("should_return_true_when_comparing_to_a_lower_quality_proper",
     {"propers": "DoNotPrefer", "file_quality": "Bluray-1080p", "movie_file_id": 1,
      "existing_file": {"quality": "Bluray-1080p", "revision": rev(2)}}, True,
     "With propers not preferred, Bluray-1080p v1 may replace an existing Bluray-1080p v2 file",
     ["ProperDownloadTypes.DoNotPrefer", "new QualityModel(Quality.Bluray1080p, new Revision(version: 2))"]),
    ("should_return_true_if_movie_file_is_null", {"movie_file_id": 1, "existing_file": None}, True,
     "A movie whose file record cannot be loaded accepts the import", ["MovieFile = null;", "MovieFileId = 1;"]),
    ("should_return_true_if_upgrade_to_custom_format_score",
     {"propers": "DoNotPrefer", "file_quality": "Bluray-1080p", "file_custom_format_score": 20, "movie_file_id": 1,
      "existing_file": {"quality": "Bluray-1080p", "revision": rev(), "custom_format_score": 10}}, True,
     "Same quality with a higher custom format score (20 over 10) is an upgrade", ["Score = 10", "CustomFormatScore = 20;"]),
    ("should_return_true_if_not_upgrade_to_custom_format_score_but_is_upgrade_to_quality",
     {"propers": "DoNotPrefer", "file_quality": "Bluray-1080p", "file_custom_format_score": 20, "movie_file_id": 1,
      "existing_file": {"quality": "Bluray-720p", "revision": rev(), "custom_format_score": 50}}, True,
     "A higher quality is an upgrade even with a lower custom format score (20 under 50)",
     ["Quality = new QualityModel(Quality.Bluray720p)", "Score = 50", "CustomFormatScore = 20;"]),
    ("should_return_false_if_not_upgrade_to_custom_format_score",
     {"propers": "DoNotPrefer", "file_quality": "Bluray-1080p", "file_custom_format_score": 20, "movie_file_id": 1,
      "existing_file": {"quality": "Bluray-1080p", "revision": rev(), "custom_format_score": 50}}, False,
     "Same quality with a lower custom format score (20 under 50) is not an upgrade", ["Score = 50", "CustomFormatScore = 20;"]),
]:
    must(UP, method, *snips)
    g = {"spec": "Upgrade", "profile_items": DEFAULT_PROFILE, "propers": "PreferAndUpgrade",
         "file_quality": "HDTV-720p", "file_revision": rev()}
    g.update(given)
    add(SPEC, test_ref(UP, method), beh, {"op": "import_spec", "given": g}, {"accepted": acc}, UP_NOTE)

# --- MediaFiles/MovieImport/ImportDecisionMakerFixture ------------------------------
DM = T + "MediaFiles/MovieImport/ImportDecisionMakerFixture.cs"
DMF = "decision_maker.json"
DM_NOTE = "Specifications are stand-ins that always accept or always reject; the file is one approved-by-parse video file."
for method, specs, expect, beh, snips in [
    ("should_call_all_specifications", ["accept"] * 3 + ["reject"] * 3, {"each_spec_evaluated_times": 1},
     "Every import specification is evaluated once per file, even after one rejects", ["Times.Once()"]),
    ("should_return_rejected_if_single_specs_fail", ["reject"], {"approved": False},
     "A file is not approved when its only specification rejects it", ["GivenSpecifications(_fail1);"]),
    ("should_return_rejected_if_one_of_specs_fail", ["accept", "reject", "accept", "accept"], {"approved": False},
     "A file is not approved when one of its specifications rejects it", ["GivenSpecifications(_pass1, _fail1, _pass2, _pass3);"]),
    ("should_return_approved_if_all_specs_pass", ["accept"] * 3, {"approved": True},
     "A file is approved when every specification accepts it", ["GivenSpecifications(_pass1, _pass2, _pass3);"]),
    ("should_have_same_number_of_rejections_as_specs_that_failed", ["accept"] * 3 + ["reject"] * 3, {"rejections": 3},
     "A file carries one rejection per rejecting specification", ["HaveCount(3)"]),
]:
    must(DM, method, *snips)
    add(DMF, test_ref(DM, method), beh, {"op": "import_decisions", "given": {"files": 1, "specs": specs}}, expect, DM_NOTE)
must(DM, "should_not_blowup_the_process_due_to_failed_parse", "Times.Exactly(_videoFiles.Count)", "ExpectedErrors(3)")
add(DMF, test_ref(DM, "should_not_blowup_the_process_due_to_failed_parse"),
    "When analysing a file throws, the remaining files are still analysed",
    {"op": "import_decisions", "given": {"files": 3, "specs": ["accept"], "augmentation": "throws"}},
    {"files_analysed": 3, "errors_logged": 3})
must(DM, "should_return_a_decision_when_exception_is_caught", "HaveCount(1)", "ExpectedErrors(1)")
add(DMF, test_ref(DM, "should_return_a_decision_when_exception_is_caught"),
    "A file whose analysis throws still yields a decision",
    {"op": "import_decisions", "given": {"files": 1, "specs": [], "augmentation": "throws"}},
    {"decisions": 1, "errors_logged": 1})
skip(DM, "should_call_parsing_service_with_filename_as_simpletitle",
     "Import decisions with an unparseable file name and folder info (asserts nothing: the test only sets up mocks)")

# --- MediaFiles/MovieImport/ImportApprovedMoviesFixture --------------------------------
IA = T + "MediaFiles/MovieImport/ImportApprovedMoviesFixture.cs"
IAF = "import_approved.json"
must_file(IA, r'var outputPath = @"C:\Test\Unsorted\TV\30.Rock.S01E01".AsOsAgnostic();', r'@"C:\Test\TV\30 Rock".AsOsAgnostic()',
          '"30 Rock - S01E01 - Pilot.avi"')
IA_MOVIE = P(r"C:\Test\TV\30 Rock")
IA_OUT = P(r"C:\Test\Unsorted\TV\30.Rock.S01E01")
IA_FILE = pj(IA_MOVIE, "30 Rock - S01E01 - Pilot.avi")
IA_NOTE = ("copy_only is UpgradeMovieFile's third argument: true copies (hardlinks when enabled) instead of moving. "
           "The approved file starts at the movie folder path; GivenNewDownload moves it under the download output path.")


def approved(path=IA_FILE, **kw):
    d = {"path": path, "movie": 1}
    d.update(kw)
    return d


for method, given, expect, beh, snips in [
    ("should_not_import_any_if_there_are_no_approved_decisions", {"new_download": False, "decisions": [{"rejected": True}] * 3},
     {"imported": 0, "files_recorded": 0}, "Nothing is imported or recorded when every decision is rejected", ["Times.Never()"]),
    ("should_import_each_approved", {"new_download": False, "decisions": [approved()]}, {"results": 1},
     "Each approved decision yields one import result", ["HaveCount(1)"]),
    ("should_only_import_approved", {"new_download": False, "decisions": [{"rejected": True}] * 3 + [approved()]},
     {"results": 4, "imported": 1}, "Only approved decisions are imported; every decision yields a result",
     ["HaveCount(all.Count)", "HaveCount(_approvedDecisions.Count)"]),
    ("should_only_import_each_movie_once", {"new_download": False, "decisions": [approved(), approved()]}, {"imported": 1},
     "Two approved files for the same movie import only one", ["all.Add(new ImportDecision(_approvedDecisions.First().LocalMovie));"]),
    ("should_move_new_downloads", {"new_download": True, "decisions": [approved()]}, {"file_placed_times": 1, "copy_only": False},
     "A new download with no download client item is moved into place", ["Times.Once()"]),
    ("should_publish_MovieImportedEvent_for_new_downloads", {"new_download": True, "decisions": [approved()]},
     {"movie_file_imported_events": 1}, "Importing a new download announces one movie-file-imported event", ["Times.Once()"]),
    ("should_not_move_existing_files", {"new_download": False, "decisions": [approved()]}, {"file_placed_times": 0},
     "A file already in the library is recorded, not moved", ["Times.Never()"]),
    ("should_import_larger_files_first",
     {"new_download": False, "decisions": [approved(size_mb=1024),
                                           approved(P(r"C:\Test\TV\30 Rock\30 Rock - 2017 - Pilot.avi"), size_mb=80)]},
     {"results": 2, "imported": 1, "imported_size_mb": 1024}, "Of two approved files for one movie the larger one is imported",
     ["1.Gigabytes()", "80.Megabytes()"]),
    ("should_copy_when_cannot_move_files_downloads",
     {"new_download": True, "decisions": [approved(pj(IA_OUT, "30 Rock - S01E01 - Pilot.avi"))],
      "download_client_item": {"title": "30.Rock.S01E01", "output_path": IA_OUT, "can_move_files": False}},
     {"file_placed_times": 1, "copy_only": True}, "A download the client still needs (cannot move files) is copied, not moved",
     ["CanMoveFiles = false;", "_approvedDecisions.First().LocalMovie, true)"]),
    ("should_use_override_importmode",
     {"new_download": True, "decisions": [approved(pj(IA_OUT, "30 Rock - S01E01 - Pilot.avi"))],
      "download_client_item": {"title": "30.Rock.S01E01", "output_path": IA_OUT, "can_move_files": False}, "import_mode": "Move"},
     {"file_placed_times": 1, "copy_only": False}, "An explicit Move import mode moves even when the client cannot move files",
     ["ImportMode.Move", "_approvedDecisions.First().LocalMovie, false)"]),
    ("should_include_scene_name_with_new_downloads",
     {"new_download": True, "decisions": [approved(scene_name="Movie.Title.2022.dvdrip-DRONE")]},
     {"file_placed_times": 1, "recorded_scene_name": "Movie.Title.2022.dvdrip-DRONE"},
     "The imported movie file records the release's scene name", ['"Movie.Title.2022.dvdrip-DRONE"']),
]:
    must(IA, method, *snips)
    add(IAF, test_ref(IA, method), beh, {"op": "import_approved", "given": given}, expect, IA_NOTE)

UNS = P("C:\\Test\\Unsorted\\TV\\")
TRANS = "Transformers.2007.720p.BluRay.x264-Radarr"
SER = "Series.Title.S01E01.720p.HDTV.x264-Sonarr"
for method, file_path, dci_out, folder, expect_rel, beh in [
    ("should_use_file_name_only_for_download_client_item_without_a_job_folder", pj(UNS, SER + ".mkv"), pj(UNS, SER + ".mkv"), None,
     SER + ".mkv", "A download that is a single file records just its file name as the original path"),
    ("should_use_folder_and_file_name_only_for_download_client_item_with_a_job_folder", pj(UNS, SER, SER + ".mkv"), pj(UNS, SER), None,
     f"{SER}/{SER}.mkv", "A download in a job folder records folder/file as the original path"),
    ("should_include_intermediate_folders_for_download_client_item_with_a_job_folder", pj(UNS, SER, "subfolder", SER + ".mkv"),
     pj(UNS, SER), None, f"{SER}/subfolder/{SER}.mkv", "The original path keeps folders between the job folder and the file"),
    ("should_use_folder_info_original_title_to_find_relative_path", pj(P("C:\\Test\\Unsorted\\movies\\"), TRANS, "subfolder", TRANS + ".mkv"),
     None, {"original_title": TRANS}, f"{TRANS}/subfolder/{TRANS}.mkv",
     "Without a download client item the original path starts at the folder named by the parsed folder title"),
    ("should_use_folder_info_original_title_to_find_relative_path_when_file_is_not_in_download_client_item_output_directory",
     pj(P("C:\\Test\\Unsorted\\movie\\"), TRANS, "subfolder", TRANS + ".mkv"), pj(P("C:\\Test\\Unsorted\\movie-Other\\"), TRANS),
     {"release_title": TRANS}, f"subfolder/{TRANS}.mkv",
     "A file outside the download's output folder gets its original path relative to the parsed release folder"),
    ("should_use_folder_info_original_title_to_find_relative_path_when_download_client_item_has_an_empty_output_path",
     pj(P("C:\\Test\\Unsorted\\movies\\"), TRANS, "subfolder", TRANS + ".mkv"), IA_OUT, {"release_title": TRANS}, f"subfolder/{TRANS}.mkv",
     "A file outside the download's output path gets its original path relative to the parsed release folder"),
]:
    must(IA, method, "OriginalFilePath ==")
    given = {"new_download": True, "decisions": [approved(file_path, **({"folder_info": folder} if folder else {}))],
             "download_client_item": {"output_path": dci_out} if dci_out else None}
    note = "Expected path is the test's backslash path through AsOsAgnostic."
    if method.endswith("empty_output_path"):
        note += " Despite the name, the download client item keeps the fixture's output path " + IA_OUT + "."
    add(IAF, test_ref(IA, method), beh, {"op": "import_approved", "given": given}, {"recorded_original_file_path": expect_rel}, note)
skip(IA, "should_get_relative_path_when_there_is_no_grandparent_windows",
     "A file at a Windows drive root records its file name as the original path (WindowsOnly; we run on Linux)")
skip(IA, "should_get_relative_path_when_there_is_no_grandparent_for_UNC_path",
     "A file at a UNC share root records its file name as the original path (WindowsOnly; we run on Linux)")

# --- MediaFiles/MovieImport/DetectSampleFixture ----------------------------------------
DS = T + "MediaFiles/MovieImport/DetectSampleFixture.cs"
DSF = "detect_sample.json"
must_file(DS, "s.Runtime = 30", r'Path = @"C:\Test\30 Rock\30.rock.s01e01.avi"')
DS_NOTE = ("Movie runtime is in minutes, the probed runtime in seconds. Minimum runtime: <=3 min 15 s, <=10 min 90 s, <=30 min 300 s, else 600 s "
           f"({src_at(C + 'MediaFiles/MovieImport/DetectSample.cs', r'private int GetMinimumAllowedRuntime')}).")
for method, path in [("should_return_false_for_flv", r"C:\Test\some.show.s01e01.flv"), ("should_return_false_for_strm", r"C:\Test\some.show.s01e01.strm"),
                     ("should_return_false_for_iso", r"C:\Test\some movie (2000).iso"), ("should_return_false_for_img", r"C:\Test\some movie (2000).img"),
                     ("should_return_false_for_m2ts", r"C:\Test\some movie (2000).m2ts")]:
    must(DS, method, f'@"{path}"', "Times.Never()")
    ext = path.rsplit(".", 1)[1]
    add(DSF, test_ref(DS, method), f"A .{ext} file is never a sample and its runtime is not probed",
        {"op": "detect_sample", "given": {"path": path, "movie_runtime_minutes": 30}}, {"result": "NotSample", "runtime_probed": False})
must(DS, "should_use_runtime", "GivenRuntime(120);", "Times.Once()")
add(DSF, test_ref(DS, "should_use_runtime"), "Sample detection of an .avi probes the file's runtime once",
    {"op": "detect_sample", "given": {"path": r"C:\Test\30 Rock\30.rock.s01e01.avi", "movie_runtime_minutes": 30, "probed_runtime_seconds": 120}},
    {"runtime_probed_times": 1}, "The test asserts the probe only, not the result.")
for method, runtime, secs, result, beh in [
    ("should_return_true_if_runtime_is_less_than_minimum", 30, 60, "Sample", "A 60 s file for a 30 min movie is a sample"),
    ("should_return_false_if_runtime_greater_than_minimum", 30, 600, "NotSample", "A 600 s file for a 30 min movie is not a sample"),
    ("should_return_false_if_runtime_greater_than_webisode_minimum", 6, 299, "NotSample", "A 299 s file for a 6 min movie is not a sample"),
    ("should_return_false_if_runtime_greater_than_anime_short_minimum", 2, 60, "NotSample", "A 60 s file for a 2 min movie is not a sample"),
    ("should_return_true_if_runtime_less_than_anime_short_minimum", 2, 10, "Sample", "A 10 s file for a 2 min movie is a sample"),
]:
    must(DS, method, f"GivenRuntime({secs});", *( [f"Runtime = {runtime};"] if runtime != 30 else []))
    add(DSF, test_ref(DS, method), beh,
        {"op": "detect_sample", "given": {"path": r"C:\Test\30 Rock\30.rock.s01e01.avi", "movie_runtime_minutes": runtime, "probed_runtime_seconds": secs}},
        {"result": result}, DS_NOTE)
must(DS, "should_return_indeterminate_if_mediainfo_result_is_null", "(TimeSpan?)null", "DetectSampleResult.Indeterminate")
add(DSF, test_ref(DS, "should_return_indeterminate_if_mediainfo_result_is_null"),
    "Sample detection is indeterminate when the runtime cannot be probed",
    {"op": "detect_sample", "given": {"path": r"C:\Test\30 Rock\30.rock.s01e01.avi", "movie_runtime_minutes": 30, "probed_runtime_seconds": None}},
    {"result": "Indeterminate", "errors_logged": 1})

# --- MediaFiles/MovieImport/GetSceneNameFixture ----------------------------------------
GS = T + "MediaFiles/MovieImport/GetSceneNameFixture.cs"
GSF = "scene_name.json"
MN = "movie.title.2022.dvdrip.x264-ingot"
must_file(GS, f'_movieName = "{MN}"', r'@"C:\Test\Movies\Movie Title".AsOsAgnostic()', '"Movie Title - 2022 - Episode Title.mkv"')
GS_PATH = pj(P(r"C:\Test\Movies\Movie Title"), "Movie Title - 2022 - Episode Title.mkv")
UNSM = P(r"C:\Test\Unsorted Movies")
for method, given, scene, beh in [
    ("should_use_download_client_item_title_as_scene_name", {"download_client_release_title": MN}, MN,
     "The download client item's title is the scene name"),
    ("should_not_use_download_client_item_title_as_scene_name_if_there_are_other_video_files",
     {"path": pj(UNSM, MN), "other_video_files": True, "download_client_release_title": MN}, None,
     "The download client item's title is not the scene name when the download has other video files"),
    ("should_use_file_name_as_scenename_only_if_it_looks_like_scenename", {"path": pj(UNSM, MN + ".mkv")}, MN,
     "A scene-looking file name is the scene name"),
    ("should_not_use_file_name_as_scenename_if_it_doesnt_look_like_scenename", {"path": pj(UNSM, MN, "aaaaa.mkv")}, None,
     "A file name that does not look like a scene name gives no scene name"),
    ("should_use_folder_name_as_scenename_only_if_it_looks_like_scenename", {"folder_release_title": MN}, MN,
     "A scene-looking folder title is the scene name when the file name is not one"),
    ("should_not_use_folder_name_as_scenename_if_it_doesnt_look_like_scenename", {"path": pj(UNSM, MN, "aaaaa.mkv"), "folder_release_title": "aaaaa"},
     None, "A folder title that does not look like a scene name gives no scene name"),
    ("should_not_use_folder_name_as_scenename_if_there_are_other_video_files",
     {"path": pj(UNSM, MN, "aaaaa.mkv"), "other_video_files": True, "folder_release_title": MN}, None,
     "The folder title is not the scene name when the folder has other video files"),
]:
    g = {"path": GS_PATH, "other_video_files": False, "download_client_release_title": None, "folder_release_title": None}
    g.update(given)
    add(GSF, test_ref(GS, method), beh, {"op": "scene_name", "given": g}, {"scene_name": scene})
for ref, (ext,) in testcase_rows(GS, "should_remove_extension_from_nzb_title_for_scene_name"):
    ext = json.loads(ext)
    add(GSF, ref, f"A '{ext}' extension is stripped from the download client item's title for the scene name",
        {"op": "scene_name", "given": {"path": GS_PATH, "other_video_files": False, "download_client_release_title": MN + ext,
                                       "folder_release_title": None}},
        {"scene_name": MN})

# --- MediaFiles/MovieImport/Aggregation ---------------------------------------------------
AG = T + "MediaFiles/MovieImport/Aggregation/Aggregators/"
AGF = "aggregation.json"

# AggregateQuality: augmenter results are stand-ins; Order sorts them (AggregateQuality.cs OrderBy(a => a.Order)).
AQ = AG + "AggregateQualityFixture.cs"
must_file(AQ, "_fileExtensionAugmenter.SetupGet(s => s.Order).Returns(1);", "_nameAugmenter.SetupGet(s => s.Order).Returns(2);",
          "_mediaInfoAugmenter.SetupGet(s => s.Order).Returns(4);", "_releaseNameAugmenter.SetupGet(s => s.Order).Returns(5);",
          "AugmentQualityResult.ResolutionOnly((int)Resolution.R1080p, Confidence.MediaInfo)",
          "new AugmentQualityResult(QualitySource.TV, Confidence.Fallback, (int)Resolution.R720p, Confidence.Fallback, Modifier.NONE, Confidence.Fallback, new Revision(), Confidence.Fallback)",
          "new AugmentQualityResult(QualitySource.TV, Confidence.Default, 480, Confidence.Default, Modifier.NONE, Confidence.Fallback, new Revision(), Confidence.Default)",
          "AugmentQualityResult.SourceOnly(QualitySource.WEBDL, Confidence.MediaInfo)")


def aq(name, order, src, src_c, res, res_c, mod_c="Default", revision=None, rev_c="Default"):
    return {"augmenter": name, "order": order, "source": src, "source_confidence": src_c, "resolution": res, "resolution_confidence": res_c,
            "modifier": "NONE", "modifier_confidence": mod_c, "revision": revision, "revision_confidence": rev_c}


A_EXT = aq("file_extension", 1, "TV", "Fallback", 720, "Fallback", "Fallback", rev(), "Fallback")
A_NAME = aq("file_name", 2, "TV", "Default", 480, "Default", "Fallback", rev(), "Default")
A_MI = aq("media_info", 4, "UNKNOWN", "Default", 1080, "MediaInfo")
A_REL = aq("release_name", 5, "WEBDL", "MediaInfo", 0, "Default")
A_NAME_TAG = lambda v: aq("file_name", 2, "TV", "Default", 480, "Default", "Default", rev(v), "Tag")
AQ_NOTE = ("Augmenter results are stand-ins (role names are the fixture's mock names). Confidence order Fallback < Default < Tag < MediaInfo "
           f"({C}MediaFiles/MovieImport/Aggregation/Aggregators/Augmenters/Quality/Confidence.cs). SourceOnly/ResolutionOnly leave the other "
           "fields UNKNOWN/0/NONE at Default confidence with no revision (AugmentQualityResult.cs). Revision() is version 1.")
for method, augs, expect, beh in [
    ("should_return_HDTV720_from_extension_when_other_augments_are_null", [{"augmenter": "null_result", "order": 0, "result": None}, A_EXT],
     {"quality": "HDTV-720p", "source_detection": "Extension", "resolution_detection": "Extension"},
     "With only the extension fallback, a file is HDTV-720p detected from the extension"),
    ("should_return_SDTV_when_HDTV720_came_from_extension", [A_EXT, A_NAME],
     {"quality": "SDTV", "source_detection": "Name", "resolution_detection": "Name"},
     "A name result (TV 480) overrides the extension fallback (TV 720): SDTV"),
    ("should_return_HDTV1080p_when_HDTV720_came_from_extension_and_mediainfo_indicates_1080", [A_EXT, A_MI],
     {"quality": "HDTV-1080p", "source_detection": "Extension", "resolution_detection": "MediaInfo"},
     "Media info resolution 1080 overrides the extension's 720: HDTV-1080p"),
    ("should_return_HDTV1080p_when_SDTV_came_from_name_and_mediainfo_indicates_1080", [A_NAME, A_MI],
     {"quality": "HDTV-1080p", "source_detection": "Name", "resolution_detection": "MediaInfo"},
     "Media info resolution 1080 overrides the name's 480: HDTV-1080p"),
    ("should_return_WEBDL480p_when_file_name_has_HDTV480p_but_release_name_indicates_webdl_source", [A_NAME, A_REL],
     {"quality": "WEBDL-480p", "source_detection": "Name", "resolution_detection": "Name"},
     "A release name source WEBDL overrides the file name's TV source: WEBDL-480p"),
    ("should_return_version_1_when_no_version_specified", [A_NAME, A_REL],
     {"revision_version": 1, "revision_detection": "Unknown"}, "With no version tag the revision is v1, detected from nothing"),
    ("should_return_version_2_when_name_indicates_proper", [A_NAME_TAG(2), A_REL],
     {"revision_version": 2, "revision_detection": "Name"}, "A v2 tag in the name gives revision 2"),
    ("should_return_version_0_when_file_name_indicates_v0", [A_NAME_TAG(0), A_REL],
     {"revision_version": 0, "revision_detection": "Name"}, "An explicit v0 tag in the file name gives revision 0"),
    ("should_return_version_2_when_file_name_indicates_v0_and_release_name_indicates_v2",
     [A_NAME_TAG(0), aq("release_name", 5, "TV", "Default", 480, "Default", "Default", rev(2), "Tag")],
     {"revision_version": 2, "revision_detection": "Name"}, "File name v0 and release name v2 at equal confidence give revision 2"),
]:
    add(AGF, test_ref(AQ, method), beh, {"op": "aggregate_quality", "given": {"augmenter_results": augs}}, expect, AQ_NOTE)
must(AQ, "should_return_version_2_when_name_indicates_proper", "new Revision(2), Confidence.Tag")
must(AQ, "should_return_version_0_when_file_name_indicates_v0", "new Revision(0), Confidence.Tag")
must(AQ, "should_return_version_2_when_file_name_indicates_v0_and_release_name_indicates_v2", "new Revision(0), Confidence.Tag", "new Revision(2), Confidence.Tag")

# AggregateSubtitleInfo: in scope although extra file import is off, because existing subtitle files in a movie folder
# (Bazarr writes them) are read on every scan through ExistingSubtitleImporter, which runs this aggregator.
AS = AG + "AggregateSubtitleInfoFixture.cs"
AS_NOTE = ("CleanSubtitleTitleInfo(movieFile, path, fileNameBeforeRename) as called for an existing subtitle file found on scan "
           f"({src_at(C + 'Extras/Subtitles/ExistingSubtitleImporter.cs', r'_aggregationService.Augment\(localMovie, null\)')}). "
           "relative_path and original_file_path are the movie file record's.")
must(AS, "should_do_basic_parse", "Title.Should().BeNull();", "Copy.Should().Be(0);")
for ref, args in testcase_rows(AS, "should_do_basic_parse"):
    rel, orig, path, before = [None if a == "null" else json.loads(a) for a in args]
    add(AGF, ref, f"Subtitle '{path}' next to movie file '{rel or orig}' parses with no title and copy 0",
        {"op": "subtitle_info", "given": {"relative_path": rel, "original_file_path": orig, "path": path, "file_name_before_rename": before}},
        {"title": None, "copy": 0}, AS_NOTE)
must(AS, "should_not_parse_default", 'LanguageTags.Should().NotContain("default");', "CleanSubtitleTitleInfo(movieFile, path, null)")
for ref, args in testcase_rows(AS, "should_not_parse_default"):
    rel, path = [json.loads(a) for a in args]
    add(AGF, ref, f"'default' in subtitle '{path}' is not kept as a language tag when the movie file is named 'Default'",
        {"op": "subtitle_info", "given": {"relative_path": rel, "original_file_path": None, "path": path, "file_name_before_rename": None}},
        {"language_tags_exclude": ["default"]}, AS_NOTE + " The test leaves OriginalFilePath unset (null).")

# AggregateLanguage
AL = AG + "AggregateLanguageFixture.cs"
AL_NOTE = ("Language confidence order Default < Filename < Foldername < DownloadClientItem < MediaInfo "
           f"({C}MediaFiles/MovieImport/Aggregation/Aggregators/Augmenters/Language/Confidence.cs). A null list means that augmenter found nothing.")
must(AL, "should_return_default_if_no_info_is_known", "Contain(_movie.MovieMetadata.Value.OriginalLanguage)")
add(AGF, test_ref(AL, "should_return_default_if_no_info_is_known"),
    "With no language information a file gets the movie's original language",
    {"op": "aggregate_language", "given": {"movie_original_language": "English", "augmenters": []}}, {"languages_contain": "English"})
for method, fn, fo, dc, mi, exp, beh in [
    ("should_return_file_language_when_only_file_info_is_known", ["French"], None, None, None, ["French"],
     "A file name language is used when nothing else is known"),
    ("should_return_folder_language_when_folder_info_is_known", ["French"], ["German"], None, None, ["German"],
     "A folder name language beats a file name language"),
    ("should_return_download_client_item_language_when_download_client_item_info_is_known", ["French"], ["German"], ["Spanish"], None, ["Spanish"],
     "A download client item language beats folder and file name languages"),
    ("should_return_multi_language", ["Unknown"], ["French", "German"], ["Unknown"], None, ["French", "German"],
     "Known folder languages (French, German) beat Unknown from more confident sources"),
    ("should_use_mediainfo_over_others", ["Unknown"], ["French", "German"], ["Unknown"], ["Japanese", "English"], ["Japanese", "English"],
     "Media info audio languages beat all other sources"),
    ("should_not_use_mediainfo_if_unknown", ["Unknown"], ["French", "German"], ["Unknown"], ["Unknown"], ["French", "German"],
     "Media info reporting only Unknown is ignored"),
]:
    must(AL, method, *[f"Language.{x}" for x in exp])
    augs = [{"confidence": c, "languages": l} for c, l in
            [("Filename", fn), ("Foldername", fo), ("DownloadClientItem", dc), ("MediaInfo", mi)]]
    add(AGF, test_ref(AL, method), beh, {"op": "aggregate_language", "given": {"movie_original_language": "English", "augmenters": augs}},
        {"languages": exp}, AL_NOTE + " The test asserts the exact ordered list.")

# AggregateReleaseGroup
RG = AG + "AggregateReleaseGroupFixture.cs"
for method in ["should_prefer_downloadclient", "should_prefer_folder", "should_fallback_to_file", "should_not_use_imdbId", "should_not_use_tmdbbId"]:
    b = body(RG, method)
    titles = re.findall(r'ParseMovieTitle\("([^"]+)", false\)', b)
    path = P(re.search(r'Path = @"([^"]+)"\.AsOsAgnostic\(\)', b)[1])
    group = re.search(r'ReleaseGroup\.Should\(\)\.Be\("([^"]+)"\)', b)[1]
    assert len(titles) == 3, method
    beh = {"should_prefer_downloadclient": "The download client item's release group beats folder and file groups",
           "should_prefer_folder": "The folder's release group beats the file's when the download title has none",
           "should_fallback_to_file": "The file's release group is used when folder and download titles have none",
           "should_not_use_imdbId": "An {imdb-..} tag in a folder name is not taken as a release group",
           "should_not_use_tmdbbId": "A {tmdb-..} tag in a folder name is not taken as a release group"}[method]
    add(AGF, test_ref(RG, method), beh,
        {"op": "aggregate_release_group", "given": {"file_title": titles[0], "folder_title": titles[1], "download_client_title": titles[2], "path": path}},
        {"release_group": group}, "Each title goes through Radarr's movie title parser (Parser.ParseMovieTitle(title, false)).")

# AugmentLanguageFromMediaInfo
LM = AG + "Augmenters/Language/AugmentLanguageFromMediaInfoFixture.cs"
must(LM, "should_return_null_if_media_info_is_null", "MediaInfo = null")
add(AGF, test_ref(LM, "should_return_null_if_media_info_is_null"), "Without media info the media info language augmenter gives nothing",
    {"op": "augment_language_media_info", "given": {"audio_languages": None}}, {"result": None})
for method, codes, exp, beh in [
    ("should_return_language_for_single_known_language", ["eng"], ["English"], "Audio language 'eng' gives English"),
    ("should_only_return_one_when_language_duplicated", ["eng", "eng"], ["English"], "Duplicate audio languages give one language"),
    ("should_return_null_if_all_unknown", ["pirate", "pirate"], None, "Only unrecognised audio languages give nothing"),
    ("should_return_known_languages_only", ["eng", "pirate"], ["English"], "Unrecognised audio languages are dropped"),
    ("should_return_multiple_known_languages", ["eng", "ger"], ["English", "German"], "Audio languages 'eng' and 'ger' give English and German"),
]:
    must(LM, method, "{ " + ", ".join(f'"{c}"' for c in codes) + " }")
    add(AGF, test_ref(LM, method), beh, {"op": "augment_language_media_info", "given": {"audio_languages": codes}},
        {"result": None} if exp is None else {"language_set": exp}, None if exp is None else "The test asserts count and membership, not order.")

# AugmentQualityFromMediaInfo
QM = AG + "Augmenters/Quality/AugmentQualityFromMediaInfoFixture.cs"
MI_NOTE = "Media info fields the test does not set are NBuilder fillers."
must(QM, "should_return_null_if_media_info_is_null", "MediaInfo = null")
add(AGF, test_ref(QM, "should_return_null_if_media_info_is_null"), "Without media info the media info quality augmenter gives nothing",
    {"op": "augment_quality_media_info", "given": {"media_info": None}}, {"result": None})
must(QM, "should_return_null_if_media_info_width_is_zero", "Width = 0")
add(AGF, test_ref(QM, "should_return_null_if_media_info_width_is_zero"), "Media info with width 0 gives nothing",
    {"op": "augment_quality_media_info", "given": {"media_info": {"width": 0}}}, {"result": None}, MI_NOTE)
for ref, (w, h, r) in testcase_rows(QM, "should_return_closest_resolution"):
    res = int(re.fullmatch(r"Resolution\.R(\d+)p", r)[1])
    add(AGF, ref, f"A {w}x{h} video resolves to {res}p with unknown source",
        {"op": "augment_quality_media_info", "given": {"media_info": {"width": int(w), "height": int(h)}}},
        {"resolution": res, "source": "UNKNOWN"}, MI_NOTE)
for method, title, src in [("should_include_source_if_extracted_from_title", "Movie.Title.2008.WEB.x264-Radarr", "WEBDL"),
                           ("should_have_unknown_source_if_no_source_extracted_from_title", "Movie.Title.2008.x264-Radarr", "UNKNOWN")]:
    must(QM, method, f'"{title}"', f"QualitySource.{src}", "Width = 1920", "Height = 1080")
    add(AGF, test_ref(QM, method), f"A 1920x1080 video titled '{title}' gives 1080p with source {src}",
        {"op": "augment_quality_media_info", "given": {"media_info": {"width": 1920, "height": 1080, "title": title}}},
        {"resolution": 1080, "source": src})

# AugmentQualityFromReleaseName
QR = AG + "Augmenters/Quality/AugmentQualityFromReleaseNameFixture.cs"
add(AGF, test_ref(QR, "should_return_null_if_download_client_item_is_null"), "Without a download client item the release name augmenter gives nothing",
    {"op": "augment_quality_release_name", "given": {"download_client_item": False}}, {"result": None})
must(QR, "should_return_null_if_no_grabbed_history", "(DownloadHistory)null")
add(AGF, test_ref(QR, "should_return_null_if_no_grabbed_history"), "With no grab in download history the release name augmenter gives nothing",
    {"op": "augment_quality_release_name", "given": {"download_client_item": True, "latest_grab_title": None}}, {"result": None})
for ref, (title, src, sc, res, rc) in testcase_rows(QR, "should_return_augmented_quality"):
    title = json.loads(title)
    exp = {"source": src.removeprefix("QualitySource."), "source_confidence": sc.removeprefix("Confidence."),
           "resolution": int(res), "resolution_confidence": rc.removeprefix("Confidence.")}
    add(AGF, ref, f"The grabbed title '{title}' gives source {exp['source']} ({exp['source_confidence']}) and {res}p ({exp['resolution_confidence']})",
        {"op": "augment_quality_release_name", "given": {"download_client_item": True, "latest_grab_title": title}}, exp)

# --- MediaFiles/DiskScanServiceTests/ScanFixture ----------------------------------------------
SC = T + "MediaFiles/DiskScanServiceTests/ScanFixture.cs"
SCF = "disk_scan.json"
must_file(SC, r'_rootFolder = @"C:\Test\Movies".AsOsAgnostic();', r'_otherMovieFolder = @"C:\Test\Movies\OtherMovie".AsOsAgnostic();',
          r'var movieFolder = @"C:\Test\Movies\Movie".AsOsAgnostic();')
ROOT, OTHER, MOVIE = P(r"C:\Test\Movies"), P(r"C:\Test\Movies\OtherMovie"), P(r"C:\Test\Movies\Movie")
SC_NOTE = ("The root folder exists only when the test lists its subfolders; a listed subfolder exists, anything else does not. "
           "import_candidates is the number of files handed to import decisions. " + MOQ_NOTE)


def scan(movie=MOVIE, root_exists=True, subfolders=None, files=None, create_empty=False):
    return {"op": "disk_scan", "given": {"root_folder": ROOT, "root_folder_exists": root_exists, "root_subfolders": subfolders or [],
                                         "movie_path": movie, "files": files or [], "create_empty_movie_folders": create_empty}}


must(SC, "should_not_scan_if_movie_root_folder_does_not_exist", "ExpectedWarns(1)", "Times.Never()")
add(SCF, test_ref(SC, "should_not_scan_if_movie_root_folder_does_not_exist"),
    "A movie whose root folder is missing is not scanned, its folder is not created and its file records are not cleaned",
    scan(root_exists=False), {"files_listed": False, "movie_folder_created": False, "records_cleaned": False, "warnings": 1}, SC_NOTE)
must(SC, "should_not_scan_if_movie_root_folder_is_empty", "GivenRootFolder();", "ExpectedWarns(1)")
add(SCF, test_ref(SC, "should_not_scan_if_movie_root_folder_is_empty"),
    "A movie whose root folder is empty is not scanned, created, cleaned or imported (guards an unmounted share)",
    scan(), {"files_listed": False, "movie_folder_created": False, "records_cleaned": False, "import_decisions_requested": False, "warnings": 1},
    SC_NOTE)
for method, create, exp, beh in [
    ("should_create_if_movie_folder_does_not_exist_but_create_folder_enabled", True, True,
     "A missing movie folder is created on scan when empty movie folders are enabled"),
    ("should_not_create_if_movie_folder_does_not_exist_and_create_folder_disabled", False, False,
     "A missing movie folder is not created on scan when empty movie folders are disabled"),
]:
    must(SC, method, f".Returns({str(create).lower()});")
    add(SCF, test_ref(SC, method), beh, scan(subfolders=[OTHER], create_empty=create), {"movie_folder_created": exp}, SC_NOTE)
must(SC, "should_clean_but_not_import_if_movie_folder_does_not_exist", "FolderExists(_movie.Path), Times.Once()", "Clean(", "Times.Once()", "GetImportDecisions(", "Times.Never()")
add(SCF, test_ref(SC, "should_clean_but_not_import_if_movie_folder_does_not_exist"),
    "A missing movie folder (root folder present) cleans the movie's file records but does not import",
    scan(subfolders=[OTHER]), {"records_cleaned": True, "import_decisions_requested": False},
    SC_NOTE + " The test also asserts the movie folder's existence is checked exactly once (an implementation detail, not ported).")


for method, movie, n, beh, extra in [
    ("should_not_scan_extras_subfolder", MOVIE, 1, "Files under an 'Extras' folder (any case) are not import candidates", None),
    ("should_not_scan_various_extras_subfolders", MOVIE, 1,
     "Files under Behind the Scenes, Deleted Scenes, Featurettes, Interviews, Sample(s), Scenes, Shorts, Trailers and Other folders are not import candidates",
     None),
    ("should_not_scan_featurettes_subfolders", MOVIE, 1, "Files under a 'Featurettes' folder are not import candidates", None),
    ("should_not_scan_AppleDouble_subfolder", MOVIE, 1, "Files under '.AppleDouble' (any case) are not import candidates", None),
    ("should_scan_extras_movie_and_subfolders", P(r"C:\Test\Movies\Extras"), 4,
     "A movie whose own folder is named 'Extras' is scanned, but its '.AppleDouble' and nested 'Extras' folders are not", None),
    ("should_not_scan_subfolders_that_start_with_period", MOVIE, 1, "Files under folders starting with '.' are not import candidates", None),
    ("should_not_scan_subfolder_of_season_folder_that_starts_with_a_period", MOVIE, 1,
     "Files under dot-folders nested in a subfolder are not import candidates", None),
    ("should_not_scan_Synology_eaDir", MOVIE, 1, "Files under Synology '@eaDir' are not import candidates", None),
    ("should_not_scan_thumb_folder", MOVIE, 1, "Files under '.@__thumb' are not import candidates", None),
    ("should_scan_dotHack_folder", P(r"C:\Test\TV\.hack"), 2, "A movie folder whose own name starts with '.' is still scanned", None),
    ("should_find_files_at_root_of_movie_folder", MOVIE, 2, "Files at the root of the movie folder are import candidates", None),
    ("should_exclude_inline_extra_files", MOVIE, 1, "Files with an inline extra suffix ('-deleted', '-other') are not import candidates", None),
    ("should_exclude_osx_metadata_files", MOVIE, 1, "macOS '._' metadata files are not import candidates", None),
    ("should_not_scan_excluded_files", MOVIE, 1, "'.DS_Store', '.unmanic' and '.unmanic.part' files are neither import candidates nor possible extras",
     {"possible_extra_files": 0}),
]:
    must(SC, method, f"l.Count == {n}")
    mp = movie
    b = body(SC, method)
    blk = b[b.index("GivenFiles("):b.index("Subject.Scan")]
    files = [pj(mp, *[json.loads(x) for x in re.findall(r'"[^"]*"', m)])
             for m in re.findall(r'Path\.Combine\(_movie\.Path((?:, "[^"]*")+)\)', blk)]
    assert len(files) == blk.count("Path.Combine(_movie.Path"), method  # every call parsed, none truncated
    assert files, method
    exp = {"import_candidates": n}
    if extra:
        exp.update(extra)
    note = SC_NOTE
    if method == "should_not_scan_extras_subfolder":
        note += " The test also asserts the disk is listed exactly twice (an implementation detail, not ported)."
    add(SCF, test_ref(SC, method), beh, scan(movie=mp, subfolders=[mp], files=files), exp, note)

# --- MediaFiles/DownloadedMoviesImportServiceFixture -------------------------------------------
DI = T + "MediaFiles/DownloadedMoviesImportServiceFixture.cs"
DIF = "downloaded_import.json"
must_file(DI, '_droneFactory = "c:\\\\drop\\\\".AsOsAgnostic();', '_subFolders = new[] { "c:\\\\root\\\\foldername".AsOsAgnostic() };',
          '_videoFiles = new[] { "c:\\\\root\\\\foldername\\\\47.ronin.2013.ext".AsOsAgnostic() };')
DROP, SUB, VID = P("c:\\drop\\"), P("c:\\root\\foldername"), P("c:\\root\\foldername\\47.ronin.2013.ext")
DI_NOTE = ("Fixture: every folder exists; the drop folder holds subfolder " + SUB + " whose video file is " + VID +
           ". 'imported' means the import step reports an imported file. Sample detection defaults to Indeterminate (DetectSampleResult's "
           "first member) unless the test sets Sample. A folder is deleted only for import mode Move (Auto becomes Move without a download "
           f"client item or when it can move files) after an import, when ShouldDeleteFolder holds ({src_at(C + 'MediaFiles/DownloadedMovieImportService.cs', r'public bool ShouldDeleteFolder')}).")


def drop(**kw):
    g = {"entry": "process_root_folder", "folder": DROP, "subfolders": [SUB], "video_files": [VID]}
    g.update(kw)
    return {"op": "downloaded_import", "given": g}


def path_(path, **kw):
    g = {"entry": "process_path", "path": path}
    g.update(kw)
    return {"op": "downloaded_import", "given": g}


DCI = {"download_id": "sab1", "status": "Downloading"}
for method, given, exp, beh, snips in [
    ("should_search_for_series_using_folder_name", drop(), {"movie_lookup": ["foldername"]},
     "A download folder is matched to a movie by its folder name", ['GetMovie("foldername"), Times.Once()']),
    ("should_skip_if_file_is_in_use_by_another_process", drop(movie_found=True, file_locked=True), {"imported": False},
     "A folder whose video file is locked by another process is not imported", ["IsFileLocked", ".Returns(true);"]),
    ("should_skip_if_no_series_found", drop(movie_found=False), {"import_decisions_requested": False, "imported": False},
     "A folder that matches no movie is not imported", ["Returns((Movie)null)"]),
    ("should_not_import_if_folder_is_a_series_path", drop(movie_found=True, folder_is_movie_path=True),
     {"folder_video_files_listed": False, "warnings": 1},
     "A folder that is an existing movie's folder is not scanned for import", ["MoviePathExists", "ExpectedWarns(1)"]),
    ("should_not_delete_folder_after_import",
     path_(DROP, import_mode="Auto", movie_found=True, imported=True, video_files_after_import=[],
           download_client_item={**DCI, "can_move_files": False}),
     {"folder_deleted": False}, "After importing from a download the client still needs (Auto mode, cannot move), the folder is kept",
     ["CanMoveFiles = false;", "ImportMode.Auto"]),
    ("should_delete_folder_if_importmode_move",
     path_(DROP, import_mode="Move", movie_found=True, imported=True, video_files_after_import=[],
           download_client_item={**DCI, "can_move_files": False}),
     {"folder_deleted": True}, "After importing in Move mode with no video files left, the download folder is deleted",
     ["ImportMode.Move", "Times.Once()"]),
    ("should_not_delete_folder_if_importmode_copy",
     path_(DROP, import_mode="Copy", movie_found=True, imported=True, video_files_after_import=[],
           download_client_item={**DCI, "can_move_files": True}),
     {"folder_deleted": False}, "After importing in Copy mode the download folder is kept", ["ImportMode.Copy", "Times.Never()"]),
    ("should_not_delete_folder_if_files_were_imported_and_video_files_remain", drop(movie_found=True, imported=True),
     {"folder_deleted": False, "warnings": 1}, "After an import, a folder still holding a non-sample video file is kept",
     ["Times.Never()", "ExpectedWarns(1)"]),
    ("should_delete_folder_if_files_were_imported_and_only_sample_files_remain", drop(movie_found=True, imported=True, sample_detection="Sample"),
     {"folder_deleted": True}, "After an import, a folder holding only sample video files is deleted",
     ["DetectSampleResult.Sample", "Times.Once()"]),
    ("should_return_importresult_on_unknown_movie",
     path_(P(r"C:\folder\file.mkv"), path_is_folder=False, path_is_file=True, movie_found=False),
     {"results": 1, "result": "Rejected", "result_path": P(r"C:\folder\file.mkv")},
     "A single file that matches no movie yields one rejected result for that file", ["ImportResultType.Rejected"]),
    ("should_not_delete_if_there_is_large_rar_file",
     drop(movie_found=True, imported=True, sample_detection="Sample", files_recursive=[VID.replace(".ext", ".rar")], file_size_mb=15),
     {"folder_deleted": False, "warnings": 1}, "After an import, a folder holding a RAR file over 10 MB is kept",
     ["15.Megabytes()", "Times.Never()"]),
    ("should_use_folder_if_folder_import",
     path_(P(r"C:\media\ba09030e-1234-1234-1234-123456789abc\[HorribleSubs] American Psycho (2000) [720p]\[HorribleSubs] American Psycho (2000) [720p].mkv"),
           path_is_folder=True, movie_found=True),
     {"import_decisions_requested": 1},
     "A path that is a folder is imported as a folder", ["Times.Once()"]),
    ("should_not_use_folder_if_file_import",
     path_(P(r"C:\media\ba09030e-1234-1234-1234-123456789abc\Torrents\[HorribleSubs] 47 Ronin (2013) [720p].mkv"),
           path_is_folder=False, path_is_file=True, movie_found=True),
     {"import_decisions_requested": 1, "folder_info": None},
     "A path that is a single file is imported without folder information", ["null, true), Times.Once()"]),
    ("should_not_process_if_file_and_folder_do_not_exist",
     path_(P(r"C:\media\ba09030e-1234-1234-1234-123456789abc\[HorribleSubs] 47 Ronin (2013) [720p]"), path_is_folder=False, path_is_file=False),
     {"results": 0, "movie_lookup": [], "errors_logged": 1}, "A path that does not exist yields no results and no movie lookup",
     ["BeEmpty()", "ExpectedErrors(1)"]),
    ("should_not_delete_if_no_files_were_imported", drop(movie_found=True, imported=False, sample_detection="Sample", file_size_mb=15),
     {"folder_deleted": False}, "A folder from which nothing was imported is kept", ["new List<ImportResult>()", "Times.Never()"]),
]:
    must(DI, method, *snips)
    note = DI_NOTE
    if method == "should_use_folder_if_folder_import":
        note += " The fixture makes every path a folder, so the .mkv path itself is processed as a folder; the test asserts import decisions are requested once with folder processing."
    add(DIF, test_ref(DI, method), beh, given, exp, note)
skip(DI, "should_not_delete_folder_if_no_files_were_imported",
     "A drop folder with nothing imported is not measured for deletion (vacuous: asserts GetFolderSize is never called, which the import service never calls)")
for ref, (prefix,) in testcase_rows(DI, "should_remove_unpack_from_folder_name"):
    prefix = json.loads(prefix)
    folder = P(f"C:\\Test\\Unsorted\\{prefix}47.ronin.2013.hdtv-lol")
    add(DIF, ref, f"A '{prefix}' prefix is stripped from a download folder name before matching it to a movie",
        drop(subfolders=[folder]), {"movie_lookup": ["47.ronin.2013.hdtv-lol"], "lookup_with_prefix": False}, DI_NOTE)
for method, ext in [("should_return_rejection_if_nothing_imported_and_contains_rar_file", ".rar"),
                    ("should_return_rejection_if_nothing_imported_and_contains_executable_file", ".exe")]:
    must(DI, method, f'Replace(".ext", "{ext}")', "ImportResultType.Rejected")
    add(DIF, test_ref(DI, method), f"A download folder with nothing importable and a {ext} file yields one rejected result",
        path_(P(r"C:\media\ba09030e-1234-1234-1234-123456789abc\[HorribleSubs] American Psycho (2000) [720p]\[HorribleSubs] American Psycho (2000) [720p].mkv"),
              path_is_folder=True, movie_found=True, imported=False, files_recursive=[VID.replace(".ext", ext)]),
        {"results": 1, "result": "Rejected"}, DI_NOTE + " The fixture makes every path a folder.")

# --- MediaFiles/MediaFileDeletionService/DeleteMovieFileFixture --------------------------------
DF = T + "MediaFiles/MediaFileDeletionService/DeleteMovieFileFixture.cs"
FOF = "file_ops.json"
must_file(DF, 'RootFolder = @"C:\\Test\\Movies"', 'Path.Combine(RootFolder, "Movie Title")', 'RelativePath = "Some SubFolder"')
DF_NOTE = ("Manual delete of a movie file. Root folder C:\\Test\\Movies, movie folder 'Movie Title', file 'Some SubFolder' (paths not made OS-agnostic). "
           "The recycle bin provider deletes permanently when no recycle bin is set (our config).")


def dfg(**kw):
    g = {"root_folder_exists": False, "root_folder_has_subfolders": False, "movie_folder_exists": False, "file_exists": False, "delete_fails": False}
    g.update(kw)
    return {"op": "delete_movie_file", "given": g}


for method, given, exp, beh, snips in [
    ("should_throw_if_root_folder_does_not_exist", dfg(), {"error": "NzbDroneClientException", "warnings": 1},
     "Deleting a movie file fails when the root folder is missing", ["Assert.Throws<NzbDroneClientException>"]),
    ("should_should_throw_if_root_folder_is_empty", dfg(root_folder_exists=True), {"error": "NzbDroneClientException", "warnings": 1},
     "Deleting a movie file fails when the root folder is empty (guards an unmounted share)", ["GivenRootFolderExists();"]),
    ("should_delete_from_db_if_movie_folder_does_not_exist", dfg(root_folder_exists=True, root_folder_has_subfolders=True),
     {"record_deleted": True, "delete_reason": "Manual", "file_deleted": False},
     "With the movie folder gone, deleting a movie file removes only its record", ["DeleteMediaFileReason.Manual), Times.Once()"]),
    ("should_delete_from_db_if_movie_file_does_not_exist",
     dfg(root_folder_exists=True, root_folder_has_subfolders=True, movie_folder_exists=True),
     {"record_deleted": True, "delete_reason": "Manual", "file_deleted": False},
     "With the file gone from disk, deleting a movie file removes only its record", ["GivenMovieFolderExists();"]),
    ("should_delete_from_disk_and_db_if_movie_file_exists",
     dfg(root_folder_exists=True, root_folder_has_subfolders=True, movie_folder_exists=True, file_exists=True),
     {"record_deleted": True, "delete_reason": "Manual", "file_deleted": True, "recycle_subfolder": "Movie Title"},
     "Deleting an existing movie file removes it from disk (via the recycle bin provider, under the movie folder name) and its record",
     ['DeleteFile(_movieFile.Path, "Movie Title"), Times.Once()']),
    ("should_handle_error_deleting_movie_file",
     dfg(root_folder_exists=True, root_folder_has_subfolders=True, movie_folder_exists=True, file_exists=True, delete_fails=True),
     {"error": "NzbDroneClientException", "file_delete_attempted": True, "record_deleted": False, "errors_logged": 1},
     "When removing the file from disk fails, the movie file record is kept and the delete fails", ["new IOException()", "Times.Never()"]),
]:
    must(DF, method, *snips)
    add(FOF, test_ref(DF, method), beh, given, exp, DF_NOTE)

# --- MediaFiles/UpgradeMediaFileServiceFixture ---------------------------------------------------
UM = T + "MediaFiles/UpgradeMediaFileServiceFixture.cs"
must_file(UM, r'Path = @"C:\Test\Movies\Movie".AsOsAgnostic()', 'RelativePath = @"A.Movie.2019.avi"')
UM_NOTE = "Upgrading movie " + P(r"C:\Test\Movies\Movie") + " whose existing file is A.Movie.2019.avi; the root folder is the movie folder's parent."


def umg(**kw):
    g = {"movie_path": P(r"C:\Test\Movies\Movie"), "existing_file": "A.Movie.2019.avi", "existing_file_on_disk": True, "root_folder_exists": True}
    g.update(kw)
    return {"op": "upgrade_movie_file", "given": g}


for method, given, exp, beh, snips in [
    ("should_delete_single_movie_file_once", umg(), {"old_file_deleted_from_disk_times": 1},
     "Importing an upgrade deletes the old file from disk once", ["Times.Once()"]),
    ("should_delete_movie_file_from_database", umg(), {"old_record_deleted_times": 1, "delete_reason": "Upgrade"},
     "Importing an upgrade deletes the old file's record with reason Upgrade", ["DeleteMediaFileReason.Upgrade), Times.Once()"]),
    ("should_delete_existing_file_fromdb_if_file_doesnt_exist", umg(existing_file_on_disk=False),
     {"old_record_deleted_times": 1, "delete_reason": "Upgrade"}, "An upgrade deletes the old file's record even when the old file is gone from disk",
     [".Returns(false);"]),
    ("should_not_try_to_recyclebin_existing_file_if_file_doesnt_exist", umg(existing_file_on_disk=False),
     {"old_file_deleted_from_disk_times": 0}, "An upgrade does not try to delete an old file that is gone from disk", ["Times.Never()"]),
    ("should_return_old_movie_file_in_oldFiles", umg(), {"old_files": 1}, "An upgrade reports the replaced file", ["OldFiles.Count.Should().Be(1)"]),
    ("should_throw_if_there_are_existing_movie_files_and_the_root_folder_is_missing", umg(root_folder_exists=False),
     {"error": "RootFolderNotFoundException", "old_record_deleted_times": 0},
     "An upgrade fails and deletes nothing when the root folder is missing", ["Assert.Throws<RootFolderNotFoundException>"]),
]:
    must(UM, method, *snips)
    add(FOF, test_ref(UM, method), beh, given, exp, UM_NOTE)

# --- MediaFiles/MovieFileMovingServiceTests/MoveMovieFileFixture ----------------------------------
MM = T + "MediaFiles/MovieFileMovingServiceTests/MoveMovieFileFixture.cs"
skip(MM, "should_catch_UnauthorizedAccessException_during_folder_inheritance",
     "Moving a movie file survives an access error while inheriting folder permissions (WindowsOnly; we run on Linux)")
skip(MM, "should_catch_InvalidOperationException_during_folder_inheritance",
     "Moving a movie file survives an invalid operation while inheriting folder permissions (WindowsOnly; we run on Linux)")
MM_NOTE = "The movie folder is " + P(r"C:\Test\Movies\Movie") + " under root " + P("C:\\Test\\Movies\\") + " (exists); the built file path is mocked."
must(MM, "should_notify_on_movie_folder_creation", "Times.Once()")
add(FOF, test_ref(MM, "should_notify_on_movie_folder_creation"), "Moving a file into a movie folder that does not exist yet announces the folder's creation",
    {"op": "move_movie_file", "given": {"root_folder_exists": True, "movie_folder_exists": False}}, {"movie_folder_created_events": 1}, MM_NOTE)
must(MM, "should_not_notify_if_movie_folder_already_exists", "Times.Never()")
add(FOF, test_ref(MM, "should_not_notify_if_movie_folder_already_exists"), "Moving a file into an existing movie folder announces no folder creation",
    {"op": "move_movie_file", "given": {"root_folder_exists": True, "movie_folder_exists": True}}, {"movie_folder_created_events": 0}, MM_NOTE)

# --- MediaFiles/MediaInfo/UpdateMediaInfoServiceFixture --------------------------------------------
UMI = T + "MediaFiles/MediaInfo/UpdateMediaInfoServiceFixture.cs"
MIF = "media_info.json"
REV_LINE = src_at(C + "MediaFiles/MediaInfo/VideoFileInfoReader.cs", r"CURRENT_MEDIA_INFO_SCHEMA_REVISION = ")
CUR = int(re.search(r"= (\d+);", lines(C + "MediaFiles/MediaInfo/VideoFileInfoReader.cs")[line_ref(C + "MediaFiles/MediaInfo/VideoFileInfoReader.cs", r"CURRENT_MEDIA_INFO_SCHEMA_REVISION = ")])[1])
MINR = int(re.search(r"= (\d+);", lines(C + "MediaFiles/MediaInfo/VideoFileInfoReader.cs")[line_ref(C + "MediaFiles/MediaInfo/VideoFileInfoReader.cs", r"MINIMUM_MEDIA_INFO_SCHEMA_REVISION = ")])[1])
MOVIE_MI = P(r"C:\movie")
UMI_NOTE = (f"Movie path {MOVIE_MI}. Schema revisions: current {CUR}, minimum {MINR} ({REV_LINE}); a file whose media info revision is below "
            "the minimum (or has none) is re-probed on a movie scan. Media info on unless the test disables it. "
            "schema_revision null: the test sets no media info on that file.")


def mi_files(n, first=None, rels=None, exist=True):
    rels = rels or ["media.mkv"] * n
    return [{"relative_path": r, "schema_revision": (first if i == 0 else None)} for i, r in enumerate(rels)]


for method, given, exp, beh, snips in [
    ("should_skip_up_to_date_media_info", {"files": mi_files(3, CUR), "files_exist": True}, {"probed": 2, "updated": 2},
     "On a movie scan, files with current media info are not re-probed", ["CURRENT_MEDIA_INFO_SCHEMA_REVISION", "Times.Exactly(2)"]),
    ("should_skip_not_yet_date_media_info", {"files": mi_files(3, MINR), "files_exist": True}, {"probed": 2, "updated": 2},
     "On a movie scan, files with media info at the minimum revision are not re-probed", ["MINIMUM_MEDIA_INFO_SCHEMA_REVISION", "Times.Exactly(2)"]),
    ("should_update_outdated_media_info", {"files": mi_files(3, 0), "files_exist": True}, {"probed": 3, "updated": 3},
     "On a movie scan, files with outdated media info are re-probed", ["new MediaInfoModel())", "Times.Exactly(3)"]),
    ("should_ignore_missing_files", {"files": mi_files(2), "files_exist": False}, {"probed": 0, "updated": 0},
     "On a movie scan, files missing from disk are not probed", ["Times.Never()"]),
    ("should_continue_after_failure", {"files": mi_files(2, rels=["media2.mkv", "media.mkv"]), "files_exist": True, "probe_fails_for": ["media2.mkv"]},
     {"probed_ok": ["media.mkv"], "updated": 1}, "On a movie scan, a failed probe of one file does not stop the others",
     ['GivenFailedScan(Path.Combine(_movie.Path, "media2.mkv"));', "Times.Exactly(1)"]),
    ("should_not_update_files_if_media_info_disabled",
     {"enable_media_info": False, "files": mi_files(2, rels=["media2.mkv", "media.mkv"]), "files_exist": True}, {"probed": 0, "updated": 0},
     "With media info disabled, a movie scan probes nothing", [".Returns(false);"]),
]:
    must(UMI, method, *snips)
    g = {"entry": "movie_scanned", "enable_media_info": True, "movie_path": P(r"C:\movie")}
    g.update(given)
    add(MIF, test_ref(UMI, method), beh, {"op": "update_media_info", "given": g}, exp, UMI_NOTE)
for method, given, exp, beh, snips in [
    ("should_not_update_if_media_info_disabled", {"enable_media_info": False, "file": {"relative_path": "media.mkv"}}, {"probed": 0, "updated": 0},
     "With media info disabled, updating one file probes nothing", [".Returns(false);"]),
    ("should_update_media_info", {"file": {"relative_path": "media.mkv", "schema_revision": 3}}, {"probed": 1, "updated": 1},
     "Updating one file probes it once and saves it", ["SchemaRevision = 3", "Times.Once()"]),
    ("should_not_update_media_info_if_new_info_is_null", {"file": {"relative_path": "media.mkv", "schema_revision": 3}, "probe_fails": True},
     {"media_info_kept": True}, "A failed probe keeps the file's existing media info", ["MediaInfo.Should().NotBeNull()"]),
    ("should_not_save_movie_file_if_new_info_is_null", {"file": {"relative_path": "media.mkv", "schema_revision": 3}, "probe_fails": True},
     {"updated": 0}, "A failed probe does not save the file", ["Times.Never()"]),
]:
    must(UMI, method, *snips)
    g = {"entry": "update_file", "enable_media_info": True, "movie_path": P(r"C:\movie"), "files_exist": True}
    g.update(given)
    add(MIF, test_ref(UMI, method), beh, {"op": "update_media_info", "given": g}, exp, UMI_NOTE)
for ref, (ext,) in testcase_rows(UMI, "should_not_update_media_info_if_file_does_not_support_media_info"):
    ext = json.loads(ext)
    add(MIF, ref, f"A '{ext}' file whose probe fails is probed once and not saved",
        {"op": "update_media_info", "given": {"entry": "update_file", "enable_media_info": True, "movie_path": P(r"C:\movie"), "files_exist": True,
                                              "file": {"path": pj(P(r"C:\movie"), "media" + ext)}, "probe_fails": True}},
        {"probed": 1, "updated": 0}, UMI_NOTE)

# --- MediaFiles/MediaInfo/VideoFileInfoReaderFixture ---------------------------------------------
VR = T + "MediaFiles/MediaInfo/VideoFileInfoReaderFixture.cs"
for method, beh in [("get_runtime", "Probing the bundled H264 sample reads a 10 s runtime"),
                    ("get_info", "Probing the bundled H264 sample reads its codecs, bitrates, size, languages and colour data"),
                    ("get_info_unicode", "Probing the H264 sample under a non-ASCII file name reads the same media info")]:
    skip(VR, method, beh + " (needs the reference's binary sample Files/Media/H264_sample.mp4 and ffprobe; not portable as data)")
for ref, (bd, prim, tf, side, dovi, hdr) in testcase_rows(VR, "should_detect_hdr_correctly"):
    side = [s.strip().removeprefix("FFMpegCore.") for s in json.loads(side).split(",") if s.strip()]
    dovi = None if dovi == "null" else int(dovi)
    hdr = hdr.removeprefix("HdrFormat.")
    add(MIF, ref, f"{bd}-bit video, primaries '{json.loads(prim)}', transfer '{json.loads(tf)}', side data {side or 'none'}"
        + (f", Dolby Vision compatibility id {dovi}" if dovi is not None else "") + f" is HDR format {hdr}",
        {"op": "hdr_format", "given": {"bit_depth": int(bd), "colour_primaries": json.loads(prim), "transfer_characteristics": json.loads(tf),
                                       "side_data": side, "dovi_bl_signal_compatibility_id": dovi}},
        {"hdr_format": hdr}, "Side data are FFMpegCore side-data type names; the compatibility id is set on DoviConfigurationRecordSideData.")

# --- MediaFiles/MediaFileServiceTests/FilterFixture ----------------------------------------------
FF = T + "MediaFiles/MediaFileServiceTests/FilterFixture.cs"
MSF = "media_file_service.json"
F3 = [P("C:\\file1.avi"), P("C:\\file2.avi"), P("C:\\file3.avi")]
FF_NOTE = "Movie 10 at " + P("C:\\") + "; known files are the movie's file records (relative paths)."
for method, files, known, exp, beh in [
    ("filter_should_return_all_files_if_no_existing_files", F3, [], {"result_set": F3}, "With no known files, every scanned file is new"),
    ("filter_should_return_none_if_all_files_exist", F3, ["file1.avi", "file2.avi", "file3.avi"], {"result": []},
     "When every scanned file is already known, none is new"),
    ("filter_should_return_none_existing_files", F3, ["file2.avi"], {"count": 2, "excludes": [P("C:\\file2.avi")]},
     "A scanned file already known is not new"),
    ("filter_should_return_none_existing_files_not_ignoring_case", [P("C:\\file1.avi"), P("C:\\FILE2.avi"), P("C:\\file3.avi")], ["file2.avi"],
     {"count": 3}, "On Linux, a known file differing only in case does not hide a scanned file"),
    ("filter_should_not_change_casing", [P("C:\\FILE1.avi")], [], {"count": 1, "contains": [P("C:\\FILE1.avi")], "excludes": [P("C:\\file1.avi")]},
     "Filtering keeps scanned file names' casing"),
]:
    add(MSF, test_ref(FF, method), beh, {"op": "filter_existing_files", "given": {"movie_path": P("C:\\"), "files": files, "known_relative_paths": known}},
        exp, FF_NOTE)
must(FF, "filter_should_return_none_existing_files_not_ignoring_case", "PosixOnly();")
skip(FF, "filter_should_return_none_existing_files_ignoring_case",
     "On Windows, a known file differing only in case hides a scanned file (WindowsOnly; we run on Linux)")

# --- MediaFiles/MediaFileTableCleanupServiceFixture -----------------------------------------------
TC = T + "MediaFiles/MediaFileTableCleanupServiceFixture.cs"
TC_NOTE = "Movie at " + P(r"C:\Test\TV\Series") + " with 10 file records."
must(TC, "should_skip_files_that_exist_in_disk", "CreateListOfSize(10)", "Times.Never()")
add(MSF, test_ref(TC, "should_skip_files_that_exist_in_disk"), "Cleaning a movie whose file records all exist on disk changes nothing",
    {"op": "clean_file_records", "given": {"records": 10, "missing_from_disk": 0}}, {"movie_updated": False}, TC_NOTE)
must(TC, "should_delete_non_existent_files", ".Random(2)", "DeleteMediaFileReason.MissingFromDisk), Times.Exactly(2)")
add(MSF, test_ref(TC, "should_delete_non_existent_files"), "Cleaning deletes the records of files missing from disk, with reason MissingFromDisk",
    {"op": "clean_file_records", "given": {"records": 10, "missing_from_disk": 2}}, {"records_deleted": 2, "delete_reason": "MissingFromDisk"}, TC_NOTE)
must(TC, "should_not_update_episode_when_episodeFile_exists", "Times.Never()")
add(MSF, test_ref(TC, "should_not_update_episode_when_episodeFile_exists"), "Cleaning a movie whose files all exist does not update the movie",
    {"op": "clean_file_records", "given": {"records": 10, "missing_from_disk": 0}}, {"movie_updated": False}, TC_NOTE)
skip(TC, "should_delete_files_that_dont_belong_to_any_episodes", "Records not linked to the movie are deleted ([Ignore] in the reference; never runs)")
skip(TC, "should_unlink_episode_when_episodeFile_does_not_exist", "A movie whose file is gone is unlinked ([Ignore] in the reference; never runs)")

# --- Extras/ExtraServiceFixture ---------------------------------------------------------------------
EX = T + "Extras/ExtraServiceFixture.cs"
EXF = "extras.json"
must_file(EX, r'_releaseFolder = @"C:\Test\Unsorted TV\Movie.Title.2022".AsOsAgnostic();', '.Returns("nfo,srt");',
          "v.ImportExtraFiles).Returns(true);", '"Movie.Title.2022.mkv"')
REL = P(r"C:\Test\Unsorted TV\Movie.Title.2022")
EX_MOVIE = pj(REL, "Movie.Title.2022.mkv")
EX_NOTE = ("Extra file import after a movie import from release folder " + REL + ". Fixture defaults: import extra files on, extensions 'nfo,srt'; "
           "two stand-in extra handlers: 'subtitle' takes only .srt, 'other' takes anything (Order 0 then 1). Our config has extra file import off.")


def exg(files, **kw):
    g = {"import_extra_files": True, "extra_file_extensions": "nfo,srt", "release_folder": REL, "movie_file": EX_MOVIE,
         "release_files": [EX_MOVIE] + files, "job_folder": False}
    g.update(kw)
    return {"op": "import_extras", "given": g}


must(EX, "should_not_pass_file_if_import_disabled", "ImportExtraFiles).Returns(false);", "Times.Never()")
add(EXF, test_ref(EX, "should_not_pass_file_if_import_disabled"), "With extra file import off, no extra file (.nfo) is considered",
    exg([pj(REL, "Movie.Title.2022.nfo")], import_extra_files=False), {"extra_handlers_consulted": False}, EX_NOTE)
for ref, (name,) in testcase_rows(EX, "should_not_pass_unwanted_file"):
    name = json.loads(name)
    add(EXF, ref, f"With extra file import off, '{name}' is not considered",
        exg([pj(REL, name)], import_extra_files=False), {"extra_handlers_consulted": False},
        EX_NOTE + " The test also turns extra file import off.")
must(EX, "should_pass_subtitle_file_to_subtitle_service", "Times.Once()", "Times.Never()")
add(EXF, test_ref(EX, "should_pass_subtitle_file_to_subtitle_service"), "With extra file import on, a matching .srt goes to the subtitle handler only",
    exg([pj(REL, "Movie.Title.2022.en.srt")]), {"subtitle_handler_files": [pj(REL, "Movie.Title.2022.en.srt")], "other_handler_got_it": False}, EX_NOTE)
must(EX, "should_pass_nfo_file_to_other_service", "Times.Never()", "Times.Once()")
add(EXF, test_ref(EX, "should_pass_nfo_file_to_other_service"), "With extra file import on, a matching .nfo goes to the other-extras handler only",
    exg([pj(REL, "Movie.Title.2022.nfo")]), {"other_handler_files": [pj(REL, "Movie.Title.2022.nfo")], "subtitle_handler_got_it": False}, EX_NOTE)
must(EX, "should_search_subtitles_when_importing_from_job_folder", "GetFiles(_releaseFolder, true), Times.Once", "GetFiles(_releaseFolder, false), Times.Never")
add(EXF, test_ref(EX, "should_search_subtitles_when_importing_from_job_folder"),
    "Importing from a job folder looks for extra files in the release folder recursively",
    exg([pj(REL, "Movie.Title.2022.en.srt")], job_folder=True), {"release_folder_listing": "recursive"}, EX_NOTE)
must(EX, "should_not_search_subtitles_when_not_importing_from_job_folder", "GetFiles(_releaseFolder, true), Times.Never", "GetFiles(_releaseFolder, false), Times.Once")
add(EXF, test_ref(EX, "should_not_search_subtitles_when_not_importing_from_job_folder"),
    "Importing a single file looks for extra files only directly in its folder",
    exg([pj(REL, "Movie.Title.2022.en.srt")], job_folder=False), {"release_folder_listing": "top_level"}, EX_NOTE)

# --- Fixtures not ported, with the config fact that rules each out ---------------------------------
EXTRA_OFF = ("extra file import is off (mediamanagement.json importExtraFiles false); ExtraService.ImportExtraFiles returns before any "
             f"extra handler runs ({src_at(C + 'Extras/ExtraService.cs', r'if \(!_configService.ImportExtraFiles\)')}); "
             "the gate itself is ported from ExtraServiceFixture")
RULED_OUT = {
    T + "Extras/Metadata/Consumers/": "every metadata writer is disabled (metadata.json enable false for all 5). Open: "
        "ExistingMetadataImporter still runs every consumer's FindMetadataFile on scan "
        f"({src_at(C + 'Extras/Metadata/ExistingMetadataImporter.cs', r'consumer.FindMetadataFile')}), so matched files get linked to the "
        "movie file and deleted with it (ExtraFileService.HandleAsync(MovieFileDeletedEvent))",
    T + "Extras/Others/OtherExtraServiceFixture.cs": EXTRA_OFF + " (tests call OtherExtraService.ImportFiles only)",
    T + "Extras/Subtitles/SubtitleServiceFixture.cs": EXTRA_OFF + " (tests call SubtitleService.ImportFiles only)",
    T + "Extras/SubtitleFileRepositoryFixture.cs": "database repository CRUD (delete/get by movie or file id): storage internals, no behaviour to port",
    T + "MediaFiles/MediaFileRepositoryFixture.cs": "database repository CRUD (get/delete by movie id): storage internals, no behaviour to port",
    T + "MediaFiles/DownloadedMoviesCommandServiceFixture.cs":
        "DownloadedMoviesScanCommand is only sent by an external caller (API or post-processing script) and we have none; completed "
        "download handling (downloadclientconfig.json, on) calls DownloadedMovieImportService directly (ported from "
        "DownloadedMoviesImportServiceFixture)",
    T + "MediaFiles/MediaInfo/MediaInfoFormatterTests/": "the formatter only feeds {MediaInfo ...} naming tokens (none in naming.json), "
        "notifications (notification.json is empty), script import (useScriptImport false) and API display",
    T + "MediaFiles/MovieImport/Specifications/MatchesFolderSpecificationFixture.cs":
        "no live tests: every test is commented out (the spec itself always accepts, MatchesFolderSpecification.cs TODO)",
    T + "MediaFiles/RenameMovieFileServiceFixture.cs": "the RenameFiles/RenameMovie commands are only sent by the UI's manual organize; "
        "renaming on import (naming.json renameMovies) goes through MovieFileMovingService (ported from MoveMovieFileFixture); the tests "
        "assert mock calls only (event published, Update count, ids passed)",
}
ported = {c["ref"].split(" ", 1)[1].rsplit(":", 1)[0] for c in cases}
for f in sorted(x for d in ("MediaFiles", "Extras") for x in (SRC / T / d).rglob("*.cs")):
    rel = str(f.relative_to(SRC))
    if "[Test" not in f.read_text(encoding="utf-8-sig"):
        continue  # helpers and base classes
    ruled = [k for k in RULED_OUT if rel.startswith(k)]
    assert (rel in ported) != bool(ruled), (rel, "ported and ruled out" if ruled else "neither ported nor ruled out")
for k in RULED_OUT:
    assert (SRC / k).exists(), k

# --- IDs, catalogue, corpus --------------------------------------------------------------------------
cases.sort(key=lambda c: (c["ref"].split(" ", 1)[1].rsplit(":", 1)[0], int(c["ref"].rsplit(":", 1)[1])))
refs = [c["ref"] for c in cases]
assert len(set(refs)) == len(refs), [r for r in refs if refs.count(r) > 1]



def key(ref):
    """(path, method, row index, row text) of a ref, read from the clone its tag names."""
    tag, loc = ref.split(" ", 1)
    rel, ln = loc.rsplit(":", 1)
    app, ver = tag.split("@")
    ls = lines(rel, SRC.parent / f"{app.lower()}@{ver}")
    i = int(ln) - 1
    j = i
    while not (m := re.search(r"\bvoid (\w+)\(", ls[j])):
        j += 1
    k = i
    while k > 0 and ls[k - 1].strip() != "}":  # back to the previous member's closing brace
        k -= 1
    return rel, m[1], sum(1 for x in ls[k:i] if x.strip().startswith("[TestCase(")), ls[i].strip()

keys = [key(r) for r in refs]
assert len(set(keys)) == len(keys), [k for k in keys if keys.count(k) > 1]

old = {}  # key -> (id, carried lines)
stale = []  # blocks of old entries whose test vanished (IDs are never reused)
if CATALOGUE.exists():
    for block in re.split(r"(?m)^(?=- id: )", CATALOGUE.read_text()):
        if not block.startswith("- id: "):
            continue
        bid = re.match(r"- id: (\S+)", block)[1]
        bref = json.loads(re.search(r"(?m)^  ref: (.*)$", block)[1])
        keep = re.findall(r"(?m)^  ((?:status|decision|tests):.*(?:\n    .*)*)", block)
        old[key(bref)] = (bid, keep, block)
next_n = max([int(i.rsplit("-", 1)[1]) for i, _, _ in old.values()] + [0]) + 1
out, by_file = [], {}
seen = set()
for c in cases:
    k = key(c["ref"])
    seen.add(k)
    if k in old:
        cid, keep, _ = old[k]
    else:
        cid, keep = f"{PREFIX}{next_n:04d}", ["status: unmapped", "tests: []"]
        next_n += 1
    out.append(f"- id: {cid}\n  behaviour: {json.dumps(c['behaviour'], ensure_ascii=False)}\n  source: reference\n"
               f"  ref: {json.dumps(c['ref'], ensure_ascii=False)}\n  in_scope: {str(c['in_scope']).lower()}\n"
               + "".join(f"  {k}\n" for k in keep))
    if c["in_scope"]:
        case = {"id": cid, "ref": c["ref"], "input": c["input"], "expect": c["expect"]}
        if c["note"]:
            case["note"] = c["note"]
        by_file.setdefault(c["file"], []).append(case)

stale = [blk for k, (_, _, blk) in old.items() if k not in seen]
for blk in stale:
    print("warning: kept stale entry", re.match(r"- id: (\S+)", blk)[1])
CATALOGUE.parent.mkdir(parents=True, exist_ok=True)
CATALOGUE.write_text("# Generated by parity/extract/radarr_import.py; IDs are stable, edit status/decision/tests only.\n"
                     "# Ruled-out fixtures: RULED_OUT in the script.\n" + "".join(out) + "".join(stale))
CORPUS.mkdir(parents=True, exist_ok=True)
for f, cs in sorted(by_file.items()):
    (CORPUS / f).write_text(json.dumps(cs, indent=1, ensure_ascii=False) + "\n")
print(f"{len(cases)} entries ({sum(c['in_scope'] for c in cases)} in scope), {len(by_file)} corpus files")
