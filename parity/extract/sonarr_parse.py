"""Port Sonarr's parser test suite (src/NzbDrone.Core.Test/ParserTests/**) into
parity/corpus/reference/sonarr/parse/<Fixture>.json and parity/catalogue/parse.sonarr.yaml.

usage: python3 -I parity/extract/sonarr_parse.py [<sonarr repo at v4.0.20.3014>]

One catalogue entry per [TestCase] row, [TestCaseSource] row and [Test] method.
Rows that cannot be expressed as data get in_scope: false and no corpus case.

Case shape (shared with the Radarr port): input {name, facet, fn, ...},
expect = only what the test asserts, in Sonarr's own terms. Extra input keys:
  fn            reference function under test (Parser.ParseTitle, QualityParser.ParseQuality, ...)
  names         used instead of name when one test row loops over several inputs; every name
                must satisfy expect
  name_template + now_offset_days: {year} {month} {day} filled from today + N days (zero-padded)
  parsed, series_type: a hand-built ParsedEpisodeInfo / series type (no release name)
Extra expect keys (reference property, snake_case): languages (set equality),
languages_include (Contain), series_name_clean (CleanSeriesTitle(ParseSeriesName(name)) ==
CleanSeriesTitle(value)), series_title_info, subtitle_language, subtitle_info, language,
is_scene_title, normalized_episode_title, release_title, no_error, valid,
is_possible_special_episode, is_season_extra, is_split_episode, quality.is_unknown,
quality.source_detection_source / resolution_detection_source.

Comparison semantics: lists and strings are exact unless the case's "compare" field says otherwise.
Where the reference uses FluentAssertions BeEquivalentTo (lists: any order; strings: ignoring
case) "compare" maps each loosened expect key (dotted for nested) to "unordered" or "ignore_case",
and the note says the same in words; see LOOSE below. languages is always set equality.

IDs: an entry keeps its ID across re-runs. The key (stored as a YAML comment above the entry)
is ref path | method | hash of the row's raw args | occurrence of identical args, so inserted
rows upstream never renumber the rest. New keys get max ID + 1 in file/line order.
"""
import hashlib, json, re, sys
from pathlib import Path

TAG = "Sonarr@v4.0.20.3014"
SUITE = "src/NzbDrone.Core.Test/ParserTests"
ROOT = Path(__file__).resolve().parents[2]
CORPUS = ROOT / "parity/corpus/reference/sonarr/parse"
CATALOGUE = ROOT / "parity/catalogue/parse.sonarr.yaml"
PREFIX = "SON-PARSE-"

# (fixture, method) -> (note, compare) for asserts made with BeEquivalentTo on multi-element lists or
# strings. compare is the structured form the harness reads: {"<expect key, dotted for nested>": mode},
# mode "unordered" (list, any order) or "ignore_case" (string). Keys not listed compare exactly.
LOOSE = {
    ("MultiEpisodeParserFixture", "should_parse_multiple_episodes"):
        ("episodes compared ignoring order (BeEquivalentTo)", {"episodes": "unordered"}),
    ("PathParserFixture", "should_parse_multi_episode_from_path"):
        ("episodes compared ignoring order (BeEquivalentTo)", {"episodes": "unordered"}),
    ("AbsoluteEpisodeNumberParserFixture", "should_parse_multi_episode_absolute_numbers"):
        ("absolute_episodes compared ignoring order (BeEquivalentTo)", {"absolute_episodes": "unordered"}),
    ("UnicodeReleaseParserFixture", "should_parse_chinese_multiepisode_releases"):
        ("absolute_episodes compared ignoring order (BeEquivalentTo)", {"absolute_episodes": "unordered"}),
    ("ParserFixture", "should_parse_multiple_series_titles"):
        ("series_title_info.all_titles compared ignoring order (BeEquivalentTo)", {"series_title_info.all_titles": "unordered"}),
    ("LanguageParserFixture", "should_parse_title_and_tags"):
        ("subtitle_info.language_tags compared ignoring order, subtitle_info.title compared ignoring case (BeEquivalentTo)",
         {"subtitle_info.language_tags": "unordered", "subtitle_info.title": "ignore_case"}),
}


class Enum(str):
    """A C# member access such as Quality.WEBDL720p."""


