"""Port Sonarr grab-decision fixtures N..Z, RssSync/ and Search/ into the parity
corpus and catalogue (stream sonarr-decision-b, IDs SON-DECISION-1000..1999).

Hand-translated: each test's C# arrange/assert is written below as data. The
script parses the reference fixtures for every [Test], [TestCase] row and
[TestCaseSource] row (so refs carry exact lines), fails if any test has no data
here or any data has no test, checks every row's C# argument text against the
`args` written here, and writes:

  parity/corpus/reference/sonarr/decision/<Fixture>.json
  parity/catalogue/decision.sonarr-b.yaml

IDs are stable: an existing entry's ref line is re-read at the pinned tag to
get its key (fixture path, method, row index, C# row arguments); keys found
again keep their ID, new keys get the next free ID. Status, decision and tests
of existing entries are kept as written.

Conventions for `given` (reference terms, a later phase maps them to Scryer):
- Fields a test does not arrange are left out. On objects built with `new`
  they take the C# default: ints 0, bools false, enums their first member
  (download_propers_and_repacks PreferAndUpgrade, cutoff Unknown), lists empty,
  mocks returning nothing. On objects built with NBuilder
  (Builder<T>.CreateNew()) unset ints are 1 and bools false, e.g.
  RemoteEpisode.CustomFormatScore = 1 (Queue, Delay) and
  DelayProfile.MinimumCustomFormatScore = TorrentDelay = 1 (Delay); no case's
  asserted outcome depends on these.
- quality: {"quality": <Sonarr name>, "revision": <version>} plus "real" /
  "repack" when the test sets them (Revision defaults: version 1, real 0).
- profile.qualities is the item order; "allowed" is "all" or a list.
  DEFAULT_QUALITIES is QualityFixture.GetDefaultQualities()
  (src/NzbDrone.Core.Test/Qualities/QualityFixture.cs:66); with an allowed
  list, that helper moves the allowed qualities to the end, in argument order.
- *_mb is Sonarr's Megabytes() (MiB). age_* is relative to "now".
- NBuilder-generated values the test relies on (ids 1..n, season/episode 1)
  are written out.
- Prioritization: each candidate has a "label" (the C# variable name); expect
  "first" when the test asserts only the head, "order" when it asserts all.
- Functions returning bool or UpgradeableRejectReason use "accepted"
  (true / reason None) and "rejection" (the asserted reason).

Run: python3 -I parity/extract/sonarr_decision_b.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REF_REPO = Path.home() / "PycharmProjects/arr-reference/sonarr@v4.0.20.3014"
TAG = "Sonarr@v4.0.20.3014"
TEST_DIR = "src/NzbDrone.Core.Test/DecisionEngineTests/"
CATALOGUE = ROOT / "parity/catalogue/decision.sonarr-b.yaml"
CORPUS = ROOT / "parity/corpus/reference/sonarr/decision"
ID_PREFIX, ID_MIN, ID_MAX = "SON-DECISION-", 1000, 1999

DEFAULT_QUALITIES = ["Unknown", "SDTV", "WEBDL-480p", "DVD", "HDTV-720p", "HDTV-1080p", "HDTV-2160p", "Raw-HD",
                     "WEBDL-720p", "WEBDL-1080p", "WEBDL-2160p", "Bluray-720p", "Bluray-1080p", "Bluray-2160p"]
MB = "size_mb"


def T(method, behaviour, given, expect, note=None):
    return dict(method=method, rows=None, behaviour=behaviour, given=given, expect=expect, note=note)


def R(method, rows):
    """rows: [(args, behaviour, given, expect[, note])], args = the C# row text."""
    return dict(method=method, rows=[r if len(r) == 5 else (*r, None) for r in rows])


def q(name, revision=1, real=0, repack=False):
    out = {"quality": name, "revision": revision}
    if real:
        out["real"] = real
    if repack:
        out["repack"] = True
    return out


def profile(allowed="all", qualities=None, **kw):
    if qualities is None:  # GetDefaultQualities(allowed): qualities.Except(allowed).Concat(allowed)
        qualities = DEFAULT_QUALITIES if allowed == "all" else \
            [x for x in DEFAULT_QUALITIES if x not in allowed] + list(allowed)
    return {"qualities": qualities, "allowed": allowed, **kw}


def items(**scores):
    return [{"format": k.replace("_", " "), "score": v} for k, v in scores.items()]


# --- PrioritizeDownloadDecisionFixture --------------------------------------

def ep(n):
    return {"id": n, "episode": n}


def cand(label, episodes, quality, language="English", age_days=0, size_mb=0, protocol="Usenet", indexer_priority=25,
         **extra):
    """GivenRemoteEpisode (PrioritizeDownloadDecisionFixture.cs:50)."""
    return {"label": label, "episodes": [ep(n) for n in episodes], "quality": quality, "languages": [language],
            "age_days": age_days, MB: size_mb, "protocol": protocol, "indexer_priority": indexer_priority, **extra}


def tcand(label, quality, seeders, size_mb=0, peers=None):
    """GivenRemoteEpisode then Release replaced by a new TorrentInfo (age 0, no indexer priority)."""
    c = {"label": label, "episodes": [ep(1)], "quality": quality, "languages": ["English"], "age_days": 0,
         MB: size_mb, "protocol": "Torrent", "seeders": seeders}
    if peers is not None:
        c["peers"] = peers
    return c


def prio(candidates, preferred="Usenet", preferred_size=None, propers=None, series_type=None):
    g = {"series": {"runtime": 60}, "profile": profile(), "delay_profile": {"preferred_protocol": preferred},
         "quality_definition": {"preferred_size": preferred_size}, "candidates": candidates}
    if series_type:
        g["series"]["series_type"] = series_type
    if propers:
        g["config"] = {"download_propers_and_repacks": propers}
    return g


H720, W1080 = "HDTV-720p", "WEBDL-1080p"
SWAP_NOTE = "The test sets torrentInfo1.Size after cloning it, so remoteEpisode1 is 250 MB and remoteEpisode2 200 MB."

