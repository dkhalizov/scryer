"""Port Radarr grab-decision fixtures A-M (DecisionEngineTests/**/[A-M]*Fixture.cs)
into parity/corpus/reference/radarr/decision/*.json and
parity/catalogue/decision.radarr-a.yaml.

Run: python3 -I parity/extract/radarr_decision_a.py [--check]
(--check: fail if the outputs on disk differ from what would be written.)

Case data below is hand-ported from each test body; this script looks up the
[Test]/[TestCase] line of every method in the reference source, checks that
every active test in each fixture is covered, parses [TestCase] arguments,
and keeps IDs stable across re-runs (key: ref path | method | row | input hash,
stored as a `# key:` comment above each catalogue entry).

Conventions: quality names are Radarr's own ("HDTV-720p"); "revision" only when
the test sets one. size_mb = n * 1024 * 1024 bytes (the tests' .Megabytes()).
search: {"kind": "rss"} for `new()` (no search criteria), {"kind": "search"}
for a MovieSearchCriteria, plus "user_invoked": true when the test sets it.
"""
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REF_REPO = Path.home() / "PycharmProjects/arr-reference/radarr@v6.4.4.10685"
TAG = "Radarr@v6.4.4.10685"
TEST_DIR = "src/NzbDrone.Core.Test/DecisionEngineTests"
CATALOGUE = ROOT / "parity/catalogue/decision.radarr-a.yaml"
CORPUS_DIR = ROOT / "parity/corpus/reference/radarr/decision"
ID_FMT, ID_MIN, ID_MAX = "RAD-DECISION-{:04d}", 1, 999

RSS = {"kind": "rss"}
SEARCH = {"kind": "search"}
USER_SEARCH = {"kind": "search", "user_invoked": True}


def Q(name, version=None, real=None):
    q = {"quality": name}
    rev = {k: v for k, v in (("version", version), ("real", real)) if v is not None}
    if rev:
        q["revision"] = rev
    return q


def merge(base, delta):
    out = dict(base)
    for k, v in delta.items():
        out[k] = merge(out[k], v) if isinstance(v, dict) and isinstance(out.get(k), dict) else v
    return out


def acc(ok):
    return "accepted" if ok else "rejected"


# A test entry: (method, [row, ...]). A row is a dict:
#   b: behaviour, g: given delta (merged onto the fixture base), e: expect,
#   n: note, out: reason (in_scope false, no corpus case), replace: given is not merged onto the base.
# For [TestCase] methods, rows is a function of the parsed argument tuple.

DEFAULT_QUALITIES = [  # QualityFixture.GetDefaultQualities(), all allowed (QualityFixture.cs:68)
    "Unknown", "CAM", "TELECINE", "DVDSCR", "REGIONAL", "SDTV", "DVD", "DVD-R", "HDTV-720p",
    "HDTV-1080p", "HDTV-2160p", "WEBDL-480p", "WEBDL-720p", "WEBDL-1080p", "WEBDL-2160p",
    "Bluray-480p", "Bluray-576p", "Bluray-720p", "Bluray-1080p", "Bluray-2160p", "BR-DISK", "Raw-HD",
]
HIST_PROFILE_NOTE = ("profile replaced by the test: format items empty (GetSampleFormatItems() with no "
                     "custom formats defined), upgrade_allowed not set so C# default false")

FIXTURES = []


def fixture(path, spec, base, tests, note=None):
    FIXTURES.append({"path": path, "spec": spec, "base": base, "tests": tests, "note": note})


# --- AcceptableSizeSpecificationFixture ------------------------------------
fixture("AcceptableSizeSpecificationFixture.cs", "AcceptableSizeSpecification",
        {"quality_definition": {"quality": "SDTV", "min_size": 2, "max_size": 10},
         "parsed_quality": Q("SDTV", version=2)},
        [
            ("single_episode", lambda runtime, size, ok: [{
                "b": f"A {size} MB SDTV release for a {runtime}-minute movie (SDTV size 2-10 MB/min) is {acc(ok)}",
                "g": {"movie": {"runtime": runtime}, "release": {"size_mb": size}},
                "e": {"accepted": ok}}]),
            ("should_return_true_if_size_is_zero", [{
                "b": "A release with size 0 is accepted regardless of the quality size limits",
                "g": {"movie": {"runtime": 120}, "release": {"size_bytes": 0},
                      "quality_definition": {"min_size": 10, "max_size": 20}},
                "e": {"accepted": True}}]),
            ("should_return_true_if_unlimited_30_minute", [{
                "b": "With no max size for the quality, a huge release for a 30-minute movie is accepted",
                "g": {"movie": {"runtime": 30}, "release": {"size_bytes": 18457280000},
                      "quality_definition": {"max_size": None}},
                "e": {"accepted": True}}]),
            ("should_return_true_if_unlimited_60_minute", [{
                "b": "With no max size for the quality, a huge release for a 60-minute movie is accepted",
                "g": {"movie": {"runtime": 60}, "release": {"size_bytes": 36857280000},
                      "quality_definition": {"max_size": None}},
                "e": {"accepted": True}}]),
            ("should_use_110_minutes_if_runtime_is_0", [
                {"b": "With runtime 0 the size check assumes 110 minutes: 1095 MB at max 10 MB/min is accepted",
                 "g": {"movie": {"runtime": 0}, "release": {"size_mb": 1095}},
                 "e": {"accepted": True},
                 "n": "first of two asserts in this test; the test also expects one logged warning"},
                {"b": "With runtime 0 the size check assumes 110 minutes: 1105 MB at max 10 MB/min is rejected",
                 "g": {"movie": {"runtime": 0}, "release": {"size_mb": 1105}},
                 "e": {"accepted": False},
                 "n": "second of two asserts in this test; the test also expects one logged warning"},
            ]),
        ],
        note="quality_definition sizes are MB per minute of runtime")