class Named(tuple):
    """A named attribute argument (Description = ...); ignored."""


# ---------- C# literal scanner ----------

def skip(t, i):
    while True:
        m = re.compile(r"\s*(//[^\n]*)?").match(t, i)
        if m.end() == i:
            return i
        i = m.end()


def agnostic(path):
    """NzbDrone.Test.Common StringExtensions.AsOsAgnostic on a non-Windows host."""
    if len(path) > 2 and path[1] == ":":
        path = "/" + path.replace(":", "")
    return path.replace("\\", "/")


def value(t, i):
    i = skip(t, i)
    if t.startswith('@"', i):
        j, buf = i + 2, []
        while True:
            if t[j] == '"':
                if t[j + 1] == '"':
                    buf.append('"'); j += 2; continue
                j += 1; break
            buf.append(t[j]); j += 1
        v = "".join(buf)
    elif t[i] == '"':
        m = re.compile(r'"((?:[^"\\]|\\.)*)"').match(t, i)
        v = re.sub(r"\\(u[0-9a-fA-F]{4}|.)",
                   lambda e: chr(int(e.group(1)[1:], 16)) if len(e.group(1)) == 5
                   else {"t": "\t", "n": "\n", "r": "\r", "0": "\0"}.get(e.group(1), e.group(1)), m.group(1))
        j = m.end()
    elif m := re.compile(r"new\s*\w*\s*\[\s*(\d*)\s*\]\s*").match(t, i):
        j = m.end()
        if t[j] == "{":
            v, j = values(t, j + 1, "}")
        else:
            v = []
    elif m := re.compile(r"-?\d+(\.\d+)?[mMdDfF]?").match(t, i):
        s = m.group(0).rstrip("mMdDfF")
        v, j = (float(s) if "." in s else int(s)), m.end()
    else:
        m = re.compile(r"[A-Za-z_][\w.]*").match(t, i)
        if not m:
            raise ValueError(f"cannot parse C# at {t[i:i + 60]!r}")
        word, j = m.group(0), m.end()
        k = skip(t, j)
        if t.startswith("=", k) and not t.startswith("==", k):
            v, j = value(t, k + 1)
            return Named((word, v)), j
        v = {"true": True, "false": False, "null": None}.get(word, Enum(word))
    while m := re.compile(r"\s*\.(\w+)\(\)").match(t, j):  # postfix calls on literals
        if m.group(1) != "AsOsAgnostic":
            raise ValueError(f"unknown call {m.group(1)}")
        v, j = agnostic(v), m.end()
    return v, j


def values(t, i, close):
    out = []
    while True:
        i = skip(t, i)
        if t[i] == close:
            return out, i + 1
        if t[i] == ",":
            i += 1; continue
        v, i = value(t, i)
        if not isinstance(v, Named):
            out.append(v)


# ---------- fixture discovery ----------

TOKENS = re.compile(
    r"^[ \t]*(?:\[TestCase\((?P<tc>)|\[Test\](?P<test>)|\[TestCaseSource\(nameof\((?P<src>\w+)\)\)\]"
    r"|public void (?P<method>\w+)\((?P<params>[^)]*)\))", re.M)


def line_of(t, i):
    return t.count("\n", 0, i) + 1


def sources(t):
    out = {}
    for m in re.finditer(r"public static object\[\] (\w+) =\s*\{", t):
        rows, i = [], m.end()
        while True:
            i = skip(t, i)
            if t[i] == "}":
                break
            if t[i] == ",":
                i += 1; continue
            start = i
            v, i = value(t, i)
            rows.append((line_of(t, start), v))
        out[m.group(1)] = rows
    return out


def params_of(sig):
    out = []
    for p in filter(None, (x.strip() for x in sig.split(","))):
        is_params = p.startswith("params ")
        decl, _, default = p.partition("=")
        name = decl.split()[-1]
        out.append((name, value(default, 0)[0] if default.strip() else None, is_params))
    return out