PRIORITIZE = [
    T("should_put_reals_before_non_reals", "A REAL release is ranked before the same release without REAL",
      prio([cand("remoteEpisode1", [1], q(H720, 1, 0)), cand("remoteEpisode2", [1], q(H720, 1, 1))]),
      {"first": "remoteEpisode2"}),
    T("should_put_propers_before_non_propers", "A proper (v2) is ranked before the same quality v1",
      prio([cand("remoteEpisode1", [1], q(H720, 1)), cand("remoteEpisode2", [1], q(H720, 2))]),
      {"first": "remoteEpisode2"}),
    T("should_put_higher_quality_before_lower", "HDTV-720p is ranked before SDTV",
      prio([cand("remoteEpisode1", [1], q("SDTV")), cand("remoteEpisode2", [1], q(H720))]),
      {"first": "remoteEpisode2"}),
    T("should_order_by_lowest_number_of_episodes", "The release for the lower episode number is ranked first",
      prio([cand("remoteEpisode1", [2], q(H720)), cand("remoteEpisode2", [1], q(H720))]),
      {"first": "remoteEpisode2"}),
    T("should_order_by_lowest_number_of_episodes_with_multiple_episodes",
      "Of two multi-episode releases, the one starting at the lower episode number is ranked first",
      prio([cand("remoteEpisode1", [2, 3], q(H720)), cand("remoteEpisode2", [1, 2], q(H720))]),
      {"first": "remoteEpisode2"}),
    T("should_order_by_age_then_largest_rounded_to_200mb",
      "Among HD releases the young large one (3000 MB, 1 day) is ranked first, ahead of SD and older or smaller ones",
      prio([cand("remoteEpisodeSd", [1], q("SDTV"), size_mb=100, age_days=1),
            cand("remoteEpisodeHdSmallOld", [1], q(H720), size_mb=1200, age_days=1000),
            cand("remoteEpisodeSmallYoung", [1], q(H720), size_mb=1250, age_days=10),
            cand("remoteEpisodeHdLargeYoung", [1], q(H720), size_mb=3000, age_days=1)]),
      {"first": "remoteEpisodeHdLargeYoung"}),
    T("should_order_by_closest_to_preferred_size_if_both_under",
      "With preferred size 200 MB/min (12000 MB at 60 min), the 10000 MB release beats the 1200 MB one",
      prio([cand("remoteEpisodeSmall", [1], q(H720), size_mb=1200, age_days=1),
            cand("remoteEpisodeLarge", [1], q(H720), size_mb=10000, age_days=1)], preferred_size=200),
      {"first": "remoteEpisodeLarge"}),
    T("should_order_by_closest_to_preferred_size_if_preferred_is_in_between",
      "With preferred size 46 MB/min, the 3000 MB release is ranked first among 500/2000/3000/5000 MB",
      prio([cand("remoteEpisode1", [1], q(H720), size_mb=500, age_days=1),
            cand("remoteEpisode2", [1], q(H720), size_mb=2000, age_days=1),
            cand("remoteEpisode3", [1], q(H720), size_mb=3000, age_days=1),
            cand("remoteEpisode4", [1], q(H720), size_mb=5000, age_days=1)], preferred_size=46),
      {"first": "remoteEpisode3"}),
    T("should_order_by_youngest", "The 5-day-old release is ranked before the 10-day-old one",
      prio([cand("remoteEpisode1", [1], q(H720), age_days=10), cand("remoteEpisode2", [1], q(H720), age_days=5)]),
      {"first": "remoteEpisode2"}),
    T("should_not_throw_if_no_episodes_are_found", "Prioritizing does not fail when a release has no episodes",
      prio([dict(cand("remoteEpisode1", [1], q(H720), size_mb=500), episodes=[]),
            cand("remoteEpisode2", [1], q(H720), size_mb=500)]),
      {"throws": False}),
    T("should_put_usenet_above_torrent_when_usenet_is_preferred", "With Usenet preferred, the Usenet release is ranked first",
      prio([cand("remoteEpisode1", [1], q(H720), protocol="Torrent"), cand("remoteEpisode2", [1], q(H720))]),
      {"first": "remoteEpisode2"}),
    T("should_put_torrent_above_usenet_when_torrent_is_preferred", "With Torrent preferred, the torrent release is ranked first",
      prio([cand("remoteEpisode1", [1], q(H720), protocol="Torrent"), cand("remoteEpisode2", [1], q(H720))],
           preferred="Torrent"),
      {"first": "remoteEpisode1"}),
    T("should_prefer_season_pack_above_single_episode", "A full-season release is ranked before a single episode",
      prio([cand("remoteEpisode1", [1, 2], q(H720), full_season=True), cand("remoteEpisode2", [1], q(H720))]),
      {"first": "remoteEpisode1"}),
    T("should_prefer_multiepisode_over_single_episode_for_anime",
      "For anime, the two-episode release is ranked before the single episode",
      prio([cand("remoteEpisode1", [1, 2], q(H720)), cand("remoteEpisode2", [1], q(H720))], series_type="Anime"),
      {"first": "remoteEpisode1"}, note="The test asserts the head has remoteEpisode1's episode count (2)."),
    T("should_prefer_single_episode_over_multi_episode_for_non_anime",
      "For non-anime, the single episode is ranked before the two-episode release",
      prio([cand("remoteEpisode1", [1, 2], q(H720)), cand("remoteEpisode2", [1], q(H720))]),
      {"first": "remoteEpisode2"}, note="The test asserts the head has remoteEpisode2's episode count (1)."),
    T("should_prefer_releases_with_more_seeders", "The torrent with 100 seeders is ranked before the one with 10",
      prio([tcand("remoteEpisode1", q(H720), 10), tcand("remoteEpisode2", q(H720), 100)]),
      {"first": "remoteEpisode2"}),
    T("should_prefer_releases_with_more_peers_given_equal_number_of_seeds",
      "With equal seeders, the torrent with 100 peers is ranked before the one with 10",
      prio([tcand("remoteEpisode1", q(H720), 10, peers=10), tcand("remoteEpisode2", q(H720), 10, peers=100)]),
      {"first": "remoteEpisode2"}),
    T("should_prefer_releases_with_more_peers_no_seeds",
      "With no seeders, the torrent with 100 peers is ranked before the one with 10",
      prio([tcand("remoteEpisode1", q(H720), 0, peers=10), tcand("remoteEpisode2", q(H720), 0, peers=100)]),
      {"first": "remoteEpisode2"}),
    T("should_prefer_first_release_if_peers_and_size_are_too_similar",
      "Torrents with similar seeders (1000 vs 1100) and size keep the first release first",
      prio([tcand("remoteEpisode1", q(H720), 1000, size_mb=250, peers=10),
            tcand("remoteEpisode2", q(H720), 1100, size_mb=200, peers=10)]),
      {"first": "remoteEpisode1"}, note=SWAP_NOTE),
    T("should_prefer_first_release_if_age_and_size_are_too_similar",
      "Releases of similar age (100 vs 150 days) and size (200 vs 250 MB) keep the first release first",
      prio([cand("remoteEpisode1", [1], q(H720), age_days=100, size_mb=200),
            cand("remoteEpisode2", [1], q(H720), age_days=150, size_mb=250)]),
      {"first": "remoteEpisode1"}),
    T("should_prefer_quality_over_the_number_of_peers", "Bluray-1080p with 100 seeders beats SDTV with 1100 seeders",
      prio([tcand("remoteEpisode1", q("Bluray-1080p"), 100, size_mb=250, peers=10),
            tcand("remoteEpisode2", q("SDTV"), 1100, size_mb=200, peers=10)]),
      {"first": "remoteEpisode1"}, note=SWAP_NOTE),
    T("should_put_higher_quality_before_lower_always", "HDTV-720p German is ranked before SDTV French",
      prio([cand("remoteEpisode1", [1], q("SDTV"), language="French"),
            cand("remoteEpisode2", [1], q(H720), language="German")]),
      {"first": "remoteEpisode2"}),
    T("should_prefer_higher_score_over_lower_score", "Custom format score 10 is ranked before score 0",
      prio([cand("remoteEpisode1", [1], q(W1080), custom_format_score=10),
            cand("remoteEpisode2", [1], q(W1080), custom_format_score=0)]),
      {"first": "remoteEpisode1"}),
    T("should_prefer_proper_over_score_when_download_propers_is_prefer_and_upgrade",
      "With PreferAndUpgrade, a proper with score 0 beats a v1 with score 10",
      prio([cand("remoteEpisode1", [1], q(W1080, 1), custom_format_score=10),
            cand("remoteEpisode2", [1], q(W1080, 2), custom_format_score=0)], propers="PreferAndUpgrade"),
      {"first": "remoteEpisode2"}),
    T("should_prefer_proper_over_score_when_download_propers_is_do_not_upgrade",
      "With DoNotUpgrade, a proper with score 0 beats a v1 with score 10",
      prio([cand("remoteEpisode1", [1], q(W1080, 1), custom_format_score=10),
            cand("remoteEpisode2", [1], q(W1080, 2), custom_format_score=0)], propers="DoNotUpgrade"),
      {"first": "remoteEpisode2"}),
    T("should_prefer_score_over_proper_when_download_propers_is_do_not_prefer",
      "With DoNotPrefer, a v1 with score 10 beats a proper with score 0",
      prio([cand("remoteEpisode1", [1], q(W1080, 1), custom_format_score=10),
            cand("remoteEpisode2", [1], q(W1080, 2), custom_format_score=0)], propers="DoNotPrefer"),
      {"first": "remoteEpisode1"}),
    T("should_prefer_score_over_real_when_download_propers_is_do_not_prefer",
      "With DoNotPrefer, a non-REAL with score 10 beats a REAL with score 0",
      prio([cand("remoteEpisode1", [1], q(W1080, 1, 0), custom_format_score=10),
            cand("remoteEpisode2", [1], q(W1080, 1, 1), custom_format_score=0)], propers="DoNotPrefer"),
      {"first": "remoteEpisode1"}),
    T("sort_download_decisions_based_on_indexer_priority",
      "Equal releases are ordered by indexer priority, lowest number first (1, 25, 50)",
      prio([cand("remoteEpisode1", [1], q(W1080, 1), indexer_priority=25),
            cand("remoteEpisode2", [1], q(W1080, 1), indexer_priority=50),
            cand("remoteEpisode3", [1], q(W1080, 1), indexer_priority=1)]),
      {"order": ["remoteEpisode3", "remoteEpisode1", "remoteEpisode2"]}),
    T("ensure_download_decisions_indexer_priority_is_not_perfered_over_quality",
      "Quality ranks before indexer priority; priority breaks ties within a quality",
      prio([cand("remoteEpisode1", [1], q(H720, 1), indexer_priority=25),
            cand("remoteEpisode2", [1], q(W1080, 1), indexer_priority=50),
            cand("remoteEpisode3", [1], q("SDTV", 1), indexer_priority=1),
            cand("remoteEpisode4", [1], q(W1080, 1), indexer_priority=25)]),
      {"order": ["remoteEpisode4", "remoteEpisode2", "remoteEpisode1", "remoteEpisode3"]}),
]

# --- ProtocolSpecificationFixture -------------------------------------------


def proto(protocol, **delay):
    return {"release": {"protocol": protocol}, "delay_profile": delay}


PROTOCOL = [
    T("should_be_true_if_usenet_and_usenet_is_enabled", "A Usenet release is accepted when Usenet is enabled",
      proto("Usenet", enable_usenet=True), {"accepted": True}),
    T("should_be_true_if_torrent_and_torrent_is_enabled", "A torrent release is accepted when torrent is enabled",
      proto("Torrent", enable_torrent=True), {"accepted": True}),
    T("should_be_false_if_usenet_and_usenet_is_disabled", "A Usenet release is rejected when Usenet is disabled",
      proto("Usenet", enable_usenet=False), {"accepted": False}),
    T("should_be_false_if_torrent_and_torrent_is_disabled", "A torrent release is rejected when torrent is disabled",
      proto("Torrent", enable_torrent=False), {"accepted": False}),
]

# --- QualityAllowedByProfileSpecificationFixture ----------------------------

QA_PROFILE = profile(allowed=["DVD", "HDTV-720p", "Bluray-1080p"], cutoff="Bluray-1080p")


def qa_row(cs, name, ok):
    verdict = "allowed" if ok else "not allowed"
    return (f"Quality.{cs}", f"A {name} release is {'accepted' if ok else 'rejected'} when the profile allows only "
            f"DVD, HDTV-720p and Bluray-1080p ({name} is {verdict})",
            {"profile": QA_PROFILE, "release": q(name, 2)}, {"accepted": ok})


QUALITY_ALLOWED = [
    R("should_allow_if_quality_is_defined_in_profile",
      [qa_row("DVD", "DVD", True), qa_row("HDTV720p", "HDTV-720p", True), qa_row("Bluray1080p", "Bluray-1080p", True)]),
    R("should_not_allow_if_quality_is_not_defined_in_profile",
      [qa_row("SDTV", "SDTV", False), qa_row("WEBDL720p", "WEBDL-720p", False),
       qa_row("Bluray720p", "Bluray-720p", False)]),
]

# --- QueueSpecificationFixture ----------------------------------------------

EP1 = {"id": 1, "series_id": 1, "season": 1, "episode": 1}
OTHER_EP = {"id": 2, "series_id": 2, "season": 2, "episode": 2}


def queued(episodes, quality=None, languages=None, series_id=1, formats=(), state="Downloading"):
    item = {"series_id": series_id, "episodes": episodes}
    if quality:
        item["quality"] = quality
    if languages:
        item["languages"] = languages
    item["custom_formats"] = list(formats)
    item["state"] = state
    return item


def queue_given(queue, cutoff=None, release_episodes=(EP1,), release_quality=None, release_formats=(),
                format_items=None, propers=None, upgrade_allowed=True):
    p = profile(upgrade_allowed=upgrade_allowed, min_format_score=0)
    if cutoff:
        p["cutoff"] = cutoff
    if format_items:
        p["format_items"] = format_items
    g = {"profile": p,
         "release": {"series_id": 1, "episodes": list(release_episodes), **(release_quality or q("DVD")),
                     "languages": ["Spanish"], "custom_formats": list(release_formats)},
         "queue": queue}
    if propers:
        g["config"] = {"download_propers_and_repacks": propers}
    return g