# --- AlreadyImportedSpecificationFixture -----------------------------------
T = "Movie.Title.2018.720p.HDTV.x264-Radarr"
H720, H1080 = Q("HDTV-720p", version=1), Q("HDTV-1080p", version=1)


def hist(download_id, quality, event):
    return {"download_id": download_id, "source_title": T, "quality": quality, "event": event}


TORRENT = lambda h: {"release": {"protocol": "torrent", "info_hash": h}}  # noqa: E731
fixture("AlreadyImportedSpecificationFixture.cs", "AlreadyImportedSpecification",
        {"config": {"enable_completed_download_handling": True},
         "movie": {"id": 1, "movie_file_id": 1},
         "parsed_quality": H720, "history": []},
        [
            ("should_be_accepted_if_CDH_is_disabled", [{
                "b": "Accepted when completed download handling is disabled",
                "g": {"config": {"enable_completed_download_handling": False}},
                "e": {"accepted": True}}]),
            ("should_be_accepted_if_movie_does_not_have_a_file", [{
                "b": "Accepted when the movie has no file",
                "g": {"movie": {"movie_file_id": 0}}, "e": {"accepted": True}}]),
            ("should_be_accepted_if_movie_does_not_have_grabbed_event", [{
                "b": "Accepted when the movie history has no grabbed event",
                "g": {}, "e": {"accepted": True}}]),
            ("should_be_accepted_if_movie_does_not_have_imported_event", [{
                "b": "Accepted when the movie history has a grab but no import",
                "g": {"history": [hist("ID-A", H720, "Grabbed")]}, "e": {"accepted": True}}]),
            ("should_be_accepted_if_grabbed_and_imported_quality_is_the_same", [{
                "b": "Accepted when grabbed and imported quality are the same",
                "g": {"history": [hist("ID-A", H720, "Grabbed"), hist("ID-A", H720, "DownloadFolderImported")]},
                "e": {"accepted": True}}]),
            ("should_be_accepted_if_grabbed_download_id_and_release_torrent_hash_is_unknown", [{
                "b": "Accepted when grab and import qualities differ but the torrent release has no info hash",
                "g": {"history": [hist("ID-A", H720, "Grabbed"), hist("ID-A", H1080, "DownloadFolderImported")],
                      **TORRENT(None)},
                "e": {"accepted": True}}]),
            ("should_be_accepted_if_grabbed_download_does_not_have_an_id", [{
                "b": "Accepted when the grabbed history event has no download id",
                "g": {"history": [hist(None, H720, "Grabbed"), hist("ID-A", H1080, "DownloadFolderImported")],
                      **TORRENT("ID-A")},
                "e": {"accepted": True}}]),
            ("should_be_rejected_if_grabbed_download_id_matches_release_torrent_hash", [{
                "b": "Rejected when the release info hash matches a grab whose import landed in a different quality",
                "g": {"history": [hist("ID-A", H720, "Grabbed"), hist("ID-A", H1080, "DownloadFolderImported")],
                      **TORRENT("ID-A")},
                "e": {"accepted": False}}]),
            ("should_be_rejected_if_release_title_matches_grabbed_event_source_title", [{
                "b": "Rejected when the release was already grabbed and imported in a different quality (title-match variant)",
                "g": {"history": [hist("ID-A", H720, "Grabbed"), hist("ID-A", H1080, "DownloadFolderImported")],
                      **TORRENT("ID-A")},
                "e": {"accepted": False},
                "n": "test body is identical to the info-hash test; it never sets the release title"}]),
        ],
        note="download ids are random uppercase GUIDs in the reference; ID-A stands for one shared value")

# --- BlockedIndexerSpecificationFixture ------------------------------------
fixture("BlockedIndexerSpecificationFixture.cs", "BlockedIndexerSpecification",
        {"release": {"indexer_id": 1}, "blocked_indexers": []},
        [
            ("should_return_true_if_no_blocked_indexer", [{
                "b": "Accepted when no indexer is blocked", "g": {}, "e": {"accepted": True}}]),
            ("should_return_false_if_blocked_indexer", [{
                "b": "Rejected, temporarily, when the release's indexer is blocked",
                "g": {"blocked_indexers": [{"provider_id": 1, "disabled_till": "now"}]},
                "e": {"accepted": False, "rejection_type": "Temporary"},
                "n": "rejection_type is the specification's own Type property (Subject.Type), not a field of the decision"}]),
        ])

# --- CustomFormatAllowedByProfileSpecificationFixture ----------------------
AW, CO = "Awesome Format", "Cool Format"
ONLY_AW = [{"format": CO, "score": -2}, {"format": AW, "score": 1}]  # GetSampleFormatItems(AW)
BOTH = [{"format": AW, "score": 1}, {"format": CO, "score": 2}]     # GetSampleFormatItems(AW, CO)


