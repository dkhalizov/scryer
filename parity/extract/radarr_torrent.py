"""Port the Radarr torrent / download-handling cases that differ from Sonarr's
fixtures of the same name (notes/11-parity-net-design.md, phase 2) to

  parity/corpus/reference/radarr/torrent/<Fixture>.json
  parity/catalogue/torrent.radarr.yaml

Run from anywhere:  python3 -I parity/extract/radarr_torrent.py

Dedup policy (stream assignment): a Radarr test whose behaviour is the same as
Sonarr's test of the same fixture is covered by that SON-TORRENT entry and gets
no RAD entry. Every Radarr [Test]/[TestCase] row of the paired fixtures is
either ported here or mapped to its SON-TORRENT id; the script asserts the
partition is complete, that the Sonarr counterpart exists with identical
[TestCase] rows, and prints the mapping. Diff of each paired fixture
(Sonarr@v4.0.20.3014 vs Radarr@v6.4.4.10685), checked by hand:
- QBittorrentTests/QBittorrentFixture.cs: renames only (category name, movie
  priority fields, remoteMovie). Radarr's missingFiles_item_... has no [Test]
  attribute, so it does not run (guarded below). Deduped.
- FailedDownloadServiceTests/ProcessFixture.cs, ProcessFailedFixture.cs,
  CompletedDownloadServiceTests/ProcessFixture.cs, DownloadServiceFixture.cs:
  renames only (episode -> movie types and events). Deduped.
- TrackedDownloads/TrackedDownloadAlreadyImportedFixture.cs: Radarr keeps the
  three single-item tests, same outcomes, and drops the multi-episode ones;
  TrackedDownloadAlreadyImported.cs checks the one movie the way Sonarr checks
  each episode. Deduped.
- IndexerTests/SeedConfigProviderFixture.cs: the first two tests are the same;
  Sonarr's season-pack seed time test becomes Radarr's movie seed time test,
  which is ported.
- CompletedDownloadServiceTests/ImportFixture.cs: Radarr's own movie rule
  (CompletedDownloadService.VerifyImport), 5 tests, all ported.
- TrackedDownloads/TrackedDownloadServiceFixture.cs: movie mapping (Map by
  parsed movie title/year, movie ids, MoviesDeletedEvent) plus two tests Sonarr
  lacks (re-grab after import, movie id from download history); all 6 ported.

Conventions match parity/extract/sonarr_torrent.py: paths are the Unix form
AsOsAgnostic() gives (C:\\X -> /C/X); {"not": v} means the test asserts the
value is not v; null means the test asserts no value; "movie"/"tracked" name
the fixture's objects. IDs are keyed by ref (path:line of the [Test] or
[TestCase] attribute) and preserved on re-run with their status/decision/tests
lines.
"""
import json
import re
from pathlib import Path

TAG = "Radarr@v6.4.4.10685"
SON_TAG = "Sonarr@v4.0.20.3014"
REF = Path.home() / "PycharmProjects/arr-reference"
SRC, SON_SRC = REF / "radarr@v6.4.4.10685", REF / "sonarr@v4.0.20.3014"
OUT = Path(__file__).resolve().parents[1]
CATALOGUE = OUT / "catalogue/torrent.radarr.yaml"
SON_CATALOGUE = OUT / "catalogue/torrent.sonarr.yaml"
CORPUS = OUT / "corpus/reference/radarr/torrent"
PREFIX = "RAD-TORRENT-"
T = "src/NzbDrone.Core.Test/"
DECL = re.compile(r"\bpublic (?:async )?(?:void|Task) (\w+)\(")


def read(root, rel):
    return (root / rel).read_text(encoding="utf-8-sig").splitlines()


def tests(lines):
    """[(method, row index or None, 1-based line, attribute text)] for every [Test] / [TestCase] row."""
    out = []
    for i, l in enumerate(lines):
        m = DECL.search(l)
        if not m:
            continue
        j, attrs = i - 1, []
        while j >= 0 and lines[j].strip().startswith("["):
            attrs.append((j + 1, lines[j].strip()))
            j -= 1
        attrs.reverse()
        rows = [(n, t) for n, t in attrs if t.startswith("[TestCase(")]
        out += [(m[1], k, n, t) for k, (n, t) in enumerate(rows)]
        out += [(m[1], None, n, t) for n, t in attrs if t == "[Test]"]
    return out


