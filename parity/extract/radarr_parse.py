"""Port Radarr's parser test suite (src/NzbDrone.Core.Test/ParserTests/**) into
the parity corpus and catalogue (notes/11-parity-net-design.md, phase 2).

usage: python3 -I parity/extract/radarr_parse.py [radarr checkout] [--selftest]

Writes parity/corpus/reference/radarr/parse/<Fixture>.json (one per fixture
with data cases) and parity/catalogue/parse.radarr.yaml. Every [TestCase] row,
TestCaseSource row and [Test] gets one catalogue entry; rows that cannot be
data are in_scope: false with no corpus case. An unknown fixture or method is
an error, so a newer tag can't drop rows silently.

IDs are stable: each entry carries a `# key:` comment (file, method, input,
occurrence of that input within the method). A re-run reuses the ID of a
matching key and keeps its status, tests and decision; new rows get the next
free number; entries whose row vanished are kept at the end, never reused.

Input conventions (beyond name/facet/ctx): `fn` names the reference function
the test calls (as in the Sonarr port); `is_dir` is ParseMovieTitle's second argument; `names` replaces
`name` when one test asserts the same thing over several generated inputs (all
must hold); `args` holds further arguments the test passes explicitly.
A case whose assert is looser than exact equality carries "compare" (see COMPARE).
"""
import hashlib, json, os, re, sys
from pathlib import Path

TAG = "Radarr@v6.4.4.10685"
DEFAULT_SRC = Path.home() / "PycharmProjects/arr-reference/radarr@v6.4.4.10685"
TESTS = "src/NzbDrone.Core.Test/ParserTests"
ROOT = Path(__file__).resolve().parents[2]
CORPUS = ROOT / "parity/corpus/reference/radarr/parse"
CATALOGUE = ROOT / "parity/catalogue/parse.radarr.yaml"
PREFIX = "RAD-PARSE-"


# ---------------------------------------------------------------- C# lexing

def read_string(t, i):
    """String/char literal at t[i] -> (value, end). Handles "..", @"..", '..'."""
    if t.startswith('@"', i):
        j, out = i + 2, []
        while True:
            if t[j] == '"':
                if t.startswith('""', j):
                    out.append('"'); j += 2; continue
                return "".join(out), j + 1
            out.append(t[j]); j += 1
    q, j, out = t[i], i + 1, []
    while t[j] != q:
        if t[j] == "\\":
            e = t[j + 1]
            if e == "u":
                out.append(chr(int(t[j + 2:j + 6], 16))); j += 6; continue
            out.append({"n": "\n", "t": "\t", "r": "\r", "0": "\0"}.get(e, e)); j += 2; continue
        out.append(t[j]); j += 1
    return "".join(out), j + 1


def strip_comments(t):
    """Blank out // and /* */ comments, keeping newlines so line numbers hold."""
    out, i = [], 0
    while i < len(t):
        if t.startswith("//", i):
            j = t.find("\n", i)
            j = len(t) if j < 0 else j
            out.append(" " * (j - i)); i = j
        elif t.startswith("/*", i):
            j = t.index("*/", i) + 2
            out.append(re.sub(r"[^\n]", " ", t[i:j])); i = j
        elif t[i] in "\"'" or t.startswith('@"', i):
            _, j = read_string(t, i)
            out.append(t[i:j]); i = j
        else:
            out.append(t[i]); i += 1
    return "".join(out)


class Enum(str):
    """A C# identifier argument, e.g. Quality.WEBDL720p."""


class Kept(str):
    """A status/decision/tests value read back verbatim; ratchet-owned, written back unchanged."""


def as_os_agnostic(p):  # NzbDrone.Test.Common StringExtensions.AsOsAgnostic, POSIX branch
    if len(p) > 2 and p[1] == ":":
        p = "/" + p.replace(":", "")
    return p.replace("\\", "/")


WS = re.compile(r"\s*")
NUM = re.compile(r"-?\d+(\.\d+)?[fFdDmM]?")
IDENT = re.compile(r"[A-Za-z_][\w.]*")
NEW = re.compile(r"new\s*(?:\w+\s*)?\[\s*(\d*)\s*\]\s*")


def parse_value(t, i):
    i = WS.match(t, i).end()
    c = t[i]
    if c in "\"'" or t.startswith('@"', i):
        v, i = read_string(t, i)
        if m := re.compile(r"\s*\.AsOsAgnostic\(\)").match(t, i):
            v, i = as_os_agnostic(v), m.end()
        return v, i
    if m := NEW.match(t, i):
        i = m.end()
        if t[i] != "{":
            return [None] * int(m.group(1) or 0), i
        c = "{"
    if c == "{":
        return parse_list(t, i + 1, "}")
    if m := NUM.match(t, i):
        s = m.group(0).rstrip("fFdDmM")
        return (float(s) if m.group(1) else int(s)), m.end()
    if m := IDENT.match(t, i):
        w = m.group(0)
        return {"true": True, "false": False, "null": None}.get(w, Enum(w)), m.end()
    raise ValueError(f"unparsed C# value at {t[i:i + 40]!r}")