def cf(formats, items, score, ok, b, min_score=None):
    g = {"release": {"custom_formats": formats, "custom_format_score": score},
         "profile": {"format_items": items}}
    if min_score is not None:
        g["profile"]["min_format_score"] = min_score
    return [{"b": b, "g": g, "e": {"accepted": ok}}]


fixture("CustomFormatAllowedByProfileSpecificationFixture.cs", "CustomFormatAllowedbyProfileSpecification",
        {"profile": {"cutoff": "Bluray-1080p", "min_format_score": 1},
         "parsed_quality": Q("DVD", version=2)},
        [
            ("should_allow_if_format_score_greater_than_min",
             cf([AW], ONLY_AW, 1, True, "Accepted when the release format score (1) reaches the profile minimum (1)")),
            ("should_deny_if_format_score_not_greater_than_min",
             cf([CO], ONLY_AW, -2, False, "Rejected when the release format score (-2) is below the profile minimum (1)")),
            ("should_deny_if_format_score_not_greater_than_min_2",
             cf([CO, AW], ONLY_AW, -1, False, "Rejected when matched formats sum to -1, below the profile minimum (1)")),
            ("should_allow_if_all_format_is_defined_in_profile",
             cf([CO, AW], BOTH, 3, True, "Accepted when matched formats sum to 3, above the profile minimum (1)")),
            ("should_deny_if_no_format_was_parsed_and_min_score_positive",
             cf([], BOTH, 0, False, "Rejected when no format matched (score 0) and the profile minimum is 1")),
            ("should_allow_if_no_format_was_parsed_min_score_is_zero",
             cf([], BOTH, 0, True, "Accepted when no format matched (score 0) and the profile minimum is 0", min_score=0)),
        ],
        note=("format_items scores come from CustomFormatsTestHelpers.GetSampleFormatItems "
              "(src/NzbDrone.Core.Test/CustomFormats/CustomFormatsTestHelpers.cs:21): allowed formats score "
              "2^index, the rest -2^(allowed count); custom_format_score is the test's "
              "QualityProfile.CalculateCustomFormatScore over the release formats"))

# --- DownloadDecisionMakerFixture ------------------------------------------
TROLLS = "Trolls.2016.720p.WEB-DL.DD5.1.H264-FGT"
MOCK = "verifies mock calls only (Moq Verify), no observable decision"
fixture("DownloadDecisionMakerFixture.cs", "DownloadDecisionMaker",
        {"mode": "rss", "reports": [TROLLS]},
        [
            ("should_call_all_specifications", [{
                "b": "Every specification is evaluated once per release, including after one fails",
                "out": MOCK + "; rejection count is covered by should_have_same_number_of_rejections_as_specs_that_failed"}]),
            ("should_return_rejected_if_single_specs_fail", [{
                "b": "A release is rejected when its only specification fails",
                "g": {"specs": ["fail"]}, "e": {"approved": False}}]),
            ("should_return_rejected_if_one_of_specs_fail", [{
                "b": "A release is rejected when one of several specifications fails",
                "g": {"specs": ["pass", "fail", "pass", "pass"]}, "e": {"approved": False}}]),
            ("should_return_pass_if_all_specs_pass", [{
                "b": "A release is approved when all specifications pass",
                "g": {"specs": ["pass", "pass", "pass"]}, "e": {"approved": True}}]),
            ("should_have_same_number_of_rejections_as_specs_that_failed", [{
                "b": "A decision carries one rejection per failed specification",
                "g": {"specs": ["pass", "pass", "pass", "fail", "fail", "fail"]},
                "e": {"rejection_count": 3}}]),
            ("should_not_attempt_to_map_episode_if_not_parsable", [{
                "b": "An unparsable release title is not mapped to a movie and no specification runs",
                "out": MOCK}]),
            ("should_return_rejected_result_for_unparsable_search", [{
                "b": "An unparsable title in a movie search is not mapped and no specification runs",
                "out": MOCK + " (despite the name, nothing about the result is asserted)"}]),
            ("should_not_attempt_to_make_decision_if_series_is_unknown", [{
                "b": "No specification runs when the release maps to no movie",
                "out": MOCK}]),
            ("broken_report_shouldnt_blowup_the_process", [{
                "b": "A mapping exception on one release does not stop the others from being mapped",
                "out": MOCK + " plus logged-error count"}]),
            ("should_return_unknown_series_rejection_if_series_is_unknown", [{
                "b": "A release that maps to no movie still yields one decision",
                "g": {"specs": ["pass", "pass", "pass"], "mapped_movie": None},
                "e": {"decision_count": 1},
                "n": "despite the name, only the decision count is asserted"}]),
            ("should_not_allow_download_if_series_is_unknown", [{
                "b": "A release that maps to no movie still yields one decision (download-allowed assert commented out)",
                "g": {"specs": ["pass", "pass", "pass"], "mapped_movie": None},
                "e": {"decision_count": 1},
                "n": "the DownloadAllowed assertion is commented out in the reference; only the count is asserted"}]),
            ("should_not_allow_download_if_no_episodes_found", [{
                "b": "Ignored series-only test (no episodes found)",
                "out": "[Ignore(\"Series\")] in the reference; never runs"}]),
            ("should_return_a_decision_when_exception_is_caught", [{
                "b": "A release whose mapping throws still yields a decision",
                "g": {"specs": ["pass"], "mapping_throws": True},
                "e": {"decision_count": 1},
                "n": "the test also expects one logged error"}]),
        ],
        note=("specs are mock specifications: pass = Accept, fail = Reject(Unknown, failN); "
              "mapping is mocked to return a movie unless mapped_movie is null"))