def tests(path):
    """Yield (method, line, args, param dict, body) per row / [Test]."""
    t = path.read_text(encoding="utf-8-sig")
    src = sources(t)
    pending, is_test, marks = [], None, list(TOKENS.finditer(t))
    for n, m in enumerate(marks):
        if m.group("tc") is not None:
            args, _ = values(t, m.end(), ")")
            pending.append((line_of(t, m.start()), args))
        elif m.group("test") is not None:
            is_test = line_of(t, m.start())
        elif m.group("src"):
            pending.extend(src[m.group("src")])
        else:
            body = t[m.end(): marks[n + 1].start() if n + 1 < len(marks) else len(t)]
            params = params_of(m.group("params"))
            if not pending and is_test:
                pending = [(is_test, [])]
            for line, args in pending:
                p = {}
                for k, (name, default, is_params) in enumerate(params):
                    if not is_params and k >= len(args) and default is None:
                        raise ValueError(f"{path.name}:{line} {m.group('method')}: missing argument {name}")
                    p[name] = args[k:] if is_params else args[k] if k < len(args) else default
                if len(args) > len(params) and not (params and params[-1][2]):
                    raise ValueError(f"{path.name}:{line} {m.group('method')}: too many arguments")
                yield m.group("method"), line, args, p, body
            pending, is_test = [], None


# ---------- reference enums ----------

def load_enums(repo):
    q = (repo / "src/NzbDrone.Core/Qualities/Quality.cs").read_text(encoding="utf-8-sig")
    qualities = {m.group(1): {"name": m.group(2), "source": m.group(3), "resolution": int(m.group(4))}
                 for m in re.finditer(r'public static Quality (\w+)\b(?:(?!public static).)*?new Quality\(\d+,\s*"([^"]+)",\s*QualitySource\.(\w+),\s*(\d+)\)', q, re.S)}
    lang = (repo / "src/NzbDrone.Core/Languages/Language.cs").read_text(encoding="utf-8-sig")
    languages = dict(re.findall(r'public static Language (\w+) => new Language\(-?\d+, "([^"]+)"\)', lang))
    return qualities, languages


# ---------- expectations per fixture/method ----------