def parse_list(t, i, close):
    """Comma-separated values up to `close`. Returns ((positional, named), end)
    for argument lists ")" and the plain list for initialisers "}"."""
    pos, named = [], {}
    while True:
        i = WS.match(t, i).end()
        if t[i] == close:
            break
        if (m := re.compile(r"(\w+)\s*=(?!=)").match(t, i)) and close == ")":
            named[m.group(1)], i = parse_value(t, m.end())
        else:
            v, i = parse_value(t, i)
            pos.append(v)
        i = WS.match(t, i).end()
        if t[i] == ",":
            i += 1
    return ((pos, named) if close == ")" else pos), i + 1


def matching_brace(t, i):
    """t[i] == '{' -> index after its matching '}' (string aware)."""
    depth = 0
    while True:
        c = t[i]
        if c in "\"'" or t.startswith('@"', i):
            _, i = read_string(t, i); continue
        depth += (c == "{") - (c == "}")
        i += 1
        if depth == 0:
            return i


SCAN = re.compile(r"\[(TestCaseSource|TestCase|Test)\b|public\s+(?:static\s+)?void\s+(\w+)\s*\(|\bclass\s+(\w+)")


def scan(text):
    """-> fixture name, [method], sources. A method: name, line, params, body,
    rows [(line, args, named)], tests [line], source name."""
    t = strip_comments(text)
    line = lambda p: t.count("\n", 0, p) + 1
    fixture, methods, pending, done = None, [], {"rows": [], "tests": [], "source": None}, 0
    for m in SCAN.finditer(t):
        if m.start() < done:
            continue
        kind, meth, cls = m.groups()
        if cls:
            fixture = fixture or cls
        elif kind == "TestCase":
            (args, named), done = parse_list(t, t.index("(", m.end()) + 1, ")")
            pending["rows"].append((line(m.start()), args, named))
        elif kind == "TestCaseSource":
            sm = re.compile(r'\(\s*(?:nameof\((\w+)\)|"(\w+)")\s*\)').match(t, m.end())
            pending["source"], done = sm.group(1) or sm.group(2), sm.end()
        elif kind == "Test":
            pending["tests"].append(line(m.start()))
        elif meth:
            close, depth = m.end(), 1
            while depth:
                depth += (t[close] == "(") - (t[close] == ")")
                close += 1
            params = t[m.end():close]
            b = t.index("{", close)
            done = matching_brace(t, b)
            methods.append(dict(pending, name=meth, line=line(m.start()), params=params, body=t[b:done]))
            pending = {"rows": [], "tests": [], "source": None}
    return fixture, methods, t


def source_rows(t, name):
    """Rows of `public static object[] <name> = { new object[] {..}, .. };`."""
    start = t.index("{", re.search(rf"object\[\]\s+{name}\s*=", t).end())
    end = matching_brace(t, start)
    rows = []
    for m in re.finditer(r"new\s+object\[\]\s*", t[start:end]):
        rows.append((t.count("\n", 0, start + m.start()) + 1, parse_list(t, start + m.end() + 1, "}")[0], {}))
    return rows


# ------------------------------------------------- reference enum name tables

def enum_names(src):
    core = src / "src/NzbDrone.Core"
    lang = dict(re.findall(r'public static Language (\w+) => new Language\(-?\d+, "([^"]+)"\)',
                           (core / "Languages/Language.cs").read_text(encoding="utf-8")))
    qual = dict(re.findall(r'public static Quality (\w+) => new Quality\(\d+, "([^"]+)"',
                           (core / "Qualities/Quality.cs").read_text(encoding="utf-8")))
    res = {k: int(v) for k, v in re.findall(r"\b(R\d+p) = (\d+)", (core / "Parser/QualityParser.cs").read_text(encoding="utf-8"))}
    return lang, qual, res


# ----------------------------------------------------------- case builders

def nest(d, dotted, v):
    *head, last = dotted.split(".")
    for k in head:
        d = d.setdefault(k, {})
    d[last] = v


CALLS = [  # (body substring, reference call), first hit wins
    ("ParseMoviePath(", "Parser.ParseMoviePath"),
    ("ParseMovieTitle(", "Parser.ParseMovieTitle"),
    ("QualityParser.ParseQuality(", "QualityParser.ParseQuality"),
    ("ParseAndVerifyQuality(", "QualityParser.ParseQuality"),
    ("ReleaseGroupParser.ParseReleaseGroup(", "ReleaseGroupParser.ParseReleaseGroup"),
    ("CleanMovieTitle(", "Parser.CleanMovieTitle"),
    ("ParseSubtitleLanguageInformation(", "LanguageParser.ParseSubtitleLanguageInformation"),
    ("ParseSubtitleLanguage(", "LanguageParser.ParseSubtitleLanguage"),
    ("LanguageParser.ParseLanguages(", "LanguageParser.ParseLanguages"),
    ("IsoLanguages.Find(", "IsoLanguages.Find"),
    ("NormalizeImdbId(", "Parser.NormalizeImdbId"),
    ("ToUrlSlug(", "Parser.ToUrlSlug"),
    ("IsSceneTitle(", "SceneChecker.IsSceneTitle"),
    ("GetSceneTitle(", "SceneChecker.GetSceneTitle"),
]