# --- FreeSpaceSpecificationFixture -----------------------------------------
def fs(b, ok, min_free=None, avail=None, size=None, extra=None):
    g = {}
    if min_free is not None:
        g["config"] = {"minimum_free_space_when_importing_mb": min_free}
    if avail is not None:
        g["available_space_mb"] = avail
    if size is not None:
        g["release"] = {"size_mb": size}
    return [{"b": b, "g": merge(g, extra or {}), "e": {"accepted": ok}}]


fixture("FreeSpaceSpecificationFixture.cs", "FreeSpaceSpecification",
        {"movie": {"path": "C:\\Test\\Films\\Movie"}},
        [
            ("should_return_true_when_available_space_is_more_than_size",
             fs("Accepted when free space (200 MB) exceeds the release size (100 MB) with no minimum", True, 0, 200, 100)),
            ("should_return_true_when_available_space_minus_size_is_more_than_minimum_free_space",
             fs("Accepted when free space minus size (100 MB) stays above the 50 MB minimum", True, 50, 200, 100)),
            ("should_return_false_available_space_is_less_than_size",
             fs("Rejected when the release (1000 MB) is larger than free space (200 MB)", False, 0, 200, 1000)),
            ("should_return_false_when_available_space_minus_size_is_less_than_minimum_free_space",
             fs("Rejected when free space minus size (100 MB) falls below the 150 MB minimum", False, 150, 200, 100)),
            ("should_return_true_if_skip_free_space_check_is_true",
             fs("Accepted when the free space check is skipped, even below the minimum", True, 150, 200, 100,
                {"config": {"skip_free_space_check_when_importing": True}})),
            ("should_return_true_if_root_folder_is_not_available",
             fs("Accepted when free space cannot be read because the folder does not exist", True, 150, None, 100,
                {"available_space_error": "DirectoryNotFoundException"})),
        ],
        note="movie path is run through AsOsAgnostic() in the reference")

# --- HistorySpecificationFixture -------------------------------------------
UPG, NOTUPG = Q("SDTV", version=1), Q("HDTV-1080p", version=2)


def h(b, ok, download_id, quality, age_hours, event="Grabbed", extra=None, n=None):
    g = {"most_recent_history": {"download_id": download_id, "quality": quality,
                                 "age_hours": age_hours, "event": event}}
    row = {"b": b, "g": merge(g, extra or {}), "e": {"accepted": ok}}
    if n:
        row["n"] = n
    return [row]


def hprof(cutoff, parsed):
    return {"profile": {"items_allowed": DEFAULT_QUALITIES, "cutoff": cutoff, "format_items": [],
                        "min_format_score": 0, "upgrade_allowed": False},
            "parsed_quality": parsed}


CDH_OFF = {"config": {"enable_completed_download_handling": False}}
fixture("HistorySpecificationFixture.cs", "HistorySpecification",
        {"config": {"enable_completed_download_handling": True},
         "movie": {"id": 1},
         "profile": {"items_allowed": DEFAULT_QUALITIES, "cutoff": "Bluray-1080p", "format_items": [],
                     "min_format_score": 0, "upgrade_allowed": True},
         "parsed_quality": Q("DVD", version=2), "release_custom_formats": [],
         "history_custom_formats": [], "search": RSS},
        [
            ("should_return_true_if_it_is_a_search", [{
                "b": "Accepted for a search: recent grab history is only checked on RSS",
                "g": {"search": SEARCH}, "e": {"accepted": True}}]),
            ("should_return_true_if_latest_history_item_is_null", [{
                "b": "Accepted when the movie has no history",
                "g": {"most_recent_history": None}, "e": {"accepted": True}}]),
            ("should_return_true_if_latest_history_item_is_not_grabbed",
             h("Accepted when the latest history event is not a grab (download failed)",
               True, "", NOTUPG, 0, "DownloadFailed")),
            ("should_return_true_if_latest_history_item_is_older_than_twelve_hours",
             h("Accepted when the latest grab is older than 12 hours (13 h), even at a better quality",
               True, "", NOTUPG, 13)),
            ("should_be_upgradable_if_only_episode_is_upgradable",
             h("Accepted when the release (DVD v2) upgrades a just-grabbed SDTV", True, "", UPG, 0)),
            ("should_not_be_upgradable_if_episode_is_of_same_quality_as_existing",
             h("Rejected when the release equals a just-grabbed WEBDL-1080p", False, "", Q("WEBDL-1080p", version=1), 0,
               extra=hprof("Bluray-1080p", Q("WEBDL-1080p", version=1)), n=HIST_PROFILE_NOTE)),
            ("should_not_be_upgradable_if_cutoff_already_met",
             h("Rejected when a just-grabbed Bluray-1080p already meets the WEBDL-1080p cutoff", False, "",
               Q("Bluray-1080p", version=1), 0, extra=hprof("WEBDL-1080p", Q("WEBDL-1080p", version=1)), n=HIST_PROFILE_NOTE)),
            ("should_return_false_if_latest_history_item_is_only_one_hour_old",
             h("Rejected when a grab one hour ago (HDTV-1080p v2) is not upgraded by the release (DVD v2)",
               False, "", NOTUPG, 1)),
            ("should_return_false_if_latest_history_has_a_download_id_and_cdh_is_disabled",
             h("Accepted with completed download handling off when the release upgrades a 100-day-old grab with a download id",
               True, "test", UPG, 2400, extra=CDH_OFF,
               n="test name says false but it asserts BeTrue")),
            ("should_return_false_if_cutoff_already_met_and_cdh_is_disabled",
             h("Rejected with completed download handling off when an old grab (WEBDL-1080p) already meets the cutoff",
               False, "test", Q("WEBDL-1080p", version=1), 2400,
               extra=merge(CDH_OFF, hprof("WEBDL-1080p", Q("Bluray-1080p", version=1))), n=HIST_PROFILE_NOTE)),
            ("should_return_false_if_only_episode_is_not_upgradable_and_cdh_is_disabled",
             h("Rejected with completed download handling off when the release does not upgrade an old grab",
               False, "test", NOTUPG, 2400, extra=CDH_OFF)),
        ],
        note=("most_recent_history is MostRecentForMovie(1); age_hours 2400 is AddDays(-100); "
              "history custom formats are mocked empty"))