B1080 = "Bluray-1080p"
QUEUE_NOTE = "Queue item custom_formats are what ICustomFormatCalculationService.ParseCustomFormat returns for it."
QUEUE = [
    T("should_return_true_when_queue_is_empty", "A release is accepted when the queue is empty",
      queue_given([]), {"accepted": True}),
    T("should_return_true_when_series_doesnt_match", "A queued item for another series does not block the release",
      queue_given([queued([EP1], series_id=2)]), {"accepted": True}),
    T("should_return_false_if_everything_is_the_same",
      "A release identical to a queued one (DVD, Spanish) is rejected",
      queue_given([queued([EP1], q("DVD"), ["Spanish"])], cutoff=B1080), {"accepted": False}),
    T("should_return_true_when_quality_in_queue_is_lower", "A DVD release is accepted over a queued SDTV item",
      queue_given([queued([EP1], q("SDTV"), ["Spanish"])], cutoff=B1080), {"accepted": True}),
    T("should_return_true_when_quality_in_queue_is_lower_but_language_is_higher",
      "A DVD release is accepted over a queued SDTV English item",
      queue_given([queued([EP1], q("SDTV"), ["English"])], cutoff=B1080), {"accepted": True}),
    T("should_return_true_when_episode_doesnt_match", "A queued item for a different episode does not block the release",
      queue_given([queued([OTHER_EP], q("DVD"))]), {"accepted": True}),
    T("should_return_true_when_qualities_are_the_same_and_languages_are_the_same_with_higher_custom_format_score",
      "Same quality and language, but a higher custom format score than the queued item, is accepted",
      queue_given([queued([EP1], q("DVD"), ["Spanish"], formats=["Bad Format"])], release_formats=["My Format"],
                  format_items=items(Bad_Format=-2, My_Format=1)),
      {"accepted": True},
      note="Scores from CustomFormatsTestHelpers.GetSampleFormatItems(\"My Format\") with formats My Format and "
           "Bad Format (src/NzbDrone.Core.Test/CustomFormats/CustomFormatsTestHelpers.cs:19). " + QUEUE_NOTE),
    T("should_return_false_when_qualities_are_the_same_and_languages_are_the_same",
      "Same quality and language as a queued item is rejected",
      queue_given([queued([EP1], q("DVD"), ["Spanish"])]), {"accepted": False}),
    T("should_return_false_when_quality_in_queue_is_better", "A DVD release is rejected when HDTV-720p is queued",
      queue_given([queued([EP1], q("HDTV-720p"), ["English"])], cutoff=B1080), {"accepted": False}),
    T("should_return_false_if_matching_multi_episode_is_in_queue",
      "A queued multi-episode item covering the episode blocks the release",
      queue_given([queued([EP1, OTHER_EP], q("HDTV-720p"), ["English"])]), {"accepted": False}),
    T("should_return_false_if_multi_episode_has_one_episode_in_queue",
      "A multi-episode release is rejected when one of its episodes is queued",
      queue_given([queued([EP1], q("HDTV-720p"), ["English"])], release_episodes=(EP1, OTHER_EP)),
      {"accepted": False}),
    T("should_return_false_if_multi_part_episode_is_already_in_queue",
      "A multi-episode release is rejected when the same episodes are queued",
      queue_given([queued([EP1, OTHER_EP], q("HDTV-720p"), ["English"])], release_episodes=(EP1, OTHER_EP)),
      {"accepted": False}),
    T("should_return_false_if_multi_part_episode_has_two_episodes_in_queue",
      "A multi-episode release is rejected when its episodes are queued as two items",
      queue_given([queued([EP1], q("HDTV-720p"), ["English"]), queued([OTHER_EP], q("HDTV-720p"), ["English"])],
                  release_episodes=(EP1, OTHER_EP)),
      {"accepted": False}),
    T("should_return_false_if_quality_in_queue_meets_cutoff",
      "A release is rejected when the queued item (HDTV-720p) meets the DVD cutoff",
      queue_given([queued([EP1], q("HDTV-720p"), ["Spanish"])], cutoff="DVD"), {"accepted": False}),
    T("should_return_false_when_quality_are_the_same_language_is_better_and_upgrade_allowed_is_false_for_language_profile",
      "Same quality as a queued item with a different language (English) is rejected",
      queue_given([queued([EP1], q("DVD"), ["English"])]), {"accepted": False}),
    T("should_return_false_when_quality_is_better_languages_are_the_same_and_upgrade_allowed_is_false_for_quality_profile",
      "With upgrades not allowed, a release is rejected when Bluray-1080p is queued",
      queue_given([queued([EP1], q(B1080), ["Spanish"])], cutoff=B1080, upgrade_allowed=False), {"accepted": False}),
    T("should_return_true_if_everything_is_the_same_for_failed_pending",
      "A FailedPending queued item does not block an identical release",
      queue_given([queued([EP1], q("DVD"), ["Spanish"], state="FailedPending")], cutoff=B1080), {"accepted": True}),
    T("should_return_false_if_same_quality_non_proper_in_queue_and_download_propers_is_do_not_upgrade",
      "With DoNotUpgrade, a proper is rejected when the same quality non-proper is queued",
      queue_given([queued([EP1], q("HDTV-720p"), ["English"])], cutoff="HDTV-720p", release_quality=q("HDTV-720p", 2),
                  propers="DoNotUpgrade"),
      {"accepted": False}),
]

# --- RawDiskSpecificationFixture --------------------------------------------


def raw(title="Series.title.s01e01", container=None):
    r = {"title": title, "protocol": "Torrent"}
    if container:
        r["container"] = container
    return {"release": r}


DISC = ["Series Title S02 Disc 1 1080i Blu-ray DTS-HD MA 2.0 AVC-TrollHD",
        "Series Title S03 Disc 1 1080p Blu-ray LPCM 2.0 AVC-TrollHD",
        "SERIES TITLE S02 1080P FULL BLURAY AVC DTS-HD MA 5 1",
        "Series.Title.S06.2016.DISC.3.BluRay.1080p.AVC.Atmos.TrueHD7.1-MTeam",
        "Series Title S05 Disc 1 BluRay 1080p AVC Atmos TrueHD 7 1-MTeam",
        "Series Title S05 Disc 1 BluRay 1080p AVC Atmos TrueHD 7 1-MTeam",
        "Someone.the.Entertainer.Presents.S01.NTSC.3xDVD9.MPEG-2.DD2.0",
        "Series.Title.S00.The.Christmas.Special.2011.PAL.DVD5.DD2.0",
        "Series.of.Desire.2000.S1_D01.NTSC.DVD5"]
DVDRIP = ["Series Title EP50 USLT NTSC DVDRemux DD2.0", "Series.Title.S01.NTSC.DVDRip.DD2.0.x264-PLAiD"]

RAW_DISK = [
    T("should_return_true_if_no_container_specified_and_does_not_match_disc_release_pattern",
      "A release with no container and a non-disc title is accepted", raw(), {"accepted": True}),
    T("should_return_true_if_mkv", "An MKV container is accepted", raw(container="MKV"), {"accepted": True}),
    T("should_return_false_if_vob", "A VOB container is rejected", raw(container="VOB"), {"accepted": False}),
    T("should_return_false_if_iso", "An ISO container is rejected", raw(container="ISO"), {"accepted": False}),
    T("should_return_false_if_m2ts", "An M2TS container is rejected", raw(container="M2TS"), {"accepted": False}),
    T("should_compare_case_insensitive", "A lower-case vob container is rejected", raw(container="vob"),
      {"accepted": False}),
    R("should_return_false_if_matches_disc_format",
      [(json.dumps(t), f"Disc-format title \"{t}\" is rejected", raw(t), {"accepted": False}) for t in DISC]),
    R("should_return_true_if_dvdrip",
      [(json.dumps(t), f"DVD rip title \"{t}\" is accepted", raw(t), {"accepted": True}) for t in DVDRIP]),
]

# --- ReleaseRestrictionsSpecificationFixture --------------------------------

DEXTER = "Dexter.S08E01.EDITED.WEBRip.x264-KYR"


def rr(profiles, title=DEXTER):
    return {"series": {"tags": []}, "release": {"title": title}, "release_profiles": profiles}


def rp(required=(), ignored=()):
    return {"required": list(required), "ignored": list(ignored)}


RELEASE_RESTRICTIONS = [
    T("should_be_true_when_restrictions_are_empty", "A release is accepted when no release profile applies",
      rr([]), {"accepted": True}),
    T("should_be_true_when_title_contains_one_required_term", "A title containing the required term WEBRip is accepted",
      rr([rp(["WEBRip"])]), {"accepted": True}),
    T("should_be_false_when_title_does_not_contain_any_required_terms",
      "A title containing none of the required terms (doesnt, exist) is rejected",
      rr([rp(["doesnt", "exist"])]), {"accepted": False}),
    T("should_be_true_when_title_does_not_contain_any_ignored_terms",
      "A title without the ignored term \"ignored\" is accepted", rr([rp(ignored=["ignored"])]), {"accepted": True}),
    T("should_be_false_when_title_contains_one_anded_ignored_terms",
      "A title containing the ignored term \"edited\" is rejected", rr([rp(ignored=["edited"])]), {"accepted": False}),
    R("should_ignore_case_when_matching_required",
      [(json.dumps(s), f"Required terms {s.split(',')} match the title case-insensitively and accept it",
        rr([rp(s.split(","))]), {"accepted": True}) for s in ["EdiTED", "webrip", "X264", "X264,NOTTHERE"]]),
    R("should_ignore_case_when_matching_ignored",
      [(json.dumps(s), f"Ignored terms {s.split(',')} match the title case-insensitively and reject it",
        rr([rp(ignored=s.split(","))]), {"accepted": False}) for s in ["EdiTED", "webrip", "X264", "X264,NOTTHERE"]]),
    T("should_be_false_when_release_contains_one_restricted_word_and_one_required_word",
      "A title with both a required term (x264) and an ignored term (www.Speed.cd) is rejected",
      rr([rp(["x264"], ["www.Speed.cd"])],
         title="[ www.Speed.cd ] -Whose.Line.is.it.Anyway.US.S10E24.720p.HDTV.x264-BAJSKORV"),
      {"accepted": False}),
    R("should_match_perl_regex", [
        ('"/WEB/", true', "Required regex /WEB/ matches the title and accepts it", rr([rp(["/WEB/"])]),
         {"accepted": True}),
        (r'"/WEB\b/", false', "Required term \"/WEB<backspace>/\" does not match and rejects the title",
         rr([rp(["/WEB\b/"])]), {"accepted": False},
         "C# regular string: \\b is a backspace character (U+0008), not a regex word boundary."),
        ('"/WEb/", false', "Required regex /WEb/ is case-sensitive, does not match, and rejects the title",
         rr([rp(["/WEb/"])]), {"accepted": False}),
        (r'@"/\.WEB/", true', "Required regex /\\.WEB/ matches \".WEB\" in the title and accepts it",
         rr([rp(["/\\.WEB/"])]), {"accepted": True}),
    ]),
]

# --- RepackSpecificationFixture ---------------------------------------------


def repack(repack=True, file=None, release_group="Sonarr", propers=None):
    g = {"release": {**q("SDTV", 2, repack=repack), "release_group": release_group}}
    e = {"episode_file_id": 0}
    if file:
        e = {"episode_file_id": 1, "file": {**q(file[0]), "release_group": file[1]}}
    g["episodes"] = [e]
    if propers:
        g["config"] = {"download_propers_and_repacks": propers}
    return g