def call_of(body):
    return next(c for s, c in CALLS if s in body)


# (fixture, method) -> (argument names, fixed expectations, fixed input).
# "name" is the title argument; dotted names nest under expect; "input.x" goes
# to the input; "_" is an argument the test ignores.
ARGS = {
    ("AnimeVersionFixture", "should_be_able_to_parse_repack"): (["name", "quality.revision.version"], {}, {}),
    ("CrapParserFixture", "should_not_parse_crap"): (["name"], {"should_parse": False}, {}),
    ("CrapParserFixture", "should_not_parse_file_name_without_proper_spacing"): (["name"], {"should_parse": False}, {}),
    ("CrapParserFixture", "should_not_parse_invalid_release_name"): (["name"], {"should_parse": False}, {}),
    ("EditionParserFixture", "should_parse_edition"): (["name", "edition"], {}, {}),
    ("EditionParserFixture", "should_not_parse_edition"): (["name"], {"edition": ""}, {}),
    ("ExtendedQualityParserRegex", "should_parse_reality_from_title"): (["name", "quality.revision.real"], {}, {}),
    ("ExtendedQualityParserRegex", "should_parse_version_from_title"): (["name", "quality.revision.version"], {}, {}),
    ("ExtendedQualityParserRegex", "should_parse_ultrahd_from_title"): (["name", "_"], {"quality.resolution": 2160}, {}),
    ("HashedReleaseFixture", "should_properly_parse_hashed_releases"): (["name", "title", "quality.name", "release_group"], {}, {}),
    ("LanguageParserFixture", "should_parse_title_and_tags"): (["name", "subtitle_info.language_tags", "subtitle_info.title", "subtitle_info.language"], {}, {}),
    ("LanguageParserFixture", "should_not_parse_false_title"): (["name"], {"subtitle_info.language": "Unknown", "subtitle_info.language_tags": [], "subtitle_info.raw_title": None}, {}),
    ("NormalizeTitleFixture", "should_normalize_series_title"): (["name", "clean_title"], {}, {}),
    ("NormalizeTitleFixture", "should_remove_special_characters_and_casing"): (["name", "clean_title"], {}, {}),
    ("NormalizeTitleFixture", "should_not_remove_from_the_beginning_of_the_title"): (["name", "clean_title"], {}, {}),
    ("ParserFixture", "should_parse_movie_title"): (["name", "title"], {}, {}),
    ("ParserFixture", "should_parse_anime_movie_title"): (["name", "title", "release_group", "year"], {}, {}),
    ("ParserFixture", "should_parse_anime_movie_title_without_year"): (["name", "title", "release_group"], {}, {}),
    ("ParserFixture", "should_parse_german_movie"): (["name", "title", "edition", "year"], {}, {}),
    ("ParserFixture", "should_parse_movie_alternative_titles"): (["name", "movie_titles"], {}, {}),
    ("ParserFixture", "should_parse_movie_folder_name"): (["name", "title"], {}, {}),
    ("ParserFixture", "should_parse_movie_year"): (["name", "year"], {}, {}),
    ("ParserFixture", "should_parse_tmdb_id"): (["name", "tmdb_id"], {}, {}),
    ("ParserFixture", "should_not_parse_wrong_language_in_title"): (["name"], {"languages": ["Unknown"]}, {}),
    ("ParserFixture", "should_parse_hardcoded_subs"): (["name", "hardcoded_subs"], {}, {}),
    ("ParserFixture", "should_parse_imdb_in_title"): (["name", "imdb_id"], {}, {}),
    ("ParserFixture", "should_normalize_imdbid"): (["name", "imdb_id"], {}, {}),
    ("QualityParserFixture", "should_be_able_to_parse_repack"): (["name", "quality.revision.is_repack", "quality.revision.version"], {}, {}),
    ("ReleaseGroupParserFixture", "should_not_parse_anime_hash_as_release_group"): (["name"], {"release_group": None}, {}),
    ("SceneCheckerFixture", "should_return_true_for_scene_names"): (["name"], {"is_scene_title": True}, {}),
    ("SceneCheckerFixture", "should_return_false_for_non_scene_names"): (["name"], {"is_scene_title": False}, {}),
    ("SceneCheckerFixture", "should_correctly_parse_scene_names"): (["name", "scene_title"], {}, {}),
    ("SlugParserFixture", "should_replace_accents"): (["name", "slug"], {}, {}),
    ("SlugParserFixture", "should_replace_special_characters"): (["name"], {"slug": "testresult"}, {}),
    ("SlugParserFixture", "should_lowercase_capitals"): (["name"], {"slug": "this-is-a-mixed-case-sensitive-value"}, {}),
    ("SlugParserFixture", "should_trim_trailing_dashes_and_underscores"): (["name"], {"slug": "test"}, {}),
    ("SlugParserFixture", "should_replace_spaces_with_dash"): (["name"], {"slug": "test-result"}, {}),
    ("SlugParserFixture", "should_replace_double_occurence"): (["name", "slug"], {}, {}),
    ("SlugParserFixture", "should_replace_special_characters_with_dash_when_enabled"): (["name"], {"slug": "test-result"}, {"args": {"invalid_dash_replacement": True}}),
    ("SlugParserFixture", "should__not_replace_special_characters_with_dash_when_disabled"): (["name"], {"slug": "testresult"}, {"args": {"invalid_dash_replacement": False}}),
    ("SlugParserFixture", "should_trim_trailing_dashes_and_underscores_based_on_list"): (["name", "input.args.trim_end_chars", "slug"], {}, {"args": {"invalid_dash_replacement": False, "deduplicate_chars": ""}}),
    ("SlugParserFixture", "should_replace_duplicate_characters_based_on_list"): (["name", "input.args.deduplicate_chars", "slug"], {}, {"args": {"invalid_dash_replacement": False, "trim_end_chars": ""}}),
    ("UrlFixture", "should_not_parse_url_in_name"): (["name", "movie_title_clean"], {}, {}),
    ("UrlFixture", "should_not_parse_url_in_group"): (["name", "release_group"], {}, {}),
}
for _m in ["should_parse_release_group", "should_parse_exception_release_group", "should_not_include_extension_in_release_group",
           "should_not_include_language_in_release_group", "should_not_include_bad_suffix_in_release_group", "should_parse_anime_release_groups"]:
    ARGS[("ReleaseGroupParserFixture", _m)] = (["name", "release_group"], {}, {})