cases = []  # (fixture stem, method, behaviour, op, given, expect, note)


def add(stem, method, behaviour, op, given, expect, note=None):
    cases.append((stem, method, behaviour, op, given, expect, note))


# --- QBittorrentFixture: the one structural difference, guarded so a re-tag notices ----------
QB = T + "Download/DownloadClientTests/QBittorrentTests/QBittorrentFixture.cs"
_qb = read(SRC, QB)
i = next(i for i, l in enumerate(_qb) if "void missingFiles_item_should_have_required_properties(" in l)
assert _qb[i - 1].strip() != "[Test]", "Radarr's missingFiles test runs again: port it"

# --- CompletedDownloadServiceTests/ImportFixture ---------------------------------------------
IM = "Download/CompletedDownloadServiceTests/ImportFixture"
ITEM = {"status": "Completed", "output_path": "/C/DropFolder/MyDownload", "title": "Drone.1998",
        "download_id": "DownloadId1", "category": "Category1"}
NOT_IMPORTED = {"state": "ImportBlocked", "download_completed_events": 0}
IMPORTED = {"state": "Imported", "download_completed_events": 1,
            "process_path": {"calls": 1, "path": "/C/DropFolder/MyDownload", "mode": "Auto",
                             "movie": "tracked", "download_item": "tracked"}}
RESULT_NOTE = ("Each import result's 'result' follows ImportResult.Result (src/NzbDrone.Core/MediaFiles/MovieImport/"
               "ImportResult.cs): errors + approved = Skipped, errors + rejected = Rejected, no errors = Imported. "
               "'movie': 'tracked' is the tracked download's movie; null means LocalMovie.Movie is unset. "
               "DownloadId/Category are NBuilder defaults.")


def res(name, movie=False, errors=(), rejected=False):
    errors = list(errors)
    result = "Imported" if not errors else ("Rejected" if rejected else "Skipped")
    return {"path": f"C:\\TestPath\\{name}", "movie": "tracked" if movie else None, "rejected": rejected,
            "errors": errors, "result": result}


def imp(results, item=None, movie_by_title=None, **extra):
    return {"tracked_state": "Downloading", "download_item": {**ITEM, **(item or {})},
            "remote_movie": {"movie": "movie"}, "client": {"id": 1, "name": "testClient"},
            "movie_by_title": {"Drone.1998": "movie"} if movie_by_title is None else movie_by_title,
            "history_by_download_id_count": 0, "import_results": results, **extra}


FAIL = ["Test Failure"]
add(IM, "should_not_mark_as_imported_if_all_files_were_rejected",
    "A completed download whose import rejects every file is not marked imported: state ImportBlocked, no completed event",
    "completed_download.import",
    imp([res("Droned.1998.mkv", errors=FAIL, rejected=True), res("Droned.1999.mkv", errors=FAIL, rejected=True)]),
    NOT_IMPORTED, RESULT_NOTE)
add(IM, "should_not_mark_as_imported_if_no_movies_were_parsed",
    "A completed download whose files are all rejected and none mapped to the movie is not marked imported: state ImportBlocked",
    "completed_download.import",
    imp([res("Droned.1998.mkv", errors=FAIL, rejected=True), res("Droned.1998.mkv", errors=FAIL, rejected=True)]),
    NOT_IMPORTED, f"The test replaces the tracked movie with a new Movie() before importing. {RESULT_NOTE}")
add(IM, "should_not_mark_as_imported_if_all_files_were_skipped",
    "A completed download whose import skips every file is not marked imported: state ImportBlocked",
    "completed_download.import",
    imp([res("Droned.1998.mkv", errors=FAIL), res("Droned.1998.mkv", errors=FAIL)]), NOT_IMPORTED, RESULT_NOTE)