# --- LanguageSpecificationFixture ------------------------------------------
def lang(b, release, ok, profile_lang=None, n=None):
    g = {"release_languages": [release]}
    if profile_lang:
        g["profile"] = {"language": profile_lang}
    row = {"b": b, "g": g, "e": {"accepted": ok}}
    if n:
        row["n"] = n
    return row


fixture("LanguageSpecificationFixture.cs", "LanguageSpecification",
        {"parsed_languages": ["English"], "profile": {"language": "English"},
         "movie": {"original_language": "French"}},
        [
            ("should_return_true_if_language_is_english", [
                lang("An English release is accepted by an English profile", "English", True)]),
            ("should_return_false_if_language_is_german", [
                lang("A German release is rejected by an English profile", "German", False)]),
            ("should_return_false_if_release_is_german_and_profile_original", [
                lang("A German release is rejected by an Original profile for a French movie", "German", False, "Original")]),
            ("should_return_true_if_release_is_french_and_profile_original", [
                lang("A French release is accepted by an Original profile for a French movie", "French", True, "Original")]),
            ("should_return_true_if_allowed_language_any", [
                lang("A German release is accepted by an Any-language profile", "German", True, "Any",
                     "first of two asserts in this test"),
                lang("An English release is accepted by an Any-language profile", "English", True, "Any",
                     "second of two asserts in this test"),
            ]),
        ],
        note="release_languages is RemoteMovie.Languages; parsed_languages is ParsedMovieInfo.Languages")

# --- MaximumSizeSpecificationFixture ---------------------------------------
def mx(b, max_mb, size_mb, ok):
    return [{"b": b, "g": {"config": {"maximum_size_mb": max_mb}, "release": {"size_mb": size_mb}},
             "e": {"accepted": ok}}]


fixture("MaximumSizeSpecificationFixture.cs", "MaximumSizeSpecification", {},
        [
            ("should_return_true_when_maximum_size_is_set_to_zero",
             mx("A maximum size of 0 means no limit (1000 MB accepted)", 0, 1000, True)),
            ("should_return_true_when_size_is_smaller_than_maximum_size",
             mx("1999 MB is accepted under a 2000 MB maximum", 2000, 1999, True)),
            ("should_return_true_when_size_is_equals_to_maximum_size",
             mx("2000 MB is accepted at a 2000 MB maximum (inclusive)", 2000, 2000, True)),
            ("should_return_false_when_size_is_bigger_than_maximum_size",
             mx("2001 MB is rejected over a 2000 MB maximum", 2000, 2001, False)),
            ("should_return_true_when_size_is_zero",
             mx("A release of unknown size (0) is accepted under a 2000 MB maximum", 2000, 0, True)),
        ])

# --- MinimumAgeSpecificationFixture ----------------------------------------
def ma(b, min_age, age, ok):
    return [{"b": b, "g": {"config": {"minimum_age_minutes": min_age}, "release": {"age_minutes": age}},
             "e": {"accepted": ok}}]


fixture("MinimumAgeSpecificationFixture.cs", "MinimumAgeSpecification", {"release": {"protocol": "usenet"}},
        [
            ("should_return_true_when_minimum_age_is_set_to_zero",
             ma("A usenet release is accepted when minimum age is 0", 0, 100, True)),
            ("should_return_true_when_age_is_greater_than_minimum_age",
             ma("A 100-minute-old usenet release is accepted at a 30-minute minimum age", 30, 100, True)),
            ("should_return_false_when_age_is_less_than_minimum_age",
             ma("A 10-minute-old usenet release is rejected at a 30-minute minimum age", 30, 10, False)),
        ])

# --- MonitoredMovieSpecificationFixture ------------------------------------
fixture("MonitoredMovieSpecificationFixture.cs", "MonitoredMovieSpecification",
        {"movie": {"monitored": True}, "search": RSS},
        [
            ("setup_should_return_monitored_episode_should_return_true", [{
                "b": "A monitored movie is accepted on RSS", "g": {}, "e": {"accepted": True},
                "n": "two asserts on identical inputs (single and multi RemoteMovie share the movie)"}]),
            ("not_monitored_series_should_be_skipped", [{
                "b": "An unmonitored movie is rejected on RSS",
                "g": {"movie": {"monitored": False}}, "e": {"accepted": False}}]),
            ("only_episode_not_monitored_should_return_false", [{
                "b": "An unmonitored movie is rejected on RSS (second variant)",
                "g": {"movie": {"monitored": False}}, "e": {"accepted": False},
                "n": "same inputs as not_monitored_series_should_be_skipped"}]),
            ("should_return_true_for_single_episode_search", [{
                "b": "An unmonitored movie is accepted in a user-invoked search",
                "g": {"movie": {"monitored": False}, "search": USER_SEARCH}, "e": {"accepted": True}}]),
        ])