NOTES = {
    ("ExtendedQualityParserRegex", "should_parse_ultrahd_from_title"): "the test's second argument (named version) is never asserted; only resolution 2160 is",
    ("HashedReleaseFixture", "should_properly_parse_hashed_releases"): "path is the test's C:\\ path after AsOsAgnostic (POSIX form); title is PrimaryMovieTitle",
    ("LanguageParserFixture", "should_parse_title_and_tags"): "FluentAssertions BeEquivalentTo: language_tags order-insensitive, title case-insensitive",
    ("UrlFixture", "should_not_parse_url_in_name"): "asserts CleanMovieTitle(parsed MovieTitle) == CleanMovieTitle(movie_title_clean)",
    ("ParserFixture", "should_normalize_imdbid"): "BeEquivalentTo on strings: case-insensitive",
    **{("IsoLanguagesFixture", _m): "result null: IsoLanguages.Find returns no IsoLanguage at all (result.Should().Be(null)), not one with a null Language"
       for _m in ("unknown_or_invalid_code_should_return_null", "should_not_return_portuguese")},
}

# (fixture, method) -> structured comparison for asserts looser than exact equality, emitted as the
# case's "compare": {"<expect key, dotted for nested>": mode}. Modes: "unordered" (list, any order),
# "ignore_case" (string; FluentAssertions BeEquivalentTo), "clean_title" (both sides through
# Parser.CleanMovieTitle before ==). Keys not listed compare exactly. The NOTES above say the same in words.
COMPARE = {
    ("LanguageParserFixture", "should_parse_title_and_tags"): {"subtitle_info.language_tags": "unordered", "subtitle_info.title": "ignore_case"},
    ("ParserFixture", "should_normalize_imdbid"): {"imdb_id": "ignore_case"},
    ("UrlFixture", "should_not_parse_url_in_name"): {"movie_title_clean": "clean_title"},
}

# NormalizeTitleFixture rows that format one word into several inputs: (formats, expected(word))
WORD_FORMATS = {
    "should_remove_common_words_from_middle_of_title": (["word.{0}.word", "word {0} word", "word-{0}-word"], lambda w: "wordword"),
    "should_not_remove_common_words_from_end_of_title": (["word.word.{0}", "word-word-{0}", "word-word {0}"], lambda w: "wordword" + w.lower()),
    "should_not_remove_common_words_in_the_middle_of_word": (["word.{0}word", "word {0}word", "word-{0}word", "word{0}.word", "word{0}-word", "word{0}-word"], lambda w: "word" + w.lower() + "word"),
    "should_not_clean_word_from_beginning_of_string": (["{0}.word.word", "{0}-word-word", "{0} word word"], lambda w: w + "wordword"),
}


def md5_chain():
    """CrapParserFixture.should_not_parse_md5: 100 chained upper-hex MD5s from the seed."""
    h, out = "CRAPPY TEST SEED", []
    for _ in range(100):
        h = hashlib.md5(h.encode("utf-8")).hexdigest().upper()
        out.append(h)
    return out


