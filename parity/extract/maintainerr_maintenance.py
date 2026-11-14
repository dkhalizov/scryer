"""Port the Maintainerr tests our cleanup rules rely on to parity/corpus/reference/maintainerr/maintenance
and parity/catalogue/maintenance.maintainerr.yaml (notes/11-parity-net-design.md, phase 2).

Run from anywhere:  python3 -I parity/extract/maintainerr_maintenance.py

Our config (parity/corpus/captured/maintainerr/config/*.json): media server Plex; rule group 1 (movies,
Radarr 1) = [Plex seenBy CONTAINS Seerr addUser AND Seerr isRequested EQUALS 1 AND Seerr mediaAddedAt
BEFORE now-30d] OR [mediaAddedAt BEFORE now-60d AND isRequested EQUALS 1]; rule group 3 (episodes,
Sonarr 1) = Plex isWatched EQUALS 1 AND Plex lastViewedAt BEFORE now-7d. Both collections: arrAction 0
(DELETE), deleteAfterDays 0, listExclusions true, forceSeerr false, cleanupLeftoverFolders false,
manualCollection false. No download client, tag exclusions off, no exclusions, no notification agents,
both groups on the global rule cron.

Every it() and every it.each row of a ported spec file gets an entry (the enumerator below finds them;
the run fails if a ported test is not found or a test has no port and no out-of-scope reason). Tests
that do not touch our operators, properties, actions, types or settings, or that test internals, are
in_scope false with no corpus case.

Every other spec file in the tag (apps/**/*.spec.ts) is listed in RULED_OUT below with the config fact
or reason that keeps it out (no entries); the run fails if a spec file is neither ported nor listed.
Entries marked DEFERRED are on our cleanup path but not ported in this pass (open decision, ledger).

JS values the JSON cannot hold are tagged: {"$date": "<ISO>"} for new Date(...), {"$undefined": true}
for undefined, {"$now_offset_seconds": n} for a Date the code computes from the clock.

IDs: the key (YAML comment above each entry) is path | describe > title, plus | row index | row text
(whitespace collapsed) for it.each/forEach rows; an entry keeps its ID and
its status/decision/tests lines on re-run; new keys take max + 1 in file/line order.
"""
import json
import re
from datetime import datetime, timezone
from pathlib import Path

TAG = "Maintainerr@v3.32.0"
SRC = Path.home() / "PycharmProjects/arr-reference/maintainerr@v3.32.0"
OUT = Path(__file__).resolve().parents[1]
CATALOGUE = OUT / "catalogue/maintenance.maintainerr.yaml"
CORPUS = OUT / "corpus/reference/maintainerr/maintenance"
PREFIX = "MNT-MAINTENANCE-"
M = "apps/server/src/modules/"


# --- TypeScript test enumerator ----------------------------------------------------------------
def skip_string(t, i):
    """t[i] is a quote; return the index after the closing quote."""
    q, i = t[i], i + 1
    while t[i] != q:
        i += 2 if t[i] == "\\" else 1
    return i + 1


def read_string(t, i):
    """First string literal at or after i: (value, end)."""
    while t[i] not in "'\"`":
        i += 1
    end = skip_string(t, i)
    return t[i + 1:end - 1].replace("\\'", "'").replace('\\"', '"'), end


def split_array(t, i):
    """t[i] == '['; returns ([(start, text) per top-level element], index after ']')."""
    assert t[i] == "[", t[i:i + 40]
    depth, elems, start, i = 1, [], None, i + 1
    while True:
        c = t[i]
        if t.startswith("//", i):
            i = t.index("\n", i)
            continue
        if t.startswith("/*", i):
            i = t.index("*/", i) + 2
            continue
        if c in "'\"`":
            if start is None:
                start = i
            i = skip_string(t, i)
            continue
        if depth == 1 and c in ",]":
            if start is not None:
                elems.append((start, t[start:i].strip()))
            start = None
            if c == "]":
                return elems, i + 1
        elif c in "[({":
            depth += 1
        elif c in "])}":
            depth -= 1
        if start is None and not c.isspace() and c != ",":
            start = i
        i += 1


def line_of(t, i):
    return t.count("\n", 0, i) + 1


def tests_of(rel):
    """[(path, title, line, rows)]; rows None for a plain it(), else [(line, text)]."""
    t = (SRC / rel).read_text(encoding="utf-8")
    lines = t.splitlines(keepends=True)
    offs = [0]
    for l in lines:
        offs.append(offs[-1] + len(l))
    describes = []  # (indent, start line, end line, title)
    for n, l in enumerate(lines):
        if m := re.match(r"(\s*)describe\(", l):
            ind = m[1]
            end = next(k for k in range(n + 1, len(lines)) if re.match(re.escape(ind) + r"\}\);?\s*$", lines[k]))
            describes.append((len(ind), n + 1, end + 1, read_string(t, offs[n] + m.end())[0]))
    out = []
    for n, l in enumerate(lines):
        m = re.match(r"(\s*)(?:it|test)(\.each)?[(<]", l)
        if not m:
            continue
        pos = offs[n] + len(m[1])
        path = " > ".join(d[3] for d in describes if d[1] < n + 1 < d[2] and d[0] > 0)
        rows = None
        if m[2]:
            i = pos + t[pos:].index("each") + 4
            if t[i] == "<":  # explicit type arguments
                depth = 0
                while True:
                    depth += {"<": 1, ">": -1}.get(t[i], 0)
                    i += 1
                    if depth == 0:
                        break
            assert t[i] == "(", t[i:i + 40]
            i += 1
            while t[i].isspace():
                i += 1
            if t[i] == "[":
                elems, i = split_array(t, i)
            else:
                ident = re.match(r"\w+", t[i:])[0]
                i += len(ident)
                d = re.search(rf"const {ident}\b[^=]*=\s*\[", t)
                elems, _ = split_array(t, d.end() - 1)
            rows = [(line_of(t, s), txt) for s, txt in elems]
            title, _ = read_string(t, i)
        else:
            title, _ = read_string(t, pos)
            if f := re.search(r"(\w+)\.forEach\(", lines[n - 1]):  # it() generated per row
                d = re.search(rf"const {f[1]}\b[^=]*=\s*\[", t)
                elems, _ = split_array(t, d.end() - 1)
                rows = [(line_of(t, s), txt) for s, txt in elems]
        out.append((path, title, n + 1, rows))
    return out


def js(text):
    """A JS literal row (strings, numbers, booleans, null, undefined, arrays, new Date('...')) as JSON."""
    s = re.sub(r"//[^\n]*", "", text)
    s = re.sub(r"new Date\('([^']*)'\)", r'{"$date": "\1"}', s)
    s = re.sub(r"\bundefined\b", '{"$undefined": true}', s)
    s = re.sub(r"'((?:[^'\\]|\\.)*)'", lambda m: json.dumps(m[1]), s)
    s = re.sub(r"(\d)_(?=\d)", r"\1", s)
    s = re.sub(r",\s*([\]}])", r"\1", s)
    return json.loads(s)