REPACK = [
    T("should_return_true_if_it_is_not_a_repack", "A release that is not a repack is accepted",
      repack(repack=False), {"accepted": True}),
    T("should_return_true_if_there_are_is_no_episode_file", "A repack is accepted when the episode has no file",
      repack(), {"accepted": True}),
    T("should_return_true_if_is_a_repack_for_a_different_quality",
      "A repack is accepted when the existing file is a different quality (DVD)",
      repack(file=("DVD", "Sonarr")), {"accepted": True}),
    T("should_return_true_if_is_a_repack_for_existing_file",
      "A repack is accepted when the existing file has the same quality and release group",
      repack(file=("SDTV", "Sonarr")), {"accepted": True}),
    T("should_return_false_if_is_a_repack_for_a_different_file",
      "A repack is rejected when the existing file's release group differs",
      repack(file=("SDTV", "NotSonarr")), {"accepted": False}),
    T("should_return_false_if_release_group_for_existing_file_is_unknown",
      "A repack is rejected when the existing file's release group is empty",
      repack(file=("SDTV", "")), {"accepted": False}),
    T("should_return_false_if_release_group_for_release_is_unknown",
      "A repack is rejected when the release's own release group is unknown",
      repack(file=("SDTV", "Sonarr"), release_group=None), {"accepted": False}),
    T("should_return_true_when_repacks_are_not_preferred", "With DoNotPrefer, a repack of the existing file is accepted",
      repack(file=("SDTV", "Sonarr"), propers="DoNotPrefer"), {"accepted": True}),
    T("should_return_true_when_repack_but_auto_download_repacks_is_true",
      "With PreferAndUpgrade, a repack of the existing file is accepted",
      repack(file=("SDTV", "Sonarr"), propers="PreferAndUpgrade"), {"accepted": True}),
    T("should_return_false_when_repack_but_auto_download_repacks_is_false",
      "With DoNotUpgrade, a repack of the existing file is rejected",
      repack(file=("SDTV", "Sonarr"), propers="DoNotUpgrade"), {"accepted": False}),
]

# --- RetentionSpecificationFixture ------------------------------------------


def ret(days, age, protocol="Usenet"):
    return {"config": {"retention_days": days}, "release": {"protocol": protocol, "age_days": age}}


RETENTION = [
    T("should_return_true_when_retention_is_set_to_zero", "Retention 0 accepts a 100-day-old Usenet release",
      ret(0, 100), {"accepted": True}),
    T("should_return_true_when_release_if_younger_than_retention", "Retention 1000 accepts a 100-day-old release",
      ret(1000, 100), {"accepted": True}),
    T("should_return_true_when_release_and_retention_are_the_same", "Retention 100 accepts a 100-day-old release",
      ret(100, 100), {"accepted": True}),
    T("should_return_false_when_old_than_retention", "Retention 10 rejects a 100-day-old Usenet release",
      ret(10, 100), {"accepted": False}),
    T("should_return_true_if_release_came_out_today_and_retention_is_zero",
      "Retention 0 accepts a release (the test arranges age 100 days, not today)", ret(0, 100), {"accepted": True},
      note="The test name says 'came out today' but it arranges WithAge(100)."),
    T("should_return_true_when_release_is_not_usenet", "Retention does not apply to a torrent release",
      ret(10, 100, "Torrent"), {"accepted": True}),
]

# --- SameEpisodesSpecificationFixture ---------------------------------------


def same(release, files):
    return {"release_episodes": release, "episodes_in_file": files}


SAME_EPISODES = [
    T("should_not_upgrade_when_new_release_contains_less_episodes",
      "A release with fewer episodes than the existing file is not an upgrade",
      same([{"id": 1, "episode_file_id": 1}], {"*": [1, 2]}), {"accepted": False}),
    T("should_upgrade_when_new_release_contains_more_episodes",
      "A release with more episodes than the existing file is an upgrade",
      same([{"id": 1, "episode_file_id": 1}, {"id": 2, "episode_file_id": 1}], {"*": [1]}), {"accepted": True}),
    T("should_upgrade_when_new_release_contains_the_same_episodes",
      "A release with the same episodes as the existing file is an upgrade",
      same([{"id": 1, "episode_file_id": 1}, {"id": 2, "episode_file_id": 1}], {"*": [1, 2]}), {"accepted": True}),
    T("should_upgrade_when_release_contains_the_same_episodes_as_multiple_files",
      "A release covering episodes held in two separate files is an upgrade",
      same([{"id": 1, "episode_file_id": 1}, {"id": 2, "episode_file_id": 2}], {"1": [1], "2": [2]}),
      {"accepted": True}),
]
SAME_NOTE = "episodes_in_file maps an episode file id to the episode ids in it; \"*\" answers for any file id."

# --- SingleEpisodeAgeDownloadDecisionFixture --------------------------------

SEASON_EPS = [{"season": 1, "episode": n, "air_age_days": a} for n, a in [(1, 400), (2, 370), (3, 340), (4, 310)]]


def season_pack_only(max_age, release_episodes, full_season=False):
    rel = {**q("SDTV", 2), "season_search_maximum_single_episode_age": max_age, "episodes": release_episodes}
    if full_season:
        rel["full_season"] = True
    return {"series": {"series_type": "Standard"},
            "search": {"kind": "SeasonSearchCriteria", "season": 1, "episodes": SEASON_EPS}, "release": rel}


def single_row(n, age, ok):
    return (f"{n}, {age}, {str(ok).lower()}",
            f"In a season search, a single-episode release of episode {n} with max single-episode age {age} days is "
            f"{'accepted' if ok else 'rejected'}",
            season_pack_only(age, [SEASON_EPS[n - 1]]), {"accepted": ok})


def multi_row(age):
    return (f"{age}, true", f"In a season search, a season pack is accepted with max single-episode age {age} days",
            season_pack_only(age, SEASON_EPS, full_season=True), {"accepted": True})


SINGLE_EPISODE_AGE = [
    R("single_episode_release", [single_row(*r) for r in
                                 [(1, 200, False), (4, 200, False), (1, 600, True), (1, 365, True), (4, 365, True),
                                  (1, 0, True)]]),
    R("multi_episode_release", [multi_row(a) for a in [200, 600, 365, 0]]),
]

# --- UpgradeAllowedSpecificationFixture -------------------------------------

UA_ITEMS = items(One=50, Two=100)


def ua(upgrade, cur, cur_cf, new, new_cf):
    return {"profile": profile(cutoff=B1080, upgrade_allowed=upgrade, cutoff_format_score=100, format_items=UA_ITEMS),
            "current": {**cur, "custom_formats": cur_cf}, "new": {**new, "custom_formats": new_cf}}


def ua_t(method, behaviour, upgrade, cur, cur_cf, new, new_cf, ok):
    return T(method, behaviour, ua(upgrade, cur, cur_cf, new, new_cf), {"accepted": ok})


DVD = q("DVD")
UPGRADE_ALLOWED = [
    ua_t("should_return_false_when_quality_is_better_custom_formats_are_the_same_and_upgrading_is_not_allowed",
         "With upgrades off, DVD to Bluray-1080p is not allowed", False, DVD, [], q(B1080), [], False),
    ua_t("should_return_false_when_quality_is_same_and_custom_format_is_upgrade_and_upgrading_is_not_allowed",
         "With upgrades off, a custom format upgrade (One 50 to Two 100) is not allowed", False, DVD, ["One"], DVD,
         ["Two"], False),
    ua_t("should_return_true_for_custom_format_upgrade_when_upgrading_is_allowed",
         "With upgrades on, a custom format upgrade (One to Two) is allowed", True, DVD, ["One"], DVD, ["Two"], True),
    ua_t("should_return_true_for_same_custom_format_score_when_upgrading_is_not_allowed",
         "With upgrades off, the same custom format score is allowed", False, DVD, ["One"], DVD, ["One"], True),
    ua_t("should_return_true_for_lower_custom_format_score_when_upgrading_is_allowed",
         "With upgrades on, a lower custom format score is allowed", True, DVD, ["Two"], DVD, ["One"], True),
    ua_t("should_return_true_for_lower_language_when_upgrading_is_not_allowed",
         "With upgrades off, a lower custom format score (Two to One) is allowed", False, DVD, ["Two"], DVD, ["One"],
         True),
    ua_t("should_return_true_for_quality_upgrade_when_upgrading_is_allowed",
         "With upgrades on, DVD to Bluray-1080p is allowed", True, DVD, [], q(B1080), [], True),
    ua_t("should_return_true_for_same_quality_when_upgrading_is_allowed",
         "With upgrades on, the same quality is allowed", True, DVD, [], DVD, [], True),
    ua_t("should_return_true_for_same_quality_when_upgrading_is_not_allowed",
         "With upgrades off, the same quality is allowed", False, DVD, [], DVD, [], True),
    ua_t("should_return_true_for_lower_quality_when_upgrading_is_allowed",
         "With upgrades on, a lower quality (SDTV) is allowed", True, DVD, [], q("SDTV"), [], True),
    ua_t("should_return_true_for_lower_quality_when_upgrading_is_not_allowed",
         "With upgrades off, a lower quality (SDTV) is allowed", False, DVD, [], q("SDTV"), [], True),
    ua_t("should_returntrue_when_quality_is_revision_upgrade_for_same_quality",
         "With upgrades off, a revision upgrade (DVD v1 to v2) is allowed", False, q("DVD", 1), ["One"], q("DVD", 2),
         ["One"], True),
]

# --- UpgradeDiskSpecificationFixture ----------------------------------------


def disk_file(quality):
    return {**quality, "languages": ["English"], "added_age_days": 0}


def disk(files=(q(B1080, 2), q(B1080, 2)), multi=False, new=None, prof=None, old_cf=(), new_cf=(), score=None,
         propers=None, episodes=None):
    f1, f2 = disk_file(files[0]), disk_file(files[1])
    if episodes is None:
        episodes = ([{"episode_file_id": 1, "file": f1}, {"episode_file_id": 1, "file": f2}, {"episode_file_id": 0}]
                    if multi else [{"episode_file_id": 1, "file": f1}, {"episode_file_id": 0}])
    rel = {**(new or q("DVD", 2)), "languages": ["English"], "custom_formats": list(new_cf)}
    if score is not None:
        rel["custom_format_score"] = score
    g = {"profile": prof or profile(upgrade_allowed=True, cutoff=B1080, min_format_score=0),
         "release": rel, "episodes": episodes, "existing_file_custom_formats": list(old_cf)}
    if propers:
        g["config"] = {"download_propers_and_repacks": propers}
    return g


def gp(cutoff, upgrade=True, **kw):
    """GivenProfile (UpgradeDiskSpecificationFixture.cs:81) clears format items and sets min_format_score 0."""
    return profile(cutoff=cutoff, upgrade_allowed=upgrade, min_format_score=0, **kw)