FACK = {  # MapFixture.Setup: the movie and search criteria every Map test uses
    "title": "Fack Ju Göthe 2", "clean_title": "fackjugoethe2", "year": 2015,
    "alternative_titles": ["Fack Ju Göthe 2: Same same"],
    "translations": [{"title": "Translated Title", "clean_title": "translatedtitle"}],
    "original_language": "English",
}


def map_case(*titles):
    return ({"facet": "movie", "fn": "ParsingService.Map", "movie": FACK,
             "parsed": [{"movie_titles": [t], "languages": ["English"], "year": 2015} for t in titles],
             "imdb_id": "", "tmdb_id": 0, "search_criteria_movie": True},
            {"maps_to_search_criteria_movie": True})


def a_formats(fmts, want):
    return ({"names": [f.format("a") for f in fmts], "facet": "movie", "fn": "Parser.CleanMovieTitle"}, {"clean_title": want})


# [Test] methods with data: (fixture, method) -> (input, expect[, note])
TESTS_DATA = {
    ("CrapParserFixture", "should_not_parse_md5"): lambda: (
        {"names": md5_chain(), "facet": "movie", "fn": "Parser.ParseMovieTitle"}, {"should_parse": False},
        "names: 100 chained MD5 digests (upper hex, no dashes) starting from the seed \"CRAPPY TEST SEED\", as the test computes them"),
    ("NormalizeTitleFixture", "should_remove_a_from_middle_of_title"): lambda: a_formats(["word.{0}.word", "word {0} word", "word-{0}-word"], "wordword"),
    ("NormalizeTitleFixture", "should_not_remove_a_when_at_start_of_acronym"): lambda: a_formats(["word.{0}.N.K.L.E.word", "word {0} N K L E word", "word-{0}-N-K-L-E-word"], "wordankleword"),
    ("NormalizeTitleFixture", "should_not_remove_a_when_at_end_of_acronym"): lambda: a_formats(["word.N.K.L.E.{0}.word", "word N K L E {0} word", "word-N-K-L-E-{0}-word"], "wordnkleaword"),
    ("NormalizeTitleFixture", "should_not_clean_trailing_a"): lambda: ({"name": "Tokyo Ghoul A", "facet": "movie", "fn": "Parser.CleanMovieTitle"}, {"clean_title": "tokyoghoula"}),
    ("ParserFixture", "should_remove_accents_from_title"): lambda: ({"name": "Carniv\u00e0le", "facet": "movie", "fn": "Parser.CleanMovieTitle"}, {"clean_title": "carnivale"}),
    ("MapFixture", "should_match_alternative_title"): lambda: map_case("Fack Ju Göthe 2: Same same"),
    ("MapFixture", "should_match_translation_title"): lambda: map_case("Translated Title"),
    ("MapFixture", "should_match_roman_title"): lambda: map_case("Fack Ju Göthe II"),
    ("MapFixture", "should_match_umlauts"): lambda: map_case("Fack Ju Goethe 2", "Fack Ju Goethe 2: Same same"),
    ("SlugParserFixture", "should_handle_null_trim_parameters"): lambda: ({"name": "test", "facet": "movie", "fn": "Parser.ToUrlSlug", "args": {"invalid_dash_replacement": False, "trim_end_chars": None, "deduplicate_chars": "-_"}}, {"slug": "test"}),
    ("SlugParserFixture", "should_handle_null_dedupe_parameters"): lambda: ({"name": "test", "facet": "movie", "fn": "Parser.ToUrlSlug", "args": {"invalid_dash_replacement": False, "trim_end_chars": "-_", "deduplicate_chars": None}}, {"slug": "test"}),
    ("SlugParserFixture", "should_handle_empty_trim_parameters"): lambda: ({"name": "test", "facet": "movie", "fn": "Parser.ToUrlSlug", "args": {"invalid_dash_replacement": False, "trim_end_chars": "", "deduplicate_chars": "-_"}}, {"slug": "test"}),
    ("SlugParserFixture", "should_handle_empty_dedupe_parameters"): lambda: ({"name": "test", "facet": "movie", "fn": "Parser.ToUrlSlug", "args": {"invalid_dash_replacement": False, "trim_end_chars": "-_", "deduplicate_chars": ""}}, {"slug": "test"}),
}

# Rows/tests that cannot be data: (fixture, method) -> behaviour sentence.
OUT = {
    ("CrapParserFixture", "should_not_parse_random"): "ParseMovieTitle rejects 500 random alphanumeric strings of this length (unseeded Random; not deterministic, a property-test candidate)",
    ("GetMovieFixture", "should_use_passed_in_title_when_it_cannot_be_parsed"): "ParsingService.GetMovie looks a title it cannot parse up verbatim (asserted only as a mock call to IMovieService.FindByTitle)",
    ("GetMovieFixture", "should_use_parsed_series_title"): "ParsingService.GetMovie looks a parseable title up by its parsed movie titles (asserted only as a mock call to IMovieService.FindByTitle)",
    ("MapFixture", "should_lookup_Movie_by_name"): "ParsingService.Map without search criteria looks the movie up by title (asserted only as a mock call to IMovieService.FindByTitle)",
    ("MapFixture", "should_use_search_criteria_movie_title"): "ParsingService.Map with search criteria never looks the movie up by title (asserted only as a mock call count)",
}