def build(fx, method, p, body, Q, L):
    """Return (input, expect, note) or a str (out of scope: why)."""
    def lang(x):  # Language.X enum or (Language)"name" cast (case-insensitive name)
        if isinstance(x, Enum):
            return L[x.split(".")[1]]
        return next(v for v in L.values() if v.lower() == x.lower())

    def qual(x):
        return dict(Q[x.split(".")[1]])

    def title(fn="Parser.ParseTitle", facet="series", **kw):
        return {"name": p.get("postTitle", p.get("title")), "facet": facet, "fn": fn, **kw}

    def episodes(n):  # EpisodeNumbers.SingleOrDefault().Should().Be(n)
        return [n] if n else []

    standard = lambda: {"title": p["title"], "season": p["seasonNumber"], "episodes": [p["episodeNumber"]],
                        "absolute_episodes": [], "full_season": False}
    pt, pq, pr = "Parser.ParseTitle", "QualityParser.ParseQuality", "Parser.ParseReleaseGroup"
    key = (fx, method)

    if fx == "AbsoluteEpisodeNumberParserFixture":
        if method == "should_parse_absolute_numbers":
            note = None if p["episodeNumber"] else "reference asserts EpisodeNumbers.SingleOrDefault() == 0"
            return title(facet="anime"), {"title": p["title"], "absolute_episodes": [p["absoluteEpisodeNumber"]], "season": p["seasonNumber"],
                                          "episodes": episodes(p["episodeNumber"]), "full_season": False}, note
        if method == "should_parse_absolute_specials":
            return title(facet="anime"), {"title": p["title"], "absolute_episodes": [p["absoluteEpisodeNumber"]], "season": 0,
                                          "episodes": [], "full_season": False, "is_special": True}, "reference asserts EpisodeNumbers.SingleOrDefault() == 0"
        if method == "should_parse_multi_episode_absolute_numbers":
            a, b = p["firstAbsoluteEpisodeNumber"], p["lastAbsoluteEpisodeNumber"]
            return title(facet="anime"), {"title": p["title"], "absolute_episodes": list(range(a, b + 1)), "full_season": False}, None
        if method == "should_parse_anime_season_packs":
            return title(facet="anime"), {"title": p["title"], "absolute_episodes": [], "full_season": True, "season": p["seasonNumber"]}, None
        if method == "should_handle_anime_recap_numbering":
            return title(facet="anime"), {"title": p["title"], "absolute_episodes": [], "special_absolute_episodes": [p["specialEpisodeNumber"]],
                                          "full_season": False}, None
        if method == "should_handle_ambiguously_named_anime_files_in_batch_release":
            name = "/C/Test/" + p["releaseName"] + "/" + p["filename"]  # Path.Combine("C:\Test".AsOsAgnostic(), ...)
            return {"name": name, "facet": "anime", "fn": "Parser.ParsePath"}, {
                "title": p["title"], "absolute_episodes": [p["absoluteEpisodeNumber"]], "season": 0, "episodes": [], "full_season": False}, None
        if method == "should_parse_absolute_followed_by_standard_as_standard":
            return {"name": p["releaseName"], "facet": "anime", "fn": pt}, {
                "title": p["title"], "season": p["seasonNumber"], "episodes": [p["episodeNumber"]],
                "special_absolute_episodes": [p["specialEpisodeNumber"]], "absolute_episodes": [], "full_season": False}, None
    if fx == "AnimeMetadataParserFixture":
        e = {"release_group": p["subGroup"]} if "subGroup" in p else {}
        return title(facet="anime"), {**e, "release_hash": p["hash"]}, None
    if fx == "AnimeVersionFixture":
        return title(pq, "unknown"), {"quality": {"revision": {"version": p["version"]}}}, None
    if fx == "CrapParserFixture":
        if method == "should_not_parse_md5":
            names, h = [], "CRAPPY TEST SEED"
            for _ in range(100):  # the test's own deterministic MD5 chain
                h = hashlib.md5(h.encode()).hexdigest().upper()
                names.append(h)
            return {"names": names, "facet": "unknown", "fn": pt}, {"should_parse": False}, "100 chained MD5 hex strings from seed \"CRAPPY TEST SEED\", as the test builds them"
        if method == "should_not_parse_random":
            return "random input (System.Random, unseeded); belongs in a property test"
        name = p.get("title", p.get("fileName"))
        ok = method == "should_parse_unknown_formats_without_error"
        return {"name": name, "facet": "unknown", "fn": pt}, {"should_parse": ok}, None
    if fx == "DailyEpisodeParserFixture":
        if method in ("should_parse_daily_episode", "should_parse_daily_episode_with_multiple_parts"):
            e = {"title": p["title"], "air_date": f'{p["year"]:04d}-{p["month"]:02d}-{p["day"]:02d}', "episodes": [],
                 "absolute_episodes": [], "full_season": False}
            if "part" in p:
                e["daily_part"] = p["part"]
            return title(), e, None
        if method == "should_not_accept_ancient_daily_series":
            name = p["title"].replace("{year}", "1950").replace("{month}", "10").replace("{day}", "14")
            return {"name": name, "facet": "series", "fn": pt}, {"should_parse": False}, None
        if method == "should_not_accept_future_dates":
            return {"name_template": p["title"], "now_offset_days": 2, "facet": "series", "fn": pt}, {"should_parse": False}, \
                "date is DateTime.Now + 2 days, month and day zero-padded"
        if method == "should_fail_if_episode_is_far_in_future":
            return {"name_template": "{year}.{month}.{day} - A Talk Show - HD TV.mkv", "now_offset_days": 2, "facet": "series", "fn": pt}, \
                {"should_parse": False}, "reference formats DateTime.Now + 2 days as yyyy.MM.dd"
        if method == "should_not_parse_ambiguous_daily_episode":
            return title(), {"should_parse": False}, None
    if fx == "ExtendedQualityParserRegex":
        k = "real" if "reality" in p else "version"
        return title(pq, "unknown"), {"quality": {"revision": {k: p.get("reality", p.get("version"))}}}, None
    if fx == "HashedReleaseFixture":
        return {"name": p["path"], "facet": "series", "fn": "Parser.ParsePath"}, {
            "title": p["title"], "quality": qual(p["quality"]), "release_group": p["releaseGroup"]}, None
    if fx == "IsPossibleSpecialEpisodeFixture":
        if method == "should_not_treat_files_without_a_series_title_as_a_special":
            return {"facet": "series", "fn": "ParsedEpisodeInfo.IsPossibleSpecialEpisode",
                    "parsed": {"episodes": [7], "season": 1, "title": ""}}, {"is_possible_special_episode": False}, None
        if method == "should_return_true_when_episode_numbers_is_empty":
            return {"facet": "series", "fn": "ParsedEpisodeInfo.IsPossibleSpecialEpisode",
                    "parsed": {"season": 1, "title": ""}}, {"is_possible_special_episode": True}, None
        return title(), {"is_possible_special_episode": "should_be_true" in method}, None
    if fx == "IsoLanguagesFixture":
        m = re.search(r"Language\.Should\(\)\.Be\(Language\.(\w+)\)", body)
        return {"name": p["isoCode"], "facet": "unknown", "fn": "IsoLanguages.Find"}, {"language": L[m.group(1)] if m else None}, None
    if fx == "LanguageParserFixture":
        inp = title("LanguageParser.ParseLanguages", "unknown")
        if method in ("should_parse_subtitle_language_unknown", "should_parse_subtitle_language_english"):
            lg = re.search(r"Be\(Language\.(\w+)\)", body).group(1)
            return {"name": p["fileName"], "facet": "unknown", "fn": "LanguageParser.ParseSubtitleLanguage"}, {"subtitle_language": L[lg]}, None
        if method == "should_parse_title_and_tags":
            return {**inp, "fn": "LanguageParser.ParseSubtitleLanguageInformation"}, {"subtitle_info": {
                "language_tags": p["expectedTags"], "title": p["expectedTitle"], "language": lang(p["expectedLanguage"])}}, None
        if method == "should_not_parse_false_title":
            return {**inp, "fn": "LanguageParser.ParseSubtitleLanguageInformation"}, {"subtitle_info": {
                "language": "Unknown", "language_tags": [], "raw_title": None}}, None
        found = [L[x] for x in re.findall(r"Language\.(\w+)", body)]
        if "Parser.Parser.ParseTitle" in body:
            count = int(re.search(r"Languages\.Count\.Should\(\)\.Be\((\d+)\)", body).group(1))
            assert count == len(set(found)), (method, count, found)
            return {**inp, "fn": pt, "facet": "series"}, {"languages": found}, None
        if "BeEquivalentTo" in body:
            return inp, {"languages": found}, None
        return inp, {"languages_include": found}, None
    if fx == "MiniSeriesEpisodeParserFixture":
        note = None
        if "airdate" in p:
            note = "air date in the row is not asserted (commented out in the reference test)"
        return title(), {"title": p["title"], "season": 1, "episodes": [p["episodeNumber"]], "absolute_episodes": [], "full_season": False}, note
    if fx == "MultiEpisodeParserFixture":
        return title(), {"title": p["title"], "season": p["season"], "episodes": p["episodes"], "absolute_episodes": [], "full_season": False}, None
    if fx == "NormalizeEpisodeTitleFixture":
        return {"name": p["input"], "facet": "unknown", "fn": "Parser.NormalizeEpisodeTitle"}, {"normalized_episode_title": p["expected"]}, None
    if fx == "NormalizeSeriesTitleFixture":
        clean = lambda names, want: ({"names": names, "facet": "unknown", "fn": "Parser.CleanSeriesTitle"}, {"clean_title": want}, None)
        one = lambda name, want: ({"name": name, "facet": "unknown", "fn": "Parser.CleanSeriesTitle"}, {"clean_title": want}, None)
        w = p.get("word")
        if method == "should_remove_common_words_from_middle_of_title":
            return clean([f.format(w) for f in ("word.{0}.word", "word {0} word", "word-{0}-word")], "wordword")
        if method == "should_not_remove_common_words_from_end_of_title":
            return clean([f.format(w) for f in ("word.word.{0}", "word-word-{0}", "word-word {0}")], "wordword" + w.lower())
        if method == "should_remove_a_from_middle_of_title":
            return clean([f.format("a") for f in ("word.{0}.word", "word {0} word", "word-{0}-word")], "wordword")
        if method == "should_not_remove_common_words_in_the_middle_of_word":
            fs = ("word.{0}word", "word {0}word", "word-{0}word", "word{0}.word", "word{0}-word", "word{0}-word")
            return clean([f.format(w) for f in fs], "word" + w.lower() + "word")
        if method == "should_not_clean_word_from_beginning_of_string":
            return clean([f.format(w) for f in ("{0}.word.word", "{0}-word-word", "{0} word word")], w + "wordword")
        if method == "should_not_clean_trailing_a":
            return one("Series Title A", "seriestitlea")
        a = list(p.values())
        return one(a[0], a[1])
    if fx == "ParserFixture":
        if method == "should_parse_series_name":
            return {"name": p["postTitle"], "facet": "series", "fn": "Parser.ParseSeriesName"}, {"series_name_clean": p["title"]}, None
        if method == "should_parse_series_title_info":
            return title(), {"series_title_info": {"title_without_year": p["titleWithoutYear"], "year": p["year"]}}, None
        if method == "should_remove_accents_from_title":
            return {"name": "Seri\u00e0es", "facet": "unknown", "fn": "Parser.CleanSeriesTitle"}, {"clean_title": "seriaes"}, None
        if method == "should_clean_up_invalid_path_characters":
            return title(), {"no_error": True}, "the test only asserts that parsing does not throw"
        if method == "should_remove_request_info_from_title":
            return title(), {"title": p["title"]}, None
        if method == "should_parse_quality_from_extension":
            return title(), {"quality": {"is_unknown": False, "source_detection_source": "Extension", "resolution_detection_source": "Extension"}}, None
        if method == "should_parse_releasetitle":
            return {"name": p["path"], "facet": "series", "fn": pt}, {"release_title": p["releaseTitle"]}, None
        if method == "should_parse_multiple_series_titles":
            return title(), {"series_title_info": {"all_titles": p["titles"]}}, None
        if method == "should_parse_language_after_parsing_title":
            return title(), {"languages": [lang(p["expectedLanguage"])]}, None
    if fx == "PathParserFixture":
        e = {"season": p["season"]}
        if "title" in p:
            e = {"title": p["title"], **e, "episodes": p["episodes"]}
        else:
            e["episodes"] = [p["episode"]]
        return {"name": agnostic(p["path"]), "facet": "series", "fn": "Parser.ParsePath"}, {**e, "absolute_episodes": [], "full_season": False}, None
    if fx == "QualityParserFixture":
        inp = title(pq, "unknown")
        if m := re.search(r"ParseAndVerifyQuality\(title, Quality\.(\w+), proper\)", body):
            return inp, {"quality": {**qual("Quality." + m.group(1)), "revision": {"version": 2 if p["proper"] else 1}}}, None
        if method == "parsing_our_own_quality_enum_name":
            q = qual(p["quality"])
            return {"name": f'My series S01E01 [{q["name"]}]', "facet": "unknown", "fn": pq}, {"quality": q}, None
        if method == "should_parse_quality_from_other_source":
            names = ["My series S01E01 " + p["qualityString"].replace(" ", c) for c in "-. _"]
            return {"names": names, "facet": "unknown", "fn": pq}, {"quality": {**qual(p["quality"]), "revision": {"version": 1}}}, None
        det = re.findall(r"(Source|Resolution)DetectionSource\.Should\(\)\.Be\(QualityDetectionSource\.(\w+)\)", body)
        if det:
            return inp, {"quality": {f"{k.lower()}_detection_source": v for k, v in det}}, None
        if method == "should_be_able_to_parse_repack":
            return inp, {"quality": {"revision": {"version": p["version"], "is_repack": p["isRepack"]}}}, None
        if method == "should_be_able_to_parse_anime_version":
            return inp, {"quality": {"revision": {"version": p["version"]}}}, None
    if fx == "ReleaseGroupParserFixture":
        if method == "should_not_include_extension_in_release_group":
            return {"name": r"C:\Test\Doctor.Series.2005.s01e01.internal.bdrip.x264-archivist.mkv", "facet": "unknown",
                    "fn": "Parser.ParsePath"}, {"release_group": "archivist"}, "path passed as written (no AsOsAgnostic)"
        return title(pr, "unknown"), {"release_group": p.get("expected")}, None
    if fx == "SceneCheckerFixture":
        return title("SceneChecker.IsSceneTitle", "unknown"), {"is_scene_title": "true" in method}, None
    if fx == "SeasonParserFixture":
        if method == "should_not_parse_season_folders":
            return {"name": "Season 3", "facet": "series", "fn": pt}, {"should_parse": False}, None
        e = {"season": p.get("season", p.get("firstSeason")), "title": p["title"], "episodes": [], "absolute_episodes": []}
        if method in ("should_parse_full_season_release",):
            e["full_season"] = True
        elif method in ("should_parse_season_extras", "should_parse_season_subpack"):
            e.update(full_season=True, is_season_extra=True)
        elif method == "should_parse_partial_season_release":
            e.update(full_season=False, is_partial_season=True, season_part=p["seasonPart"])
        elif method == "should_parse_multi_season_release":
            e.update(full_season=True, is_partial_season=False, multi_season=True)
        return title(), e, None
    if fx == "SeriesTitleInfoFixture":
        name = re.search(r'const string title = "([^"]+)"', body).group(1)
        info = {
            "should_have_year_zero_when_title_doesnt_have_a_year": {"year": 0},
            "should_have_same_title_for_title_and_title_without_year_when_title_doesnt_have_a_year": {"title_equals_title_without_year": True},
            "should_have_year_when_title_has_a_year": {"year": 2004},
            "should_have_year_in_title_when_title_has_a_year": {"title": "Series Title 2004"},
            "should_title_without_year_should_not_contain_year": {"title_without_year": "Series Title"},
        }[method]
        return {"name": name, "facet": "series", "fn": pt}, {"series_title_info": info}, None
    if fx == "SingleEpisodeParserFixture":
        e = standard()
        if method == "should_parse_decimal_number_as_special":
            e["is_special"] = True
        if method == "should_parse_split_episode":
            e["is_split_episode"] = True
        return title(), e, None
    if fx == "UnicodeReleaseParserFixture":
        name = p["postTitle"]
        if "XmlCleaner.ReplaceUnicode" in body:  # the test's own pre-step
            name = re.sub("[^\x09\x0A\x0D\u0020-\ud7ff\ue000-\ufffd]", "", name)
        inp = {"name": name, "facet": "anime", "fn": pt}
        if method == "should_parse_chinese_multiepisode_releases":
            return inp, {"release_group": p["subgroup"], "absolute_episodes": p["absoluteEpisodeNumbers"], "title": p["title"], "full_season": False}, None
        if method == "should_parse_gm_team_releases_and_files":
            return inp, {"absolute_episodes": [p["absoluteEpisodeNumber"]], "title": p["title"], "full_season": False, "release_group": "GM-Team"}, None
        if method == "should_parse_unicode_digits":
            return inp, {"title": p["title"], "absolute_episodes": [p["absoluteEpisodeNumber"]], "special_absolute_episodes": [], "full_season": False}, None
        return inp, {"release_group": p["subgroup"], "absolute_episodes": [p["absoluteEpisodeNumber"]], "title": p["title"], "full_season": False}, None
    if fx == "UrlFixture":
        if method == "should_not_parse_url_in_name":
            return {"name": p["postTitle"], "facet": "series", "fn": "Parser.ParseSeriesName"}, {"series_name_clean": p["title"]}, None
        return title(pr, "unknown"), {"release_group": p["expected"]}, None
    if fx == "ValidateParsedEpisodeInfoFixture":
        if method == "should_not_log_warning_if_warnIfInvalid_is_false":
            return "asserts only that no warning is logged"
        daily = "is_daily" in method
        inp = {"facet": "series", "fn": "ValidateParsedEpisodeInfo.ValidateForSeriesType",
               "parsed": {"air_date": "2018-05-21" if daily else None},
               "series_type": "Daily" if "for_daily_series" in method else "Standard"}
        return inp, {"valid": "return_true" in method}, "other ParsedEpisodeInfo fields are NBuilder filler, not part of the test"
    if fx in ("GetSeriesFixture", "MapFixture", "GetEpisodesFixture"):
        return "ParsingService lookup verified through mocked series/episode/scene-mapping services"
    raise KeyError(key)