add(IM, "should_mark_as_imported_if_all_movies_were_imported_but_extra_files_were_not",
    "A completed download is marked imported when the movie file imports even though another file was skipped",
    "completed_download.import",
    imp([res("Droned.S01E01.mkv", movie=True), res("Droned.S01E01.mkv", errors=FAIL)], movie_by_title={"*": "movie"}),
    IMPORTED,
    "Radarr's rule (CompletedDownloadService.VerifyImport): any imported file for the movie completes the download. "
    "The tracked movie is replaced by a new Movie() before import; every title lookup returns the original one. "
    + RESULT_NOTE)
add(IM, "should_mark_as_imported_if_the_download_can_be_tracked_using_the_source_movieid",
    "A badly named download (title 'Droned Pilot', grabbed as 'Droned 1998') is marked imported when its movie file imports",
    "completed_download.import",
    imp([res("Droned.S01E01.mkv", movie=True)], item={"download_id": "1234", "title": "Droned Pilot"},
        movie_by_title={"Droned 1998": "movie"}, most_recent_history={"source_title": "Droned 1998"},
        movie_by_id={"*": "movie"}),
    IMPORTED,
    "Titles other than 'Droned 1998' map to no movie; the title and id lookups return a new Movie(), not the "
    "tracked one. " + RESULT_NOTE)

# --- TrackedDownloads/TrackedDownloadServiceFixture -----------------------------------------
TD = "Download/TrackedDownloads/TrackedDownloadServiceFixture"
CLIENT = {"id": 1, "protocol": "Torrent"}
ITEM_35238 = {"title": "A Movie 1998", "download_id": "35238"}
HIST = {"download_id": "35238", "source_title": "TV Series S01", "movie_id": 3}
PARSED = {"movie_titles": ["A Movie"], "year": 1998}
REMOTE3 = {"movie_id": 3, "parsed": PARSED}
REMOTE5 = {"movie_id": 5, "parsed": PARSED}
BLACKHOLE_ITEM = {"title": "A Movie 1998", "download_id": "12345",
                  "client_info": {"id": 1, "type": "Blackhole", "name": "Blackhole Client", "protocol": "Torrent"}}
MOVIES_DELETED = {"type": "MoviesDeleted", "movie_ids": [3], "delete_files": False, "add_import_list_exclusion": False}
UNMAPPED = {"tracked_count_before": 1, "tracked_count": 1, "first": {"remote_movie": None}}
OVERRIDE_NOTE = ("GivenDownloadHistory's 35238 history is overridden by a later catch-all setup returning no history "
                 "(the item id is 12345 anyway).")
IMPORTED_HIST = {"35238": {"movie_id": 5, "event_type": "DownloadImported"}}
MAP5 = [{"when": {"year": 1998, "movie_title": "A Movie"}, "returns": REMOTE5}]

add(TD, "should_track_downloads_using_the_source_title_if_it_cannot_be_found_using_the_download_title",
    "A download 'A Movie 1998' with grab history for movie 3 is tracked and mapped to movie 3",
    "tracked_download.track",
    {"client": CLIENT, "item": ITEM_35238, "history_by_download_id": {"35238": [HIST]},
     "map": [{"when": {"primary_movie_title": "A Movie"}, "returns": REMOTE3}]},
    {"remote_movie": {"movie_id": 3}})
add(TD, "should_set_indexer",
    "A tracked download takes its release's indexer name from the grab history",
    "tracked_download.track",
    {"client": CLIENT, "item": ITEM_35238,
     "history_by_download_id": {"35238": [{**HIST, "event_type": "Grabbed", "data": {"indexer": "MyIndexer (Prowlarr)"}}]},
     "indexers": [{"id": 1, "name": "MyIndexer (Prowlarr)", "multi_languages": ["Original", "French"]}],
     "map": [{"when": "any", "returns": REMOTE3}]},
    {"remote_movie": {"release": {"indexer": "MyIndexer (Prowlarr)"}}})
add(TD, "should_unmap_tracked_download_if_movie_deleted",
    "When a tracked download's movie is deleted, it stays tracked but loses its mapping",
    "tracked_download.track_then_event",
    {"client": CLIENT, "item": BLACKHOLE_ITEM, "history_by_download_id": {"*": []},
     "map": [{"when": "any", "returns": REMOTE3}],
     "then": {"remap_returns": None, "event": MOVIES_DELETED}},
    UNMAPPED, OVERRIDE_NOTE)