MOVIE_FNS = {"Parser.ParseMovieTitle", "Parser.ParseMoviePath", "Parser.CleanMovieTitle", "ParsingService.Map"}


def with_facet(inp):
    """facet "movie" for movie-specific functions, "unknown" for the rest (as the Sonarr port does)."""
    if inp is not None:
        inp["facet"] = "movie" if inp["fn"] in MOVIE_FNS else "unknown"


def behaviour(inp, exp):
    if "name" in inp:
        subj = f'"{inp["name"]}"'
    elif "names" in inp:
        subj = f'each of {len(inp["names"])} inputs like "{inp["names"][0]}"'
    elif "number" in inp:
        subj = str(inp["number"])
    elif "parsed" in inp:
        subj = "parsed movie titles " + " and ".join(json.dumps(t, ensure_ascii=False) for p in inp["parsed"] for t in p["movie_titles"])
    else:
        raise ValueError(f"behaviour(): no subject for input {inp}")
    call = inp["fn"].split(".")[-1]
    flat = []

    def walk(d, pre=""):
        for k, v in d.items():
            if isinstance(v, dict):
                walk(v, pre + k + ".")
            else:
                flat.append(f"{pre}{k} {json.dumps(v, ensure_ascii=False)}")
    walk(exp)
    if exp == {"should_parse": False}:
        return f"{call} returns no result for {subj}"
    extra = f" (args {json.dumps(inp['args'], ensure_ascii=False)})" if "args" in inp else ""
    return f"{call} of {subj}{extra} gives {', '.join(flat)}"


def quality_from_body(body):
    """ParseAndVerifyQuality(title, source, proper, resolution[, modifier]) semantics:
    resolution only when not Unknown, modifier only when not NONE, version 2 if proper else 1."""
    m = re.search(r"ParseAndVerifyQuality\(\w+, (?:QualitySource\.)?(\w+), (\w+), (?:Resolution\.)?(\w+)(?:, (?:Modifier\.)?(\w+))?\)", body)
    return m and m.groups()


def quality_expect(source, proper, res, mod, res_table):
    q = {"source": source}
    if res != "Unknown":
        q["resolution"] = res_table[res]
    if mod and mod != "NONE":
        q["modifier"] = mod
    q["revision"] = {"version": 2 if proper else 1}
    return {"quality": q}