# --- RssSync/DelaySpecificationFixture -------------------------------------
def d(b, ok, extra, n=None):
    row = {"b": b, "g": extra, "e": {"accepted": ok}}
    if n:
        row["n"] = n
    return [row]


UPGRADE_STUBS = {"stubs": {"is_upgradable": "None", "is_revision_upgrade": True}}
STUB_NOTE = ("the test stubs IUpgradableSpecification: IsUpgradable returns UpgradeableRejectReason.None "
             "and IsRevisionUpgrade returns true")
fixture("RssSync/DelaySpecificationFixture.cs", "DelaySpecification",
        {"profile": {"items_allowed": ["HDTV-720p", "WEBDL-720p", "Bluray-720p"], "cutoff": "WEBDL-720p"},
         "delay_profile": {"preferred_protocol": "usenet"},
         "release": {"protocol": "usenet"}, "pending_releases": [], "search": RSS},
        [
            ("should_be_true_when_user_invoked_search", [{
                "b": "Accepted in a user-invoked search, regardless of delay",
                "g": {"remote_movie": "empty", "search": USER_SEARCH}, "e": {"accepted": True}, "replace": True,
                "n": "the reference passes an empty RemoteMovie, not the fixture one"}]),
            ("should_be_false_when_system_invoked_search_and_release_is_younger_than_delay",
             d("Rejected in a system-invoked search when a fresh SDTV release is inside the 720-minute usenet delay",
               False, {"search": SEARCH, "parsed_quality": Q("SDTV"), "release": {"age_minutes": 0},
                       "delay_profile": {"usenet_delay": 720}})),
            ("should_be_true_when_profile_does_not_have_a_delay",
             d("Accepted when the delay profile's usenet delay is 0", True, {"delay_profile": {"usenet_delay": 0}})),
            ("should_be_false_when_quality_is_last_allowed_in_profile_and_bypass_disabled",
             d("Rejected inside the delay even at the profile's highest quality when bypass is off", False,
               {"parsed_quality": Q("Bluray-720p"), "release": {"age_minutes": 0},
                "delay_profile": {"usenet_delay": 720, "bypass_if_highest_quality": False}},
               "bypass_if_highest_quality is not set by the test (Builder<DelayProfile>.CreateNew() fills it); false is "
               "inferred from the test name and its passing BeFalse assertion, since with the bypass on a "
               "Bluray-720p release (the profile's highest allowed quality) would be accepted")),
            ("should_be_true_when_quality_is_last_allowed_in_profile_and_bypass_enabled",
             d("Accepted inside the delay at the profile's highest quality when bypass-if-highest is on", True,
               {"parsed_quality": Q("Bluray-720p"), "release": {"age_minutes": 0},
                "delay_profile": {"usenet_delay": 720, "bypass_if_highest_quality": True}})),
            ("should_be_true_when_release_is_older_than_delay",
             d("Accepted when the release (10 h old) is older than the 60-minute delay", True,
               {"parsed_quality": Q("HDTV-720p"), "release": {"age_minutes": 600}, "delay_profile": {"usenet_delay": 60}})),
            ("should_be_false_when_release_is_younger_than_delay",
             d("Rejected when a fresh release is inside the 720-minute usenet delay", False,
               {"parsed_quality": Q("SDTV"), "release": {"age_minutes": 0}, "delay_profile": {"usenet_delay": 720}})),
            ("should_be_true_when_release_is_a_proper_for_existing_movie",
             d("Accepted inside the delay when the release is a proper (v2) of the existing file's quality", True,
               merge({"parsed_quality": Q("HDTV-720p", version=2), "release": {"age_minutes": 0},
                      "existing_file": {"quality": Q("HDTV-720p")}, "delay_profile": {"usenet_delay": 720}},
                     UPGRADE_STUBS), STUB_NOTE)),
            ("should_be_true_when_release_is_a_real_for_existing_movie",
             d("Accepted inside the delay when the release is a REAL of the existing file's quality", True,
               merge({"parsed_quality": Q("HDTV-720p", real=1), "release": {"age_minutes": 0},
                      "existing_file": {"quality": Q("HDTV-720p")}, "delay_profile": {"usenet_delay": 720}},
                     UPGRADE_STUBS), STUB_NOTE)),
            ("should_be_false_when_release_is_proper_for_existing_movie_of_different_quality",
             d("Rejected inside the delay when the proper (HDTV-720p v2) is for a different quality than the file (SDTV)",
               False, {"parsed_quality": Q("HDTV-720p", version=2), "release": {"age_minutes": 0},
                       "existing_file": {"quality": Q("SDTV")}, "delay_profile": {"usenet_delay": 720}},
               "IUpgradableSpecification is not stubbed here (Moq defaults)")),
            ("should_be_false_when_custom_format_score_is_above_minimum_but_bypass_disabled",
             d("Rejected inside the delay with format score 100 over minimum 50 when the score bypass is off", False,
               {"release": {"age_minutes": 0, "custom_format_score": 100},
                "delay_profile": {"usenet_delay": 720, "minimum_custom_format_score": 50,
                                  "bypass_if_above_custom_format_score": False}},
               "bypass_if_above_custom_format_score is not set by the test (Builder<DelayProfile>.CreateNew() fills it); "
               "false is inferred from the test name and its passing BeFalse assertion, since with the bypass on "
               "score 100 >= minimum 50 would be accepted")),
            ("should_be_false_when_custom_format_score_is_above_minimum_and_bypass_enabled_but_under_minimum",
             d("Rejected inside the delay when the score bypass is on but the score (5) is under the minimum (50)", False,
               {"release": {"age_minutes": 0, "custom_format_score": 5},
                "delay_profile": {"usenet_delay": 720, "bypass_if_above_custom_format_score": True,
                                  "minimum_custom_format_score": 50}})),
            ("should_be_true_when_custom_format_score_is_above_minimum_and_bypass_enabled",
             d("Accepted inside the delay when the score bypass is on and the score (100) reaches the minimum (50)", True,
               {"release": {"age_minutes": 0, "custom_format_score": 100},
                "delay_profile": {"usenet_delay": 720, "bypass_if_above_custom_format_score": True,
                                  "minimum_custom_format_score": 50}})),
        ],
        note=("delay values are minutes; fields the test does not set (other delay-profile fields, "
              "quality profile name etc.) are NBuilder-filled in the reference and left out here"))

