"""Port Prowlarr indexer category-mapping fixtures (area: indexers).

Scope: the category rule that decides which of our indexers Prowlarr offers to
Sonarr and Radarr (IndexerCapabilitiesCategories.SupportedCategories, called
from Applications/Sonarr/Sonarr.cs:135 and Radarr/Radarr.cs:131) and the
tracker <-> Newznab category mapping behind it. Prowlarr has no reference tests
for app sync itself; those behaviours are config entries
(indexers.prowlarr-config.yaml).

Hand-translated: each test's arrange/assert is written below as data. The
script finds each test's [Test] line in the reference repo (so refs are exact),
checks every [Test]/[TestCase] method of the fixture is ported, keeps the IDs
already in the catalogue (keyed by ref = path:line) and writes:

  parity/corpus/reference/prowlarr/indexers/<Fixture>.json
  parity/catalogue/indexers.prowlarr.yaml

Category names are NewznabStandardCategory members; their ids are copied from
src/NzbDrone.Core/Indexers/NewznabStandardCategory.cs:28-32 (Movies 2000,
MoviesSD 2030, MoviesHD 2040). A mapping is AddCategoryMapping(tracker
category, newznab category, tracker description).

Run: python3 -I parity/extract/prowlarr_indexers.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REF_REPO = Path.home() / "PycharmProjects/arr-reference/prowlarr@v2.7.0.5723"
TAG = "Prowlarr@v2.7.0.5723"
TEST_DIR = "src/NzbDrone.Core.Test/"
CATALOGUE = ROOT / "parity/catalogue/indexers.prowlarr.yaml"
CORPUS = ROOT / "parity/corpus/reference/prowlarr/indexers"
ID_PREFIX, ID_MAX = "PRW-INDEXERS-", 4999  # 5001+ belong to indexers.prowlarr-config.yaml

SD = {"tracker_category": 1, "newznab_category": "MoviesSD", "newznab_id": 2030, "description": "Filme SD"}
HD = {"tracker_category": 2, "newznab_category": "MoviesHD", "newznab_id": 2040, "description": "Filme HD"}


def T(method, behaviour, op, given, expect):
    return dict(method=method, behaviour=behaviour, op=op, given=given, expect=expect)


CATEGORIES = [
    T("should_support_parent_if_child_mapping",
      "An indexer that maps only the child category Movies/SD (2030) supports a request for its parent Movies (2000)",
      "capabilities.supported_categories", {"mappings": [SD], "categories": [2000]}, {"count": 1}),
    T("should_support_category_if_mapped",
      "An indexer that maps Movies/SD (2030) supports a request for 2030",
      "capabilities.supported_categories", {"mappings": [SD], "categories": [2030]}, {"count": 1}),
    T("should_not_support_category_if_not_mapped",
      "An indexer that maps only Movies/SD (2030) does not support a request for the sibling Movies/HD (2040)",
      "capabilities.supported_categories", {"mappings": [SD], "categories": [2040]}, {"count": 0}),
    T("should_get_tracker_category_list",
      "With tracker categories 1 and 2 mapped (with descriptions), the tracker category list has 2 entries, '1' first",
      "capabilities.tracker_categories", {"mappings": [SD, HD]}, {"count": 2, "first": "1"}),
    T("should_get_category_by_tracker_id",
      "Tracker category '2' (mapped to Movies/HD with a description) maps to 2 Newznab categories, Movies/HD (2040) first",
      "capabilities.map_tracker_cat_to_newznab", {"mappings": [SD, HD], "tracker_category": "2"},
      {"count": 2, "first_id": 2040}),
    T("should_get_category_by_tracker_desc",
      "Tracker description 'Filme HD' (mapped to Movies/HD) maps to 2 Newznab categories, Movies/HD (2040) first",
      "capabilities.map_tracker_cat_desc_to_newznab", {"mappings": [SD, HD], "description": "Filme HD"},
      {"count": 2, "first_id": 2040}),
]

FIXTURES = [("IndexerTests/IndexerCapabilitiesCategoriesFixture", CATEGORIES)]

# Fixtures ruled out entirely: config fact (captured prowlarr/config/*.json) or assignment boundary.
RULED_OUT = {
    "IndexerTests/NewznabTests/*, IndexerTests/TorznabTests/TorznabFixture.cs":
        "Prowlarr's own Newznab/Torznab indexer types (incl. NewznabCapabilitiesProviderFixture, caps XML parsing); "
        "our indexers are Cardigann definitions (indexer.json), none of Newznab/Torznab type.",
    "IndexerTests/{AnimeBytes,Avistaz,BroadcastheNet,FileList,GazelleGames,HDBits,IPTorrents,NzbIndex,Orpheus,PTP,Rarbg,Redacted,SecretCinema}Tests/*":
        "Native indexer types we do not configure (indexer.json).",
    "IndexerTests/IntegrationTests/IndexerIntegrationTests.cs": "Class-level [Ignore(\"Nyaa is down!\")] with only a [SetUp] and no [Test] methods: nothing to port.",
    "IndexerTests/IndexerServiceFixture.cs": "Deletes definitions missing on startup; Prowlarr storage internals.",
    "IndexerTests/IndexerStatusServiceFixture.cs, IndexerStatsTests/*":
        "Failure backoff and statistics bookkeeping; not category mapping or sync.",
    "IndexerTests/BasicRssParserFixture.cs, IndexerTests/XElementExtensionsFixture.cs":
        "Generic RSS/Torznab XML parsing used by Newznab/Torznab-type indexers, none configured.",
    "IndexerTests/CardigannTests/ApplyGoTemplateTextFixture.cs, ParserTests/{ParseUtil,DateTimeUtil,DateTimeRoutines}Fixture.cs, IndexerSearchTests/*":
        "Cardigann engine and search-query building used by our indexers, but outside this "
        "stream's assignment (category mapping, capabilities, app sync): open decision.",
    "Datastore/Migration/040_newznab_category_to_capabilities_settingsFixture.cs": "Schema migration; no migrations in the fork.",
    "HealthCheck/Checks/ApplicationStatusCheckFixture.cs, Housekeeping/Housekeepers/*ApplicationStatus*":
        "Application health/status bookkeeping, not what is synced.",
}


# --- reference anchoring ----------------------------------------------------

DECL = re.compile(r"\bpublic (?:async )?(?:void|Task) (\w+)\(")


def attributes(lines, method):
    """[(line_no, text)] of the attribute lines directly above the method declaration."""
    hits = [i for i, l in enumerate(lines) if (m := DECL.search(l)) and m.group(1) == method]
    assert len(hits) == 1, f"{method}: {len(hits)} declarations"
    i, attrs = hits[0] - 1, []
    while i >= 0 and lines[i].strip().startswith("["):
        attrs.append((i + 1, lines[i].strip()))
        i -= 1
    return attrs[::-1]


def test_methods(lines):
    return {m.group(1) for l in lines if (m := DECL.search(l))
            and any(t == "[Test]" or t.startswith("[TestCase(") for _, t in attributes(lines, m.group(1)))}


def anchor(path, lines, case):
    attrs = attributes(lines, case["method"])
    assert not any(t.startswith("[TestCase(") for _, t in attrs), f"{path}:{case['method']}: has rows, port them"
    tests = [n for n, t in attrs if t == "[Test]"]
    assert len(tests) == 1, f"{path}:{case['method']}: no [Test]"
    return tests[0]


def existing_ids():
    if not CATALOGUE.exists():
        return {}
    return dict((r, i) for i, r in re.findall(r'- id: (\S+)\n(?:  .*\n)*?  ref: "(.*)"', CATALOGUE.read_text()))


def kept_lines():
    """{id: [status/decision/tests lines as written]} from the current catalogue (ratchet-owned)."""
    if not CATALOGUE.exists():
        return {}
    return {m[0]: re.findall(r"(?m)^  ((?:status|decision|tests):.*(?:\n    .*)*)", m[1])
            for m in re.findall(r"(?m)^- id: (\S+)\n((?:  .*\n?|    .*\n?)*)", CATALOGUE.read_text())}


def main():
    ids = existing_ids()
    kept = kept_lines()
    next_n = max((int(i.rsplit("-", 1)[1]) for i in ids.values()), default=0) + 1
    entries = []
    for stem, cases in FIXTURES:
        rel = f"{TEST_DIR}{stem}.cs"
        lines = (REF_REPO / rel).read_text(encoding="utf-8-sig").splitlines()
        declared, covered = test_methods(lines), {c["method"] for c in cases}
        assert declared == covered, f"{stem}: uncovered {sorted(declared - covered)}, unknown {sorted(covered - declared)}"
        corpus = []
        for line, case in sorted(((anchor(rel, lines, c), c) for c in cases), key=lambda x: x[0]):
            ref = f"{TAG} {rel}:{line}"
            if ref not in ids:
                assert next_n <= ID_MAX, "ID range exhausted"
                ids[ref] = f"{ID_PREFIX}{next_n:04d}"
                next_n += 1
            entries.append((ids[ref], case["behaviour"], ref))
            corpus.append({"id": ids[ref], "ref": ref, "input": {"op": case["op"], "given": case["given"]},
                           "expect": case["expect"]})
        CORPUS.mkdir(parents=True, exist_ok=True)
        (CORPUS / f"{stem.rsplit('/', 1)[1]}.json").write_text(json.dumps(corpus, indent=1, ensure_ascii=False) + "\n")

    out = ["# Prowlarr indexer category-mapping fixtures, ported by parity/extract/prowlarr_indexers.py.",
           "# Corpus: corpus/reference/prowlarr/indexers/.", ""]
    for cid, behaviour, ref in entries:
        out += [f"- id: {cid}", f"  behaviour: {json.dumps(behaviour, ensure_ascii=False)}", "  source: reference",
                f"  ref: {json.dumps(ref)}", "  in_scope: true"]
        out += [f"  {k}" for k in kept.get(cid, ["status: unmapped", "tests: []"])]
    CATALOGUE.write_text("\n".join(out) + "\n")
    print(f"entries {len(entries)}, corpus cases {len(entries)}, id range {min(ids.values())}..{max(ids.values())}")
    for what, why in RULED_OUT.items():
        print(f"ruled out: {what}: {why}")


if __name__ == "__main__":
    sys.exit(main())