SD = q("SDTV")
H720_2 = q("HDTV-720p", 2)
GP_NOTE = ("GivenProfile replaces the profile's format items with an empty list (no custom formats are registered), "
           "so the initializer's GetSampleFormatItems(\"My Format\") has no effect.")
UPGRADE_DISK = [
    T("should_return_true_if_episode_has_no_existing_file", "A release is accepted when no episode has a file",
      disk(episodes=[{"episode_file_id": 0, "file": disk_file(q(B1080, 2))}, {"episode_file_id": 0}]),
      {"accepted": True}),
    T("should_return_true_if_single_episode_doesnt_exist_on_disk", "A release with no episodes is accepted",
      disk(episodes=[]), {"accepted": True}),
    T("should_be_upgradable_if_only_episode_is_upgradable", "A DVD release upgrades an SDTV file",
      disk(files=(SD, q(B1080, 2))), {"accepted": True}),
    T("should_be_upgradable_if_both_episodes_are_upgradable", "A DVD release upgrades two SDTV files",
      disk(files=(SD, SD), multi=True), {"accepted": True}),
    T("should_be_not_upgradable_if_both_episodes_are_not_upgradable",
      "A DVD release does not upgrade two Bluray-1080p files", disk(multi=True), {"accepted": False}),
    T("should_be_not_upgradable_if_only_first_episodes_is_upgradable",
      "A multi-episode release is rejected when only the first file is upgradable",
      disk(files=(SD, q(B1080, 2)), multi=True), {"accepted": False}),
    T("should_be_not_upgradable_if_only_second_episodes_is_upgradable",
      "A multi-episode release is rejected when only the second file is upgradable",
      disk(files=(q(B1080, 2), SD), multi=True), {"accepted": False}),
    T("should_not_be_upgradable_if_qualities_are_the_same", "WEBDL-1080p does not upgrade a WEBDL-1080p file",
      disk(files=(q(W1080), q(B1080, 2)), new=q(W1080)), {"accepted": False}),
    T("should_not_be_upgradable_if_revision_downgrade_and_preferred_word_upgrade_if_propers_are_preferred",
      "A WEBDL-1080p v1 with score 10 does not replace a WEBDL-1080p v2 file",
      disk(files=(q(W1080, 2), q(B1080, 2)), new=q(W1080), score=10), {"accepted": False}),
    T("should_return_false_if_current_episode_is_equal_to_cutoff", "An existing file at the HDTV-720p cutoff is not upgraded",
      disk(files=(H720_2, H720_2), prof=gp("HDTV-720p")), {"accepted": False}),
    T("should_return_false_if_current_episode_is_greater_than_cutoff",
      "An existing Bluray-1080p file above the HDTV-720p cutoff is not upgraded",
      disk(files=(q(B1080, 2), q(B1080, 2)), prof=gp("HDTV-720p")), {"accepted": False}),
    T("should_return_true_when_new_episode_is_proper_but_existing_is_not",
      "A HDTV-720p proper upgrades a HDTV-720p v1 file at the cutoff",
      disk(files=(q("HDTV-720p", 1), q("HDTV-720p", 1)), new=H720_2, prof=gp("HDTV-720p")), {"accepted": True}),
    T("should_return_false_if_cutoff_is_met_and_quality_is_higher",
      "With the HDTV-720p cutoff met, Bluray-1080p is not an upgrade",
      disk(files=(H720_2, H720_2), new=q(B1080, 2), prof=gp("HDTV-720p")), {"accepted": False}),
    T("should_return_false_if_quality_cutoff_is_met_and_quality_is_higher_but_language_is_met",
      "With the HDTV-720p cutoff met, Bluray-1080p is not an upgrade (language variant)",
      disk(files=(H720_2, H720_2), new=q(B1080, 2), prof=gp("HDTV-720p")), {"accepted": False}),
    T("should_return_false_if_cutoff_is_met_and_quality_is_higher_and_language_is_higher",
      "With the HDTV-720p cutoff met, Bluray-1080p is not an upgrade (higher language variant)",
      disk(files=(H720_2, H720_2), new=q(B1080, 2), prof=gp("HDTV-720p")), {"accepted": False}),
    T("should_return_true_if_cutoff_is_not_met_and_new_quality_is_higher_and_language_is_higher",
      "Below the HDTV-720p cutoff, Bluray-1080p upgrades an SDTV file",
      disk(files=(q("SDTV", 2), q("SDTV", 2)), new=q(B1080, 2), prof=gp("HDTV-720p")), {"accepted": True}),
    T("should_return_true_if_cutoff_is_not_met_and_language_is_higher",
      "Below the HDTV-720p cutoff, a DVD release upgrades an SDTV file",
      disk(files=(q("SDTV", 2), q("SDTV", 2)), prof=gp("HDTV-720p")), {"accepted": True}),
    T("should_return_false_if_custom_formats_is_met_and_quality_and_format_higher",
      "With the HDTV-720p cutoff met, Bluray-1080p with a new custom format is not an upgrade",
      disk(files=(q("HDTV-720p"), q("HDTV-720p")), new=q(B1080), prof=gp("HDTV-720p"), new_cf=["My Format"]),
      {"accepted": False}, note=GP_NOTE),
    T("should_return_true_if_cutoffs_are_met_but_is_a_revision_upgrade",
      "Above the HDTV-1080p cutoff, a WEBDL-1080p proper still upgrades WEBDL-1080p v1",
      disk(files=(q(W1080, 1), q(W1080, 1)), new=q(W1080, 2), prof=gp("HDTV-1080p")), {"accepted": True}),
    T("should_return_false_if_quality_profile_does_not_allow_upgrades_but_cutoff_is_set_to_highest_quality",
      "With upgrades off and cutoff Raw-HD, Bluray-1080p does not replace WEBDL-1080p",
      disk(files=(q(W1080), q(W1080)), new=q(B1080), prof=gp("Raw-HD", upgrade=False)), {"accepted": False}),
    T("should_return_false_if_quality_profile_does_not_allow_upgrades_but_format_cutoff_is_above_current_score",
      "With upgrades off, a custom format gain (0 to 50, cutoff score 10000) is not an upgrade",
      disk(files=(q(W1080), q(W1080)), new=q(W1080), new_cf=["My Format"],
           prof=gp("SDTV", upgrade=False, cutoff_format_score=10000, format_items=items(My_Format=50))),
      {"accepted": False}),
    T("should_return_false_if_quality_profile_does_not_allow_upgrades_but_format_cutoff_is_above_current_score_and_is_revision_upgrade",
      "With upgrades off and DoNotPrefer, a proper with a custom format gain is not an upgrade",
      disk(files=(q(W1080, 1), q(W1080, 1)), new=q(W1080, 2), new_cf=["My Format"], propers="DoNotPrefer",
           prof=gp("SDTV", upgrade=False, cutoff_format_score=10000, format_items=items(My_Format=50))),
      {"accepted": False}),
]

# --- UpgradeSpecificationFixture --------------------------------------------

CS_Q = {"SDTV": "SDTV", "WEBDL720p": "WEBDL-720p", "HDTV720p": "HDTV-720p", "Bluray720p": "Bluray-720p",
        "WEBDL1080p": "WEBDL-1080p"}


def upg(prof, cur, new, cur_cf=(), new_cf=(), propers=None):
    g = {"profile": prof, "current": {**cur, "custom_formats": list(cur_cf)},
         "new": {**new, "custom_formats": list(new_cf)}}
    if propers:
        g["config"] = {"download_propers_and_repacks": propers}
    return g


def reason(r):
    return {"accepted": True} if r == "None" else {"accepted": False, "rejection": r}


def upgrade_row(c, cv, n, nv, cutoff, r):
    return (f"Quality.{c}, {cv}, Quality.{n}, {nv}, Quality.{cutoff}, UpgradeableRejectReason.{r}",
            f"{CS_Q[c]} v{cv} to {CS_Q[n]} v{nv} gives {r}",
            upg(profile(upgrade_allowed=True), q(CS_Q[c], cv), q(CS_Q[n], nv), propers="PreferAndUpgrade"), reason(r),
            f"The row's cutoff argument ({CS_Q[cutoff]}) is never applied to the profile by the test.")


MIN_CF_ITEMS = items(One=10, Two=20)
UPGRADE = [
    R("IsUpgradeTest", [upgrade_row(*r) for r in [
        ("SDTV", 1, "SDTV", 2, "SDTV", "None"),
        ("WEBDL720p", 1, "WEBDL720p", 2, "WEBDL720p", "None"),
        ("SDTV", 1, "SDTV", 1, "SDTV", "CustomFormatScore"),
        ("WEBDL720p", 1, "HDTV720p", 2, "Bluray720p", "BetterQuality"),
        ("WEBDL720p", 1, "HDTV720p", 2, "WEBDL720p", "BetterQuality"),
        ("WEBDL720p", 1, "WEBDL720p", 1, "WEBDL720p", "CustomFormatScore"),
        ("WEBDL1080p", 1, "WEBDL1080p", 1, "WEBDL1080p", "CustomFormatScore")]]),
    T("should_return_true_if_proper_and_download_propers_is_do_not_download",
      "With DoNotUpgrade, DVD v1 to DVD v2 is upgradable",
      upg(profile(), q("DVD", 1), q("DVD", 2), propers="DoNotUpgrade"), reason("None")),
    T("should_return_false_if_proper_and_autoDownloadPropers_is_do_not_prefer",
      "With DoNotPrefer and upgrades off, DVD v1 to DVD v2 gives UpgradesNotAllowed",
      upg(profile(), q("DVD", 1), q("DVD", 2), propers="DoNotPrefer"), reason("UpgradesNotAllowed")),
    T("should_return_false_if_release_and_existing_file_are_the_same",
      "With upgrades off, HDTV-720p v1 to the same gives UpgradesNotAllowed",
      upg(profile(), q("HDTV-720p", 1), q("HDTV-720p", 1)), reason("UpgradesNotAllowed")),
    T("should_return_true_if_release_has_higher_quality_and_cutoff_is_not_already_met",
      "Below the HDTV-1080p cutoff, HDTV-720p to HDTV-1080p is upgradable",
      upg(profile(upgrade_allowed=True, cutoff="HDTV-1080p"), q("HDTV-720p"), q("HDTV-1080p")), reason("None")),
    T("should_return_false_if_release_has_higher_quality_and_cutoff_is_already_met",
      "At the HDTV-720p cutoff, HDTV-720p to HDTV-1080p gives QualityCutoff",
      upg(profile(upgrade_allowed=True, cutoff="HDTV-720p"), q("HDTV-720p"), q("HDTV-1080p")),
      reason("QualityCutoff")),
    T("should_return_false_if_minimum_custom_score_is_not_met",
      "A custom format gain of 10 below the minimum increment 11 gives MinCustomFormatScore",
      upg(profile(upgrade_allowed=True, min_upgrade_format_score=11, cutoff_format_score=100,
                  format_items=MIN_CF_ITEMS), DVD, DVD, ["One"], ["Two"]), reason("MinCustomFormatScore")),
    T("should_return_true_if_minimum_custom_score_is_met",
      "A custom format gain of 10 at the minimum increment 10 is upgradable",
      upg(profile(upgrade_allowed=True, min_upgrade_format_score=10, cutoff_format_score=100,
                  format_items=MIN_CF_ITEMS), DVD, DVD, ["One"], ["Two"]), reason("None")),
]