# --- RssSync/IndexerTagSpecificationFixture --------------------------------
def it(b, idx_tags, movie_tags, ok, indexer_id=None, extra_search=None):
    g = {"indexer": {"tags": idx_tags}, "movie": {"tags": movie_tags}}
    if indexer_id is not None:
        g["release"] = {"indexer_id": indexer_id}
    if extra_search:
        g["search"] = extra_search
    return [{"b": b, "g": g, "e": {"accepted": ok}}]


MEO = {"kind": "search", "monitored_episodes_only": True}
fixture("RssSync/IndexerTagSpecificationFixture.cs", "IndexerTagSpecification",
        {"indexer": {"id": 1}, "unknown_indexer_ids": "ModelNotFoundException",
         "movie": {"monitored": True}, "release": {"indexer_id": 1}, "search": SEARCH},
        [
            ("indexer_and_movie_without_tags_should_return_true",
             it("Accepted when neither the indexer nor the movie has tags", [], [], True)),
            ("indexer_with_tags_movie_without_tags_should_return_false",
             it("Rejected when the indexer has tags and the movie has none", [123], [], False)),
            ("indexer_without_tags_movie_with_tags_should_return_true",
             it("Accepted when the indexer has no tags and the movie has some", [], [123], True)),
            ("indexer_with_tags_movie_with_matching_tags_should_return_true",
             it("Accepted when indexer and movie share a tag", [123, 456], [123, 789], True)),
            ("indexer_with_tags_movie_with_different_tags_should_return_false",
             it("Rejected when indexer and movie tags do not overlap", [456], [123, 789], False)),
            ("release_without_indexerid_should_return_true",
             it("Accepted when the release has no indexer id (0)", [456], [123, 789], True, 0, MEO)),
            ("release_with_invalid_indexerid_should_return_true",
             it("Accepted when the release's indexer id is not a known indexer", [456], [123, 789], True, 2, MEO)),
        ],
        note="indexer 1 is the only known indexer; any other id throws ModelNotFoundException from the factory")

# --- Search/MovieSpecificationFixture --------------------------------------
fixture("Search/MovieSpecificationFixture.cs", "MovieSpecification",
        {"movie": {"id": 1}, "search": SEARCH},
        [
            ("should_return_false_if_series_doesnt_match", [{
                "b": "Rejected in a search when the release maps to a different movie than the one searched",
                "g": {"search": {"movie_id": 2}}, "e": {"accepted": False}}]),
            ("should_return_true_when_series_ids_match", [{
                "b": "Accepted in a search when the release maps to the searched movie",
                "g": {"search": {"movie_id": 1}}, "e": {"accepted": True}}]),
        ])


# --- machinery -------------------------------------------------------------
ATTR = re.compile(r"^\s*\[(Test|TestCase)\b(?:\((.*)\))?\]\s*$")
METHOD = re.compile(r"^\s*public\s+void\s+(\w+)\s*\(")


def active_lines(text):
    """Source lines with //- and /* */-comments blanked (line numbers kept)."""
    out, in_block = [], False
    for line in text.splitlines():
        if in_block:
            if "*/" in line:
                in_block, line = False, line.split("*/", 1)[1]
            else:
                line = ""
        if "/*" in line:
            line, in_block = line.split("/*", 1)[0], "*/" not in line.split("/*", 1)[1]
        out.append(line.split("//", 1)[0])
    return out


def scan(lines):
    """method -> [(line_no, kind, args)] for every attributed test method."""
    tests, pending = {}, []
    for i, line in enumerate(lines, 1):
        m = ATTR.match(line)
        if m:
            pending.append((i, m.group(1), m.group(2)))
            continue
        m = METHOD.match(line)
        if m and pending:
            tests[m.group(1)] = pending
        if line.strip() and not line.strip().startswith("["):
            pending = []
    return tests