def date(epoch_s):
    return {"$date": datetime.fromtimestamp(epoch_s, timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z")}


U = {"$undefined": True}

# --- Ports -------------------------------------------------------------------------------------
FILES = {}   # rel -> corpus file name
PORTS = {}   # (rel, title, row) -> (behaviour, input, expect, note)
OUTS = {}    # rel -> [(regex over "path > title | row", reason)], last one usually the default


def port(rel, title, behaviour, op, given, expect, note=None, row=None):
    key = (rel, title, row)
    assert key not in PORTS, key
    PORTS[key] = (behaviour, {"op": op, "given": given}, expect, note)


# --- rules/tests/rule.comparator.service.doRuleAction.spec.ts ----------------------------------
DR = M + "rules/tests/rule.comparator.service.doRuleAction.spec.ts"
FILES[DR] = "comparator.json"
_dr = {title: rows for _, title, _, rows in tests_of(DR)}
for title, action, what in [
        ("should return %s when val1 is %o and val2 is %o with action EQUALS", "EQUALS",
         "spec table :50: strings compare case-insensitively, lists with the same entries in the same order are "
         "equal and lists of a different length or entries are not, a one-entry list equals its lone value, equal "
         "dates are equal, an undefined value never matches, and true equals the number 1 while false does not"),
        ("should return %s when val1 is %o and val2 is %o with action CONTAINS", "CONTAINS",
         "spec table :79: a string contains a substring of it; a list contains a value when one of the given "
         "values is an entry of it; empty-string and undefined given values match nothing; 'imdb' is not contained "
         "in a list holding 'ImDb top 250'")]:
    for r, (_, text) in enumerate(_dr[title]):
        expected, v1, v2 = js(text)
        port(DR, title, f"{action} of {json.dumps(v1)} and {json.dumps(v2)} is {str(expected).lower()} ({what})",
             "doRuleAction", {"action": action, "val1": v1, "val2": v2}, {"result": expected}, row=r)
for title, v1, v2, expected in [
        ("should return true when comparing two dates with action BEFORE", "2022-01-01", "2022-01-02", True),
        ("should return false when comparing two dates with action BEFORE and value is undefined", "2022-01-01",
         None, False),
        ("should return false when comparing two dates with action BEFORE", "2022-01-03", "2022-01-02", False)]:
    port(DR, title, f"BEFORE of {v1} and {v2 or 'undefined'} is {str(expected).lower()} (an earlier date is "
         "before a later one, a later date is not, an undefined second date never matches)", "doRuleAction",
         {"action": "BEFORE", "val1": {"$date": v1}, "val2": {"$date": v2} if v2 else U}, {"result": expected})
OUTS[DR] = [(r"NOT_|EXISTS|CONTAINS_PARTIAL|CONTAINS_ALL|SMALLER|AFTER|IN_LAST|IN_NEXT|COUNT_",
             "operator not used by our rules (we use CONTAINS, EQUALS, BEFORE: rules.json#id=1, #id=3)")]

# --- rules/tests/rule.comparator.service.executeRulesWithData.spec.ts --------------------------
ER = M + "rules/tests/rule.comparator.service.executeRulesWithData.spec.ts"
FILES[ER] = "rule_group.json"
PROPS = {("PLEX", 31): ("imdb", "NUMBER"), ("PLEX", 5): ("viewCount", "NUMBER"),
         ("PLEX", 6): ("lastViewedAt", "DATE"), ("JELLYFIN", 0): ("lastViewedAt", "DATE")}
ER_NOTE = ("Property ids and types are the test's mocked rule constants (spec :82), not the real table. "
           "getterValues are the mocked getter results in call order.")


def rule(i, action, first, section=0, operator=None, last=None, custom=None):
    r = {"id": i, "section": section, "operator": operator, "action": action,
         "firstVal": {"app": first[0], "prop": first[1], "name": PROPS[first][0], "type": PROPS[first][1]}}
    if last:
        r["lastVal"] = {"app": last[0], "prop": last[1], "name": PROPS[last][0], "type": PROPS[last][1]}
    if custom:
        r["customVal"] = {"ruleTypeId": custom[0], "value": custom[1]}
    return r


def er(title, behaviour, given, expect, row=None):
    port(ER, title, behaviour, "executeRulesWithData", {"dataType": "movie", **given}, expect, ER_NOTE, row)


IMDB_SMALLER_6 = [rule(1, "SMALLER", ("PLEX", 31), custom=("NUMBER", "6"))]
er("rethrows evaluation failures instead of returning undefined for the chunk (#3307)",
   "A getter that throws fails the whole evaluation instead of leaving the items unmatched",
   {"rules": IMDB_SMALLER_6, "media": ["media-1"], "getterValues": [{"$throws": "boom"}]}, {"throws": "boom"})
er("fails closed when the first value is missing for a custom comparison",
   "A rule whose first value is null does not match and records result false",
   {"rules": IMDB_SMALLER_6, "media": ["media-1"], "getterValues": [None]},
   {"matched": [], "result": False,
    "ruleResult": {"action": "smaller", "firstValue": None, "secondValue": 6, "result": False}})
er("preserves 3.1.0 numeric lastVal behavior when the second value is missing",
   "A rule comparing two properties still compares when the second value is null (10 > null matches)",
   {"rules": [rule(1, "BIGGER", ("PLEX", 5), last=("PLEX", 6))], "media": ["media-1"], "getterValues": [10, None]},
   {"matched": ["media-1"], "result": True, "sectionResult": True,
    "ruleResult": {"action": "bigger", "firstValue": 10, "secondValue": None, "result": True}})
er("fails closed when a second value could not be read (#3886)",
   "A rule comparing two properties does not match when the second value could not be read (undefined)",
   {"rules": [rule(1, "NOT_EQUALS", ("PLEX", 5), last=("PLEX", 6))], "media": ["media-1"],
    "getterValues": [[], U]}, {"matched": []})
er("keeps BEFORE date rules fail-closed when lastViewedAt is null",
   "BEFORE a fixed date does not match an item whose date is null",
   {"rules": [rule(1, "BEFORE", ("PLEX", 6), custom=("DATE", "2024-01-01T00:00:00.000Z"))], "media": ["media-1"],
    "getterValues": [None]},
   {"matched": [], "result": False,
    "ruleResult": {"action": "before", "firstValue": None, "result": False,
                   "secondValue": {"$date": "2024-01-01T00:00:00.000Z"}}})
er("formats custom_days second value as a past Date when first value is null (issue #2582)",
   "On a date property, a NUMBER custom value with BEFORE means now minus that many seconds",
   {"rules": [rule(1, "BEFORE", ("JELLYFIN", 0), custom=("NUMBER", str(330 * 86400)))], "media": ["media-1"],
    "getterValues": [None]},
   {"ruleResult": {"result": False, "secondValue": {"$now_offset_seconds": -330 * 86400}}})
TWO_SECTIONS = [rule(1, "EQUALS", ("PLEX", 5), custom=("NUMBER", "0")),
                rule(2, "BIGGER", ("PLEX", 5), section=1, custom=("NUMBER", "0"))]
for r, (_, text) in enumerate(next(rows for _, t, _, rows in tests_of(ER) if t.startswith("with viewCount"))):
    v, included = js(text)
    er("with viewCount %p includes the item: %p",
       f"Sections are OR'ed: viewCount {json.dumps(v)} against [=0] OR [>0] is {'' if included else 'not '}matched",
       {"rules": TWO_SECTIONS, "media": ["media-1"], "getterValues": [v, v]},
       {"matchedCount": 1 if included else 0, "result": included}, row=r)
er("includes an item matching both overlapping sections exactly once (no duplicate)",
   "An item matching two OR'ed sections is in the result once",
   {"rules": [rule(1, "EQUALS", ("PLEX", 5), custom=("NUMBER", "0")),
              rule(2, "SMALLER", ("PLEX", 5), section=1, custom=("NUMBER", "1"))],
    "media": ["media-1"], "getterValues": [0, 0]}, {"matched": ["media-1"], "result": True})
er("narrows a section with a same-section AND rule and drops a missing operand",
   "A rule joined by AND (operator stored as the string \"0\") in the same section keeps only items matching both; "
   "an item whose operand is null drops out",
   {"rules": [rule(1, "BIGGER", ("PLEX", 5), custom=("NUMBER", "0")),
              rule(2, "SMALLER", ("PLEX", 5), operator="0", custom=("NUMBER", "10"))],
    "media": ["in-range", "too-high", "missing"],
    "getterValuesByMedia": {"in-range": [5, 5], "too-high": [15, 15], "missing": [5, None]}},
   {"matched": ["in-range"]})
for title, behaviour, rules, values, failed in [
        ("tracks firstVal undefined as a transient failure",
         "An item whose first value could not be read (undefined) is reported as a transient failure",
         IMDB_SMALLER_6, [U], True),
        ("does not track firstVal null as a transient failure",
         "An item whose first value is null (confirmed absent) is not a transient failure", IMDB_SMALLER_6, [None],
         False),
        ("tracks secondVal undefined when rule.lastVal is used",
         "An item whose second property value could not be read (undefined) is a transient failure",
         [rule(1, "BIGGER", ("PLEX", 5), last=("PLEX", 6))], [10, U], True)]:
    er(title, behaviour, {"rules": rules, "media": ["media-1"], "getterValues": values},
       {"transientFailure": {"media-1": failed}})
OUTS[ER] = [
    (r"not_exists|NOT_EXISTS|unary rule value", "operator NOT_EXISTS is not used by our rules"),
    (r"future Date for equality", "EQUALS on a date with a seconds offset is not used (our date rules use BEFORE)"),
    (r"reports %p as a failed lookup", "wording of the value-unavailable reason shown in the rule test UI"),
    (r"short-circuits later OR rules", "OR between rules inside one section is not used (our sections AND their rules)"),
    (r"explicit AND section operator", "AND between sections is not used (rules.json#id=1 joins its sections by OR)"),
    (r"batches capped", "evaluation concurrency, an internal")]

# --- rules/getter/seerr-getter.service.spec.ts -------------------------------------------------
SG = M + "rules/getter/seerr-getter.service.spec.ts"
FILES[SG] = "seerr_getter.json"
SG_NOTE = ("Request facts kept from the spec's movieRequest/mediaInfo builders (:75, :85): type movie, status "
           "APPROVED, createdAt 2026-01-01, requestedBy plexUsername PlexUser, media status AVAILABLE, "
           "mediaAddedAt 2026-02-02, tmdbId 12345; overrides as listed.")
REQ = {"type": "movie", "status": "APPROVED", "is4k": False,
       "requestedBy": {"id": 10, "userType": 1, "plexUsername": "PlexUser"},
       "media": {"tmdbId": 12345, "status": "AVAILABLE", "mediaAddedAt": "2026-02-02"}}


def req(**kw):
    r = json.loads(json.dumps(REQ))
    for k, v in kw.items():
        r[k] = {**r[k], **v} if isinstance(v, dict) and k in r else v
    return r


def sg(title, behaviour, prop, given, expect):
    port(SG, title, behaviour, "seerr_get", {"property": prop, "item": {"type": "movie"}, **given}, expect, SG_NOTE)


sg("should return empty array when the title has no requests",
   "addUser of a movie with no Seerr requests is an empty list", "addUser", {"requests": []}, {"value": []})
sg("should return deduplicated usernames when same user has multiple requests",
   "addUser lists each requesting user once, even with several requests (normal and 4K) by the same user",
   "addUser", {"requests": [req(id=1, requestedBy={"plexUsername": "SameUser"}),
                            req(id=2, is4k=True, requestedBy={"plexUsername": "SameUser"})]},
   {"value": ["SameUser"]})
sg("should return 1 when the movie has a request", "isRequested of a requested movie is 1", "isRequested",
   {"requests": [req()]}, {"value": 1})
sg("should return 0 when the title has no requests for movies", "isRequested of a movie with no requests is 0",
   "isRequested", {"requests": []}, {"value": 0})
sg("should return media mediaAddedAt when available for movies",
   "mediaAddedAt of an available movie is the request media's mediaAddedAt", "mediaAddedAt",
   {"requests": [req(media={"status": "AVAILABLE", "mediaAddedAt": "2026-08-01"})]},
   {"value": {"$date": "2026-08-01"}})
sg("mediaAddedAt (property id=4) > should return null when the media is not yet available for movies",
   "mediaAddedAt of a movie whose media is not yet available (PROCESSING) is null", "mediaAddedAt",
   {"requests": [req(media={"status": "PROCESSING"})]}, {"value": None})
sg("should return undefined (transient) without querying Seerr",
   "With no resolvable TMDB id the value is undefined (transient) and Seerr is not queried", "isRequested",
   {"tmdbId": U}, {"value": U, "seerrQueried": False})
OUTS[SG] = [
    (r"amountRequested|watchlist|requestDate|approvalDate|releaseDate",
     "property not used by our rules (we read addUser, isRequested, mediaAddedAt)"),
    (r"season|episode", "season and episode items: our episode group (rules.json#id=3) reads no Seerr property")]

# --- rules/getter/plex-getter.service.spec.ts ---------------------------------------------------
PG = M + "rules/getter/plex-getter.service.spec.ts"
FILES[PG] = "plex_getter.json"
PG_NOTE = ("The item is the spec's movie fixture (makeMetadata, :48, ratingKey 12345); the spec has no episode case. "
           "In the getter source (plex-getter.service.ts) isWatched (:157) and lastViewedAt (:279) have no type "
           "branch; lastViewedAt passes metadata.type on to getWatchHistory.")


def pg(title, behaviour, prop, given, expect, note=PG_NOTE):
    port(PG, title, behaviour, "plex_get", {"property": prop, "item": {"type": "movie"}, **given}, expect, note)


pg("should return the adapter watched state for the isWatched rule",
   "isWatched is the media server's current watched state, read live", "isWatched",
   {"watchState": {"viewCount": 0, "isWatched": False}}, {"value": False})
pg("returns the item lastViewedAt when no history row records that view (id 7)",
   "lastViewedAt falls back to the item's own last-viewed date when watch history has no row for it",
   "lastViewedAt", {"itemLastViewedAt": date(1_730_000_000), "history": []}, {"value": date(1_730_000_000)})
pg("prefers a newer history date over the item lastViewedAt (id 7)",
   "lastViewedAt is the newer of the item's last-viewed date and the newest history view", "lastViewedAt",
   {"itemLastViewedAt": date(1_730_000_000), "history": [{"viewedAt": date(1_740_000_000)}]},
   {"value": date(1_740_000_000)})
pg("returns the newest direct watch-history date for lastViewedAt (id 7)",
   "lastViewedAt is the newest view in the item's watch history", "lastViewedAt",
   {"history": [{"viewedAt": date(e)} for e in (1_700_000_000, 1_720_000_000, 1_710_000_000)]},
   {"value": date(1_720_000_000)})
pg("maps watch-history account ids to known Plex usernames",
   "seenBy maps history account ids to known usernames, in the server's user order, dropping unknown accounts",
   "seenBy", {"users": [{"plexId": 2, "username": "bob"}, {"plexId": 1, "username": "alice"},
                        {"plexId": 3, "username": "charlie"}],
              "history": [{"accountID": 1}, {"accountID": 999}, {"accountID": 2}]},
   {"value": ["bob", "alice"]})
pg("returns [] for confirmed-empty history (no one has watched the item)",
   "seenBy of an item nobody has watched is an empty list", "seenBy",
   {"users": [{"plexId": 1, "username": "alice"}], "history": []}, {"value": []})
pg('returns undefined when watch-history lookup fails so the comparator skips the item rather than misclassifying '
   'it as "viewed by no one"', "seenBy is undefined (transient) when the watch history cannot be read", "seenBy",
   {"users": [{"plexId": 1, "username": "alice"}], "history": {"$throws": "plex unreachable"}}, {"value": U})
pg("returns undefined when the plex.tv username enrichment fails so degraded names never mis-evaluate rules (#3307)",
   "seenBy is undefined (transient) when the user list cannot be read", "seenBy",
   {"users": {"$throws": "plex.tv user data unavailable"}, "history": [{"accountID": 1}]}, {"value": U})
OUTS[PG] = [(r".", "property not used by our rules (we read seenBy, isWatched, lastViewedAt)")]

# --- actions/radarr-action-handler.spec.ts -----------------------------------------------------
RA = M + "actions/radarr-action-handler.spec.ts"
FILES[RA] = "radarr_action.json"
RA_COLL = {"arrAction": "DELETE", "type": "movie", "radarrSettingsId": 1}
RA_NOTE = "listExclusions is the createCollection default (test/utils/data.ts:88), false."
NONE_RADARR = {"radarr": {"updateMovie": False, "deleteMovie": False}}


def ra(title, behaviour, given, expect, row=None, note=None):
    port(RA, title, behaviour, "radarr_handle_action", {"collection": RA_COLL, **given}, expect, note, row)


ra("should do nothing when tmdbid failed lookup",
   "DELETE takes no Radarr action when the movie's TMDB id cannot be resolved",
   {"media": {"tmdbId": U}, "lookupCandidates": []}, NONE_RADARR)
ra("should fail closed without deleting when the Radarr lookup is transient (undefined) and action is DELETE",
   "DELETE fails closed (result false, nothing deleted) when the Radarr lookup fails transiently",
   {"media": {"tmdbId": 1}, "radarrMovie": U}, {"result": False, "mediaServerDeleteFromDisk": False, **NONE_RADARR})
ra("should remove via media server when the movie is confirmed not tracked (null) and action is DELETE",
   "DELETE of a movie Radarr confirms it does not track deletes it through the media server",
   {"media": {"tmdbId": 1}, "radarrMovie": None},
   {"result": True, "mediaServerDeleteFromDisk": True, **NONE_RADARR})
ra("should delete movie when action is $title",
   "DELETE removes the movie from Radarr with its files, passing the collection's listExclusions as the import "
   "exclusion flag, and does not just unmonitor it",
   {"media": {"tmdbId": 1}, "radarrMovie": {"id": 5}, "collection": {**RA_COLL, "listExclusions": False}},
   {"radarr": {"deleteMovie": {"id": 5, "deleteFiles": True, "addImportExclusion": False}, "updateMovie": False}},
   row=0, note=RA_NOTE)
OUTS[RA] = [
    (r"download", "download_client_type null: no download client"),
    (r"leftover-folder", "cleanupLeftoverFolders false on both collections"),
    (r".", "action not used by our collections (arrAction 0, DELETE)")]

# --- actions/sonarr-action-handler.spec.ts -----------------------------------------------------
SA = M + "actions/sonarr-action-handler.spec.ts"
FILES[SA] = "sonarr_action.json"
EP_COLL = {"arrAction": "DELETE", "type": "episode", "sonarrSettingsId": 1}
NONE_SONARR = {"sonarr": {"unmonitorSeasons": False, "UnmonitorDeleteEpisodes": False, "deleteShow": False,
                          "delete": False}}
SHARED = ("The test uses a show; the series lookup it guards runs before every Sonarr action, the episode "
          "DELETE included (sonarr-action-handler.ts:116, :128).")
RANDOM = ("season/episode come from the createMediaItem fixture (random numbers); the assertion echoes them, "
          "written here as {\"$given\": <field>}.")


def sa(title, behaviour, given, expect, row=None, note=None):
    port(SA, title, behaviour, "sonarr_handle_action", given, expect, note, row)


sa("should do nothing for $title when Show tmdbid failed lookup",
   "DELETE takes no Sonarr action and deletes nothing when the series' TVDB id cannot be resolved",
   {"collection": {**EP_COLL, "type": "show"}, "media": {"tmdbId": U}, "tvdbId": U},
   {"sonarrLookup": False, "mediaServerDeleteFromDisk": False, **NONE_SONARR}, row=0, note=SHARED)
sa("should delete $title in Plex if not found in Sonarr and action is $action",
   "Episode DELETE of a series Sonarr confirms it does not track deletes the episode through the media server",
   {"collection": EP_COLL, "media": {"tmdbId": 1}, "tvdbId": 1, "sonarrSeries": None},
   {"mediaServerDeleteFromDisk": True, **NONE_SONARR}, row=4)
sa("fails closed (no delete, no action) when the Sonarr lookup fails transiently for a DELETE action",
   "DELETE fails closed (result false, nothing deleted) when the Sonarr series lookup fails transiently",
   {"collection": {**EP_COLL, "type": "show"}, "media": {"tmdbId": 1}, "tvdbId": 1, "sonarrSeries": U},
   {"result": False, "mediaServerDeleteFromDisk": False, **NONE_SONARR}, note=SHARED)
sa("should unmonitor and delete episode when type EPISODES and action DELETE",
   "Episode DELETE unmonitors the episode and deletes its file in Sonarr, by season and episode number, and touches "
   "nothing else",
   {"collection": EP_COLL, "media": {"tmdbId": 1, "mediaData": {"parentIndex": "random", "index": "random"}},
    "tvdbId": 1, "sonarrSeries": {"id": "random"}},
   {"mediaServerDeleteFromDisk": False,
    "sonarr": {"UnmonitorDeleteEpisodes": {"seriesId": {"$given": "sonarrSeries.id"},
                                           "seasonNumber": {"$given": "mediaData.parentIndex"},
                                           "episodeNumbers": [{"$given": "mediaData.index"}],
                                           "deleteFiles": True, "airDate": U},
               "unmonitorSeasons": False, "deleteShow": False, "delete": False}}, note=RANDOM)
sa("should fall back to episode air date when episode index is undefined",
   "An episode without an episode number is deleted by its air date",
   {"collection": EP_COLL, "media": {"tmdbId": 1, "mediaData": {"index": U, "parentIndex": 2026,
                                                                "originallyAvailableAt": "2026-01-05T00:00:00.000Z"}},
    "tvdbId": 1, "sonarrSeries": {"id": "random"}},
   {"sonarr": {"UnmonitorDeleteEpisodes": {"seriesId": {"$given": "sonarrSeries.id"}, "seasonNumber": 2026,
                                           "episodeNumbers": [], "deleteFiles": True,
                                           "airDate": {"$date": "2026-01-05T00:00:00.000Z"}}}})
sa("acts on the episode the item's TVDB id names, by Sonarr's numbers (#3819)",
   "An episode whose TVDB id names a Sonarr episode is deleted by Sonarr's season and episode numbers, not the "
   "media server's",
   {"collection": EP_COLL, "media": {"tmdbId": 1, "mediaData": {"parentIndex": 1, "index": 1171,
                                                                "providerIds": {"tvdb": ["501"]}}},
    "tvdbId": 1, "sonarrSeries": {"id": "random"},
    "sonarrEpisodes": [{"seasonNumber": 23, "episodeNumber": 16, "tvdbId": 501}]},
   {"sonarr": {"UnmonitorDeleteEpisodes": {"seriesId": {"$given": "sonarrSeries.id"}, "seasonNumber": 23,
                                           "episodeNumbers": [16], "deleteFiles": True, "airDate": U}}})
FILE_MATCH = {"collection": EP_COLL, "tvdbId": 1, "sonarrSeries": {"id": "random"},
              "media": {"tmdbId": 1, "mediaData": {
                  "parentIndex": 2026, "index": 1, "providerIds": {"tvdb": ["501"]},
                  "path": "/media/tv/Sample Series/Season 2026/Sample Series - 2026-06-10.mkv"}},
              "sonarrEpisodes": [{"seasonNumber": 2026, "episodeNumber": 1, "tvdbId": 501, "episodeFileId": 0},
                                 {"seasonNumber": 2026, "episodeNumber": 101, "episodeFileId": 8}],
              "sonarrEpisodeFiles": [{"id": 8, "relativePath": "Season 2026/Sample Series - 2026-06-10.mkv"}]}
sa("acts on the episode the item's file holds, over its numbers and TVDB id",
   "An episode whose file path matches a Sonarr episode file is deleted as that Sonarr episode, over its numbers "
   "and TVDB id", FILE_MATCH,
   {"sonarr": {"UnmonitorDeleteEpisodes": {"seriesId": {"$given": "sonarrSeries.id"}, "seasonNumber": 2026,
                                           "episodeNumbers": [101], "deleteFiles": True, "airDate": U}}})
sa("takes no action when %s",
   "Episode DELETE takes no action when Sonarr's episode files cannot be read",
   {**FILE_MATCH, "sonarrEpisodeFiles": U}, {"sonarr": {"UnmonitorDeleteEpisodes": False}}, row=0)
sa("takes no action when %s",
   "Episode DELETE takes no action when Sonarr's episodes cannot be read, even if the item's numbers are known",
   {**FILE_MATCH, "sonarrEpisodes": U,
    "media": {"tmdbId": 1, "mediaData": {
        "parentIndex": 2026, "index": 1, "providerIds": {},
        "path": "/media/tv/Sample Series/Season 2026/Sample Series - 2026-06-12.mkv"}}},
   {"sonarr": {"UnmonitorDeleteEpisodes": False}}, row=1)
sa("should skip episode action when no season, episode number, or air date is available",
   "Episode DELETE does nothing when the item has no season, episode number or air date",
   {"collection": EP_COLL, "media": {"tmdbId": 1, "mediaData": {"index": U, "parentIndex": U,
                                                                "originallyAvailableAt": U}},
    "tvdbId": 1, "sonarrSeries": {"id": "random"}},
   {"sonarr": {"UnmonitorDeleteEpisodes": False}, "warned": True})
OUTS[SA] = [
    (r"download client", "download_client_type null: no download client"),
    (r"leftover-folder", "cleanupLeftoverFolders false on both collections"),
    (r"caches the matched series TMDB id", "feeds the Seerr request removal, off (forceSeerr false)"),
    (r".", "type/action not used by our collections (the Sonarr one is episode + DELETE)")]

# --- api/servarr-api/helpers/sonarr.helper.spec.ts ---------------------------------------------
SH = M + "api/servarr-api/helpers/sonarr.helper.spec.ts"
FILES[SH] = "sonarr_episode_delete.json"


def sh(title, behaviour, op, given, expect, row=None, note=None):
    port(SH, title, behaviour, op, given, expect, note, row)


ADATE = {"$date": "2026-01-05T00:00:00.000Z"}
JULY = {"$date": "2026-07-01T00:00:00.000Z"}
E101 = {"id": 101, "seasonNumber": 2026, "episodeNumber": 5, "airDate": "2026-01-05", "episodeFileId": 501,
        "monitored": True}
E101J = {**E101, "airDate": "2026-07-01"}
E102J = {"id": 102, "seasonNumber": 2026, "episodeNumber": 6, "airDate": "2026-07-01", "episodeFileId": 502,
         "monitored": True}
UDE = "UnmonitorDeleteEpisodes"
sh("should match episodes by air date when episode numbers are empty",
   "Deleting by air date unmonitors and deletes the file of only the episode aired that day", UDE,
   {"seriesId": 1, "seasonNumber": 2026, "episodeNumbers": [], "deleteFiles": True, "airDate": ADATE,
    "episodes": [{"id": 101, "seasonNumber": 2026, "episodeNumber": 5, "airDate": "2026-01-05", "episodeFileId": 501},
                 {"id": 102, "seasonNumber": 2026, "episodeNumber": 6, "airDate": "2026-01-06",
                  "episodeFileId": 502}]},
   {"unmonitored": [101], "deletedFiles": [501]})
sh("should return no matches when explicit episode numbers are all undefined",
   "Asking for episode numbers that are all undefined matches no episode", "getEpisodes",
   {"seriesId": 1, "seasonNumber": 1, "episodeNumbers": [U], "episodes": [{"episodeNumber": 1}, {"episodeNumber": 2}]},
   {"value": []})
sh("should include season 0 when fetching specials",
   "Season 0 (specials) is a real season filter, not 'no season'", "getEpisodes",
   {"seriesId": 1, "seasonNumber": 0}, {"value": [], "request": "/episode?seriesId=1&seasonNumber=0"})
sh("should rethrow when episode lookup fails", "An episode read that fails raises instead of returning no episodes",
   "getEpisodes", {"seriesId": 1, "seasonNumber": 1, "episodeNumbers": [1], "episodes": {"$throws": "boom"}},
   {"throws": "boom"})
sh("should refuse an episode action without a season number (#3415)",
   "Deleting episodes without a season number is refused (false) before anything is read, unmonitored or deleted",
   UDE, {"seriesId": 1, "seasonNumber": U, "episodeNumbers": [1]},
   {"result": False, "episodesRead": False, "unmonitored": [], "deletedFiles": []})
sh("unmonitors every episode a deleted file holds and deletes the file once",
   "Deleting an episode whose file also holds other episodes unmonitors all of them and deletes the file once",
   UDE, {"seriesId": 1, "seasonNumber": 1, "episodeNumbers": [1],
         "episodes": [{"id": 1, "episodeNumber": 1, "episodeFileId": 9}, {"id": 2, "episodeNumber": 2, "episodeFileId": 9},
                      {"id": 3, "episodeNumber": 3, "episodeFileId": 10}]},
   {"result": True, "unmonitored": [1, 2], "deletedFiles": [9]})
sh("keeps a shared file while an episode on it may still be monitored",
   "A shared file is kept (result false) when another episode on it could not be unmonitored and still reads "
   "as monitored", UDE,
   {"seriesId": 1, "seasonNumber": 1, "episodeNumbers": [2],
    "episodes": [{"id": 1, "episodeNumber": 1, "episodeFileId": 9}, {"id": 2, "episodeNumber": 2, "episodeFileId": 9}],
    "unmonitorFails": [2], "recheck": {"2": {"monitored": True}}},
   {"result": False, "deletedFiles": []})
sh("deletes the episode file when PUT timed out but Sonarr confirms unmonitored (timeout race)",
   "When the unmonitor call fails but a fresh read shows the episode unmonitored, its current file is deleted",
   UDE, {"seriesId": 1, "seasonNumber": 2026, "episodeNumbers": [], "deleteFiles": True, "airDate": ADATE,
         "episodes": [E101], "unmonitorFails": [101],
         "recheck": {"101": {"monitored": False, "episodeFileId": 601}}},
   {"result": True, "deletedFiles": [601]})
sh("skips the file delete and warns when PUT failed and Sonarr confirms still monitored (genuine failure)",
   "When the unmonitor call fails and a fresh read shows the episode still monitored, its file is kept",
   UDE, {"seriesId": 1, "seasonNumber": 2026, "episodeNumbers": [], "deleteFiles": True, "airDate": ADATE,
         "episodes": [E101], "unmonitorFails": [101], "recheck": {"101": {"monitored": True}}},
   {"result": False, "deletedFiles": [], "warned": True})
sh("skips the file delete when PUT failed and the verification lookup returns undefined (fail closed)",
   "When the unmonitor call fails and the fresh read fails too, the file is kept", UDE,
   {"seriesId": 1, "seasonNumber": 2026, "episodeNumbers": [], "deleteFiles": True, "airDate": ADATE,
    "episodes": [E101], "unmonitorFails": [101], "recheck": {"101": U}},
   {"result": False, "deletedFiles": []})
sh("processes remaining episodes after a single failure (batch no longer aborts)",
   "One episode failing to unmonitor does not stop the others: their files are still deleted, the result is false",
   UDE, {"seriesId": 1, "seasonNumber": 2026, "episodeNumbers": [], "deleteFiles": True, "airDate": JULY,
         "episodes": [E101J, E102J], "unmonitorFails": [101], "recheck": {"101": {"monitored": True}}},
   {"result": False, "deletedFiles": [502]})
sh("continues deleting remaining files after one file delete fails (delete failure no longer aborts)",
   "One file delete failing does not stop the others; the result is false", UDE,
   {"seriesId": 1, "seasonNumber": 2026, "episodeNumbers": [], "deleteFiles": True, "airDate": JULY,
    "episodes": [E101J, E102J], "deleteFails": [501]},
   {"result": False, "deleteAttempts": [501, 502]})
sh("reads the season episodes uncached before an episode delete",
   "An episode delete reads that season's episodes fresh and deletes the matching episode's file", UDE,
   {"seriesId": 1, "seasonNumber": 3, "episodeNumbers": [7],
    "episodes": [{"seasonNumber": 3, "episodeNumber": 7, "episodeFileId": 700}]},
   {"request": "/episode?seriesId=1&seasonNumber=3", "cached": False, "deletedFiles": [700]})
FEMI = "findEpisodeForMediaItem"
for r, name, item_path, rel_path in [
        (0, "a Windows path", "D:\\TV\\Sample Series\\Season 1\\Sample Series - S01E05.mkv",
         "Season 1/Sample Series - S01E05.mkv"),
        (1, "a decomposed Unicode file name", "/media/tv/Sample Series/Season 1/Sample Series - Cafe\u0301.mkv",
         "Season 1/Sample Series - Caf\u00e9.mkv")]:
    sh("matches the file from %s", f"An episode is matched to its Sonarr file from {name}: the item path's tail "
       "equals the file's relative path", FEMI,
       {"seasonNumber": 1, "item": {"type": "episode", "index": U, "providerIds": {}, "path": item_path},
        "episodes": [{"episodeNumber": 5, "episodeFileId": 9}], "episodeFiles": [{"id": 9, "relativePath": rel_path}]},
       {"episodeNumber": 5}, row=r)
sh("matches no file whose folder only ends the item's folder name",
   "A file is not matched when the item's folder only ends with the file's folder name", FEMI,
   {"seasonNumber": 1, "item": {"type": "episode", "index": U, "providerIds": {},
                                "path": "/media/tv/Sample Series/Extras Season 1/Sample Series - S01E05.mkv"},
    "episodes": [{"episodeNumber": 5, "episodeFileId": 9}],
    "episodeFiles": [{"id": 9, "relativePath": "Season 1/Sample Series - S01E05.mkv"}]},
   {"episodeNumber": U})
OUTS[SH] = [
    (r"getSeriesDownloadHistory|getDownloadIdsForSeries", "download_client_type null: no download client"),
    (r"setSeriesTags", "tagExclusions false: Maintainerr never tags series"),
    (r"lists every series|episode files uncached|failed episode-file listing",
     "fences the leftover-folder cleanup, off (cleanupLeftoverFolders false)"),
    (r"getSeriesByTvdbId|runPut|runDelete",
     "transport contract of the Sonarr HTTP client; the handler-level behaviour it protects is ported from "
     "sonarr-action-handler.spec.ts (not tracked, transient lookup)"),
    (r".", "season scope (unmonitorSeasons) is not used: the Sonarr collection deletes episodes")]

# --- collections/collection-worker.server.spec.ts ---------------------------------------------
CW = M + "collections/collection-worker.server.spec.ts"
FILES[CW] = "collection_worker.json"


def cw(title, behaviour, given, expect, note=None):
    port(CW, title, behaviour, "collection_worker_execute", given, expect, note)


DEL = {"arrAction": "DELETE"}
cw("should abort if the media server is unreachable",
   "A collection run stops before reading any collection when the media server is unreachable, emitting one failed "
   "and one finished event", {"mediaServer": "unreachable"},
   {"collectionsRead": False, "events": {"CollectionHandler_Failed": 1, "CollectionHandler_Finished": 1}})
cw("should not handle media for a collection with no deletion window set",
   "A DELETE collection with no deletion window (null, unlike our 0) handles no media",
   {"collections": [{**DEL, "deleteAfterDays": None}], "media": ["any"]}, {"handled": []})
cw("only treats members as due once the delete window has passed",
   "Members are due once they were added deleteAfterDays days ago or earlier (cutoff = now - 30 days for 30)",
   {"collections": [{**DEL, "deleteAfterDays": 30}], "media": []},
   {"dueCutoff": {"$now_offset_seconds": -30 * 86400}}, "The test allows 60 s of slack on the cutoff.")
cw("skips flagged rule-owned media but still handles flagged manual media",
   "A rule-owned member whose last rule evaluation failed is not acted on; a manual member is",
   {"collections": [{**DEL, "type": "movie"}],
    "media": [{"mediaServerId": "rule-owned", "includedByRule": True, "manualMembershipSource": None,
               "ruleEvaluationFailed": True},
              {"mediaServerId": "manual", "includedByRule": False, "manualMembershipSource": "LOCAL",
               "ruleEvaluationFailed": True}]},
   {"handled": ["manual"]})
cw("defers currently-playing media to the next run when the server reports active sessions",
   "A member being played right now is left for the next run; the others are handled",
   {"collections": [{**DEL, "type": "movie"}], "media": ["playing", "idle"], "activeSessions": ["playing"]},
   {"handled": ["idle"], "deferredLog": True})
cw("should not report failed media as handled",
   "A member whose action failed raises the failed event, not the handled event, and triggers no Seerr sync",
   {"seerrConfigured": True, "collections": [{**DEL, "type": "show"}], "media": ["m"], "handleResult": ["failed"]},
   {"events": {"CollectionHandler_Failed": ["m"], "CollectionMedia_Handled": 0}, "seerrSync": False})
cw("does not notify or trigger availability sync when media was pruned as missing",
   "A member pruned because it is already gone raises no event and triggers no Seerr sync",
   {"seerrConfigured": True, "collections": [{**DEL, "type": "show"}], "media": ["m"],
    "handleResult": ["removed-missing"]},
   {"events": {"CollectionHandler_Failed": 0, "CollectionMedia_Handled": 0}, "seerrSync": False})
cw("should emit failure and continue when media handling throws",
   "A member whose handling throws raises the failed event and the run goes on to the next member",
   {"seerrConfigured": True, "collections": [{**DEL, "type": "show"}], "media": ["1", "2"],
    "handleResult": [{"$throws": "boom"}, "handled"]},
   {"handleCalls": 2, "events": {"CollectionHandler_Failed": ["1"]}, "seerrSync": True})
OUTS[CW] = [
    (r"exclusion", "no exclusions configured (exclusions-summary.json count 0)"),
    (r"Do Nothing|quality profile|profile change", "action not used by our collections (arrAction 0, DELETE)"),
    (r"title before handling", "content of the handled notification; notifications.json is empty")]

# --- collections/collection-handler.spec.ts ----------------------------------------------------
CH = M + "collections/collection-handler.spec.ts"
FILES[CH] = "collection_handler.json"


def ch(title, behaviour, given, expect, row=None, note=None):
    port(CH, title, behaviour, "collection_handle_media", given, expect, note, row)


MOVIE = {"arrAction": "DELETE", "type": "movie", "radarrSettingsId": 1}
ch("should delete from disk",
   "A show DELETE on a collection with no *arr server deletes the item through the media server and removes it "
   "from the collection; the result is handled", {"collection": {"arrAction": "DELETE", "type": "show"}},
   {"result": "handled", "removeFromCollection": 1, "mediaServerDeleteFromDisk": True})
ch("prunes the item from sibling collections after a file-removal action",
   "After a DELETE the item leaves its own collection first, then every other collection listing it, each marked "
   "recently handled", {"collection": {"arrAction": "DELETE", "type": "show"}, "siblingsListingItem": [42, 43]},
   {"order": ["removeFromCollection", "removeMediaFromOtherCollections"], "markedHandled": [42, 43]})
ch("should call Radarr action handler",
   "A movie DELETE runs the Radarr action, then removes the item from the collection; the result is handled",
   {"collection": MOVIE, "radarrResult": True},
   {"result": "handled", "order": ["radarrHandleAction", "removeFromCollection"], "removeFromCollection": 1})
for r, (path, folder) in enumerate([("/media/movies/Movie A (2021)/Movie A (2021).mkv", "/media/movies/Movie A (2021)"),
                                    ("C:\\Media\\Movie A (2021)\\Movie A (2021).mkv", "C:\\Media\\Movie A (2021)")]):
    ch("asks the media server to rescan the movie folder after an *arr delete (%s)",
       f"After the Radarr delete the media server rescans the movie's folder ({folder})",
       {"collection": MOVIE, "radarrResult": True, "itemPath": path},
       {"scanFolder": {"libraryId": {"$given": "collection.libraryId"}, "folder": folder},
        "order": ["radarrHandleAction", "scanFolder"]}, row=r)
ch("still handles the item when its folder cannot be read",
   "The item is still handled when its folder cannot be read; no rescan is asked",
   {"collection": MOVIE, "radarrResult": True, "itemMetadata": {"$throws": "offline"}},
   {"result": "handled", "scanFolder": False})
ch("keeps the action handled when the rescan request fails",
   "A failed rescan request leaves the item handled, with a warning",
   {"collection": MOVIE, "radarrResult": True, "itemPath": "/media/movies/Movie A (2021)/Movie A (2021).mkv",
    "scanFolder": {"$throws": "offline"}}, {"result": "handled", "warned": True})
ch("should not remove media from collection when Radarr action fails",
   "When the Radarr action fails the item stays in the collection, unlogged and uncounted; the result is failed",
   {"collection": MOVIE, "radarrResult": False},
   {"result": "failed", "removeFromCollection": 0, "logRecord": False, "saveCollection": False})
ch("prunes the item from all collections when it no longer exists on the media server",
   "When the action fails because the item is gone from the media server, it is pruned from this and every other "
   "collection, marked handled, and not counted as handled",
   {"collection": {"arrAction": "DELETE", "type": "show", "sonarrSettingsId": 1}, "sonarrResult": False,
    "itemExists": False, "siblingsListingItem": [42]},
   {"result": "removed-missing", "removeFromCollection": {"scope": "all"},
    "markedHandled": [42, {"$given": "collection.id"}], "logRecord": False, "saveCollection": False})
ch("keeps the item when the existence check is inconclusive (throws)",
   "When the action fails and the existence check errors, the item stays in every collection; the result is failed",
   {"collection": {"arrAction": "DELETE", "type": "show", "sonarrSettingsId": 1}, "sonarrResult": False,
    "itemExists": {"$throws": "media server down"}},
   {"result": "failed", "removeFromCollection": 0, "removeMediaFromOtherCollections": False})
ch("persists the cleared link when removing the last item empties (and deletes) the collection",
   "When the last item leaves, the emptied media-server collection is deleted and its cleared link is saved with "
   "the handled count", {"collection": {"arrAction": "DELETE", "type": "movie", "mediaServerId": "dead-boxset-id"},
                         "removeFromCollectionReturns": {"mediaServerId": None}},
   {"saveCollection": {"mediaServerId": None, "handledMediaAmount": 1}})
ch("leaves Seerr and Ombi requests alone when the collection has not opted in",
   "With forceSeerr and forceOmbi false a movie DELETE leaves its Seerr and Ombi requests in place",
   {"collection": {"arrAction": "DELETE", "type": "movie", "forceSeerr": False, "forceOmbi": False},
    "seerrConfigured": True, "ombiConfigured": True},
   {"result": "handled", "seerrRemoveMedia": False, "seerrRemoveSeason": False, "ombiRemoveMedia": False})
ch("credits cached sizeBytes to handledMediaSizeBytes for delete-style actions",
   "An episode DELETE adds the member's cached size to the collection's handled bytes without a size lookup",
   {"collection": {"arrAction": "DELETE", "type": "episode"}, "sizeBytes": 1_500_000_000},
   {"resolveItemSize": False, "saveCollection": {"handledMediaAmount": 1, "handledMediaSizeBytes": 1_500_000_000}})
ch("falls back to media-server lookup when sizeBytes is null on a delete-style action",
   "An episode DELETE with no cached size looks the size up before the delete and adds it to the handled bytes",
   {"collection": {"arrAction": "DELETE", "type": "episode"}, "sizeBytes": None, "resolvedSize": 2_000_000_000},
   {"order": ["resolveItemSize", "deleteFromDisk"],
    "saveCollection": {"handledMediaAmount": 1, "handledMediaSizeBytes": 2_000_000_000}})
OUTS[CH] = [
    (r"DO_NOTHING|unmonitor|UNMONITOR|CHANGE_QUALITY_PROFILE|SHOW_IF_EMPTY|rule-removal marker|season the server",
     "action/type not used by our collections (arrAction 0, DELETE, on movies and episodes)"),
    (r"Seerr|Ombi|season request|season number before|removeMediaByTmdbId|as tv when",
     "forceSeerr/forceOmbi true: ours are false (collections.json); the opt-out case is ported")]

# --- rules/tasks/rule-executor-scheduler.service.spec.ts ---------------------------------------
SC = M + "rules/tasks/rule-executor-scheduler.service.spec.ts"
FILES[SC] = "scheduler.json"
port(SC, "registers cron jobs for scheduled rule groups and global schedule on bootstrap",
     "At startup each rule group with its own cron gets its own job; every other group runs on one global job with "
     "rules_handler_job_cron", "scheduler_bootstrap",
     {"rules_handler_job_cron": "*/5 * * * * *",
      "ruleGroups": [{"id": 1, "isActive": True, "ruleHandlerCronSchedule": "*/10 * * * * *"},
                     {"id": 2, "isActive": True, "ruleHandlerCronSchedule": U},
                     {"id": 3, "isActive": True, "ruleHandlerCronSchedule": "*/15 * * * * *"}]},
     {"jobs": {"rule-group-executor-1": "*/10 * * * * *", "rule-group-executor-3": "*/15 * * * * *",
               "execute-global-schedule-rules": "*/5 * * * * *"}})
OUTS[SC] = [(r".", "reacts to rule groups or settings edited at runtime; our schedules are fixed config "
             "(both groups on the global cron)")]

# --- Spec files not ported (checked against every apps/**/*.spec.ts at the end) ---------------
DEFERRED = "DEFERRED (open decision, ledger): "
RULED_OUT = [  # (regex over the path from the repo root, reason); first match wins
    (r"rules/tasks/rule-executor\.service|collections/collections\.service|collections/recently-handled-media"
     r"|collections/collection-media-rule-removal|collections/stop-media-server-sync"
     r"|rules/tasks/rule-executor-job-manager",
     DEFERRED + "rule run to collection membership reconciliation with the media server, the re-add guard, the "
     "rule queue and its media-server outage handling"),
    (r"modules/tasks/", DEFERRED + "job scheduling and the rules/collections execution lock"),
    (r"api/media-server/(plex/|(?!media-server\.controller)[^/]+\.spec)|api/plex-api/|api/lib/plex(tv)?Api|rules/rule-users\.service"
     r"|rules/helpers/rule-property\.helper|rules/getter/getter\.service",
     DEFERRED + "Plex client and media-server layer under the ported plex-getter and handler cases (watch "
     "history, users, item presence, deleteFromDisk, scanFolder, getter dispatch)"),
    (r"api/seerr-api/", DEFERRED + "Seerr HTTP client under the ported seerr-getter cases; Scryer's built-in "
     "requests replace Seerr"),
    (r"metadata/providers/sportarr", "no Sportarr server (servarr.json, sportarrSettingsId null)"),
    (r"modules/metadata/|api/tmdb-api/|api/tvdb-api/",
     DEFERRED + "TMDB/TVDB id resolution used by both action handlers (metadata_provider_preference "
     "tmdb_primary); Scryer keys titles by its own ids"),
    (r"rules/constants/constants\.service", DEFERRED + "rule property table and custom value type parsing "
     "(rule-constants.json captured)"),
    (r"exclusion|rule-maintenance", "exclusions-summary.json count 0 and tagExclusions false"),
    (r"leftover-folder-cleanup", "cleanupLeftoverFolders false on both collections"),
    (r"servarr-tag", "tagExclusions false, no tagInArr"),
    (r"download-client-api/", "download_client_type null: no download client"),
    (r"modules/notifications/", "notifications.json is empty"),
    (r"modules/overlays/|collection-poster", "overlay-settings.json enabled false"),
    (r"sportarr", "no Sportarr server (servarr.json, sportarrSettingsId null)"),
    (r"emby|jellyfin", "media_server_type plex"),
    (r"ombi", "ombi_url null"),
    (r"streamystats|tracearr", "streamystats_url and tracearr_url null"),
    (r"tautulli", "Tautulli is set but no rule reads a Tautulli property (rules.json reads Plex and Seerr only)"),
    (r"rules/getter/(radarr|sonarr|metadata-rule-value)-?getter|metadata-rule-value|arr-lookup-cache"
     r"|diskspace\.utils",
     "property source not read by our rules (rules.json reads Plex seenBy/isWatched/lastViewedAt and Seerr "
     "addUser/isRequested/mediaAddedAt only)"),
    (r"servarr-api/helpers/radarr\.helper",
     "Radarr HTTP client: deleteMovie (our DELETE path) has no test here; the rest covers updateMovie and the "
     "import-exclusion bulk endpoint (UNMONITOR paths), tags, download ids, uncached reads, and the "
     "getMovieByTmdbId null/undefined contract whose outcome is ported at handler level (radarr_action.json)"),
    (r"servarr-api/common/", "*arr HTTP transport (retries, caching, error mapping), not cleanup behaviour"),
    (r"rules/rules\.(controller|service)|rules/helpers/(yaml\.service|rule-application-availability)"
     r"|modules/settings/|collections/collections\.controller|collections/sort-utils|media-server\.controller",
     "rule, collection and settings editing, Test Media, UI helpers and the per-run getter cache reset, all "
     "through the API/UI; our rules, collections and settings are fixed config"),
    (r"storage-metrics|telemetry|modules/version/|modules/logging/|modules/events/|server/src/app/|server/src/database/"
     r"|server/src/utils/|api/lib/(cache|httpRetry)|api/external-api/|api/github-api/|api/media-server/guards/",
     "infrastructure (health, CORS, API setup guard, DB migrations, logging, events, telemetry, version check, HTTP helpers, "
     "dashboard metrics), not cleanup behaviour"),
    (r"^apps/ui/", "web UI"),
]

# --- IDs, catalogue, corpus --------------------------------------------------------------------
entries = []  # (rel, line, slug, behaviour, in_scope, file, input, expect, note)
used = set()
for rel in FILES:
    for path, title, line, rows in tests_of(rel):
        for r, (row_line, row_text) in enumerate(rows) if rows is not None else [(None, (line, None))]:
            slug = f"{path} > {title}" if path else title
            if r is not None:
                slug += f"|{r}|{re.sub(r'\s+', ' ', row_text).strip()}"
            hits = [k for k in ((rel, f"{path} > {title}", r), (rel, title, r)) if k in PORTS]
            if hits:
                assert hits[0] not in used, f"port matches two tests, qualify it with the describe path: {hits[0]}"
                used.add(hits[0])
                b, inp, exp, note = PORTS[hits[0]]
                entries.append((rel, row_line, slug, b, True, FILES[rel], inp, exp, note))
                continue
            subject = f"{slug} | {row_text or ''}"
            reason = next((why for pat, why in OUTS.get(rel, []) if re.search(pat, subject)), None)
            assert reason, f"no port and no reason: {rel}:{row_line} {subject}"
            what = title if row_text is None else f"{title} [row {re.sub(r'\s+', ' ', row_text)[:140]}]"
            entries.append((rel, row_line, slug, f"Tests: {what}; not ported: {reason}", False, None, None, None,
                            None))
spec_files = sorted(str(f.relative_to(SRC)) for f in (SRC / "apps").rglob("*.spec.ts") if "node_modules" not in f.parts)
not_ported = {f: next((why for pat, why in RULED_OUT if re.search(pat, f)), None) for f in spec_files if f not in FILES}
assert all(not_ported.values()), f"spec files neither ported nor ruled out: {[f for f, w in not_ported.items() if not w]}"
missing = set(PORTS) - used
assert not missing, f"ported tests not found: {missing}"

old = {}  # key -> (id, {status, decision, tests})
if CATALOGUE.exists():
    key = cur = None
    for l in CATALOGUE.read_text(encoding="utf-8").splitlines():
        if l.startswith("# key: "):
            key = l[7:]
        elif l.startswith("- id: "):
            cur = old[key] = (l[6:], {})
        elif cur and (m := re.match(r"  (status|decision|tests): (.*)", l)):
            cur[1][m[1]] = m[2]
next_n = max([int(i[len(PREFIX):]) for i, _ in old.values()] + [0]) + 1

entries.sort(key=lambda e: (e[0], e[1], e[2]))
keys = [f"{e[0]}|{e[2]}" for e in entries]
assert len(set(keys)) == len(keys), "duplicate key"
rows, corpora, skipped = [], {}, []
for k, (rel, line, slug, behaviour, in_scope, file, inp, expect, note) in zip(keys, entries):
    if k in old:
        eid, keep = old[k]
    else:
        eid, keep = f"{PREFIX}{next_n:04d}", {}
        next_n += 1
    ref = f"{TAG} {rel}:{line}"
    rows += [f"# key: {k}", f"- id: {eid}", f"  behaviour: {json.dumps(behaviour, ensure_ascii=False)}",
             "  source: reference", f"  ref: {json.dumps(ref, ensure_ascii=False)}",
             f"  in_scope: {'true' if in_scope else 'false'}", f"  status: {keep.get('status', 'unmapped')}"]
    if "decision" in keep:
        rows.append(f"  decision: {keep['decision']}")
    rows.append(f"  tests: {keep.get('tests', '[]')}")
    if file:
        case = {"id": eid, "ref": ref, "input": inp, "expect": expect}
        if note:
            case["note"] = note
        corpora.setdefault(file, []).append(case)
    else:
        skipped.append((eid, ref, behaviour))

CATALOGUE.parent.mkdir(parents=True, exist_ok=True)
CATALOGUE.write_text(f"# Maintainerr cleanup behaviours ({TAG}). Generated by parity/extract/maintainerr_maintenance.py;\n"
                     "# status/decision/tests are kept on re-run, everything else is regenerated.\n\n"
                     + "\n".join(rows) + "\n", encoding="utf-8")
CORPUS.mkdir(parents=True, exist_ok=True)
for f, cs in sorted(corpora.items()):
    (CORPUS / f).write_text(json.dumps(cs, indent=1, ensure_ascii=False, sort_keys=True) + "\n", encoding="utf-8")
print(f"{len(entries)} entries: {sum(map(len, corpora.values()))} corpus cases, {len(skipped)} out of scope; "
      f"{len(corpora)} corpus files")
for rel in FILES:
    n_in = sum(1 for e in entries if e[0] == rel and e[4])
    n_all = sum(1 for e in entries if e[0] == rel)
    print(f"  {rel}: {n_in} in scope / {n_all}")
deferred = [f for f, why in not_ported.items() if why.startswith(DEFERRED)]
print(f"spec files: {len(FILES)} ported, {len(not_ported) - len(deferred)} ruled out, {len(deferred)} deferred")