# --- RssSync/DelaySpecificationFixture --------------------------------------

DELAY_PROFILE_NOTE = "Other delay profile fields are NBuilder-generated and not arranged by the test."


def delay(quality=None, age=None, usenet_delay=None, languages=None, existing=None, cf_score=None, search=None,
          **dp):
    rel = {"protocol": "Usenet"}
    if quality:
        rel.update(quality)
    if languages:
        rel["languages"] = languages
    if age is not None:
        rel.update(age)
    if cf_score is not None:
        rel["custom_format_score"] = cf_score
    e = {"id": 1, "episode_file_id": 0}
    if existing:
        e = {"id": 1, "episode_file_id": 1, "file": {**existing, "languages": ["English"],
                                                     "scene_name": "Series.Title.S01E01.720p.HDTV.x264-Sonarr"}}
    d = {"preferred_protocol": "Usenet", **dp}
    if usenet_delay is not None:
        d["usenet_delay"] = usenet_delay
    g = {"profile": profile(qualities=["HDTV-720p", "WEBDL-720p", "Bluray-720p"], cutoff="WEBDL-720p"),
         "delay_profile": d, "release": rel, "episodes": [e], "pending_releases": []}
    if search:
        g["search"] = search
    return g


NOW = {"age_minutes": 0}
EXISTING_MOCK_NOTE = ("The test also mocks IUpgradableSpecification.IsUpgradable to None and IsRevisionUpgrade to true; "
                      "DelaySpecification compares revisions itself.")
DELAY = [
    T("should_be_true_when_user_invoked_search", "A user-invoked search bypasses the delay",
      {"search": {"kind": "SingleEpisodeSearchCriteria", "user_invoked": True}},
      {"accepted": True}, note="The subject is an empty RemoteEpisode (no series, release or episodes)."),
    T("should_be_false_when_system_invoked_search_and_release_is_younger_than_delay",
      "A system-invoked search does not bypass a 720-minute delay for a new SDTV release",
      delay(q("SDTV"), NOW, 720, search={"kind": "SingleEpisodeSearchCriteria", "user_invoked": False}),
      {"accepted": False}, note=DELAY_PROFILE_NOTE),
    T("should_be_true_when_profile_does_not_have_a_delay", "A release is accepted when the Usenet delay is 0",
      delay(usenet_delay=0), {"accepted": True}),
    T("should_be_false_when_quality_and_language_is_last_allowed_in_profile_and_bypass_disabled",
      "The profile's best quality (Bluray-720p) is still delayed when bypass is disabled",
      delay(q("Bluray-720p"), NOW, 720, languages=["French"], bypass_if_highest_quality=False), {"accepted": False},
      note="bypass_if_highest_quality is not set by the test (NBuilder default false), as the test name states."),
    T("should_be_true_when_quality_and_language_is_last_allowed_in_profile_and_bypass_enabled",
      "The profile's best quality (Bluray-720p) bypasses the delay when bypass is enabled",
      delay(q("Bluray-720p"), NOW, 720, languages=["French"], bypass_if_highest_quality=True), {"accepted": True}),
    T("should_be_true_when_release_is_older_than_delay", "A 10-hour-old release passes a 60-minute delay",
      delay(q("HDTV-720p"), {"age_hours": 10}, 60), {"accepted": True}),
    T("should_be_false_when_release_is_younger_than_delay", "A new SDTV release waits out a 720-minute delay",
      delay(q("SDTV"), NOW, 720), {"accepted": False}),
    T("should_be_true_when_release_is_a_proper_for_existing_episode",
      "A proper of the existing file's quality (HDTV-720p) bypasses the delay",
      delay(q("HDTV-720p", 2), NOW, 720, existing=q("HDTV-720p")), {"accepted": True}, note=EXISTING_MOCK_NOTE),
    T("should_be_true_when_release_is_a_real_for_existing_episode",
      "A REAL of the existing file's quality (HDTV-720p) bypasses the delay",
      delay(q("HDTV-720p", 1, real=1), NOW, 720, existing=q("HDTV-720p")), {"accepted": True},
      note=EXISTING_MOCK_NOTE),
    T("should_be_false_when_release_is_proper_for_existing_episode_of_different_quality",
      "A WEBDL-720p proper does not bypass the delay over an HDTV-720p file",
      delay(q("WEBDL-720p", 2), NOW, 720, existing=q("HDTV-720p")), {"accepted": False}),
    T("should_be_false_when_custom_format_score_is_above_minimum_but_bypass_disabled",
      "A custom format score of 100 above minimum 50 does not bypass the delay when the bypass is disabled",
      delay(age=NOW, usenet_delay=720, cf_score=100, minimum_custom_format_score=50,
            bypass_if_above_custom_format_score=False), {"accepted": False},
      note="Release quality is not set. bypass_if_above_custom_format_score is not set by the test (NBuilder "
           "default false), as the test name states."),
    T("should_be_false_when_custom_format_score_is_above_minimum_and_bypass_enabled_but_under_minimum",
      "A custom format score of 5 under minimum 50 does not bypass the delay",
      delay(age=NOW, usenet_delay=720, cf_score=5, minimum_custom_format_score=50,
            bypass_if_above_custom_format_score=True), {"accepted": False}, note="Release quality is not set."),
    T("should_be_true_when_custom_format_score_is_above_minimum_and_bypass_enabled",
      "A custom format score of 100 above minimum 50 bypasses the delay when enabled",
      delay(age=NOW, usenet_delay=720, cf_score=100, minimum_custom_format_score=50,
            bypass_if_above_custom_format_score=True), {"accepted": True}, note="Release quality is not set."),
]

# --- RssSync/DeletedEpisodeFileSpecificationFixture -------------------------

DEL_FILES = [{"id": 1, "relative_path": "My.Series.S01E01.mkv", **q(B1080, 1)},
             {"id": 2, "relative_path": "My.Series.S01E02.mkv", **q(B1080, 1)}]


def deleted(on_disk=(), multi=False, unmonitor=True, search=None):
    eps = [{"episode_file_id": f["id"], "file": f} for f in DEL_FILES[:2 if multi else 1]]
    g = {"series": {"path": "C:\\Series\\My.Series"}, "profile": {"cutoff": B1080}, "release": q("DVD", 2),
         "episodes": eps, "config": {"auto_unmonitor_previously_downloaded_episodes": unmonitor},
         "files_on_disk": [f"C:\\Series\\My.Series\\{p}" for p in on_disk]}
    if search:
        g["search"] = search
    return g


DEL_NOTE = "Paths go through AsOsAgnostic() in the test (C:\\ prefix becomes a POSIX root on Linux)."
DELETED_EPISODE_FILE = [
    T("should_return_true_when_unmonitor_deleted_episdes_is_off",
      "A release is accepted when auto-unmonitoring of deleted episodes is off", deleted(unmonitor=False),
      {"accepted": True}, note=DEL_NOTE),
    T("should_return_true_when_searching", "A search is accepted even if the file is missing",
      deleted(search={"kind": "SeasonSearchCriteria"}), {"accepted": True}, note=DEL_NOTE),
    T("should_return_true_if_file_exists", "A release is accepted when the episode file exists on disk",
      deleted(["My.Series.S01E01.mkv"]), {"accepted": True}, note=DEL_NOTE),
    T("should_return_false_if_file_is_missing", "An RSS release is rejected when the episode file is missing from disk",
      deleted(), {"accepted": False}, note=DEL_NOTE),
    T("should_return_true_if_both_of_multiple_episode_exist",
      "A multi-episode release is accepted when both files exist", deleted(
          ["My.Series.S01E01.mkv", "My.Series.S01E02.mkv"], multi=True), {"accepted": True}, note=DEL_NOTE),
    T("should_return_false_if_one_of_multiple_episode_is_missing",
      "A multi-episode release is rejected when one file is missing", deleted(["My.Series.S01E01.mkv"], multi=True),
      {"accepted": False}, note=DEL_NOTE),
]

# --- RssSync/HistorySpecificationFixture ------------------------------------

UPG_H = {**q("SDTV", 1), "languages": ["English"]}
NOT_UPG_H = {**q("HDTV-1080p", 2), "languages": ["English"]}
SINGLE_H = [{"id": 1, "season": 12, "episode": 3}]
MULTI_H = SINGLE_H + [{"id": 2, "season": 12, "episode": 4}, {"id": 3, "season": 12, "episode": 5}]


def hist(item, age, event="Grabbed", download_id=""):
    return {"download_id": download_id, **item, "event_type": event, **age}


def history(records, multi=True, cdh=True, prof=None, release=None, search=None):
    g = {"profile": prof or profile(upgrade_allowed=True, cutoff=B1080, min_format_score=0),
         "config": {"enable_completed_download_handling": cdh},
         "release": {**(release or q("DVD", 2)), "languages": ["English"], "custom_formats": []},
         "episodes": MULTI_H if multi else SINGLE_H,
         "most_recent_history": {str(k): v for k, v in records.items()}}
    if search:
        g["search"] = search
    return g