def parse_args(s):
    vals = []
    for tok in (t.strip() for t in s.split(",")):
        vals.append({"true": True, "false": False}[tok] if tok in ("true", "false") else int(tok))
    return tuple(vals)


def canon(o):
    return json.dumps(o, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


def build():
    entries = []  # dicts with key, behaviour, ref, in_scope, case
    for fx in FIXTURES:
        rel = f"{TEST_DIR}/{fx['path']}"
        src = (REF_REPO / rel).read_text(encoding="utf-8-sig")
        found = scan(active_lines(src))
        covered = [m for m, _ in fx["tests"]]
        missing = sorted(set(found) - set(covered))
        extra = sorted(set(covered) - set(found))
        assert not missing and not extra, f"{rel}: uncovered {missing}, unknown {extra}"
        for method, rows_spec in fx["tests"]:
            attrs = found[method]
            cases = [a for a in attrs if a[1] == "TestCase"]
            if cases:
                assert callable(rows_spec), f"{rel} {method}: TestCase rows need a function"
                rows = [(line, r) for line, _, a in cases for r in rows_spec(*parse_args(a))]
            else:
                line = next(n for n, k, _ in attrs if k == "Test")
                rows = [(line, r) for r in rows_spec]
            for idx, (line, r) in enumerate(rows):
                ref = f"{TAG} {rel}:{line}"
                case = None
                if "out" not in r:
                    given = r["g"] if r.get("replace") else merge(fx["base"], r["g"])
                    case = {"input": {"spec": fx["spec"], "given": given}, "expect": r["e"]}
                    note = "; ".join(x for x in (r.get("n"), fx["note"]) if x)
                    if note:
                        case["note"] = note
                h = hashlib.sha1(canon(case["input"] if case else None).encode()).hexdigest()[:12]
                entries.append({"key": f"{rel}|{method}|{idx}|{h}", "behaviour": r["b"], "ref": ref,
                                "in_scope": case is not None, "case": case, "fixture": fx["path"],
                                "out": r.get("out"), "method": method})
    return entries


def assign_ids(entries):
    old, nxt = {}, ID_MIN
    if CATALOGUE.exists():
        text = CATALOGUE.read_text()
        old = dict(re.findall(r"^# key: (.+)\n- id: (\S+)$", text, re.M))
        m = re.search(r"^# next-id: (\d+)$", text, re.M)  # survives dropped entries: IDs are never reused
        nxt = max([nxt, int(m.group(1)) if m else 0] + [int(v.rsplit("-", 1)[1]) + 1 for v in old.values()])
    for e in entries:
        if e["key"] in old:
            e["id"] = old[e["key"]]
        else:
            e["id"] = ID_FMT.format(nxt)
            nxt += 1
    assert nxt - 1 <= ID_MAX, "ID range RAD-DECISION-0001..0999 exhausted"
    return nxt


def kept_lines():
    """{id: [status/decision/tests lines as written]} from the current catalogue (ratchet-owned, never regenerated)."""
    if not CATALOGUE.exists():
        return {}
    return {m[0]: re.findall(r"(?m)^  ((?:status|decision|tests):.*(?:\n    .*)*)", m[1])
            for m in re.findall(r"(?m)^- id: (\S+)\n((?:  .*\n?|    .*\n?)*)", CATALOGUE.read_text())}


def render(entries, nxt):
    q = lambda s: json.dumps(s, ensure_ascii=False)  # noqa: E731  JSON strings are valid YAML scalars
    out = [f"# Generated by parity/extract/radarr_decision_a.py from {TAG} {TEST_DIR} fixtures A-M.",
           "# Re-run the script; do not hand-edit IDs or keys (status/tests may be edited).",
           f"# next-id: {nxt}", ""]
    kept = kept_lines()
    for e in entries:
        out += [f"# key: {e['key']}", f"- id: {e['id']}", f"  behaviour: {q(e['behaviour'])}",
                "  source: reference", f"  ref: {q(e['ref'])}", f"  in_scope: {str(e['in_scope']).lower()}",
                *[f"  {k}" for k in kept.get(e["id"], ["status: unmapped", "tests: []"])]]
    yaml = "\n".join(out) + "\n"
    corpora = {}
    for e in entries:
        if e["case"]:
            name = Path(e["fixture"]).stem + ".json"
            corpora.setdefault(name, []).append({"id": e["id"], "ref": e["ref"], **e["case"]})
    files = {CATALOGUE: yaml}
    for name, cases in corpora.items():
        files[CORPUS_DIR / name] = json.dumps(cases, indent=1, ensure_ascii=False) + "\n"
    return files


def main():
    entries = build()
    files = render(entries, assign_ids(entries))
    if "--check" in sys.argv:
        stale = [str(p) for p, t in files.items() if not p.exists() or p.read_text() != t]
        sys.exit(f"stale: {stale}" if stale else 0)
    CORPUS_DIR.mkdir(parents=True, exist_ok=True)
    for p, t in files.items():
        p.write_text(t)
    ins = [e for e in entries if e["in_scope"]]
    print(f"{len(entries)} entries ({len(ins)} in scope, {len(entries) - len(ins)} out), "
          f"{entries[0]['id']}..{entries[-1]['id']}, {len(files) - 1} corpus files")
    for e in entries:
        if not e["in_scope"]:
            print(f"  skipped {e['id']} {e['fixture']}::{e['method']}: {e['out']}")


if __name__ == "__main__":
    main()
