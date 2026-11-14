"""Port Radarr grab-decision fixtures N-Z (DecisionEngineTests/**/[N-Z]*Fixture.cs)
into parity/corpus/reference/radarr/decision/*.json and
parity/catalogue/decision.radarr-b.yaml. Fixtures A-M (including RssSync/Delay,
RssSync/IndexerTag, Search/Movie) belong to radarr_decision_a.py.

Run: python3 -I parity/extract/radarr_decision_b.py [--check]
(--check: fail if the outputs on disk differ from what would be written.)

Case data below is hand-ported from each test body; this script looks up the
[Test]/[TestCase] line (or the TestCaseSource row line) of every case in the
reference source, checks that every attributed test in each fixture is covered,
parses [TestCase]/TestCaseSource arguments as C# literals, and keeps IDs stable
across re-runs (key: ref path | method | row | input hash, stored as a
`# key:` comment above each catalogue entry).

Conventions (same as radarr_decision_a.py): quality names are Radarr's own
("HDTV-720p"); "revision" only when the test sets one (C# default is version 1,
real 0, not a repack). size_mb = n * 1024 * 1024 bytes (.Megabytes()).
Profile fields the test builds but does not set are written out with their C#
default (cutoff id 0 is "Unknown", upgrade_allowed false). Config the spec reads
but the test does not set is written out with its Moq default (first enum value).
"items" (full profile order) is given only when the test builds the profile with
GetDefaultQualities(subset), which moves the allowed qualities to the end.
Prioritization cases list releases by the test's variable names; "order" is the
prefix of the sorted result the test asserts.
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
CATALOGUE = ROOT / "parity/catalogue/decision.radarr-b.yaml"
CORPUS_DIR = ROOT / "parity/corpus/reference/radarr/decision"
ID_FMT, ID_MIN, ID_MAX = "RAD-DECISION-{:04d}", 1000, 1999

RSS = {"kind": "rss"}
SEARCH = {"kind": "search"}

# Quality identifier -> display name, read from the reference itself.
QUALITY_NAMES = dict(re.findall(
    r'public static Quality (\w+) => new Quality\(\d+, "([^"]+)"',
    (REF_REPO / "src/NzbDrone.Core/Qualities/Quality.cs").read_text(encoding="utf-8-sig")))


def Q(name, version=None, real=None, is_repack=None):
    q = {"quality": name}
    rev = {k: v for k, v in (("version", version), ("real", real), ("is_repack", is_repack)) if v is not None}
    if rev:
        q["revision"] = rev
    return q


class R(dict):
    """A given value that replaces the base value instead of merging into it."""


def merge(base, delta):
    out = dict(base)
    for k, v in delta.items():
        # A QualityModel ({"quality", "revision"?}) is always set whole: never inherit the base revision.
        if type(v) is dict and isinstance(out.get(k), dict) and not isinstance(v.get("quality"), str):
            out[k] = merge(out[k], v)
        else:
            out[k] = dict(v) if isinstance(v, R) else v
    return out


def acc(ok):
    return "accepted" if ok else "rejected"


DEFAULT_QUALITIES = [  # QualityFixture.GetDefaultQualities() order (QualityFixture.cs:68)
    "Unknown", "CAM", "TELECINE", "DVDSCR", "REGIONAL", "SDTV", "DVD", "DVD-R", "HDTV-720p",
    "HDTV-1080p", "HDTV-2160p", "WEBDL-480p", "WEBDL-720p", "WEBDL-1080p", "WEBDL-2160p",
    "Bluray-480p", "Bluray-576p", "Bluray-720p", "Bluray-1080p", "Bluray-2160p", "BR-DISK", "Raw-HD",
]


def default_qualities(*allowed):
    """GetDefaultQualities(allowed...): the rest in default order, then the allowed ones in the given order."""
    if not allowed:
        return {"items_allowed": list(DEFAULT_QUALITIES)}
    return {"items": [q for q in DEFAULT_QUALITIES if q not in allowed] + list(allowed),
            "items_allowed": list(allowed)}


PAU = {"config": {"download_propers_and_repacks": "PreferAndUpgrade"}}
MOQ_NOTE = ("download_propers_and_repacks is the Moq default PreferAndUpgrade (first value of "
            "src/NzbDrone.Core/Qualities/ProperDownloadTypes.cs:5) unless the test sets it")
NB_NOTE = "movie id 1 is the NBuilder default for the first built Movie"

# A test entry: (method, rows). A row is a dict:
#   b: behaviour, g: given delta (merged onto the fixture base), e: expect,
#   n: note, out: reason (in_scope false, no corpus case).
# For [TestCase]/TestCaseSource methods, rows is a function of the parsed argument tuple.

FIXTURES = []


def fixture(path, spec, base, tests, note=None):
    FIXTURES.append({"path": path, "spec": spec, "base": base, "tests": tests, "note": note})


def one(b, g, e, n=None):
    row = {"b": b, "g": g, "e": e}
    if n:
        row["n"] = n
    return [row]


# --- PrioritizeDownloadDecisionFixture -------------------------------------
CF1, CF2 = "My Format 1", "My Format 2"


def rm(label, quality, age_days=0, size_mb=0, protocol="usenet", indexer_priority=25, **extra):
    """GivenRemoteMovie(...) defaults (PrioritizeDownloadDecisionFixture.cs:54)."""
    r = {"label": label, "quality": quality, "age_days": age_days, "size_mb": size_mb, "protocol": protocol,
         "indexer_priority": indexer_priority, "title": "A Movie 1998", "custom_formats": [],
         "custom_format_score": 0}
    r.update(extra)
    return r


def torrent(label, quality, **extra):
    """Release replaced by a new TorrentInfo (age 0, size 0, torrent; IndexerPriority left at C# default 0)."""
    return rm(label, quality, protocol="torrent", indexer_priority=0, release_class="TorrentInfo", **extra)


def prio(b, releases, order, extra=None, n=None):
    return one(b, merge({"releases": releases}, extra or {}), {"order": order}, n)


H7, W10, B7 = Q("HDTV-720p"), Q("WEBDL-1080p"), Q("Bluray-720p")
fixture("PrioritizeDownloadDecisionFixture.cs", "DownloadDecisionPriorizationService.PrioritizeDecisionsForMovies",
        merge({"delay_profile": {"preferred_protocol": "usenet"},
               "quality_definition": {"preferred_size": None},
               "profile": {**default_qualities(), "cutoff": "Unknown", "upgrade_allowed": False,
                           "format_items": [{"format": CF1, "score": 1}, {"format": CF2, "score": 2}],
                           "min_format_score": 0},
               "movie": {"title": "A Movie", "runtime": 150}}, PAU),
        [
            ("should_put_reals_before_non_reals",
             prio("A REAL release sorts before the same quality without REAL",
                  [rm("remoteMovie1", Q("HDTV-720p", version=1, real=0)),
                   rm("remoteMovie2", Q("HDTV-720p", version=1, real=1))], ["remoteMovie2"])),
            ("should_put_propers_before_non_propers",
             prio("A proper (v2) sorts before the same quality v1",
                  [rm("remoteMovie1", Q("HDTV-720p", version=1)), rm("remoteMovie2", Q("HDTV-720p", version=2))],
                  ["remoteMovie2"])),
            ("should_put_higher_quality_before_lower",
             prio("HDTV-720p sorts before SDTV",
                  [rm("remoteMovie1", Q("SDTV")), rm("remoteMovie2", H7)], ["remoteMovie2"])),
            ("should_order_by_age_then_largest_rounded_to_200mb",
             prio("Among HDTV-720p releases with no preferred size, the young 3000 MB one sorts first",
                  [rm("remoteMovieSd", Q("SDTV"), size_mb=100, age_days=1),
                   rm("remoteMovieHdSmallOld", H7, size_mb=1200, age_days=1000),
                   rm("remoteMovieSmallYoung", H7, size_mb=1250, age_days=10),
                   rm("remoteMovieHdLargeYoung", H7, size_mb=3000, age_days=1)], ["remoteMovieHdLargeYoung"])),
            ("should_order_by_closest_to_preferred_size_if_both_over",
             prio("With preferred size 2 MB/min x 150 min, a 400 MB release sorts before a 15000 MB one",
                  [rm("remoteMovieSmall", H7, size_mb=400, age_days=1),
                   rm("remoteMovieLarge", H7, size_mb=15000, age_days=1)], ["remoteMovieSmall"],
                  {"quality_definition": {"preferred_size": 2}})),
            ("should_order_by_largest_to_if_zero_runtime",
             prio("With runtime 0 the preferred size is ignored and the larger release sorts first",
                  [rm("remoteMovieSmall", H7, size_mb=400, age_days=1),
                   rm("remoteMovieLarge", H7, size_mb=15000, age_days=1)], ["remoteMovieLarge"],
                  {"quality_definition": {"preferred_size": 2}, "movie": {"runtime": 0}})),
            ("should_order_by_closest_to_preferred_size_if_both_under",
             prio("With preferred size 390 MB/min x 150 min, a 15000 MB release sorts before a 100 MB one",
                  [rm("remoteMovieSmall", H7, size_mb=100, age_days=1),
                   rm("remoteMovieLarge", H7, size_mb=15000, age_days=1)], ["remoteMovieLarge"],
                  {"quality_definition": {"preferred_size": 390}})),
            ("should_order_by_closest_to_preferred_size_if_preferred_is_in_between",
             prio("With preferred size 46 MB/min x 150 min (6900 MB), the 7000 MB release sorts first",
                  [rm("remoteMovie1", H7, size_mb=100, age_days=1), rm("remoteMovie2", H7, size_mb=500, age_days=1),
                   rm("remoteMovie3", H7, size_mb=7000, age_days=1), rm("remoteMovie4", H7, size_mb=15000, age_days=1)],
                  ["remoteMovie3"], {"quality_definition": {"preferred_size": 46}})),
            ("should_order_by_youngest",
             prio("A 5-day-old release sorts before a 10-day-old one",
                  [rm("remoteMovie1", H7, age_days=10), rm("remoteMovie2", H7, age_days=5)], ["remoteMovie2"])),
            ("should_put_usenet_above_torrent_when_usenet_is_preferred",
             prio("With usenet preferred, the usenet release sorts before the torrent",
                  [rm("remoteMovie1", H7, protocol="torrent"), rm("remoteMovie2", H7, protocol="usenet")],
                  ["remoteMovie2"])),
            ("should_put_torrent_above_usenet_when_torrent_is_preferred",
             prio("With torrent preferred, the torrent release sorts before the usenet one",
                  [rm("remoteMovie1", H7, protocol="torrent"), rm("remoteMovie2", H7, protocol="usenet")],
                  ["remoteMovie1"], {"delay_profile": {"preferred_protocol": "torrent"}})),
            ("should_prefer_releases_with_more_seeders",
             prio("A torrent with 100 seeders sorts before one with 10",
                  [torrent("remoteMovie1", H7, seeders=10), torrent("remoteMovie2", H7, seeders=100)],
                  ["remoteMovie2"])),
            ("should_prefer_releases_with_more_peers_given_equal_number_of_seeds",
             prio("With equal seeders (10), a torrent with 100 peers sorts before one with 10",
                  [torrent("remoteMovie1", H7, seeders=10, peers=10), torrent("remoteMovie2", H7, seeders=10, peers=100)],
                  ["remoteMovie2"])),
            ("should_prefer_releases_with_more_peers_no_seeds",
             prio("With no seeders, a torrent with 100 peers sorts before one with 10",
                  [torrent("remoteMovie1", H7, seeders=0, peers=10), torrent("remoteMovie2", H7, seeders=0, peers=100)],
                  ["remoteMovie2"])),
            ("should_prefer_first_release_if_peers_and_size_are_too_similar",
             prio("Torrents with 1000 vs 1100 seeders, equal peers and sizes 250 vs 200 MB keep input order",
                  [torrent("remoteMovie1", H7, seeders=1000, peers=10, size_mb=250),
                   torrent("remoteMovie2", H7, seeders=1100, peers=10, size_mb=200)], ["remoteMovie1"],
                  n=("the test clones torrentInfo1 at 200 MB, then sets torrentInfo1 (not 2) to 250 MB; "
                     "the sizes here are what the objects hold"))),
            ("should_prefer_first_release_if_age_and_size_are_too_similar",
             prio("Releases 100 vs 150 days old and 200 vs 250 MB keep input order",
                  [rm("remoteMovie1", H7, age_days=100, size_mb=200), rm("remoteMovie2", H7, age_days=150, size_mb=250)],
                  ["remoteMovie1"], n="publish dates here use UtcNow; the GivenRemoteMovie default uses Now")),
            ("should_prefer_better_custom_format",
             prio("A release matching My Format 1 (score 1) sorts before one with no formats",
                  [rm("remoteMovie1", B7), rm("remoteMovie2", B7, custom_formats=[CF1], custom_format_score=1)],
                  ["remoteMovie2"])),
            ("should_prefer_better_custom_format2",
             prio("A release matching My Format 2 (score 2) sorts before one matching My Format 1 (score 1)",
                  [rm("remoteMovie1", B7, custom_formats=[CF1], custom_format_score=1),
                   rm("remoteMovie2", B7, custom_formats=[CF2], custom_format_score=2)], ["remoteMovie2"])),
            ("should_prefer_2_custom_formats",
             prio("A release matching both formats (score 3) sorts before one matching My Format 1 (score 1)",
                  [rm("remoteMovie1", B7, custom_formats=[CF1], custom_format_score=1),
                   rm("remoteMovie2", B7, custom_formats=[CF1, CF2], custom_format_score=3)], ["remoteMovie2"])),
            ("should_prefer_proper_over_score_when_download_propers_is_prefer_and_upgrade",
             prio("With PreferAndUpgrade, a proper with score 0 sorts before a v1 with score 10",
                  [rm("remoteMovie1", Q("WEBDL-1080p", version=1), custom_format_score=10),
                   rm("remoteMovie2", Q("WEBDL-1080p", version=2), custom_format_score=0)], ["remoteMovie2"])),
            ("should_prefer_proper_over_score_when_download_propers_is_do_not_upgrade",
             prio("With DoNotUpgrade, a proper with score 0 still sorts before a v1 with score 10",
                  [rm("remoteMovie1", Q("WEBDL-1080p", version=1), custom_format_score=10),
                   rm("remoteMovie2", Q("WEBDL-1080p", version=2), custom_format_score=0)], ["remoteMovie2"],
                  {"config": {"download_propers_and_repacks": "DoNotUpgrade"}})),
            ("should_prefer_score_over_proper_when_download_propers_is_do_not_prefer",
             prio("With DoNotPrefer, a v1 with score 10 sorts before a proper with score 0",
                  [rm("remoteMovie1", Q("WEBDL-1080p", version=1), custom_format_score=10),
                   rm("remoteMovie2", Q("WEBDL-1080p", version=2), custom_format_score=0)], ["remoteMovie1"],
                  {"config": {"download_propers_and_repacks": "DoNotPrefer"}},
                  "the test asserts the first result's quality, version 1 and score 10")),
            ("should_prefer_score_over_real_when_download_propers_is_do_not_prefer",
             prio("With DoNotPrefer, a non-REAL with score 10 sorts before a REAL with score 0",
                  [rm("remoteMovie1", Q("WEBDL-1080p", version=1, real=0), custom_format_score=10),
                   rm("remoteMovie2", Q("WEBDL-1080p", version=1, real=1), custom_format_score=0)], ["remoteMovie1"],
                  {"config": {"download_propers_and_repacks": "DoNotPrefer"}},
                  "the test asserts the first result's quality, version 1, real 0 and score 10")),
            ("sort_download_decisions_based_on_indexer_priority",
             prio("Equal releases sort by indexer priority, lowest number first (1, 25, 50)",
                  [rm("remoteMovie1", W10, indexer_priority=25), rm("remoteMovie2", W10, indexer_priority=50),
                   rm("remoteMovie3", W10, indexer_priority=1)], ["remoteMovie3", "remoteMovie1", "remoteMovie2"])),
            ("ensure_download_decisions_indexer_priority_is_not_perfered_over_quality",
             prio("Quality outranks indexer priority; indexer priority breaks ties within a quality",
                  [rm("remoteMovie1", H7, indexer_priority=25), rm("remoteMovie2", W10, indexer_priority=50),
                   rm("remoteMovie3", Q("SDTV"), indexer_priority=1), rm("remoteMovie4", W10, indexer_priority=25)],
                  ["remoteMovie4", "remoteMovie2", "remoteMovie1", "remoteMovie3"])),
        ],
        note=("each release has its own Movie with the same profile; format scores come from "
              "GetSampleFormatItems(My Format 1, My Format 2) = 1 and 2; quality_definition.preferred_size is "
              "MB per minute; age_days is days before now; expect.order is the prefix of the sorted result the "
              "test asserts, not necessarily the full order; " + MOQ_NOTE))

# --- ProtocolSpecificationFixture ------------------------------------------
fixture("ProtocolSpecificationFixture.cs", "ProtocolSpecification", {"delay_profile": {}},
        [
            ("should_be_true_if_usenet_and_usenet_is_enabled",
             one("A usenet release is accepted when the delay profile enables usenet",
                 {"release": {"protocol": "usenet"}, "delay_profile": {"enable_usenet": True}}, {"accepted": True})),
            ("should_be_true_if_torrent_and_torrent_is_enabled",
             one("A torrent release is accepted when the delay profile enables torrent",
                 {"release": {"protocol": "torrent"}, "delay_profile": {"enable_torrent": True}}, {"accepted": True})),
            ("should_be_false_if_usenet_and_usenet_is_disabled",
             one("A usenet release is rejected when the delay profile disables usenet",
                 {"release": {"protocol": "usenet"}, "delay_profile": {"enable_usenet": False}}, {"accepted": False})),
            ("should_be_false_if_torrent_and_torrent_is_disabled",
             one("A torrent release is rejected when the delay profile disables torrent",
                 {"release": {"protocol": "torrent"}, "delay_profile": {"enable_torrent": False}}, {"accepted": False})),
        ],
        note="the delay profile comes from BestForTags(movie tags); fields the test does not set are left out")

# --- QualityAllowedByProfileSpecificationFixture ---------------------------
QA_ALLOWED = ("DVD", "HDTV-720p", "Bluray-1080p")
fixture("QualityAllowedByProfileSpecificationFixture.cs", "QualityAllowedByProfileSpecification",
        {"profile": {"cutoff": "Bluray-1080p", **default_qualities(*QA_ALLOWED)}},
        [
            ("should_allow_if_quality_is_defined_in_profile", lambda q: one(
                f"A {q} release is accepted when the profile allows DVD, HDTV-720p and Bluray-1080p",
                {"parsed_quality": Q(q, version=2)}, {"accepted": True})),
            ("should_not_allow_if_quality_is_not_defined_in_profile", lambda q: one(
                f"A {q} release is rejected when the profile allows only DVD, HDTV-720p and Bluray-1080p",
                {"parsed_quality": Q(q, version=2)}, {"accepted": False})),
        ],
        note="the test swaps only the quality of the setup's DVD v2 parsed quality, so the revision stays version 2")

# --- QueueSpecificationFixture ---------------------------------------------
def queued(quality=None, movie_id=1, state="Downloading", **extra):
    item = {"movie_id": movie_id, "tracked_download_state": state}
    if quality:
        item["quality"] = quality
    item.update(extra)
    return item


def qs(b, queue, ok, profile=None, extra=None):
    g = {"queue": queue}
    if profile:
        g["profile"] = profile
    return one(b, merge(g, extra or {}), {"accepted": ok})


fixture("QueueSpecificationFixture.cs", "QueueSpecification",
        merge({"movie": {"id": 1},
               "profile": {**default_qualities(), "cutoff": "Unknown", "format_items": [], "min_format_score": 0,
                           "upgrade_allowed": True},
               "parsed_quality": Q("DVD"), "release": {"custom_formats": []}, "queue": []}, PAU),
        [
            ("should_return_true_when_queue_is_empty", qs("Accepted when the download queue is empty", [], True)),
            ("should_return_true_when_movie_doesnt_match",
             qs("Accepted when the queued download is for a different movie", [queued(movie_id=2)], True)),
            ("should_return_true_when_quality_in_queue_is_lower",
             qs("Accepted when the queued download (SDTV) is lower than the release (DVD) and below cutoff",
                [queued(Q("SDTV"))], True, {"cutoff": "Bluray-1080p"})),
            ("should_return_false_when_qualities_are_the_same",
             qs("Rejected when the queued download has the same quality (DVD) as the release", [queued(Q("DVD"))], False)),
            ("should_return_false_when_quality_in_queue_is_better",
             qs("Rejected when the queued download (HDTV-720p) is better than the release (DVD)",
                [queued(Q("HDTV-720p"))], False, {"cutoff": "Bluray-1080p"})),
            ("should_return_false_if_quality_in_queue_meets_cutoff",
             qs("Rejected when the queued download (HDTV-720p) already meets the cutoff (DVD)",
                [queued(Q("HDTV-720p"))], False, {"cutoff": "DVD"})),
            ("should_return_false_when_quality_is_better_and_upgrade_allowed_is_false_for_quality_profile",
             qs("Rejected when the queued download (Bluray-1080p) is better and the profile disallows upgrades",
                [queued(Q("Bluray-1080p"))], False, {"cutoff": "Bluray-1080p", "upgrade_allowed": False})),
            ("should_return_true_if_everything_is_the_same_for_failed_pending",
             qs("Accepted when the matching queued download (same DVD) is in FailedPending state",
                [queued(Q("DVD"), state="FailedPending")], True, {"cutoff": "Bluray-1080p"})),
            ("should_return_false_if_same_quality_non_proper_in_queue_and_download_propers_is_do_not_upgrade",
             qs("Rejected when a proper (HDTV-720p v2) would replace the same quality in the queue and propers are DoNotUpgrade",
                [queued(Q("HDTV-720p"), languages=["English"], custom_formats=[])], False, {"cutoff": "HDTV-720p"},
                {"parsed_quality": Q("HDTV-720p", version=2),
                 "config": {"download_propers_and_repacks": "DoNotUpgrade"}})),
        ],
        note=(NB_NOTE + "; queued items' custom formats are parsed by a mock that returns none; " + MOQ_NOTE))

# --- RawDiskSpecificationFixture -------------------------------------------
def rd(b, ok, **release):
    return one(b, {"release": release}, {"accepted": ok})


fixture("RawDiskSpecificationFixture.cs", "RawDiskSpecification",
        {"release": {"title": "Movie.title.1998", "protocol": "torrent"}},
        [
            ("should_return_true_if_no_container_specified", rd("Accepted when the release has no container", True)),
            ("should_return_true_if_mkv", rd("Accepted when the container is MKV", True, container="MKV")),
            ("should_return_false_if_vob", rd("Rejected when the container is VOB", False, container="VOB")),
            ("should_return_false_if_iso", rd("Rejected when the container is ISO", False, container="ISO")),
            ("should_return_false_if_m2ts", rd("Rejected when the container is M2TS", False, container="M2TS")),
            ("should_compare_case_insensitive",
             rd("Rejected when the container is lowercase vob (case-insensitive)", False, container="vob")),
            ("should_return_false_if_matches_disc_format",
             lambda t: rd(f"Rejected as a disc release: {t}", False, title=t)),
            ("should_return_true_if_dvdrip", lambda t: rd(f"Accepted as a rip, not a disc: {t}", True, title=t)),
        ])

# --- ReleaseRestrictionsSpecificationFixture -------------------------------
def rr(b, ok, required, ignored, extra=None):
    g = {"release_profiles": [{"required": required, "ignored": ignored}]}
    return one(b, merge(g, extra or {}), {"accepted": ok})


fixture("ReleaseRestrictionsSpecificationFixture.cs", "ReleaseRestrictionsSpecification",
        {"movie": {"tags": []}, "release": {"title": "Dexter.S08E01.EDITED.WEBRip.x264-KYR"}, "release_profiles": []},
        [
            ("should_be_true_when_restrictions_are_empty",
             one("Accepted when no release profile applies", {"release_profiles": []}, {"accepted": True})),
            ("should_be_true_when_title_contains_one_required_term",
             rr("Accepted when the title contains the one required term (WEBRip)", True, ["WEBRip"], [])),
            ("should_be_false_when_title_does_not_contain_any_required_terms",
             rr("Rejected when the title contains none of the required terms", False, ["doesnt", "exist"], [])),
            ("should_be_true_when_title_does_not_contain_any_ignored_terms",
             rr("Accepted when the title contains no ignored term", True, [], ["ignored"])),
            ("should_be_false_when_title_contains_one_anded_ignored_terms",
             rr("Rejected when the title contains an ignored term (edited, case-insensitive)", False, [], ["edited"])),
            ("should_ignore_case_when_matching_required", lambda s: rr(
                f"Required terms {s.split(',')} match the title case-insensitively, so it is accepted",
                True, s.split(","), [])),
            ("should_ignore_case_when_matching_ignored", lambda s: rr(
                f"Ignored terms {s.split(',')} match the title case-insensitively, so it is rejected",
                False, [], s.split(","))),
            ("should_be_false_when_release_contains_one_restricted_word_and_one_required_word",
             rr("Rejected when the title has a required term (x264) and an ignored term (www.Speed.cd)", False,
                ["x264"], ["www.Speed.cd"],
                {"release": {"title": "[ www.Speed.cd ] -Whose.Line.is.it.Anyway.US.S10E24.720p.HDTV.x264-BAJSKORV"}})),
            ("should_match_perl_regex", lambda p, ok: [{
                "b": f"Required term {json.dumps(p, ensure_ascii=True)} is a /regex/ and the release is {acc(ok)}",
                "g": {"release_profiles": [{"required": p.split(","), "ignored": []}]}, "e": {"accepted": ok},
                **({"n": "\"/WEB\\b/\" is a regular C# string, so \\b is a backspace character (U+0008), "
                         "not a regex word boundary"} if "\b" in p else {})}]),
        ],
        note="the real TermMatcherService is used (not a mock); release profiles come from EnabledForTags(movie tags)")

# --- RepackSpecificationFixture --------------------------------------------
def rp(b, ok, file_quality=None, group="Radarr", extra=None):
    g = {"parsed_quality": {"revision": {"is_repack": True}}}
    if file_quality:
        g["movie"] = {"movie_file_id": 1}
        g["existing_file"] = {"quality": file_quality, "release_group": group}
    return one(b, merge(g, extra or {}), {"accepted": ok})


fixture("RepackSpecificationFixture.cs", "RepackSpecification",
        merge({"parsed_quality": Q("SDTV", version=2, real=0, is_repack=False), "parsed_release_group": "Radarr",
               "movie": {"movie_file_id": 0}}, PAU),
        [
            ("should_return_true_if_it_is_not_a_repack",
             one("Accepted when the release is not a repack", {}, {"accepted": True})),
            ("should_return_true_if_there_are_is_no_movie_file",
             rp("Accepted when the release is a repack and the movie has no file", True)),
            ("should_return_true_if_is_a_repack_for_a_different_quality",
             rp("Accepted when the repack (SDTV) is for a different quality than the file (DVD)", True, Q("DVD"))),
            ("should_return_true_if_is_a_repack_for_existing_file",
             rp("Accepted when the repack matches the file's quality and release group", True, Q("SDTV"))),
            ("should_return_false_if_is_a_repack_for_a_different_file",
             rp("Rejected when the repack's release group (Radarr) differs from the file's (NotRadarr)", False,
                Q("SDTV"), "NotRadarr")),
            ("should_return_false_if_release_group_for_existing_file_is_unknown",
             rp("Rejected when the existing file's release group is empty", False, Q("SDTV"), "")),
            ("should_return_false_if_release_group_for_release_is_unknown",
             rp("Rejected when the repack's release group is unknown (null)", False, Q("SDTV"),
                extra={"parsed_release_group": None})),
            ("should_return_false_when_repack_but_auto_download_repack_is_false",
             rp("Rejected when a matching repack arrives and propers/repacks are DoNotUpgrade", False, Q("SDTV"),
                extra={"config": {"download_propers_and_repacks": "DoNotUpgrade"}})),
            ("should_return_true_when_repack_but_auto_download_repack_is_true",
             rp("Accepted when a matching repack arrives and propers/repacks are PreferAndUpgrade", True, Q("SDTV"),
                extra={"config": {"download_propers_and_repacks": "PreferAndUpgrade"}})),
            ("should_return_true_when_repacks_are_not_preferred",
             rp("Accepted when propers/repacks are DoNotPrefer (the repack check is skipped)", True,
                Q("SDTV"), extra={"config": {"download_propers_and_repacks": "DoNotPrefer"}})),
        ],
        note=MOQ_NOTE)

# --- RetentionSpecificationFixture -----------------------------------------
def ret(b, ok, retention, age, extra=None):
    return one(b, merge({"config": {"retention_days": retention}, "release": {"age_days": age}}, extra or {}),
               {"accepted": ok})


fixture("RetentionSpecificationFixture.cs", "RetentionSpecification", {"release": {"protocol": "usenet"}},
        [
            ("should_return_true_when_retention_is_set_to_zero",
             ret("Accepted when retention is 0 (unlimited), even for a 100-day-old release", True, 0, 100)),
            ("should_return_true_when_release_if_younger_than_retention",
             ret("Accepted when the release (100 days) is younger than retention (1000 days)", True, 1000, 100)),
            ("should_return_true_when_release_and_retention_are_the_same",
             ret("Accepted when the release age equals retention (100 days)", True, 100, 100)),
            ("should_return_false_when_old_than_retention",
             ret("Rejected when the release (100 days) is older than retention (10 days)", False, 10, 100)),
            ("should_return_true_if_release_came_out_today_and_retention_is_zero",
             [dict(ret("Accepted when retention is 0 for a 100-day-old release", True, 0, 100)[0],
                   n="despite the name, the test sets the age to 100 days, not today")]),
            ("should_return_true_when_release_is_not_usenet",
             ret("Accepted for a torrent release regardless of retention (10 days, release 100 days)", True, 10, 100,
                 {"release": {"protocol": "torrent"}})),
        ],
        note="age_days is days before now (PublishDate = UtcNow - age)")

# --- UpgradeAllowedSpecificationFixture ------------------------------------
def side(quality, formats):
    return {"quality": quality, "custom_formats": formats}


def ua(b, allowed, cur, cur_f, new, new_f, ok):
    return one(b, {"profile": {"upgrade_allowed": allowed}, "current": side(cur, cur_f), "new": side(new, new_f)},
               {"accepted": ok})


DVD, BR10, SD = Q("DVD"), Q("Bluray-1080p"), Q("SDTV")
fixture("UpgradeAllowedSpecificationFixture.cs", "UpgradableSpecification.IsUpgradeAllowed",
        {"profile": {"cutoff": "Bluray-1080p", **default_qualities(), "upgrade_allowed": False,
                     "cutoff_format_score": 100, "format_items": [{"format": "One", "score": 50},
                                                                  {"format": "Two", "score": 100}]}},
        [
            ("should_return_false_when_quality_is_better_custom_formats_are_the_same_and_upgrading_is_not_allowed",
             ua("A quality upgrade (DVD to Bluray-1080p) is not allowed when the profile disallows upgrades",
                False, DVD, [], BR10, [], False)),
            ("should_return_false_when_quality_is_same_and_custom_format_is_upgrade_and_upgrading_is_not_allowed",
             ua("A format upgrade (One 50 to Two 100) is not allowed when the profile disallows upgrades",
                False, DVD, ["One"], DVD, ["Two"], False)),
            ("should_return_true_for_custom_format_upgrade_when_upgrading_is_allowed",
             ua("A format upgrade (One 50 to Two 100) is allowed when the profile allows upgrades",
                True, DVD, ["One"], DVD, ["Two"], True)),
            ("should_return_true_for_same_custom_format_score_when_upgrading_is_not_allowed",
             ua("The same quality and format score is allowed even when upgrades are disallowed",
                False, DVD, ["One"], DVD, ["One"], True)),
            ("should_return_true_for_lower_custom_format_score_when_upgrading_is_allowed",
             ua("A lower format score is allowed when upgrades are allowed", True, DVD, ["Two"], DVD, ["One"], True)),
            ("should_return_true_for_lower_language_when_upgrading_is_not_allowed",
             ua("A lower format score is allowed even when upgrades are disallowed (not an upgrade)",
                False, DVD, ["Two"], DVD, ["One"], True)),
            ("should_return_true_for_quality_upgrade_when_upgrading_is_allowed",
             ua("A quality upgrade (DVD to Bluray-1080p) is allowed when the profile allows upgrades",
                True, DVD, [], BR10, [], True)),
            ("should_return_true_for_same_quality_when_upgrading_is_allowed",
             ua("The same quality is allowed when upgrades are allowed", True, DVD, [], DVD, [], True)),
            ("should_return_true_for_same_quality_when_upgrading_is_not_allowed",
             ua("The same quality is allowed when upgrades are disallowed", False, DVD, [], DVD, [], True)),
            ("should_return_true_for_lower_quality_when_upgrading_is_allowed",
             ua("A lower quality (DVD to SDTV) is allowed when upgrades are allowed", True, DVD, [], SD, [], True)),
            ("should_return_true_for_lower_quality_when_upgrading_is_not_allowed",
             ua("A lower quality (DVD to SDTV) is allowed when upgrades are disallowed", False, DVD, [], SD, [], True)),
            ("should_returntrue_when_quality_is_revision_upgrade_for_same_quality",
             ua("A revision upgrade (DVD v1 to v2) is allowed even when upgrades are disallowed",
                False, Q("DVD", version=1), ["One"], Q("DVD", version=2), ["One"], True)),
        ],
        note="calls IsUpgradeAllowed(profile, current, current formats, new, new formats); accepted is its return value")

# --- UpgradeDiskSpecificationFixture ---------------------------------------
def ud(b, ok, profile=None, file_quality=None, new_quality=None, extra=None, n=None):
    g = {}
    if profile:
        g["profile"] = profile
    if file_quality:
        g["existing_file"] = {"quality": file_quality}
    if new_quality:
        g["parsed_quality"] = new_quality
    return one(b, merge(g, extra or {}), {"accepted": ok}, n)


def gp(cutoff, upgrade=True, **extra):
    """GivenProfile(new QualityProfile{...}): format items reset to none, min format score 0."""
    return {"cutoff": cutoff, **default_qualities(), "upgrade_allowed": upgrade, "format_items": [],
            "min_format_score": 0, "cutoff_format_score": 0, **extra}


MYF = "My Format"
FMT_NOTE = ("the profile literal's GetSampleFormatItems(\"My Format\") runs while no custom formats are "
            "registered, and GivenProfile then resets format items, so My Format scores nothing")
fixture("UpgradeDiskSpecificationFixture.cs", "UpgradeDiskSpecification",
        merge({"profile": {**gp("Bluray-1080p")},
               "existing_file": {"quality": Q("Bluray-1080p", version=2), "custom_formats": []},
               "parsed_quality": Q("DVD", version=2), "release": {"custom_formats": []}}, PAU),
        [
            ("should_return_true_if_movie_has_no_existing_file",
             ud("Accepted when the movie has no existing file", True, extra={"existing_file": None})),
            ("should_be_upgradable_if_only_movie_is_upgradable",
             ud("Accepted when the file (SDTV) is below the release (DVD v2) and the cutoff", True,
                file_quality=Q("SDTV"))),
            ("should_not_be_upgradable_if_qualities_are_the_same",
             ud("Rejected when file and release are both WEBDL-1080p", False, file_quality=Q("WEBDL-1080p"),
                new_quality=Q("WEBDL-1080p"))),
            ("should_not_be_upgradable_if_revision_downgrade_if_propers_are_preferred",
             ud("Rejected when the release (WEBDL-1080p v1) is a revision below the file (v2)", False,
                file_quality=Q("WEBDL-1080p", version=2), new_quality=Q("WEBDL-1080p"))),
            ("should_return_false_if_current_movie_is_equal_to_cutoff",
             ud("Rejected when the file (HDTV-720p v2) equals the cutoff (HDTV-720p)", False, gp("HDTV-720p"),
                Q("HDTV-720p", version=2))),
            ("should_return_false_if_current_movie_is_greater_than_cutoff",
             ud("Rejected when the file (Bluray-1080p v2) is above the cutoff (HDTV-720p)", False, gp("HDTV-720p"),
                Q("Bluray-1080p", version=2))),
            ("should_return_true_when_new_movie_is_proper_but_existing_is_not",
             ud("Accepted when the release is a proper (v2) of the file's quality at cutoff (HDTV-720p v1)", True,
                gp("HDTV-720p"), Q("HDTV-720p", version=1), Q("HDTV-720p", version=2))),
            ("should_return_false_if_cutoff_is_met_and_quality_is_higher",
             ud("Rejected when the file meets the cutoff (HDTV-720p), even for a higher release (Bluray-1080p)", False,
                gp("HDTV-720p"), Q("HDTV-720p", version=2), Q("Bluray-1080p", version=2))),
            ("should_return_false_if_quality_cutoff_is_met_and_quality_is_higher_but_language_is_met",
             ud("Rejected when the file meets the cutoff (HDTV-720p), even for a higher release (language-named variant)",
                False, gp("HDTV-720p"), Q("HDTV-720p", version=2), Q("Bluray-1080p", version=2),
                n="the body is identical to should_return_false_if_cutoff_is_met_and_quality_is_higher; no language is set")),
            ("should_return_false_if_cutoff_is_met_and_quality_is_higher_and_language_is_higher",
             ud("Rejected when the file meets the cutoff (HDTV-720p), even for a higher release (second language-named variant)",
                False, gp("HDTV-720p"), Q("HDTV-720p", version=2), Q("Bluray-1080p", version=2),
                n="the body is identical to should_return_false_if_cutoff_is_met_and_quality_is_higher; no language is set")),
            ("should_return_true_if_cutoff_is_not_met_and_new_quality_is_higher_and_language_is_higher",
             ud("Accepted when the file (SDTV v2) is below the cutoff (HDTV-720p) and the release (Bluray-1080p v2) is higher",
                True, gp("HDTV-720p"), Q("SDTV", version=2), Q("Bluray-1080p", version=2),
                n="no language is set despite the name")),
            ("should_return_true_if_cutoff_is_not_met_and_language_is_higher",
             ud("Accepted when the file (SDTV v2) is below the cutoff (HDTV-720p) and the release (DVD v2) is higher",
                True, gp("HDTV-720p"), Q("SDTV", version=2), n="no language is set despite the name")),
            ("should_return_false_if_custom_formats_is_met_and_quality_and_format_higher",
             ud("Rejected when the file meets the quality cutoff (HDTV-720p) and the profile scores no formats", False,
                gp("HDTV-720p"), Q("HDTV-720p"), Q("Bluray-1080p"),
                {"existing_file": {"custom_formats": []}, "release": {"custom_formats": [MYF]}}, FMT_NOTE)),
            ("should_return_true_if_cutoffs_are_met_but_is_a_revision_upgrade",
             ud("Accepted when the file is past the cutoff but the release is a revision upgrade (WEBDL-1080p v1 to v2)",
                True, gp("HDTV-1080p"), Q("WEBDL-1080p", version=1), Q("WEBDL-1080p", version=2))),
            ("should_return_false_if_quality_profile_does_not_allow_upgrades_but_cutoff_is_set_to_highest_quality",
             ud("Rejected when the profile disallows upgrades, even with cutoff Raw-HD and a higher release", False,
                gp("Raw-HD", upgrade=False), Q("WEBDL-1080p"), Q("Bluray-1080p"))),
            ("should_return_false_if_quality_profile_does_not_allow_upgrades_but_format_cutoff_is_above_current_score",
             ud("Rejected when upgrades are disallowed, even if the release adds a format (score 50) under the format cutoff (10000)",
                False, gp("SDTV", upgrade=False, cutoff_format_score=10000, format_items=[{"format": MYF, "score": 50}]),
                Q("WEBDL-1080p"), Q("WEBDL-1080p"),
                {"existing_file": {"custom_formats": []}, "release": {"custom_formats": [MYF]}})),
            ("should_return_false_if_quality_profile_does_not_allow_upgrades_but_format_cutoff_is_above_current_score_and_is_revision_upgrade",
             ud("Rejected when upgrades are disallowed and propers are DoNotPrefer, even for a revision upgrade adding a format",
                False, gp("SDTV", upgrade=False, cutoff_format_score=10000, format_items=[{"format": MYF, "score": 50}]),
                Q("WEBDL-1080p", version=1), Q("WEBDL-1080p", version=2),
                {"existing_file": {"custom_formats": []}, "release": {"custom_formats": [MYF]},
                 "config": {"download_propers_and_repacks": "DoNotPrefer"}})),
        ],
        note=("existing_file.custom_formats is what the mocked ParseCustomFormat(MovieFile) returns; "
              "setup format items GetSampleFormatItems(\"None\") are empty (no custom formats registered); " + MOQ_NOTE))

# --- UpgradeSpecificationFixture -------------------------------------------
UCF = {"CustomFormat1": "My Format 1", "CustomFormat2": "My Format 2"}


def is_upgrade_row(cur, cur_v, cur_f, new, new_v, new_f, expected):
    cur_f, new_f = [UCF[x] for x in cur_f], [UCF[x] for x in new_f]
    return [{"b": (f"{cur} v{cur_v} {cur_f} to {new} v{new_v} {new_f} gives {expected}"),
             "g": {"profile": {**default_qualities(), "cutoff": "Unknown", "upgrade_allowed": True,
                               "format_items": [{"format": UCF["CustomFormat1"], "score": 1},
                                                {"format": UCF["CustomFormat2"], "score": 2}]},
                   "current": side(Q(cur, version=cur_v), cur_f), "new": side(Q(new, version=new_v), new_f)},
             "e": {"rejection": expected},
             "n": "format scores from GetSampleFormatItems(My Format 1, My Format 2) = 1 and 2"}]


def up(b, profile, cur, new, expected, cur_f=(), new_f=(), propers=None):
    g = {"profile": {**default_qualities(), "cutoff": "Unknown", "upgrade_allowed": False, **profile},
         "current": side(cur, list(cur_f)), "new": side(new, list(new_f))}
    if propers:
        g["config"] = {"download_propers_and_repacks": propers}
    return one(b, g, {"rejection": expected})


ONE_TWO = [{"format": "One", "score": 10}, {"format": "Two", "score": 20}]
fixture("UpgradeSpecificationFixture.cs", "UpgradableSpecification.IsUpgradable", PAU,
        [
            ("IsUpgradeTest", is_upgrade_row),
            ("should_return_true_if_proper_and_download_propers_is_do_not_download",
             up("A proper (DVD v1 to v2) is an upgrade (None) with DoNotUpgrade, even when the profile disallows upgrades",
                {}, Q("DVD", version=1), Q("DVD", version=2), "None", propers="DoNotUpgrade")),
            ("should_return_false_if_proper_and_autoDownloadPropers_is_do_not_prefer",
             up("A proper (DVD v1 to v2) gives UpgradesNotAllowed with DoNotPrefer when the profile disallows upgrades",
                {}, Q("DVD", version=1), Q("DVD", version=2), "UpgradesNotAllowed", propers="DoNotPrefer")),
            ("should_return_false_if_release_and_existing_file_are_the_same",
             up("The same quality and revision (HDTV-720p v1) gives UpgradesNotAllowed when the profile disallows upgrades",
                {}, Q("HDTV-720p", version=1), Q("HDTV-720p", version=1), "UpgradesNotAllowed")),
            ("should_return_true_if_release_has_higher_quality_and_cutoff_is_not_already_met",
             up("HDTV-720p to HDTV-1080p is an upgrade (None) when the cutoff is HDTV-1080p",
                {"upgrade_allowed": True, "cutoff": "HDTV-1080p"}, Q("HDTV-720p", version=1), Q("HDTV-1080p", version=1),
                "None")),
            ("should_return_false_if_release_has_higher_quality_and_cutoff_is_already_met",
             up("HDTV-720p to HDTV-1080p gives QualityCutoff when the cutoff is HDTV-720p",
                {"upgrade_allowed": True, "cutoff": "HDTV-720p"}, Q("HDTV-720p", version=1), Q("HDTV-1080p", version=1),
                "QualityCutoff")),
            ("should_return_false_if_minimum_custom_score_is_not_met",
             up("A format gain of 10 (One 10 to Two 20) gives MinCustomFormatScore when the minimum increment is 11",
                {"upgrade_allowed": True, "min_upgrade_format_score": 11, "cutoff_format_score": 100,
                 "format_items": ONE_TWO}, DVD, DVD, "MinCustomFormatScore", ["One"], ["Two"])),
            ("should_return_true_if_minimum_custom_score_is_met",
             up("A format gain of 10 (One 10 to Two 20) is an upgrade (None) when the minimum increment is 10",
                {"upgrade_allowed": True, "min_upgrade_format_score": 10, "cutoff_format_score": 100,
                 "format_items": ONE_TWO}, DVD, DVD, "None", ["One"], ["Two"])),
        ],
        note=("calls IsUpgradable(profile, current, current formats, new, new formats); rejection is the returned "
              "UpgradeableRejectReason (None = upgrade); " + MOQ_NOTE))

# --- RssSync/PendingSpecificationFixture -----------------------------------
fixture("RssSync/PendingSpecificationFixture.cs", "PendingSpecification",
        {"movie": {"id": 1},
         "profile": {**default_qualities(), "cutoff": "Unknown", "upgrade_allowed": True, "format_items": [],
                     "min_format_score": 0},
         "parsed_quality": Q("DVD"), "parsed_languages": ["Spanish"], "release": {"custom_formats": []},
         "search": RSS, "pending_queue": []},
        [
            ("should_return_true_when_pending_queue_is_empty",
             one("Accepted when the pending queue is empty, reading it once", {},
                 {"accepted": True, "pending_queue_reads": 1})),
            ("should_return_true_when_not_pushed_release",
             one("Accepted without reading the pending queue when the release came from RSS",
                 {"release_source": "Rss"}, {"accepted": True, "pending_queue_reads": 0})),
            ("should_return_true_when_movie_is_not_pending",
             one("Accepted when the pending queue holds only another movie", {"pending_queue": [{"movie_id": 2}]},
                 {"accepted": True, "pending_queue_reads": 1})),
            ("should_return_false_when_movie_is_pending",
             one("Rejected when the pending queue already holds a release for this movie",
                 {"pending_queue": [{"movie_id": 1}]}, {"accepted": False, "pending_queue_reads": 1})),
        ],
        note=(NB_NOTE + "; release_source is NBuilder-filled unless set, and the read-once verification shows it "
              "is not Rss; search kind rss = ReleaseDecisionInformation(false, null); pending_queue_reads is the "
              "asserted GetPendingQueue() call count (Times.Once / Times.Never)"))

# --- RssSync/ProperSpecificationFixture ------------------------------------
def pr(b, ok, file_quality, days_ago, extra=None):
    return one(b, merge({"existing_file": {"quality": file_quality, "added_days_ago": days_ago}}, extra or {}),
               {"accepted": ok})


fixture("RssSync/ProperSpecificationFixture.cs", "ProperSpecification",
        merge({"profile": {"cutoff": "Bluray-1080p"},
               "existing_file": {"quality": Q("Bluray-1080p", version=1), "added_days_ago": 0},
               "parsed_quality": Q("DVD", version=2), "search": RSS}, PAU),
        [
            ("should_return_false_when_movieFile_was_added_more_than_7_days_ago",
             pr("Rejected when a proper (DVD v2) arrives for a DVD v1 file added 30 days ago", False,
                Q("DVD", version=1), 30)),
            ("should_return_true_when_movieFile_was_added_more_than_7_days_ago_but_proper_is_for_better_quality",
             pr("Accepted when the proper (DVD v2) is for a better quality than an old SDTV file", True, Q("SDTV"), 30)),
            ("should_return_true_when_episodeFile_was_added_more_than_7_days_ago_but_is_for_search",
             pr("Accepted in a search even when the file was added 30 days ago", True, Q("SDTV"), 30,
                {"search": SEARCH})),
            ("should_return_false_when_proper_but_auto_download_propers_is_false",
             pr("Rejected when a proper for today's DVD v1 file arrives and propers are DoNotUpgrade", False,
                Q("DVD", version=1), 0, {"config": {"download_propers_and_repacks": "DoNotUpgrade"}})),
            ("should_return_true_when_movieFile_was_added_today",
             pr("Accepted when a proper arrives for a DVD v1 file added today with PreferAndUpgrade", True,
                Q("DVD", version=1), 0, {"config": {"download_propers_and_repacks": "PreferAndUpgrade"}})),
        ],
        note=("added_days_ago counts from today (setup: DateTime.Now, tests: DateTime.Today minus n days); "
              "search kind search = a MovieSearchCriteria; " + MOQ_NOTE))

# --- Search/TorrentSeedingSpecificationFixture -----------------------------
TS_TITLE = "Series.Title.S01.720p.BluRay.X264-RlsGrp"


def ts(b, ok, extra):
    return one(b, extra, {"accepted": ok})


fixture("Search/TorrentSeedingSpecificationFixture.cs", "TorrentSeedingSpecification",
        {"movie": {"id": 1},
         "release": {"release_class": "TorrentInfo", "indexer_id": 1, "title": TS_TITLE, "seeders": 0},
         "indexer": {"id": 1, "settings": {"minimum_seeders": 5}}},
        [
            ("should_return_true_if_not_torrent",
             ts("Accepted when the release is not a torrent", True,
                {"release": R({"release_class": "ReleaseInfo", "indexer_id": 1, "title": TS_TITLE})})),
            ("should_return_true_if_indexer_not_specified",
             ts("Accepted when the torrent has no indexer (id 0)", True, {"release": {"indexer_id": 0}})),
            ("should_return_true_if_indexer_no_longer_exists",
             ts("Accepted when the release's indexer no longer exists", True,
                {"indexer": None, "indexer_lookup_throws": "ModelNotFoundException"})),
            ("should_return_true_if_seeds_unknown",
             ts("Accepted when the seeder count is unknown (null)", True, {"release": {"seeders": None}})),
            ("should_return_true_if_seeds_above_or_equal_to_limit",
             lambda s: ts(f"Accepted with {s} seeders when the indexer minimum is 5", True, {"release": {"seeders": s}})),
            ("should_return_false_if_seeds_belove_limit",
             lambda s: ts(f"Rejected with {s} seeders when the indexer minimum is 5", False, {"release": {"seeders": s}})),
        ],
        note="indexer settings are a TorrentRssIndexerSettings with MinimumSeeders 5")


# --- C# literal parsing ----------------------------------------------------
ESC = {"\\": "\\", '"': '"', "'": "'", "0": "\0", "a": "\a", "b": "\b", "f": "\f", "n": "\n", "r": "\r",
       "t": "\t", "v": "\v"}


def split_top(s):
    """Split on top-level commas, respecting strings and brackets."""
    parts, depth, cur, i = [], 0, "", 0
    while i < len(s):
        c = s[i]
        if c == '"' or (c == "@" and s[i + 1:i + 2] == '"'):
            j = i + (2 if c == "@" else 1)
            while True:
                if s[j] == "\\" and c != "@":
                    j += 2
                    continue
                if s[j] == '"':
                    if c == "@" and s[j + 1:j + 2] == '"':
                        j += 2
                        continue
                    break
                j += 1
            cur += s[i:j + 1]
            i = j + 1
            continue
        if c in "({<[":
            depth += 1
        elif c in ")}>]":
            depth -= 1
        if c == "," and depth == 0:
            parts.append(cur.strip())
            cur = ""
        else:
            cur += c
        i += 1
    if cur.strip():
        parts.append(cur.strip())
    return parts


def cs_value(tok):
    if tok.startswith('@"'):
        return tok[2:-1].replace('""', '"')
    if tok.startswith('"'):
        out, body, i = "", tok[1:-1], 0
        while i < len(body):
            if body[i] == "\\":
                if body[i + 1] == "u":
                    out += chr(int(body[i + 2:i + 6], 16))
                    i += 6
                    continue
                out += ESC[body[i + 1]]
                i += 2
            else:
                out += body[i]
                i += 1
        return out
    if tok in ("true", "false"):
        return tok == "true"
    if tok == "null":
        return None
    if re.fullmatch(r"-?\d+", tok):
        return int(tok)
    m = re.fullmatch(r"new List<\w+>\s*(?:\(\)|\{(.*)\})", tok, re.S)
    if m:
        return [cs_value(t) for t in split_top(m.group(1) or "")]
    m = re.fullmatch(r"(\w+)\.(\w+)", tok)
    if m:
        return QUALITY_NAMES[m.group(2)] if m.group(1) == "Quality" else m.group(2)
    assert re.fullmatch(r"\w+", tok), f"unsupported C# literal: {tok}"
    return tok


def parse_args(s):
    return tuple(cs_value(t) for t in split_top(s))


assert parse_args(r'"/WEB\b/", true') == ("/WEB\b/", True)
assert parse_args(r'@"/\.WEB/", false') == ("/\\.WEB/", False)
assert parse_args("Quality.SDTV, 1, new List<CustomFormat> { CustomFormat1 }, new List<CustomFormat>(), "
                  "UpgradeableRejectReason.None") == ("SDTV", 1, ["CustomFormat1"], [], "None")


# --- machinery -------------------------------------------------------------
ATTR = re.compile(r"^\s*\[(Test|TestCase|TestCaseSource)\b(?:\((.*)\))?\]\s*$")
METHOD = re.compile(r"^\s*public\s+void\s+(\w+)\s*\(")


def active_lines(text):
    """Source lines with //- and /* */-comments blanked outside strings (line numbers kept)."""
    out, in_block = [], False
    for line in text.splitlines():
        res, i, in_str = "", 0, None
        while i < len(line):
            two = line[i:i + 2]
            if in_block:
                if two == "*/":
                    in_block, i = False, i + 2
                else:
                    i += 1
                continue
            c = line[i]
            if in_str:
                res += c
                if in_str == '"' and c == "\\":
                    res += line[i + 1:i + 2]
                    i += 2
                    continue
                if c == '"':
                    in_str = None
                i += 1
                continue
            if two == "//":
                break
            if two == "/*":
                in_block, i = True, i + 2
                continue
            if c == '"':
                in_str = "@" if line[i - 1:i] == "@" else '"'
            res += c
            i += 1
        out.append(res)
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


def source_rows(lines, attr_args):
    """TestCaseSource rows: [(line_no, args_text)] of `new object[] { ... }` lines in the named array."""
    name = re.fullmatch(r'\s*(?:nameof\((\w+)\)|"(\w+)")\s*', attr_args)
    name = name.group(1) or name.group(2)
    start = next(i for i, l in enumerate(lines) if re.search(rf"\bobject\[\]\s+{name}\s*=", l))
    rows = []
    for i in range(start + 1, len(lines)):
        if lines[i].strip().startswith("};"):
            break
        m = re.match(r"^\s*new object\[\]\s*\{(.*)\}\s*,?\s*$", lines[i])
        if m:
            rows.append((i + 1, m.group(1)))
    assert rows, f"no rows in {name}"
    return rows


def canon(o):
    return json.dumps(o, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


def build():
    entries = []
    for fx in FIXTURES:
        rel = f"{TEST_DIR}/{fx['path']}"
        lines = active_lines((REF_REPO / rel).read_text(encoding="utf-8-sig"))
        found = scan(lines)
        covered = [m for m, _ in fx["tests"]]
        missing = sorted(set(found) - set(covered))
        extra = sorted(set(covered) - set(found))
        assert not missing and not extra, f"{rel}: uncovered {missing}, unknown {extra}"
        for method, rows_spec in fx["tests"]:
            attrs = found[method]
            cases = [(n, a) for n, k, a in attrs if k == "TestCase"]
            cases += [row for n, k, a in attrs if k == "TestCaseSource" for row in source_rows(lines, a)]
            if cases:
                assert callable(rows_spec), f"{rel} {method}: parameterised rows need a function"
                rows = [(line, r) for line, a in cases for r in rows_spec(*parse_args(a))]
            else:
                assert not callable(rows_spec), f"{rel} {method}: no [TestCase] rows found"
                line = next(n for n, k, _ in attrs if k == "Test")
                rows = [(line, r) for r in rows_spec]
            for idx, (line, r) in enumerate(rows):
                ref = f"{TAG} {rel}:{line}"
                case = None
                if "out" not in r:
                    case = {"input": {"spec": fx["spec"], "given": merge(fx["base"], r["g"])}, "expect": r["e"]}
                    note = "; ".join(x for x in (r.get("n"), fx["note"]) if x)
                    if note:
                        case["note"] = note
                h = hashlib.sha1(canon(case["input"] if case else None).encode()).hexdigest()[:12]
                entries.append({"key": f"{rel}|{method}|{idx}|{h}", "param": bool(cases),
                                "behaviour": r["b"], "ref": ref,
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
    # A [Test] method has one row, so a corrected input keeps its ID; [TestCase] rows need the exact input.
    by_row = {k.rsplit("|", 1)[0]: v for k, v in old.items()}
    for e in entries:
        row = e["key"].rsplit("|", 1)[0]
        if e["key"] in old:
            e["id"] = old[e["key"]]
        elif not e["param"] and row in by_row:
            e["id"] = by_row[row]
        else:
            e["id"] = ID_FMT.format(nxt)
            nxt += 1
    assert nxt - 1 <= ID_MAX, "ID range RAD-DECISION-1000..1999 exhausted"
    return nxt


def kept_lines():
    """{id: [status/decision/tests lines as written]} from the current catalogue (ratchet-owned, never regenerated)."""
    if not CATALOGUE.exists():
        return {}
    return {m[0]: re.findall(r"(?m)^  ((?:status|decision|tests):.*(?:\n    .*)*)", m[1])
            for m in re.findall(r"(?m)^- id: (\S+)\n((?:  .*\n?|    .*\n?)*)", CATALOGUE.read_text())}


def render(entries, nxt):
    q = lambda s: json.dumps(s, ensure_ascii=False)  # noqa: E731  JSON strings are valid YAML scalars
    out = [f"# Generated by parity/extract/radarr_decision_b.py from {TAG} {TEST_DIR} fixtures N-Z.",
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