H0 = {"age_hours": 0}
HIST_NOTE = "most_recent_history maps an episode id to its latest history item; history custom formats are []."
HISTORY = [
    T("should_return_true_if_it_is_a_search", "A search is not blocked by history",
      history({}, search={"kind": "SeasonSearchCriteria"}), {"accepted": True}),
    T("should_return_true_if_latest_history_item_is_null", "A release is accepted when no episode has history",
      history({}), {"accepted": True}),
    T("should_return_true_if_latest_history_item_is_not_grabbed",
      "A latest history item that is DownloadFailed does not block the release",
      history({1: hist(NOT_UPG_H, H0, "DownloadFailed")}), {"accepted": True}),
    T("should_return_true_if_latest_history_item_is_older_than_twelve_hours",
      "A grab older than 12 hours does not block the release",
      history({1: hist(NOT_UPG_H, {"age_hours": 13})}), {"accepted": True}),
    T("should_be_upgradable_if_only_episode_is_upgradable",
      "A DVD release is accepted over a recent SDTV grab", history({1: hist(UPG_H, H0)}, multi=False),
      {"accepted": True}),
    T("should_be_upgradable_if_both_episodes_are_upgradable",
      "A DVD release is accepted over recent SDTV grabs of both episodes",
      history({1: hist(UPG_H, H0), 2: hist(UPG_H, H0)}), {"accepted": True}),
    T("should_not_be_upgradable_if_both_episodes_are_not_upgradable",
      "A DVD release is rejected over recent HDTV-1080p grabs of both episodes",
      history({1: hist(NOT_UPG_H, H0), 2: hist(NOT_UPG_H, H0)}), {"accepted": False}),
    T("should_be_not_upgradable_if_only_first_episodes_is_upgradable",
      "A DVD release is rejected when episode 1's latest grab is HDTV-1080p",
      history({1: hist(NOT_UPG_H, H0)}), {"accepted": False},
      note="The test sets episode 1's history twice (SDTV, then HDTV-1080p); the later mock setup wins."),
    T("should_be_not_upgradable_if_only_second_episodes_is_upgradable",
      "A DVD release is rejected when episode 1's recent grab is HDTV-1080p even if episode 2's is SDTV",
      history({1: hist(NOT_UPG_H, H0), 2: hist(UPG_H, H0)}), {"accepted": False}),
    T("should_not_be_upgradable_if_episode_is_of_same_quality_as_existing",
      "A WEBDL-1080p release is rejected over a recent WEBDL-1080p grab",
      history({1: hist({**q(W1080, 1), "languages": ["English"]}, H0)}, multi=False, release=q(W1080, 1),
              prof=profile(cutoff=B1080)), {"accepted": False}),
    T("should_not_be_upgradable_if_cutoff_already_met",
      "A release is rejected when the recent grab (WEBDL-1080p) meets the WEBDL-1080p cutoff",
      history({1: hist({**q(W1080, 1), "languages": ["Spanish"]}, H0)}, multi=False, release=q(W1080, 1),
              prof=profile(cutoff=W1080)), {"accepted": False}),
    T("should_return_false_if_latest_history_item_is_only_one_hour_old",
      "A one-hour-old HDTV-1080p grab blocks a DVD release", history({1: hist(NOT_UPG_H, {"age_hours": 1})}),
      {"accepted": False}),
    T("should_return_false_if_latest_history_has_a_download_id_and_cdh_is_disabled",
      "With completed download handling off, a 100-day-old SDTV grab with a download id does not block a DVD release",
      history({1: hist(UPG_H, {"age_days": 100}, download_id="test")}, cdh=False), {"accepted": True},
      note="The test name says 'false' but it asserts BeTrue."),
    T("should_return_false_if_cutoff_already_met_and_cdh_is_disabled",
      "With completed download handling off, a grab meeting the WEBDL-1080p cutoff blocks a Bluray-1080p release",
      history({1: hist({**q(W1080, 1), "languages": ["Spanish"]}, {"age_days": 100}, download_id="test")},
              multi=False, cdh=False, release=q(B1080, 1), prof=profile(cutoff=W1080)), {"accepted": False}),
    T("should_return_false_if_only_episode_is_not_upgradable_and_cdh_is_disabled",
      "With completed download handling off, an HDTV-1080p grab blocks a DVD release",
      history({1: hist(NOT_UPG_H, {"age_days": 100}, download_id="test")}, multi=False, cdh=False),
      {"accepted": False}),
]

# --- RssSync/IndexerTagSpecificationFixture ---------------------------------


def itag(indexer_tags, series_tags, indexer_id=1):
    return {"indexers": {"1": {"tags": indexer_tags}}, "series": {"monitored": True, "tags": series_tags},
            "release": {"indexer_id": indexer_id}, "episodes": [{"monitored": True}, {"monitored": True}],
            "search": {"kind": "SingleEpisodeSearchCriteria", "monitored_episodes_only": True}}


INDEXER_TAG = [
    T("indexer_and_series_without_tags_should_return_true", "An untagged indexer serves an untagged series",
      itag([], []), {"accepted": True}),
    T("indexer_with_tags_series_without_tags_should_return_false", "A tagged indexer does not serve an untagged series",
      itag([123], []), {"accepted": False}),
    T("indexer_without_tags_series_with_tags_should_return_true", "An untagged indexer serves a tagged series",
      itag([], [123]), {"accepted": True}),
    T("indexer_with_tags_series_with_matching_tags_should_return_true",
      "An indexer sharing a tag (123) with the series serves it", itag([123, 456], [123, 789]), {"accepted": True}),
    T("indexer_with_tags_series_with_different_tags_should_return_false",
      "An indexer with no tag in common with the series does not serve it", itag([456], [123, 789]),
      {"accepted": False}),
    T("release_without_indexerid_should_return_true", "A release with no indexer id is accepted",
      itag([456], [123, 789], indexer_id=0), {"accepted": True}),
    T("release_with_invalid_indexerid_should_return_true", "A release from an unknown indexer (id 2) is accepted",
      itag([456], [123, 789], indexer_id=2), {"accepted": True}, note="Only indexer 1 exists; any other id is not found."),
]

# --- RssSync/ProperSpecificationFixture -------------------------------------


def proper(f1, f2=q(B1080, 1), multi=False, propers=None, search=None):
    eps = [{"episode_file_id": 1, "file": f1}]
    if multi:
        eps.append({"episode_file_id": 1, "file": f2})
    eps.append({"episode_file_id": 0})
    g = {"profile": {"cutoff": B1080}, "release": q("DVD", 2), "episodes": eps}
    if propers:
        g["config"] = {"download_propers_and_repacks": propers}
    if search:
        g["search"] = search
    return g


def pf(name, days_ago=0):
    return {**q(name, 1), "added_days_ago": days_ago}


PROPER_NOTE = "added_days_ago counts from DateTime.Today (0 = added today/now)."
PROPER = [
    T("should_return_false_when_episodeFile_was_added_more_than_7_days_ago",
      "A DVD proper is rejected in RSS when the DVD file was added 30 days ago", proper(pf("DVD", 30)),
      {"accepted": False}, note=PROPER_NOTE),
    T("should_return_false_when_first_episodeFile_was_added_more_than_7_days_ago",
      "A DVD proper is rejected in RSS when the first of two DVD files was added 30 days ago",
      proper(pf("DVD", 30), pf("DVD"), multi=True), {"accepted": False}, note=PROPER_NOTE),
    T("should_return_false_when_second_episodeFile_was_added_more_than_7_days_ago",
      "A DVD proper is rejected in RSS when the second of two DVD files was added 30 days ago",
      proper(pf("DVD"), pf("DVD", 30), multi=True), {"accepted": False}, note=PROPER_NOTE),
    T("should_return_true_when_episodeFile_was_added_more_than_7_days_ago_but_proper_is_for_better_quality",
      "A DVD proper is accepted over a 30-day-old SDTV file", proper(pf("SDTV", 30)), {"accepted": True},
      note=PROPER_NOTE),
    T("should_return_true_when_episodeFile_was_added_more_than_7_days_ago_but_is_for_search",
      "A search accepts the proper regardless of file age", proper(pf("SDTV", 30), search={
          "kind": "SingleEpisodeSearchCriteria"}), {"accepted": True}, note=PROPER_NOTE),
    T("should_return_false_when_proper_but_auto_download_propers_is_false",
      "With DoNotUpgrade, a DVD proper over a DVD file added today is rejected",
      proper(pf("DVD"), propers="DoNotUpgrade"), {"accepted": False}, note=PROPER_NOTE),
    T("should_return_true_when_episodeFile_was_added_today",
      "With PreferAndUpgrade, a DVD proper over a DVD file added today is accepted",
      proper(pf("DVD"), propers="PreferAndUpgrade"), {"accepted": True}, note=PROPER_NOTE),
    T("should_return_true_when_propers_are_not_preferred",
      "With DoNotPrefer, a DVD proper over a DVD file added today is accepted",
      proper(pf("DVD"), propers="DoNotPrefer"), {"accepted": True}, note=PROPER_NOTE),
]

# --- Search/SeriesSpecificationFixture --------------------------------------

SEARCH_SERIES = [
    T("should_return_false_if_series_doesnt_match", "A release for series 1 is rejected in a search for series 2",
      {"release": {"series_id": 1}, "search": {"kind": "SingleEpisodeSearchCriteria", "series_id": 2}},
      {"accepted": False}),
    T("should_return_true_when_series_ids_match", "A release for series 1 is accepted in a search for series 1",
      {"release": {"series_id": 1}, "search": {"kind": "SingleEpisodeSearchCriteria", "series_id": 1}},
      {"accepted": True}),
]

# --- Search/TorrentSeedingSpecificationFixture ------------------------------

TITLE = "Series.Title.S01.720p.BluRay.X264-RlsGrp"


def seeding(seeders=0, indexer_id=1, kind="TorrentInfo", indexers=True):
    rel = {"type": kind, "indexer_id": indexer_id, "title": TITLE}
    if kind == "TorrentInfo":
        rel["seeders"] = seeders
    return {"series": {"id": 1}, "release": rel,
            "indexers": {"1": {"minimum_seeders": 5}} if indexers else {}}


TORRENT_SEEDING = [
    T("should_return_true_if_not_torrent", "A non-torrent release is accepted", seeding(kind="ReleaseInfo"),
      {"accepted": True}),
    T("should_return_true_if_indexer_not_specified", "A torrent with no indexer id is accepted", seeding(indexer_id=0),
      {"accepted": True}),
    T("should_return_true_if_indexer_no_longer_exists", "A torrent from a deleted indexer is accepted",
      seeding(indexers=False), {"accepted": True}),
    T("should_return_true_if_seeds_unknown", "A torrent with unknown seeders is accepted", seeding(seeders=None),
      {"accepted": True}),
    R("should_return_true_if_seeds_above_or_equal_to_limit",
      [(str(s), f"A torrent with {s} seeders meets the indexer minimum of 5", seeding(s), {"accepted": True})
       for s in (5, 6)]),
    R("should_return_false_if_seeds_belove_limit",
      [(str(s), f"A torrent with {s} seeders is below the indexer minimum of 5", seeding(s), {"accepted": False})
       for s in (0, 4)]),
]

# --- Search/SingleEpisodeSearchMatchSpecificationTests/ ----------------------
# Both fixtures initialise _remoteEpisode/_searchCriteria once per fixture
# instance; SetUp resets only what is written here as the base.