add(TD, "should_not_throw_when_processing_deleted_movie",
    "An unmapped tracked download survives a movie-deleted event without error",
    "tracked_download.track_then_event",
    {"client": CLIENT, "item": BLACKHOLE_ITEM, "history_by_download_id": {"*": []},
     "map": [{"when": "any", "returns": None}],
     "then": {"remap_returns": None, "event": MOVIES_DELETED}},
    UNMAPPED, OVERRIDE_NOTE)
add(TD, "should_retrack_as_downloading_when_grabbed_again_after_being_imported",
    "A download whose latest download history is DownloadImported tracks as Imported, and after it is grabbed again "
    "(latest history DownloadGrabbed) it tracks as Downloading",
    "tracked_download.track_then_event",
    {"client": CLIENT, "item": ITEM_35238, "history_by_download_id": {"35238": []},
     "latest_download_history": IMPORTED_HIST, "map": MAP5,
     "then": {"event": {"type": "MovieGrabbed", "download_id": "35238", "remote_movie": "mapped"},
              "latest_download_history": {"35238": {"movie_id": 5, "event_type": "DownloadGrabbed"}},
              "track_again": True}},
    {"states": ["Imported", "Downloading"]})
add(TD, "should_track_downloads_using_the_movie_id_for_already_imported_downloads",
    "An already-imported download with no grab history is mapped via its download history's movie id (movie 5, "
    "parsed year 1998)",
    "tracked_download.track",
    {"client": CLIENT, "item": ITEM_35238, "history_by_download_id": {"35238": []},
     "latest_download_history": IMPORTED_HIST, "map": MAP5},
    {"remote_movie": {"movie_id": 5, "parsed_year": 1998}})

# --- IndexerTests/SeedConfigProviderFixture --------------------------------------------------
SC = "IndexerTests/SeedConfigProviderFixture"
add(SC, "should_return_seed_time_for_movies",
    "A torrent movie release from an indexer with seed time 10 gets a seed configuration with seed time 10 minutes",
    "seed_config.get",
    {"release": {"protocol": "Torrent", "indexer_id": 1}, "parsed": {},
     "indexer_settings": {"fail_downloads": ["Executables"], "seed_criteria": {"seed_time": 10}}},
    {"seed_config": {"not": None}, "seed_time_min": 10},
    "Other seed criteria are TorznabSettings defaults (unset). Radarr has one seed time for movies; Sonarr's "
    "counterpart tests the season-pack seed time.")

# Every paired fixture: (stem, corpus file name or None when fully deduped).
FIXTURES = [
    ("Download/DownloadClientTests/QBittorrentTests/QBittorrentFixture", None),
    ("Download/CompletedDownloadServiceTests/ProcessFixture", None),
    (IM, "CompletedDownloadService.ImportFixture"),
    ("Download/FailedDownloadServiceTests/ProcessFailedFixture", None),
    ("Download/FailedDownloadServiceTests/ProcessFixture", None),
    ("Download/TrackedDownloads/TrackedDownloadAlreadyImportedFixture", None),
    (TD, "TrackedDownloadServiceFixture"),
    ("Download/DownloadServiceFixture", None),
    (SC, "SeedConfigProviderFixture"),
]


def son_ids():
    text = SON_CATALOGUE.read_text()
    return {r: i for i, r in re.findall(r'- id: (\S+)\n(?:  .*\n)*?  ref: "(.*)"', text)}