# ---------- output ----------

def describe(inp, expect):
    if "names" in inp:
        subj = f'each of {len(inp["names"])} inputs like "{inp["names"][0]}"'
    elif "name_template" in inp:
        subj = f'"{inp["name_template"]}" dated today+{inp["now_offset_days"]}'
    elif "name" in inp:
        subj = json.dumps(inp["name"], ensure_ascii=False)
    else:
        subj = json.dumps({k: v for k, v in inp.items() if k not in ("fn", "facet")}, ensure_ascii=False)
    if expect == {"should_parse": False}:
        return f'{inp["fn"]} of {subj} returns no result'
    parts = ", ".join(f"{k} {json.dumps(v, ensure_ascii=False)}" for k, v in expect.items())
    return f'{inp["fn"]} of {subj} gives {parts}'


def read_catalogue():
    """key -> (id, preserved lines for status/decision/tests/in_scope)."""
    out, key, cur = {}, None, None
    if not CATALOGUE.exists():
        return out
    for line in CATALOGUE.read_text(encoding="utf-8").splitlines():
        if line.startswith("# key: "):
            key = line[len("# key: "):]
        elif line.startswith("- id: "):
            cur = out[key] = [line[len("- id: "):], {}]
        elif cur and (m := re.match(r"  (status|decision|tests): (.*)", line)):
            cur[1][m.group(1)] = m.group(2)
    return out