def build(src):
    lang, qual, res = enum_names(src)
    base = src / TESTS
    out = []  # (relpath, line, sub, fixture, method, input|None, expect|None, note|None, behaviour)
    unknown = []
    for path in sorted(base.rglob("*.cs"), key=lambda p: p.relative_to(base).as_posix()):
        rel = path.relative_to(src).as_posix()
        fixture, methods, t = scan(path.read_text(encoding="utf-8-sig"))
        for m in methods:
            key = (fixture, m["name"])
            rows = m["rows"] + (source_rows(t, m["source"]) if m["source"] else [])
            if not rows and not m["tests"]:
                continue  # helper or [SetUp]
            body = m["body"]
            emit = lambda line, sub, inp, exp, note=None, beh=None: out.append(
                (rel, line, sub, fixture, m["name"], inp, exp, note, beh or behaviour(inp, exp)))
            if key in OUT:
                for line, args, _ in rows or [(m["tests"][0], [], {})]:
                    emit(line, 0, None, None, None, OUT[key] + (f"; length {args[0]}" if args else ""))
                continue
            if key in TESTS_DATA and not rows:
                inp, exp, *note = TESTS_DATA[key]()
                emit(m["tests"][0], 0, inp, exp, note[0] if note else None)
                continue
            if fixture == "RomanNumeralConversionFixture":
                lo, hi = map(int, re.search(r"\[Range\((\d+),\s*(\d+)\)\]", m["params"]).groups())
                table = json.loads((src / "src/NzbDrone.Core.Test/Files/ArabicRomanNumeralDictionary.JSON").read_text(encoding="utf-8-sig"))
                note = "expected values from src/NzbDrone.Core.Test/Files/ArabicRomanNumeralDictionary.JSON"
                for n in range(lo, hi + 1):
                    if ".ToRomanNumeral()" in body:
                        emit(m["tests"][0], n, {"number": n, "facet": "movie", "fn": "RomanNumeral.ToRomanNumeral"}, {"roman": table[str(n)]}, note)
                    else:
                        emit(m["tests"][0], n, {"name": table[str(n)], "facet": "movie", "fn": "RomanNumeral.ToInt"}, {"number": n}, note)
                continue
            if not rows:
                unknown.append(f"{rel} {m['name']} ([Test] without data)")
                continue
            call = call_of(body)
            for line, args, named in rows:
                note = named.get("Description") or NOTES.get(key)
                inp = {"name": args[0], "facet": "movie", "fn": call}
                if call == "Parser.ParseMovieTitle" and re.search(r"ParseMovieTitle\(\w+, true\)", body):
                    inp["is_dir"] = True
                exp = {}
                if m["name"] in WORD_FORMATS and fixture == "NormalizeTitleFixture":
                    fmts, want = WORD_FORMATS[m["name"]]
                    inp = {"names": [f.format(args[0]) for f in fmts], "facet": "movie", "fn": call}
                    exp = {"clean_title": want(args[0])}
                elif key in ARGS:
                    names, fixed, extra = ARGS[key]
                    if len(args) != len(names):
                        raise ValueError(f"{rel}:{line} {m['name']}: {len(args)} args, spec has {len(names)}")
                    inp.update(json.loads(json.dumps(extra)))
                    for n, v in zip(names[1:], args[1:]):
                        if n == "_":
                            continue
                        if isinstance(v, Enum):
                            kind, _, val = v.partition(".")
                            v = {"Quality": qual, "Language": lang}[kind][val]
                        if n.startswith("input."):
                            nest(inp, n[len("input."):], v)
                        else:
                            nest(exp, n, v)
                    for n, v in fixed.items():
                        nest(exp, n, v)
                elif key == ("QualityParserFixture", "should_parse_quality_from_other_source"):
                    qs, source, resolution, *mod = args
                    inp = {"names": [f"My series S01E01 {qs.replace(' ', c)}" for c in "-. _"], "facet": "movie", "fn": call}
                    exp = quality_expect(source.split(".")[1], False, resolution.split(".")[1], mod[0].split(".")[1] if mod else None, res)
                elif q := quality_from_body(body):
                    source, proper, resolution, mod = q
                    exp = quality_expect(source, args[1] if proper == "proper" else False, resolution, mod, res)
                    if len(args) != (2 if proper == "proper" else 1):
                        raise ValueError(f"{rel}:{line} {m['name']}: unexpected args {args}")
                elif fixture == "QualityParserFixture" and "DetectionSource" in body:
                    for field in ("Source", "Resolution"):
                        v = re.search(rf"\.{field}DetectionSource\.Should\(\)\.Be\(QualityDetectionSource\.(\w+)\)", body).group(1)
                        nest(exp, f"quality.{field.lower()}_detection_source", v)
                elif fixture in ("LanguageParserFixture", "IsoLanguagesFixture"):
                    asserts = [
                        (r"Should\(\)\.Contain\(Language\.(\w+)\)", lambda v: exp.setdefault("languages_include", []).append(lang[v])),
                        (r"Languages\.Count\.Should\(\)\.Be\((\d+)\)", lambda v: exp.__setitem__("languages_count", int(v))),
                        (r"First\(\)\.Id\.Should\(\)\.Be\(Language\.(\w+)\.Id\)", lambda v: exp.__setitem__("languages_first", lang[v])),
                        (r"result\.Language\.Should\(\)\.Be\(Language\.(\w+)\)", lambda v: exp.__setitem__("language", lang[v])),
                        (r"result\.Should\(\)\.Be\(Language\.(\w+)\)", lambda v: exp.__setitem__("subtitle_language", lang[v])),
                        (r"result\.Should\(\)\.Be\((null)\)", lambda v: exp.__setitem__("result", None)),
                    ]
                    hits = sum(len(re.findall(rx, body)) for rx, _ in asserts)
                    if hits != body.count(".Should()") or len(args) != 1:
                        raise ValueError(f"{rel} {m['name']}: unrecognised assertions or args")
                    for rx, f in asserts:
                        for v in re.findall(rx, body):
                            f(v)
                else:
                    unknown.append(f"{rel}:{line} {m['name']}")
                    continue
                emit(line, 0, inp, exp, note)
    if unknown:
        raise SystemExit("unhandled reference tests:\n  " + "\n  ".join(unknown))
    return out


# ------------------------------------------------------- catalogue in/out

def yq(s):
    """YAML double-quoted scalar (JSON escapes, plus \\U for non-BMP)."""
    body = json.dumps(s, ensure_ascii=False)[1:-1]
    return '"' + "".join(c if c.isprintable() else (f"\\u{ord(c):04x}" if ord(c) < 0x10000 else f"\\U{ord(c):08x}") for c in body) + '"'


def yval(v):
    if v is None or isinstance(v, (bool, list)):
        return json.dumps(v, ensure_ascii=False)
    return yq(v) if isinstance(v, str) else str(v)