def main():
    old = {}  # ref -> (id, carried lines)
    if CATALOGUE.exists():
        for block in re.split(r"(?m)^(?=- id: )", CATALOGUE.read_text()):
            if not block.startswith("- id: "):
                continue
            bid = re.match(r"- id: (\S+)", block)[1]
            bref = json.loads(re.search(r"(?m)^  ref: (.*)$", block)[1])
            keep = re.findall(r"(?m)^  ((?:status|decision|tests):.*(?:\n    .*)*)", block)
            old[bref] = (bid, keep)
    next_n = max([int(i.rsplit("-", 1)[1]) for i, _ in old.values()] + [0]) + 1
    son = son_ids()
    entries, by_file, dedup = [], {}, []
    for stem, out_name in FIXTURES:
        rel = f"{T}{stem}.cs"
        rad = tests(read(SRC, rel))
        son_by = {(m, k): (n, t) for m, k, n, t in tests(read(SON_SRC, rel))}
        mine = {c[1]: c for c in cases if c[0] == stem}
        declared = {m for m, *_ in rad}
        assert set(mine) <= declared, f"{stem}: unknown {sorted(set(mine) - declared)}"
        for m, k, n, t in sorted(rad, key=lambda x: x[2]):
            ref = f"{TAG} {rel}:{n}"
            if m not in mine:  # deduped: must have a Sonarr twin with the same row
                sm = m if any(x == m for x, _ in son_by) else m.replace("movie", "episode")
                assert (sm, k) in son_by, f"{stem}.{m}[{k}]: no Sonarr counterpart, port it"
                sn, st = son_by[(sm, k)]
                assert st == t, f"{stem}.{m}[{k}]: row differs from Sonarr ({st} vs {t}), port it"
                sref = f"{SON_TAG} {rel}:{sn}"
                assert sref in son, f"{sref} has no SON-TORRENT entry"
                dedup.append((stem, f"{m}" + ("" if k is None else f"[{k}]"), n, son[sref]))
                continue
            assert k is None, f"{stem}.{m}: [TestCase] rows need per-row cases"
            _, _, behaviour, op, given, expect, note = mine[m]
            if ref in old:
                cid, keep = old[ref]
            else:
                cid, keep = f"{PREFIX}{next_n:04d}", ["status: unmapped", "tests: []"]
                next_n += 1
            entries.append(f"- id: {cid}\n  behaviour: {json.dumps(behaviour, ensure_ascii=False)}\n"
                           f"  source: reference\n  ref: {json.dumps(ref, ensure_ascii=False)}\n  in_scope: true\n"
                           + "".join(f"  {x}\n" for x in keep))
            case = {"id": cid, "ref": ref, "input": {"op": op, "given": given}, "expect": expect}
            if note:
                case["note"] = note
            by_file.setdefault(out_name, []).append(case)
        missing = set(mine) - {m for m, *_ in rad}
        assert not missing, f"{stem}: ported methods without [Test]: {missing}"

    per_fixture = {}
    for stem, _, _, sid in dedup:
        per_fixture.setdefault("/".join(stem.split("/")[-2:]), []).append(int(sid.rsplit("-", 1)[1]))
    header = ["# Generated by parity/extract/radarr_torrent.py; IDs are stable, edit status/decision/tests only.",
              "# Only Radarr tests whose behaviour differs from Sonarr's same-named fixture are listed here.",
              "# The other Radarr tests of the paired fixtures are covered by these SON-TORRENT entries",
              "# (torrent.sonarr.yaml; the extractor asserts the twin exists and prints the full mapping):"]
    for f, ids in per_fixture.items():
        runs = []
        for n in sorted(ids):
            if runs and n == runs[-1][1] + 1:
                runs[-1][1] = n
            else:
                runs.append([n, n])
        spans = ", ".join(f"{a:04d}" if a == b else f"{a:04d}..{b:04d}" for a, b in runs)
        header.append(f"#   {f}: {len(ids)} -> SON-TORRENT {spans}")
    CATALOGUE.write_text("\n".join(header) + "\n" + "".join(entries))
    CORPUS.mkdir(parents=True, exist_ok=True)
    for f in CORPUS.glob("*.json"):
        if f.stem not in by_file:
            f.unlink()
    for f, cs in sorted(by_file.items()):
        (CORPUS / f"{f}.json").write_text(json.dumps(cs, indent=1, ensure_ascii=False) + "\n")
    print(f"{len(entries)} entries, {len(by_file)} corpus files, {len(dedup)} Radarr tests/rows deduped to SON-TORRENT")
    for stem, what, n, sid in dedup:
        print(f"dedup: {TAG} {T}{stem}.cs:{n} {what} -> {sid}")


if __name__ == "__main__":
    main()