def main(repo):
    repo = Path(repo).expanduser()
    Q, L = load_enums(repo)
    old = read_catalogue()
    next_n = max([int(v[0][len(PREFIX):]) for v in old.values()] + [0]) + 1
    rows, corpora, skipped = [], {}, []
    for path in sorted((repo / SUITE).rglob("*.cs")):
        rel = path.relative_to(repo).as_posix()
        fx, seen = path.stem, {}
        for method, line, args, p, body in sorted(tests(path), key=lambda r: r[1]):  # line order
            raw = json.dumps(args, ensure_ascii=False, sort_keys=True)
            h = hashlib.sha1(f"{method}|{raw}".encode()).hexdigest()[:12]
            seen[h] = seen.get(h, -1) + 1
            key = f"{rel}|{method}|{h}|{seen[h]}"
            if key in old:
                eid, keep = old[key]
            else:
                eid, keep = f"{PREFIX}{next_n:04d}", {}
                next_n += 1
            ref = f"{TAG} {rel}:{line}"
            built = build(fx, method, p, body, Q, L)
            if isinstance(built, str):
                arg = f" ({', '.join(f'{k}={v}' for k, v in p.items())})" if p else ""
                behaviour = f"{fx}.{method}{arg}: not data, {built}"
                skipped.append((eid, ref, built))
                in_scope = False
            else:
                inp, expect, note = built
                loose_note, compare = LOOSE.get((fx, method), (None, None))
                note = "; ".join(filter(None, (note, loose_note)))
                case = {"id": eid, "ref": ref, "input": inp, "expect": expect}
                if note:
                    case["note"] = note
                if compare:
                    case["compare"] = compare
                corpora.setdefault(fx, []).append(case)
                behaviour, in_scope = describe(inp, expect), True
            rows.append((key, eid, behaviour, ref, in_scope, keep))
    CORPUS.mkdir(parents=True, exist_ok=True)
    for fx, cases in corpora.items():
        (CORPUS / f"{fx}.json").write_text(json.dumps(cases, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    out = [f"# Sonarr parser suite ({TAG} {SUITE}). Generated by parity/extract/sonarr_parse.py;",
           "# status/decision/tests are kept on re-run, everything else is regenerated.", ""]
    for key, eid, behaviour, ref, in_scope, keep in rows:
        out += [f"# key: {key}", f"- id: {eid}", f"  behaviour: {json.dumps(behaviour, ensure_ascii=False)}",
                "  source: reference", f"  ref: {json.dumps(ref, ensure_ascii=False)}",
                f"  in_scope: {'true' if in_scope else 'false'}", f"  status: {keep.get('status', 'unmapped')}"]
        if "decision" in keep:
            out.append(f"  decision: {keep['decision']}")
        out.append(f"  tests: {keep.get('tests', '[]')}")
    CATALOGUE.parent.mkdir(parents=True, exist_ok=True)
    CATALOGUE.write_text("\n".join(out) + "\n", encoding="utf-8")
    n_cases = sum(len(c) for c in corpora.values())
    print(f"{len(rows)} entries ({n_cases} in scope, {len(skipped)} out of scope), {len(corpora)} corpus files")
    for eid, ref, why in skipped:
        print(f"  skipped {eid} {ref}: {why}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "~/PycharmProjects/arr-reference/sonarr@v4.0.20.3014")