def read_catalogue(path):
    """Read back the format write_catalogue emits -> list of dicts (with 'key')."""
    if not path.exists():
        return []
    entries = []
    for n, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not raw.strip() or raw.startswith("#"):
            continue
        if raw.startswith("- id: "):
            entries.append({"id": raw[6:].strip()})
        elif raw.startswith("  # key: "):
            entries[-1]["key"] = json.loads(raw[9:])
        elif m := re.match(r"  (\w+): (.*)$", raw):
            k, v = m.groups()
            if k in ("status", "decision", "tests"):
                v = Kept(v)
            elif v.startswith('"'):
                v = json.loads(re.sub(r"\\U([0-9a-fA-F]{8})", lambda e: chr(int(e.group(1), 16)), v))
            elif v.startswith("[") or v in ("true", "false", "null"):
                v = json.loads(v)
            entries[-1][k] = v
        else:
            raise SystemExit(f"{path}:{n}: can't read this line back; keep entries one field per line")
    return entries


def write_catalogue(path, entries):
    lines = [f"# Radarr parser suite ({TAG} {TESTS}), ported by parity/extract/radarr_parse.py.",
             "# Re-run the extractor to refresh; it keeps IDs (by the key comment), status, tests and decision."]
    for e in entries:
        lines.append(f"- id: {e['id']}")
        lines.append(f"  # key: {json.dumps(e['key'], ensure_ascii=True)}")
        for k in ("behaviour", "source", "ref", "in_scope", "status", "decision", "tests"):
            if k in e and not (k == "decision" and e[k] is None):
                lines.append(f"  {k}: {e[k] if k in ('source', 'status') or isinstance(e[k], Kept) else yval(e[k])}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main(src):
    rows = build(src)
    old = read_catalogue(CATALOGUE)
    by_key = {e["key"]: e for e in old if "key" in e}
    by_ref = {(e.get("ref"), e.get("behaviour")): e for e in old}
    nxt = max([int(e["id"][len(PREFIX):]) for e in old] + [0]) + 1
    seen, entries, corpus = {}, [], {}
    for rel, line, sub, fixture, method, inp, exp, note, beh in rows:
        with_facet(inp)
        ident = hashlib.sha1(json.dumps(inp, ensure_ascii=False, sort_keys=True).encode()).hexdigest()[:16] if inp is not None else str(sub)
        base = f"{rel.split(TESTS + '/', 1)[1]}|{method}|{ident}"
        occ = seen[base] = seen.get(base, -1) + 1
        key = f"{base}|{occ}"
        ref = f"{TAG} {rel}:{line}"
        prev = by_key.get(key) or by_ref.get((ref, beh))
        if prev:
            ident_id = prev["id"]
        else:
            ident_id, nxt = f"{PREFIX}{nxt:04d}", nxt + 1
        e = {"id": ident_id, "key": key, "behaviour": beh, "source": "reference", "ref": ref,
             "in_scope": inp is not None, "status": "unmapped", "tests": []}
        if prev:
            for k in ("status", "tests", "decision"):
                if k in prev:
                    e[k] = prev[k]
        entries.append(e)
        if inp is not None:
            case = {"id": ident_id, "ref": ref, "input": inp, "expect": exp}
            if note:
                case["note"] = note
            if (fixture, method) in COMPARE:
                case["compare"] = COMPARE[(fixture, method)]
            corpus.setdefault(fixture, []).append(case)
    used = {e["id"] for e in entries}
    orphans = [e for e in old if e["id"] not in used]
    for e in orphans:
        print(f"warning: {e['id']} no longer extracted; kept, never reused", file=sys.stderr)
    write_catalogue(CATALOGUE, entries + orphans)
    CORPUS.mkdir(parents=True, exist_ok=True)
    for fixture, cases in corpus.items():
        (CORPUS / f"{fixture}.json").write_text(json.dumps(cases, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    inscope = sum(e["in_scope"] for e in entries)
    print(f"{len(entries)} entries ({inscope} in scope, {len(entries) - inscope} out), "
          f"{sum(map(len, corpus.values()))} cases in {len(corpus)} files, ids {entries[0]['id']}..{max(used)}")


def selftest():
    assert strip_comments('a // x\n"//no" /* y\nz */b') == 'a     \n"//no"     \n    b'
    (args, named), _ = parse_list(r'"a\"b", @"C:\x""y", new[] { 1, 2 }, new string[0], null, Quality.X, Description = "d")', 0, ")")
    assert args == ['a"b', 'C:\\x"y', [1, 2], [], None, "Quality.X"] and named == {"Description": "d"}
    assert as_os_agnostic(r"C:\Test\a.mkv") == "/C/Test/a.mkv"
    assert md5_chain()[0] == hashlib.md5(b"CRAPPY TEST SEED").hexdigest().upper()
    assert yq("a\u2028\U0001F600") == '"a\\u2028\U0001F600"'
    print("selftest ok")


if __name__ == "__main__":
    argv = [a for a in sys.argv[1:] if a != "--selftest"]
    if "--selftest" in sys.argv:
        selftest()
    else:
        main(Path(argv[0]) if argv else DEFAULT_SRC)