ANIME_SEARCH = [
    T("should_return_false_if_full_season_result_for_single_episode_search",
      "A full-season release is rejected in an anime single-episode search",
      {"release": {"full_season": True}, "search": {"kind": "AnimeEpisodeSearchCriteria"}}, {"accepted": False}),
    T("should_return_true_if_not_a_full_season_result",
      "A release that is not a full season is accepted in an anime episode search",
      {"release": {"full_season": False}, "search": {"kind": "AnimeEpisodeSearchCriteria"}}, {"accepted": True},
      note="is_season_search is not set by this test; the fixture shares _searchCriteria across tests, so it may "
           "be true from an earlier test. The outcome is accepted either way."),
    T("should_return_true_if_full_season_result_for_full_season_search",
      "A full-season release is accepted in an anime season search",
      {"release": {"full_season": True}, "search": {"kind": "AnimeEpisodeSearchCriteria", "is_season_search": True}},
      {"accepted": True}),
]


def std_search(season=5, episodes=(1,), mapped_season=5, search_season=5):
    return {"release": {"season": season, "episodes": list(episodes), "mapped_season": mapped_season},
            "search": {"kind": "SingleEpisodeSearchCriteria", "season": search_season, "episode": 1}}


STANDARD_SEARCH = [
    T("should_return_false_if_season_does_not_match",
      "A S10E01 release is rejected in a single-episode search for S05E01",
      std_search(season=10, mapped_season=10), {"accepted": False}),
    T("should_return_true_if_season_matches_after_scenemapping",
      "A release parsed as season 10 (mapped to season 5) is accepted in a search for scene season 10 episode 1",
      std_search(season=10, mapped_season=5, search_season=10), {"accepted": True}),
    T("should_return_false_if_season_does_not_match_after_scenemapping",
      "A release parsed as season 10 (mapped to season 6) is rejected in a search for scene season 9 episode 1",
      std_search(season=10, mapped_season=6, search_season=9), {"accepted": False}),
    T("should_return_false_if_full_season_result_for_single_episode_search",
      "A season-5 release with no episode numbers is rejected in a single-episode search for S05E01",
      std_search(episodes=()), {"accepted": False}),
    T("should_return_false_if_episode_number_does_not_match_search_criteria",
      "A S05E02 release is rejected in a single-episode search for S05E01",
      std_search(episodes=(2,)), {"accepted": False}),
    T("should_return_true_if_full_season_result_for_full_season_search",
      "A S05E01 release is accepted in a single-episode search for S05E01",
      std_search(), {"accepted": True},
      note="The test only uses the SetUp facts (a S05E01 release, a S05E01 search) despite its name."),
]

FIXTURES = [
    ("PrioritizeDownloadDecisionFixture.cs", "DownloadDecisionPriorizationService.PrioritizeDecisions", PRIORITIZE, None),
    ("ProtocolSpecificationFixture.cs", "ProtocolSpecification", PROTOCOL, None),
    ("QualityAllowedByProfileSpecificationFixture.cs", "QualityAllowedByProfileSpecification", QUALITY_ALLOWED, None),
    ("QueueSpecificationFixture.cs", "QueueSpecification", QUEUE, None),
    ("RawDiskSpecificationFixture.cs", "RawDiskSpecification", RAW_DISK, None),
    ("ReleaseRestrictionsSpecificationFixture.cs", "ReleaseRestrictionsSpecification", RELEASE_RESTRICTIONS, None),
    ("RepackSpecificationFixture.cs", "RepackSpecification", REPACK, None),
    ("RetentionSpecificationFixture.cs", "RetentionSpecification", RETENTION, None),
    ("SameEpisodesSpecificationFixture.cs", "SameEpisodesSpecification.IsSatisfiedBy(episodes)", SAME_EPISODES,
     SAME_NOTE),
    ("SingleEpisodeAgeDownloadDecisionFixture.cs", "SeasonPackOnlySpecification", SINGLE_EPISODE_AGE, None),
    ("UpgradeAllowedSpecificationFixture .cs", "UpgradableSpecification.IsUpgradeAllowed", UPGRADE_ALLOWED, None),
    ("UpgradeDiskSpecificationFixture.cs", "UpgradeDiskSpecification", UPGRADE_DISK, None),
    ("UpgradeSpecificationFixture.cs", "UpgradableSpecification.IsUpgradable", UPGRADE, None),
    ("RssSync/DelaySpecificationFixture.cs", "RssSync.DelaySpecification", DELAY, None),
    ("RssSync/DeletedEpisodeFileSpecificationFixture.cs", "RssSync.DeletedEpisodeFileSpecification",
     DELETED_EPISODE_FILE, None),
    ("RssSync/HistorySpecificationFixture.cs", "RssSync.HistorySpecification", HISTORY, HIST_NOTE),
    ("RssSync/IndexerTagSpecificationFixture.cs", "RssSync.IndexerTagSpecification", INDEXER_TAG, None),
    ("RssSync/ProperSpecificationFixture.cs", "RssSync.ProperSpecification", PROPER, None),
    ("Search/SeriesSpecificationFixture.cs", "Search.SeriesSpecification", SEARCH_SERIES, None),
    ("Search/TorrentSeedingSpecificationFixture.cs", "TorrentSeedingSpecification", TORRENT_SEEDING, None),
    ("Search/SingleEpisodeSearchMatchSpecificationTests/AnimeSearchFixture.cs",
     "Search.SingleEpisodeSearchMatchSpecification", ANIME_SEARCH, None),
    ("Search/SingleEpisodeSearchMatchSpecificationTests/StandardEpisodeSearch.cs",
     "Search.SingleEpisodeSearchMatchSpecification", STANDARD_SEARCH, None),
]

# --- reference parsing -------------------------------------------------------

RE_METHOD = re.compile(r"^\s*public\s+void\s+(\w+)\s*\(")
RE_CASE = re.compile(r"^\s*\[TestCase\((.*)\)\]\s*$")
RE_TEST = re.compile(r"^\s*\[Test\]\s*$")
RE_SOURCE = re.compile(r"^\s*\[TestCaseSource\(nameof\((\w+)\)\)\]\s*$")
RE_ROW = re.compile(r"^\s*new object\[\]\s*\{\s*(.*?)\s*\},?\s*$")


def parse_fixture(lines):
    """[(line, method, row_index or None, args or None)] in file order (1-based lines)."""
    sources = {}
    for i, l in enumerate(lines):
        m = re.match(r"^\s*public static object\[\] (\w+) =", l)
        if m:
            rows, j = [], i + 1
            while not lines[j].strip().startswith("};"):
                r = RE_ROW.match(lines[j])
                if r:
                    rows.append((j + 1, r.group(1)))
                j += 1
            sources[m.group(1)] = rows
    out, cases, test_line, source = [], [], None, None
    for i, l in enumerate(lines):
        if m := RE_CASE.match(l):
            cases.append((i + 1, m.group(1)))
        elif RE_TEST.match(l):
            test_line = i + 1
        elif m := RE_SOURCE.match(l):
            source = m.group(1)
        elif m := RE_METHOD.match(l):
            name = m.group(1)
            if source:
                out += [(ln, name, k, a) for k, (ln, a) in enumerate(sources[source])]
            elif cases:
                out += [(ln, name, k, a) for k, (ln, a) in enumerate(cases)]
            elif test_line:
                out.append((test_line, name, None, None))
            cases, test_line, source = [], None, None
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


def main():
    parsed = {}
    for rel, _, _, _ in FIXTURES:
        path = TEST_DIR + rel
        parsed[path] = parse_fixture((REF_REPO / path).read_text(encoding="utf-8-sig").splitlines())

    # Existing IDs: ref line -> key at the pinned tag.
    old = read_catalogue()
    key_to_id = {}
    for eid, e in old.items():
        path, line = e["ref"][len(TAG) + 1:].rsplit(":", 1)
        hits = [(p, m, r, a) for (ln, m, r, a) in parsed.get(path, []) for p in [path] if ln == int(line)]
        if hits:
            key_to_id[hits[0]] = eid
    used = {int(i[len(ID_PREFIX):]) for i in old}
    next_n = max([n for n in used if ID_MIN <= n <= ID_MAX], default=ID_MIN - 1) + 1

    entries, problems = [], []
    for rel, spec, data, file_note in FIXTURES:
        path = TEST_DIR + rel
        by_method = {d["method"]: d for d in data}
        if len(by_method) != len(data):
            problems.append(f"{rel}: duplicate method in data")
        tests = parsed[path]
        for m in sorted(set(by_method) - {t[1] for t in tests}):
            problems.append(f"{rel}: data for {m} but no such test")
        cases = []
        for ln, method, row, args in tests:
            d = by_method.get(method)
            if d is None:
                problems.append(f"{rel}:{ln}: no data for {method}")
                continue
            if row is None:
                if d["rows"] is not None:
                    problems.append(f"{rel}:{ln}: {method} is a [Test] but data has rows")
                    continue
                behaviour, given, expect, note = d["behaviour"], d["given"], d["expect"], d["note"]
            else:
                if d["rows"] is None or row >= len(d["rows"]):
                    problems.append(f"{rel}:{ln}: {method} row {row} has no data")
                    continue
                want, behaviour, given, expect, note = d["rows"][row]
                if want != args:
                    problems.append(f"{rel}:{ln}: row args {args!r} != data {want!r}")
                    continue
                if row == 0 and len(d["rows"]) != sum(1 for t in tests if t[1] == method):
                    problems.append(f"{rel}: {method} row count differs")
            key = (path, method, row, args)
            eid = key_to_id.get(key)
            if eid is None:
                assert next_n <= ID_MAX, "ID range exhausted"
                eid = f"{ID_PREFIX}{next_n:04d}"
                next_n += 1
            ref = f"{TAG} {path}:{ln}"
            note = " ".join(n for n in (file_note, note) if n) or None
            case = {"id": eid, "ref": ref, "input": {"spec": spec, "given": given}, "expect": expect}
            if note:
                case["note"] = note
            cases.append(case)
            entries.append((eid, behaviour, ref))
        out = CORPUS / (rel.replace("/", ".").replace(" ", "")[:-3] + ".json")
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(cases, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")

    if problems:
        sys.exit("\n".join(problems))

    lines = ["# Sonarr grab-decision fixtures N..Z plus RssSync/ and Search/ (DecisionEngineTests), ported by",
             "# parity/extract/sonarr_decision_b.py. Corpus: corpus/reference/sonarr/decision/.", ""]
    seen = set()
    for eid, behaviour, ref in entries:
        seen.add(eid)
        raw = old.get(eid, {}).get("raw", {})
        lines += [f"- id: {eid}", f"  behaviour: {yaml_str(behaviour)}", "  source: reference",
                  f"  ref: {yaml_str(ref)}", raw.get("in_scope", "  in_scope: true"),
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
    print(f"{len(entries)} entries, {len(stale)} stale, ids {entries[0][0]}..{max(e[0] for e in entries)}")


if __name__ == "__main__":
    main()
