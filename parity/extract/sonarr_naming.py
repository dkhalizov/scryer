"""Port Sonarr's naming suites (OrganizerTests) to parity/corpus/reference/sonarr/naming
and parity/catalogue/naming.sonarr.yaml (notes/11-parity-net-design.md, phase 2).

Run from anywhere:  python3 -I parity/extract/sonarr_naming.py
Scope: fixtures exercising our naming config (naming.json: standard format
'{Series Title} - S{season:00}E{episode:00} - {Episode Title} {Quality Full}',
series folder '{Series Title}', season folder 'Season {season}', PrefixedRange,
Smart colons, illegal chars replaced) plus every illegal-char / path-cleaning
fixture. Every row of an in-scope fixture is catalogued; rows needing a daily or
anime series (all 23 of ours are standard), ignored upstream, or asserting mock
calls are in_scope: false with no corpus case. Fixtures ruled out whole are named
in RULED_OUT; every OrganizerTests/*.cs must be ported or ruled out (asserted).

Refs are located by anchor. Expected values come from the [TestCase] row or the
method's `.Should().Be(<literal>)`; a value typed here is checked against that
literal, and every string typed into `given` must be a literal of the fixture
file (or derived from one, see `allow`), so a typo fails loudly. IDs are keyed
by ref (path:line, unique) and preserved on re-run with status/decision/tests.
"""
import copy
import json
import re
from pathlib import Path

TAG = "Sonarr@v4.0.20.3014"
SRC = Path.home() / "PycharmProjects/arr-reference/sonarr@v4.0.20.3014"
OUT = Path(__file__).resolve().parents[1]
CATALOGUE = OUT / "catalogue/naming.sonarr.yaml"
CORPUS = OUT / "corpus/reference/sonarr/naming"
PREFIX = "SON-NAMING-"

O = "src/NzbDrone.Core.Test/OrganizerTests/"
F = O + "FileNameBuilderTests/"
_lines = {}

LIT = r'@"(?:[^"]|"")*"|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])\''
ESC = {"n": "\n", "t": "\t", "0": "\0", "\\": "\\", '"': '"', "'": "'"}


def lines(rel):
    if rel not in _lines:
        _lines[rel] = (SRC / rel).read_text(encoding="utf-8-sig").splitlines()
    return _lines[rel]


def decode(tok):
    if tok.startswith('@"'):
        return tok[2:-1].replace('""', '"')
    return re.sub(r"\\(u[0-9a-fA-F]{4}|.)",
                  lambda m: chr(int(m[1][1:], 16)) if m[1][0] == "u" and len(m[1]) == 5 else ESC[m[1]], tok[1:-1])


def as_os_agnostic(path):
    """NzbDrone.Test.Common/StringExtensions.cs AsOsAgnostic on Linux."""
    if len(path) > 2 and path[1] == ":":
        path = "/" + path.replace(":", "")
    return path.replace("\\", "/")


def args(text):
    """C# [TestCase] arguments: strings, chars, ints, bools, enum members (-> member name)."""
    out, pos = [], 0
    for m in re.finditer(rf"\s*({LIT}|[\w.\-]+)\s*(?:,|$)", text):
        assert m.start() == pos, (text, pos)
        pos = m.end()
        t = m[1]
        if t[0] in "@\"'":
            out.append(decode(t))
        elif re.fullmatch(r"-?\d+", t):
            out.append(int(t))
        elif t in ("true", "false"):
            out.append(t == "true")
        else:
            out.append(t.rsplit(".", 1)[-1])
    assert pos == len(text), text
    return out


_lits = {}


def file_literals(rel):
    if rel not in _lits:
        found = {decode(t) for t in re.findall(LIT, "\n".join(lines(rel)))}
        # plus derived forms: AsOsAgnostic paths, and GivenMediaInfoModel's Split("/") parts
        _lits[rel] = found | {as_os_agnostic(s) for s in found} | {p for s in found for p in s.split("/")}
    return _lits[rel]


def method_line(rel, method):
    hits = [i for i, l in enumerate(lines(rel)) if re.search(rf"\bvoid {method}\(", l)]
    assert len(hits) == 1, (rel, method, hits)
    return hits[0]


def body(rel, method):
    text = "\n".join(lines(rel)[method_line(rel, method):])
    depth = 0
    for m in re.finditer(rf"{LIT}|[{{}}]", text):
        if m[0] == "{":
            depth += 1
        elif m[0] == "}":
            depth -= 1
            if depth == 0:
                return text[:m.end()]
    raise AssertionError((rel, method))


def be_literal(rel, method):
    b = body(rel, method)
    if len(re.findall(r"\.Should\(\)\s*\.Be\(", b)) != 1:
        return None
    m = re.search(rf"\.Should\(\)\s*\.Be\(({LIT})(\.AsOsAgnostic\(\))?\)", b)
    if not m:
        return None
    v = decode(m[1])
    return as_os_agnostic(v) if m[2] else v


def test_ref(rel, method):
    i = method_line(rel, method)
    while not lines(rel)[i].strip().startswith("[Test]"):
        i -= 1
        assert i >= 0 and not lines(rel)[i].strip().startswith("public"), (rel, method)
    return f"{TAG} {rel}:{i + 1}"


def testcase_rows(rel, method):
    """[(ref, args)] of the [TestCase(...)] rows directly above `method`."""
    i = method_line(rel, method) - 1
    rows = []
    while (s := lines(rel)[i].strip()).startswith("[TestCase(") or s == "":
        if s:
            rows.append((f"{TAG} {rel}:{i + 1}", args(re.fullmatch(r"\[TestCase\((.*)\)\]", s)[1])))
        i -= 1
    assert rows, (rel, method)
    return rows[::-1]


# Quality names as Sonarr declares them (src/NzbDrone.Core/Qualities/Quality.cs).
QNAME = {}
for l in lines("src/NzbDrone.Core/Qualities/Quality.cs"):
    if m := re.search(r"public static Quality (\w+) => new Quality\(\d+,\s*\"([^\"]+)\"", l):
        QNAME[m[1]] = m[2]
if not QNAME:  # declared as get-only properties with a body on the next lines
    src = "\n".join(lines("src/NzbDrone.Core/Qualities/Quality.cs"))
    for m in re.finditer(r"public static Quality (\w+)[^;]*?new Quality\(\d+,\s*\"([^\"]+)\"", src):
        QNAME[m[1]] = m[2]
assert QNAME["HDTV720p"] == "HDTV-720p" and QNAME["Bluray1080p"] == "Bluray-1080p", QNAME

# NamingConfig.Default (src/NzbDrone.Core/Organizer/NamingConfig.cs), API field names.
NC = "src/NzbDrone.Core/Organizer/NamingConfig.cs"
DEFAULT = {}
for l in lines(NC):
    if m := re.fullmatch(r"\s*(\w+) = (.+?),?", l):
        k, v = m[1][0].lower() + m[1][1:], m[2]
        DEFAULT[k] = (v == "true" if v in ("true", "false") else "" if v == "string.Empty"
                      else decode(v) if v.startswith('"') else v.rsplit(".", 1)[-1])
assert len(DEFAULT) == 11 and DEFAULT["multiEpisodeStyle"] == "PrefixedRange", DEFAULT
ENUMS = {"Extend", "Duplicate", "Repeat", "Scene", "Range", "PrefixedRange",
         "Delete", "Dash", "SpaceDash", "SpaceDashSpace", "Smart", "Custom",
         "Standard", "None", "Hlg10", "Hdr10", "Pq10"}

cases = []  # dicts: file, ref, behaviour, in_scope, input?, expect?, note?


def strings(x):
    if isinstance(x, str):
        yield x
    elif isinstance(x, dict):
        for v in x.values():
            yield from strings(v)
    elif isinstance(x, list):
        for v in x:
            yield from strings(v)


def add(file, ref, behaviour, input=None, expect=None, note=None, in_scope=True, allow=()):
    assert (input is None) == (not in_scope), ref
    if input:
        rel = ref.split(" ", 1)[1].rsplit(":", 1)[0]
        known = file_literals(rel) | set(QNAME.values()) | set(map(str, DEFAULT.values())) | ENUMS | set(allow)
        bad = [s for s in strings(input["given"]) if s not in known]
        assert not bad, (ref, bad)
    cases.append(dict(file=file, ref=ref, behaviour=behaviour, in_scope=in_scope,
                      input=input, expect=expect, note=note))


def out_of_scope(rel, method, behaviour):
    add(None, test_ref(rel, method), behaviour, in_scope=False)


def naming(base, **over):
    n = copy.deepcopy(base)
    for k, v in over.items():
        assert k in n, k
        n[k] = v
    return n


def given(naming_cfg, series, episodes, episode_file, **extra):
    g = {"naming": naming_cfg, "series": series, "episodes": episodes}
    if episode_file is not None:
        g["episodeFile"] = episode_file
    g.update(extra)
    return copy.deepcopy(g)


def name(rel, method, expected=None):
    lit = be_literal(rel, method)
    if expected is None:
        assert lit is not None, (rel, method, "expected is not a literal: pass it")
        return lit
    assert lit is None or lit == expected, (rel, method, lit, expected)
    return expected


DAILY = "needs a daily series (all 23 of ours are standard, naming.json/series-summary.json)"
ANIME = "needs an anime series (all 23 of ours are standard, naming.json/series-summary.json)"
MOCK = "asserts calls on the IUpdateMediaInfo mock, not a name"

RENAMING = naming(DEFAULT, renameEpisodes=True)  # every FileNameBuilderTests Setup sets RenameEpisodes = true
STANDARD = "Standard"
EP_FILE = {"quality": QNAME["HDTV720p"], "revision": {"version": 1, "real": 0}, "releaseGroup": "SonarrTest"}
PROPER = {**EP_FILE, "revision": {"version": 2, "real": 0}}
REAL = {**EP_FILE, "revision": {"version": 1, "real": 1}}
REV_NOTE = "QualityModel(quality) starts at Revision version 1, real 0 (src/NzbDrone.Core/Qualities/Revision.cs); GivenProper sets version 2, GivenReal sets real 1."


def ep(title, season, number, absolute=None):
    e = {"title": title, "seasonNumber": season, "episodeNumber": number}
    if absolute is not None:
        e["absoluteEpisodeNumber"] = absolute
    return e


def media_info(video="h264", audio="dts", channels=6, bit_depth=8, hdr="None",
               audio_languages="eng", subtitles="eng/spa/ita", schema=5):
    """FileNameBuilderFixture.GivenMediaInfoModel."""
    return {"videoFormat": video, "audioFormat": audio, "audioChannels": channels,
            "audioLanguages": audio_languages.split("/"), "subtitles": subtitles.split("/"),
            "videoBitDepth": bit_depth, "videoHdrFormat": hdr, "schemaRevision": schema}


# --- FileNameBuilderTests/FileNameBuilderFixture ------------------------------------
FB = F + "FileNameBuilderFixture.cs"
SP = {"title": "South Park", "seriesType": STANDARD}
EP1 = ep("City Sushi", 15, 6, 100)


def fb(method, behaviour, fmt=None, series=SP, episodes=(EP1,), episode_file=EP_FILE, expected=None,
       note=None, cfg=None, extension=None, allow=()):
    cfg = cfg or (naming(RENAMING, standardEpisodeFormat=fmt) if fmt is not None else RENAMING)
    extra = {"extension": extension} if extension else {}
    add("file_name_builder.json", test_ref(FB, method), behaviour,
        {"op": "build_file_name", "given": given(cfg, series, list(episodes), episode_file, **extra)},
        {"name": name(FB, method, expected)}, note, allow=allow)


fb("should_replace_Series_space_Title", "{Series Title} renders the series title", "{Series Title}")
fb("should_replace_Series_underscore_Title", "{Series_Title} joins the series title words with underscores", "{Series_Title}")
fb("should_replace_Series_dot_Title", "{Series.Title} joins the series title words with periods", "{Series.Title}")
fb("should_replace_Series_dash_Title", "{Series-Title} joins the series title words with dashes", "{Series-Title}")
fb("should_replace_SERIES_TITLE_with_all_caps", "{SERIES TITLE} renders the series title upper-cased", "{SERIES TITLE}")
fb("should_replace_SERIES_TITLE_with_random_casing_should_keep_original_casing",
   "A mixed-case token {sErIES-tItLE} keeps the title's own casing", "{sErIES-tItLE}", expected="South-Park",
   note="Expected is _series.Title.Replace(' ', '-').")
fb("should_replace_series_title_with_all_lower_case", "{series title} renders the series title lower-cased", "{series title}")
fb("should_cleanup_Series_Title", "{Series.CleanTitle} strips parentheses from 'South Park (1997)'", "{Series.CleanTitle}",
   series={**SP, "title": "South Park (1997)"})
fb("should_replace_episode_title", "{Episode Title} renders the episode title", "{Episode Title}")
fb("should_replace_episode_title_if_pattern_has_random_casing", "A mixed-case {ePisOde-TitLe} renders the title with dashes", "{ePisOde-TitLe}")
S1 = [{**EP1, "seasonNumber": 1}]
fb("should_replace_season_number_with_single_digit", "{season} renders season 1 without padding", "{season}x{episode}", episodes=S1)
fb("should_replace_season00_number_with_two_digits", "{season:00} pads season 1 to two digits", "{season:00}x{episode}", episodes=S1)
fb("should_replace_episode_number_with_single_digit", "{episode} renders episode 6 without padding", "{season}x{episode}", episodes=S1)
fb("should_replace_episode00_number_with_two_digits", "{episode:00} pads episode 6 to two digits", "{season}x{episode:00}", episodes=S1)
fb("should_replace_quality_title", "{Quality Title} renders the quality name", "{Quality Title}")
fb("should_replace_quality_proper_with_proper", "{Quality Proper} renders 'Proper' for revision version 2", "{Quality Proper}",
   episode_file=PROPER, note=REV_NOTE)
fb("should_replace_quality_real_with_real", "{Quality Real} renders 'REAL' for revision real 1", "{Quality Real}",
   episode_file=REAL, note=REV_NOTE)
fb("should_replace_all_contents_in_pattern", "A full standard pattern renders series, SxxEyy, episode title and quality",
   "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]")
for ref, (fmt, expected) in testcase_rows(FB, "should_escape_token_in_format"):
    add("file_name_builder.json", ref, f"Doubled braces in format '{fmt}' are literal braces: '{expected}'",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat=fmt), SP, [EP1], EP_FILE)},
        {"name": expected})
fb("should_escape_token_in_title", "A token-like '{Quality Full}' inside the series title is not expanded",
   "Some Unescaped {Series Title} String", series={**SP, "title": "My {Quality Full} Title"})
NO_RENAME = naming(RENAMING, renameEpisodes=False)
fb("use_file_name_when_sceneName_is_null", "With renaming off and no scene name, the file keeps its relative path's name",
   cfg=NO_RENAME, episode_file={**EP_FILE, "relativePath": "30 Rock - S01E01 - Test"},
   expected="30 Rock - S01E01 - Test", note="Expected is Path.GetFileNameWithoutExtension(RelativePath); SceneName is unset (null).")
fb("use_path_when_sceneName_and_relative_path_are_null",
   "With renaming off and no scene name or relative path, the file keeps its path's name",
   cfg=NO_RENAME, episode_file={**EP_FILE, "relativePath": None, "path": as_os_agnostic(r"C:\Test\Unsorted\Series - S01E01 - Test")},
   expected="Series - S01E01 - Test",
   note="Path is the test's AsOsAgnostic form on Linux; expected is Path.GetFileNameWithoutExtension(Path).")
fb("use_file_name_when_sceneName_is_not_null", "With renaming off, the scene name wins over the relative path",
   cfg=NO_RENAME, episode_file={**EP_FILE, "sceneName": "30.Rock.S01E01.xvid-LOL", "relativePath": "30 Rock - S01E01 - Test"})
fb("should_replace_illegal_characters_when_renaming_is_disabled",
   "With renaming off, a colon in the scene name is still replaced (Smart: ':' -> '-')",
   cfg=naming(NO_RENAME, replaceIllegalCharacters=True, colonReplacementFormat="Smart"),
   episode_file={**EP_FILE, "sceneName": "30.Rock.S01E01.xvid:LOL", "relativePath": "30 Rock - S01E01 - Test"})
out_of_scope(FB, "should_use_airDate_if_series_isDaily_and_not_a_special", f"A daily series uses the daily format with the air date; {DAILY}")
out_of_scope(FB, "should_use_standard_if_series_isDaily_special", f"A daily series special (season 0) uses the standard format; {DAILY}")
out_of_scope(FB, "should_set_airdate_to_unknown_if_not_available", f"A daily episode with no air date renders 'Unknown'; {DAILY}")
fb("should_not_clean_episode_title_if_there_is_only_one", "A single episode keeps a '(1)' suffix in its title",
   "{Episode Title}", episodes=[{**EP1, "title": "City Sushi (1)"}], expected="City Sushi (1)",
   note="Expected is the episode title set by the test.")
fb("should_should_replace_release_group", "{Release Group} renders the file's release group", "{Release Group}",
   expected="SonarrTest", note="Expected is _episodeFile.ReleaseGroup.")
fb("should_be_able_to_use_original_title", "{Original Title} renders the scene name", "{Series Title} - {Original Title}",
   series={**SP, "title": "30 Rock"},
   episode_file={**EP_FILE, "sceneName": "30.Rock.S01E01.xvid-LOL", "relativePath": "30 Rock - S01E01 - Test"})
ROCK = {"title": "30 Rock", "seriesType": STANDARD}
SCENE = naming(RENAMING, standardEpisodeFormat="{Series Title} - S{season:00}E{episode:00} - {Episode Title}", multiEpisodeStyle="Scene")
fb("should_trim_periods_from_end_of_episode_title", "A trailing period is trimmed from the episode title",
   cfg=SCENE, series=ROCK, episodes=[ep("Part 1.", 6, 6)])
fb("should_trim_question_marks_from_end_of_episode_title", "A trailing question mark is trimmed from the episode title",
   cfg=SCENE, series=ROCK, episodes=[ep("Part 1?", 6, 6)])
DOTS = "{Series.Title}.S{season:00}E{episode:00}.{Episode.Title}"
fb("should_replace_double_period_with_single_period", "A title ending in a period does not double the period separator",
   DOTS, series={"title": "Chicago P.D.", "seriesType": STANDARD}, episodes=[ep("Part 1", 6, 6)])
fb("should_replace_triple_period_with_single_period", "A title ending in two periods collapses to one period separator",
   DOTS, series={"title": "Chicago P.D..", "seriesType": STANDARD}, episodes=[ep("Part 1", 6, 6)])
fb("should_not_replace_absolute_numbering_when_series_is_not_anime",
   "{absolute:00} renders nothing for a standard series and its separator is dropped",
   "{Series.Title}.S{season:00}E{episode:00}.{absolute:00}.{Episode.Title}")
for m, b in [("should_replace_standard_and_absolute_numbering_when_series_is_anime", "An anime format renders standard and absolute numbers"),
             ("should_replace_standard_numbering_when_series_is_anime", "An anime format renders standard numbering"),
             ("should_replace_absolute_numbering_when_series_is_anime", "An anime format renders the absolute number"),
             ("should_replace_duplicate_numbering_individually", "Repeated numbering tokens in an anime format render individually"),
             ("should_replace_individual_season_episode_tokens", "Season/episode tokens with custom padding in an anime format"),
             ("should_use_standard_naming_when_anime_episode_has_no_absolute_number", "An anime episode without an absolute number falls back to the standard format")]:
    out_of_scope(FB, m, f"{b}; {ANIME}")
fb("should_include_affixes_if_value_not_empty", "Prefix/suffix characters inside a token are kept when the value is not empty",
   "{Series.Title}.S{season:00}E{episode:00}{_Episode.Title_}{Quality.Title}")
fb("should_not_include_affixes_if_value_empty", "Prefix/suffix characters inside a token are dropped when the value is empty",
   "{Series.Title}.S{season:00}E{episode:00}{_Episode.Title_}", episodes=[{**EP1, "title": ""}])
MI_FULL = "{Series.Title}.S{season:00}E{episode:00}.{Episode.Title}.{MEDIAINFO.FULL}"
fb("should_format_mediainfo_properly", "{MEDIAINFO.FULL} renders codec, audio and language tags", MI_FULL,
   episode_file={**EP_FILE, "mediaInfo": {"videoFormat": "h264", "audioFormat": "dts", "audioLanguages": ["eng", "spa"],
                                          "subtitles": ["eng", "spa", "ita"]}})
for ref, (lang, code) in testcase_rows(FB, "should_format_languagecodes_properly"):
    add("file_name_builder.json", ref, f"{{MEDIAINFO.FULL}} renders subtitle language '{lang}' as [{code}]",
        {"op": "build_file_name", "given": given(
            naming(RENAMING, standardEpisodeFormat=MI_FULL), SP, [EP1],
            {**EP_FILE, "mediaInfo": {"videoFormat": "h264", "audioFormat": "dts", "audioChannels": 6, "audioLanguages": ["eng"],
                                      "subtitles": [lang], "schemaRevision": 3}})},
        {"name": f"South.Park.S15E06.City.Sushi.H264.DTS.[{code}]"})
fb("should_exclude_english_in_mediainfo_audio_language", "{MEDIAINFO.FULL} omits an English-only audio language", MI_FULL,
   episode_file={**EP_FILE, "mediaInfo": {"videoFormat": "h264", "audioFormat": "dts", "audioLanguages": ["eng"],
                                          "subtitles": ["eng", "spa", "ita"]}})
out_of_scope(FB, "should_remove_duplicate_non_word_characters",
             "Duplicate non-word characters are removed from '{Series.Title}' (marked [Ignore] 'not currently supported' upstream)")
fb("should_use_existing_filename_when_scene_name_is_not_available",
   "{Original Title} falls back to the existing file name when there is no scene name", "{Original Title}",
   episode_file={**EP_FILE, "sceneName": None, "relativePath": "existing.file.mkv"}, expected="existing.file",
   note="Expected is Path.GetFileNameWithoutExtension(RelativePath).")
fb("should_be_able_to_use_only_original_title", "A format of only {Original Title} renders the scene name", "{Original Title}",
   series={**SP, "title": "30 Rock"},
   episode_file={**EP_FILE, "sceneName": "30.Rock.S01E01.xvid-LOL", "relativePath": "30 Rock - S01E01 - Test"})
fb("should_allow_period_between_season_and_episode", "A period between season and episode tokens is kept",
   "{Series.Title}.S{season:00}.E{episode:00}.{Episode.Title}")
fb("should_allow_space_between_season_and_episode", "A space between season and episode tokens is kept",
   "{Series Title} - S{season:00} E{episode:00} - {Episode Title}")
out_of_scope(FB, "should_replace_quality_proper_with_v2_for_anime_v2", f"{{Quality Proper}} renders 'v2' for an anime proper; {ANIME}")
fb("should_not_include_quality_proper_when_release_is_not_a_proper", "{Quality Proper} renders nothing (and its separator is dropped) for a non-proper",
   "{Quality Title} {Quality Proper}")
PB = "{Series Title} - S{season:00}E{episode:00} [{Quality Title}] {[Quality Proper]}"
fb("should_wrap_proper_in_square_brackets", "{[Quality Proper]} wraps 'Proper' in brackets for a proper", PB, episode_file=PROPER, note=REV_NOTE)
fb("should_not_wrap_proper_in_square_brackets_when_not_a_proper", "{[Quality Proper]} renders nothing, brackets included, for a non-proper", PB)
QF = "{Series Title} - S{season:00}E{episode:00} [{Quality Full}]"
fb("should_replace_quality_full_with_quality_title_only_when_not_a_proper", "{Quality Full} is just the quality name for a non-proper", QF)
fb("should_replace_quality_full_with_quality_title_and_proper_only_when_a_proper",
   "{Quality Full} is the quality name plus 'Proper' for a proper", QF, episode_file=PROPER, note=REV_NOTE)
fb("should_replace_quality_full_with_quality_title_and_real_when_a_real",
   "{Quality Full} is the quality name plus 'REAL' for a real", QF, episode_file=REAL, note=REV_NOTE)
for ref, (sep,) in testcase_rows(FB, "should_trim_extra_separators_from_end_when_quality_proper_is_not_included"):
    fmt = f"{{Quality{sep}Title}}{sep}{{Quality{sep}Proper}}"
    add("file_name_builder.json", ref, f"With separator '{sep}', an empty {{Quality Proper}} at the end leaves no trailing separator",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat=fmt), SP, [EP1], EP_FILE)},
        {"name": "HDTV-720p"}, "Format is string.Format(\"{{Quality{0}Title}}{0}{{Quality{0}Proper}}\", separator).", allow=[fmt])
for ref, (sep,) in testcase_rows(FB, "should_trim_extra_separators_from_middle_when_quality_proper_is_not_included"):
    fmt = f"{{Quality{sep}Title}}{sep}{{Quality{sep}Proper}}{sep}{{Episode{sep}Title}}"
    add("file_name_builder.json", ref, f"With separator '{sep}', an empty {{Quality Proper}} in the middle leaves a single separator",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat=fmt), SP, [EP1], EP_FILE)},
        {"name": f"HDTV-720p{sep}City{sep}Sushi"},
        "Format is string.Format(\"{{Quality{0}Title}}{0}{{Quality{0}Proper}}{0}{{Episode{0}Title}}\", separator).", allow=[fmt])
out_of_scope(FB, "should_not_require_a_separator_between_tokens", f"Tokens need no separator between them in an anime format; {ANIME}")
fb("should_be_able_to_use_original_filename_only", "{Original Filename} renders the relative path's name, not the scene name",
   "{Original Filename}", series={**SP, "title": "30 Rock"},
   episode_file={**EP_FILE, "sceneName": "30.Rock.S01E01.xvid-LOL", "relativePath": "30 Rock - S01E01 - Test"})
fb("should_use_Sonarr_as_release_group_when_not_available", "{Release Group} renders 'Sonarr' when the file has no release group",
   "{Release Group}", episode_file={**EP_FILE, "releaseGroup": None})
for ref, (fmt, expected) in testcase_rows(FB, "should_not_use_Sonarr_as_release_group_if_pattern_has_separator"):
    add("file_name_builder.json", ref, f"'{fmt}' with no release group renders no group and no separator",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat=fmt), SP, [EP1], {**EP_FILE, "releaseGroup": None})},
        {"name": expected})
for ref, (group,) in testcase_rows(FB, "should_use_existing_casing_for_release_group"):
    add("file_name_builder.json", ref, f"{{Release Group}} keeps the casing of '{group}'",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat="{Release Group}"), SP, [EP1],
                                                 {**EP_FILE, "releaseGroup": group})},
        {"name": group})
for ref, (culture,) in testcase_rows(FB, "should_replace_all_tokens_for_different_cultures"):
    add("file_name_builder.json", ref, f"{{MediaInfo AudioLanguages}} renders [EN+DE] under culture {culture}",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat="{MediaInfo AudioLanguages}"), SP, [EP1],
                                                 {**EP_FILE, "releaseGroup": None, "mediaInfo": media_info(audio_languages="eng/deu")},
                                                 culture=culture)},
        {"name": "[EN+DE]"}, "mediaInfo is GivenMediaInfoModel(audioLanguages: \"eng/deu\") with its defaults.")
for method, token in [("should_format_audio_languages", "{MediaInfo AudioLanguages}"),
                      ("should_format_audio_languages_all", "{MediaInfo AudioLanguagesAll}")]:
    for ref, (langs, expected) in testcase_rows(FB, method):
        add("file_name_builder.json", ref, f"{token} renders audio languages {langs} as '{expected}'",
            {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat=token), SP, [EP1],
                                                     {**EP_FILE, "releaseGroup": None, "mediaInfo": media_info(audio_languages=langs)})},
            {"name": expected}, "mediaInfo is GivenMediaInfoModel(audioLanguages: ...) with its defaults.")
for ref, (subs, fmt, expected) in testcase_rows(FB, "should_format_subtitle_languages_all"):
    full = "{MediaInfo SubtitleLanguages" + fmt + "}End"
    add("file_name_builder.json", ref, f"'{full}' renders subtitles {subs} as '{expected}End'",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat=full), SP, [EP1],
                                                 {**EP_FILE, "releaseGroup": None, "mediaInfo": media_info(subtitles=subs)})},
        {"name": expected + "End"}, "mediaInfo is GivenMediaInfoModel(subtitles: ...) with its defaults.", allow=[full])
HDR_FMT = "{Series.Title}.S{season:00}E{episode:00}.{Episode.Title}.{MediaInfo VideoDynamicRange}"
for ref, (hdr, expected) in testcase_rows(FB, "should_include_hdr_for_mediainfo_videodynamicrange_with_valid_properties"):
    add("file_name_builder.json", ref, f"{{MediaInfo VideoDynamicRange}} for HDR format {hdr} renders '{expected}'",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat=HDR_FMT), SP, [EP1],
                                                 {**EP_FILE, "mediaInfo": media_info(hdr=hdr)})},
        {"name": expected}, "mediaInfo is GivenMediaInfoModel(hdrFormat: ...) with its defaults.")
for m, b in [("should_update_media_info_if_token_configured_and_revision_is_old", "Media info is refreshed once when a media-info token is used and the schema revision is old"),
             ("should_not_update_media_info_if_no_series_path_available", "Media info is not refreshed when the series has no path"),
             ("should_not_update_media_info_if_token_not_configured_and_revision_is_old", "Media info is not refreshed when no media-info token is used"),
             ("should_not_update_media_info_if_token_configured_and_revision_is_current", "Media info is not refreshed when the schema revision is current"),
             ("should_not_update_media_info_if_token_configured_and_revision_is_newer", "Media info is not refreshed when the schema revision is newer")]:
    out_of_scope(FB, m, f"{b}; {MOCK}")
for ref, (fmt,) in testcase_rows(FB, "should_use_updated_media_info_if_token_configured_and_revision_is_old"):
    add(None, ref, f"'{fmt}' uses the refreshed media info (ends with HDR); {MOCK} (the refresh is a mock callback)", in_scope=False)
fb("should_replace_release_hash_with_stored_hash", "{Release Hash} renders the stored release hash", "{Release Hash}",
   episode_file={**EP_FILE, "releaseHash": "ABCDEFGH"})
fb("should_replace_null_release_hash_with_empty_string", "{Release Hash} renders nothing when the hash is null", "{Release Hash}",
   episode_file={**EP_FILE, "releaseHash": None}, expected="", note="Expected is string.Empty.")
fb("should_maintain_ellipsis_in_naming_format", "A literal '...' in the format is kept",
   "{Series.Title}.S{season:00}.E{episode:00}...{Episode.CleanTitle}")

# --- FileNameBuilderTests/MultiEpisodeFixture -----------------------------------------
ME = F + "MultiEpisodeFixture.cs"
E2, E3 = ep("City Sushi", 15, 7, 101), ep("City Sushi", 15, 8, 102)
STD = "{Series Title} - S{season:00}E{episode:00} - {Episode Title}"


def me(method, behaviour, episodes, style=None, fmt=STD):
    cfg = naming(RENAMING, standardEpisodeFormat=fmt)
    if style:
        cfg["multiEpisodeStyle"] = style
    add("multi_episode.json", test_ref(ME, method), behaviour,
        {"op": "build_file_name", "given": given(cfg, SP, episodes, EP_FILE)}, {"name": name(ME, method)})


me("should_replace_Series_space_Title", "{Series Title} renders the series title", [EP1], fmt="{Series Title}")
me("should_format_extend_multi_episode_properly", "Extend style renders two episodes as S15E06-07", [EP1, E2], "Extend")
me("should_format_duplicate_multi_episode_properly", "Duplicate style repeats the SxxEyy block per episode", [EP1, E2], "Duplicate")
me("should_format_repeat_multi_episode_properly", "Repeat style renders two episodes as S15E06E07", [EP1, E2], "Repeat")
me("should_format_scene_multi_episode_properly", "Scene style renders two episodes as S15E06-E07", [EP1, E2], "Scene")
out_of_scope(ME, "should_use_dash_as_separator_when_multi_episode_style_is_extend_for_anime", f"Anime absolute numbers join with a dash; {ANIME}")
out_of_scope(ME, "should_duplicate_absolute_pattern_when_multi_episode_style_is_duplicate", f"Duplicate style repeats the anime absolute block; {ANIME}")
me("should_get_proper_filename_when_multi_episode_is_duplicated_and_bracket_follows_pattern",
   "Duplicate style keeps a bracketed group after the episode block intact", [EP1, E2], "Duplicate",
   "{Series Title} - S{season:00}E{episode:00} - ({Quality Title}, {MediaInfo Full}, {Release Group}) - {Episode Title}")
me("should_format_range_multi_episode_properly", "Range style renders three episodes as S15E06-08", [EP1, E2, E3], "Range")
out_of_scope(ME, "should_format_range_multi_episode_anime_properly", f"Range style for anime absolute numbers; {ANIME}")
out_of_scope(ME, "should_format_repeat_multi_episode_anime_properly", f"Repeat style for anime absolute numbers; {ANIME}")
me("should_format_single_episode_with_range_multi_episode_properly", "Range style renders a single episode as S15E06", [EP1], "Range")
out_of_scope(ME, "should_format_single_anime_episode_with_range_multi_episode_properly", f"Range style for a single anime episode; {ANIME}")
out_of_scope(ME, "should_default_to_dash_when_serparator_is_not_set_for_absolute_number", f"Anime absolute numbers default to a dash separator; {ANIME}")
me("should_format_prefixed_range_multi_episode_properly", "Prefixed Range style renders three episodes as S15E06-E08", [EP1, E2, E3], "PrefixedRange")
out_of_scope(ME, "should_format_prefixed_range_multi_episode_anime_properly", f"Prefixed Range style for anime absolute numbers; {ANIME}")
me("should_format_single_episode_with_prefixed_range_multi_episode_properly", "Prefixed Range style renders a single episode as S15E06",
   [EP1], "PrefixedRange")
out_of_scope(ME, "should_format_single_anime_episode_with_prefixed_range_multi_episode_properly", f"Prefixed Range style for a single anime episode; {ANIME}")
me("should_format_prefixed_range_multi_episode_using_episode_separator",
   "Prefixed Range style with an 'x' separator renders 15x06-x08", [EP1, E2, E3], "PrefixedRange",
   "{Series Title} - {season:0}x{episode:00} - {Episode Title}")
me("should_format_range_multi_episode_wrapped_in_brackets", "Range style inside parentheses renders (S15E06-08)", [EP1, E2, E3], "Range",
   "{Series Title} (S{season:00}E{episode:00}) {Episode Title}")

# --- FileNameBuilderTests/ColonReplacementFixture -------------------------------------
CR = F + "ColonReplacementFixture.cs"
CSI = {"title": "CSI: Vegas", "seriesType": STANDARD}
CSI_EP = ep("What Happens in Vegas", 1, 6, 100)
add("colon_replacement.json", test_ref(CR, "should_replace_colon_followed_by_space_with_space_dash_space_by_default"),
    "By default (Smart) 'CSI: Vegas' renders 'CSI - Vegas'",
    {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat="{Series Title}"), CSI, [CSI_EP], EP_FILE)},
    {"name": name(CR, "should_replace_colon_followed_by_space_with_space_dash_space_by_default")})
for method in ["should_replace_colon_followed_by_space_with_expected_result", "should_replace_colon_with_expected_result"]:
    for ref, (title, fmt, expected) in testcase_rows(CR, method):
        add("colon_replacement.json", ref, f"Colon replacement {fmt} renders '{title}' as '{expected}'",
            {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat="{Series Title}", colonReplacementFormat=fmt),
                                                     {**CSI, "title": title}, [CSI_EP], EP_FILE)},
            {"name": expected})
for ref, (title, fmt, custom, expected) in testcase_rows(CR, "should_replace_colon_with_custom_format"):
    add("colon_replacement.json", ref, f"Custom colon replacement '{custom}' renders '{title}' as '{expected}'",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat="{Series Title}", colonReplacementFormat=fmt,
                                                        customColonReplacementFormat=custom),
                                                 {**CSI, "title": title}, [CSI_EP], EP_FILE)},
        {"name": expected})

# --- FileNameBuilderTests/ReplaceCharacterFixure ----------------------------------------
RC = F + "ReplaceCharacterFixure.cs"
for ref, (title, expected) in testcase_rows(RC, "should_replace_illegal_characters"):
    add("replace_character.json", ref, f"An illegal character in series title '{title}' renders '{expected}'",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat="{Series Title}"),
                                                 {"title": title, "seriesType": STANDARD}, [EP1], EP_FILE)},
        {"name": expected})

# --- FileNameBuilderTests/EpisodeTitleCollapseFixture -----------------------------------
EC = F + "EpisodeTitleCollapseFixture.cs"
for ref, (t1, t2, expected) in testcase_rows(EC, "should_collapse_episode_titles_when_episode_titles_are_the_same"):
    add("episode_title_collapse.json", ref, f"Episode titles '{t1}' and '{t2}' collapse to '{expected}'",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat="{Episode Title}"), SP,
                                                 [{**EP1, "title": t1}, {**E2, "title": t2}], EP_FILE)},
        {"name": expected})
add("episode_title_collapse.json", test_ref(EC, "should_not_collapse_episode_titles_when_episode_titles_are_not_the_same"),
    "Different episode titles are joined with ' + '",
    {"op": "build_file_name", "given": given(SCENE, SP, [{**EP1, "title": "Hello"}, {**E2, "title": "World"}], EP_FILE)},
    {"name": name(EC, "should_not_collapse_episode_titles_when_episode_titles_are_not_the_same")})
add("episode_title_collapse.json", test_ref(EC, "should_not_collaspe_when_result_is_empty"),
    "Titles 'Part 1' and 'Part 2' do not collapse to an empty title; they are joined with ' + '",
    {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat="{Episode Title}"), SP,
                                             [{**EP1, "title": "Part 1"}, {**E2, "title": "Part 2"}], EP_FILE)},
    {"name": name(EC, "should_not_collaspe_when_result_is_empty")})

# --- FileNameBuilderTests/MultiEpisodeTitleFixture ---------------------------------------
MT = F + "MultiEpisodeTitleFixture.cs"
for ref, (t1, t2) in testcase_rows(MT, "should_replace_Series_space_Title"):
    add("multi_episode_title.json", ref, f"Episode titles '{t1}' and '{t2}' collapse to 'Episode Title' before {{Quality Full}}",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat="{Episode Title} {Quality Full}"), SP,
                                                 [ep(t1, 15, 6, 100), ep(t2, 15, 7, 101)], EP_FILE)},
        {"name": name(MT, "should_replace_Series_space_Title")})

# --- FileNameBuilderTests/ReservedDeviceNameFixture --------------------------------------
RD = F + "ReservedDeviceNameFixture.cs"
for ref, (title, expected) in testcase_rows(RD, "should_replace_reserved_device_name_in_series_folder"):
    add("reserved_device_name.json", ref, f"A reserved device name in series title '{title}' is escaped in the series folder",
        {"op": "get_series_folder", "given": {"naming": naming(RENAMING, seriesFolderFormat="{Series.Title}"),
                                              "series": {"title": title, "seriesType": STANDARD}}},
        {"name": expected})
for ref, (title, expected) in testcase_rows(RD, "should_replace_reserved_device_name_in_season_folder"):
    add("reserved_device_name.json", ref, f"A reserved device name in series title '{title}' is escaped in the season folder",
        {"op": "get_season_folder", "given": {"naming": naming(RENAMING, seasonFolderFormat="{Series.Title} - Season {Season:00}"),
                                              "series": {"title": title, "seriesType": STANDARD}, "seasonNumber": 1}},
        {"name": f"{expected} - Season 01"})
for ref, (title, expected) in testcase_rows(RD, "should_replace_reserved_device_name_in_file_name"):
    add("reserved_device_name.json", ref, f"A reserved device name in series title '{title}' is escaped in the file name",
        {"op": "build_file_name", "given": given(naming(RENAMING, standardEpisodeFormat="{Series.Title} - S{Season:00}E{Episode:00}"),
                                                 {"title": title, "seriesType": STANDARD}, [EP1], EP_FILE)},
        {"name": f"{expected} - S15E06"})

# --- FileNameBuilderTests/RequiresEpisodeTitleFixture ------------------------------------
RT = F + "RequiresEpisodeTitleFixture.cs"
for method, cfg, b, v in [
    ("should_return_false_when_episode_title_is_not_part_of_the_pattern",
     naming(RENAMING, standardEpisodeFormat="{Series Title} S{season:00}E{episode:00}"),
     "A standard format without {Episode Title} does not require an episode title", False),
    ("should_return_false_if_renaming_episodes_is_off", NO_RENAME,
     "With renaming off an episode title is not required", False),
    ("should_return_true_when_episode_title_is_part_of_the_pattern", RENAMING,
     "The default standard format (with {Episode Title}) requires an episode title", True),
]:
    assert re.search(rf"\.Should\(\)\.Be{v}\(\)", body(RT, method)), method
    add("requires_episode_title.json", test_ref(RT, method), b,
        {"op": "requires_episode_title", "given": given(cfg, SP, [EP1], None)}, {"requires_episode_title": v})

# --- FileNameBuilderTests/TruncatedEpisodeTitlesFixture ----------------------------------
TT = F + "TruncatedEpisodeTitlesFixture.cs"
TT_EPS = [ep("Episode Title 1", 1, 1), ep("Another Episode Title", 1, 2), ep("Yet Another Episode Title", 1, 3),
          ep("Yet Another Episode Title Take 2", 1, 4), ep("Yet Another Episode Title Take 3", 1, 5),
          ep("Yet Another Episode Title Take 4", 1, 6), ep("A Really Really Really Really Long Episode Title", 1, 7)]
TT_CFG = naming(RENAMING, multiEpisodeStyle="Extend", standardEpisodeFormat=DEFAULT["standardEpisodeFormat"])
LOREM = "Lorem ipsum dolor sit amet, consectetur adipiscing elit Maecenas et magna sem Morbi vitae volutpat quam, id porta arcu Orci varius natoque penatibus et magnis dis parturient montes"
LOREM2 = LOREM + " nascetur ridiculus musu Cras vestibulum"
LOREM2_E = LOREM2.replace("Lorem", "Lor\u00e9m", 1)
BLURAY = {**EP_FILE, "quality": QNAME["Bluray1080p"]}
for method, b, series, episodes, ef, ext, limit in [
    ("should_truncate_with_extension", "A file name with its extension over 255 characters truncates the episode title with '...'",
     "The Fantastic Life of Mr. Sisko",
     [ep("This title has to be 197 characters in length, combined with the series title, quality and episode number it becomes 254ish and the extension puts it above the 255 limit and triggers the truncation", 2, 18)],
     BLURAY, ".mkv", "max_length"),
    ("should_truncate_with_ellipsis_between_first_and_last_episode_titles",
     "Seven episode titles that do not fit become first...last", "Series Title", TT_EPS, EP_FILE, None, "max_length"),
    ("should_truncate_with_ellipsis_if_only_first_episode_title_fits",
     "With a long series title only the first episode title fits, followed by '...'", LOREM, TT_EPS, EP_FILE, None, "max_length"),
    ("should_truncate_first_episode_title_with_ellipsis_if_only_partially_fits",
     "With a very long series title the single episode title is cut with '...'", LOREM2, TT_EPS[:1], EP_FILE, None, "max_length"),
    ("should_truncate_titles_measuring_series_title_bytes",
     "Truncation counts UTF-8 bytes of a non-ASCII series title", LOREM2_E, TT_EPS[:1], EP_FILE, None, "max_utf8_bytes"),
    ("should_truncate_titles_measuring_episode_title_bytes",
     "Truncation counts UTF-8 bytes of a non-ASCII episode title", LOREM2, [{**TT_EPS[0], "title": "Episod\u00e9 Title"}], EP_FILE, None,
     "max_utf8_bytes"),
    ("should_truncate_titles_measuring_episode_title_bytes_middle",
     "Truncation does not split a multi-byte character in the episode title", LOREM2,
     [{**TT_EPS[0], "title": "Episode T\u00e9tle"}], EP_FILE, None, "max_utf8_bytes"),
]:
    extra = {"extension": ext} if ext else {}
    note = "Seven Setup episodes S01E01-07 (titles in Setup); the test passes all of them, or only the first. Episodes use NBuilder defaults otherwise."
    add("truncated_episode_titles.json", test_ref(TT, method), b,
        {"op": "build_file_name", "given": given(TT_CFG, {"title": series, "seriesType": STANDARD}, episodes, ef, **extra)},
        {"name": name(TT, method), limit: 255}, note)

# --- OrganizerTests/GetSeasonFolderFixture, GetSeriesFolderFixture -----------------------
GS = O + "GetSeasonFolderFixture.cs"
for ref, (title, season, fmt, expected) in testcase_rows(GS, "should_use_seriesFolderFormat_to_build_folder_name"):
    add("season_folder.json", ref, f"Season folder format '{fmt}' renders '{title}' season {season} as '{expected}'",
        {"op": "get_season_folder", "given": {"naming": naming(DEFAULT, seasonFolderFormat=fmt),
                                              "series": {"title": title}, "seasonNumber": season}},
        {"name": expected})
GF = O + "GetSeriesFolderFixture.cs"
for ref, (title, fmt, expected) in testcase_rows(GF, "should_use_seriesFolderFormat_to_build_folder_name"):
    add("series_folder.json", ref, f"Series folder format '{fmt}' renders '{title}' as '{expected}'",
        {"op": "get_series_folder", "given": {"naming": naming(DEFAULT, seriesFolderFormat=fmt), "series": {"title": title}}},
        {"name": expected})

# --- OrganizerTests/BuildFilePathFixture ---------------------------------------------------
BP = O + "BuildFilePathFixture.cs"
BP_NOTE = ("Paths are the test's AsOsAgnostic form on Linux (C:\\Test -> /C/Test). NamingConfig.Default has renameEpisodes off, "
           "so the file name is the cleaned scene name.")
for ref, (scene, season, use_folder, fmt, expected) in testcase_rows(BP, "CalculateFilePath_SeasonFolder_SingleNumber"):
    add("build_file_path.json", ref,
        f"Season {season} with season folders {'on' if use_folder else 'off'} and format '{fmt}' builds '{as_os_agnostic(expected)}'",
        {"op": "build_file_path", "given": given(
            naming(DEFAULT, seasonFolderFormat=fmt, specialsFolderFormat="MySpecials"),
            {"title": "30 Rock", "path": as_os_agnostic(r"C:\Test\30 Rock"), "seasonFolder": use_folder, "seriesType": STANDARD},
            [ep("Episode Title", season, 5)], {"sceneName": scene}, extension=".mkv")},
        {"path": as_os_agnostic(expected)}, BP_NOTE)
add("build_file_path.json", test_ref(BP, "should_clean_season_folder_when_it_contains_illegal_characters_in_series_title"),
    "A colon in the series title is cleaned in a season folder built from {Series Title}",
    {"op": "build_file_path", "given": given(
        naming(DEFAULT, seasonFolderFormat="{Series Title} Season {season:0}"),
        {"title": "NCIS: Los Angeles", "path": as_os_agnostic(r"C:\Test\NCIS - Los Angeles"), "seasonFolder": True, "seriesType": STANDARD},
        [ep("Episode Title", 1, 5)], {"sceneName": "S01E05 - Episode Title"}, extension=".mkv")},
    {"path": as_os_agnostic(r"C:\Test\NCIS - Los Angeles\NCIS - Los Angeles Season 1\S01E05 - Episode Title.mkv")}, BP_NOTE)

# --- OrganizerTests/CleanFilenameFixture ---------------------------------------------------
CF = O + "CleanFilenameFixture.cs"
for method, b in [("should_replaace_invalid_characters", "CleanFileName replaces illegal characters in '{0}'"),
                  ("should_remove_periods_from_start", "CleanFileName removes leading periods from '{0}'"),
                  ("should_remove_spaces_from_start_and_end", "CleanFileName trims spaces around '{0}'")]:
    for ref, (raw, expected) in testcase_rows(CF, method):
        add("clean_file_name.json", ref, b.format(raw),
            {"op": "clean_file_name", "given": {"name": raw, "naming": DEFAULT}}, {"name": expected},
            "Static FileNameBuilder.CleanFileName(name) uses NamingConfig.Default.")

# --- Fixtures ruled out whole (no catalogue entries) ----------------------------------------
# Our formats (naming.json) use only {Series Title}, {season:00}, {episode:00}, {Episode Title},
# {Quality Full}, 'Season {season}', 'Specials'; standard series only; CF-in-name off.
NOT_OURS = "a token our naming.json formats never use"
RULED_OUT = {
    F + "AbsoluteEpisodeFormatFixture.cs": "{absolute:00} / {ReleaseGroup}: anime absolute numbering and " + NOT_OURS,
    F + "CleanTitleFixture.cs": "{Series CleanTitle} / {Episode CleanTitle}: " + NOT_OURS,
    F + "CleanTitleTheFixture.cs": "{Series CleanTitleThe}: " + NOT_OURS,
    F + "CleanTitleTheWithoutYearFixture.cs": "{Series CleanTitleTheWithoutYear}: " + NOT_OURS,
    F + "CleanTitleTheYearFixture.cs": "{Series CleanTitleTheYear} / {Series TitleTheYear}: " + NOT_OURS,
    F + "CleanTitleWithoutYearFixture.cs": "{Series CleanTitleWithoutYear}: " + NOT_OURS,
    F + "CleanTitleYearFixture.cs": "{Series CleanTitleYear}: " + NOT_OURS,
    F + "CustomFormatsFixture.cs": "{Custom Format(s)}: " + NOT_OURS + " (includeCustomFormatWhenRenaming off)",
    F + "IdFixture.cs": "{TvdbId} / {TmdbId} / {ImdbId} / {TvMazeId}: " + NOT_OURS,
    F + "OriginalTitleFixture.cs": "{Original Title}: " + NOT_OURS,
    F + "RequiresAbsoluteEpisodeNumberFixture.cs": "{absolute:00}: anime absolute numbering, " + NOT_OURS,
    F + "SeriesTitleFirstCharacterFixture.cs": "{Series TitleFirstCharacter}: " + NOT_OURS,
    F + "SeriesYearFixture.cs": "{Series Year}: " + NOT_OURS,
    F + "TitleTheFixture.cs": "{Series TitleThe}: " + NOT_OURS,
    F + "TitleTheWithoutYearFixture.cs": "{Series TitleTheWithoutYear}: " + NOT_OURS,
    F + "TitleTheYearFixture.cs": "{Series TitleTheYear}: " + NOT_OURS,
    F + "TitleWithoutYearFixture.cs": "{Series TitleWithoutYear}: " + NOT_OURS,
    F + "TitleYearFixture.cs": "{Series TitleYear}: " + NOT_OURS,
    F + "TruncatedReleaseGroupFixture.cs": "{ReleaseGroup:N} truncation: " + NOT_OURS,
    F + "TruncatedSeriesTitleFixture.cs": "{Series Title:N} / {Series CleanTitle:N} / {Series TitleThe:N} truncation: our {Series Title} has no length",
}
ported = {c["ref"].split(" ", 1)[1].rsplit(":", 1)[0] for c in cases}
present = {str(p.relative_to(SRC)) for p in (SRC / O).rglob("*.cs")}
assert not ported & RULED_OUT.keys(), ported & RULED_OUT.keys()
assert present == ported | RULED_OUT.keys(), ("unclassified OrganizerTests fixture", present ^ (ported | RULED_OUT.keys()))

# --- IDs, catalogue, corpus ------------------------------------------------------------------
cases.sort(key=lambda c: (c["ref"].split(" ", 1)[1].rsplit(":", 1)[0], int(c["ref"].rsplit(":", 1)[1])))
refs = [c["ref"] for c in cases]
assert len(set(refs)) == len(refs), "ref is the ID key and must be unique"

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
out, by_file = [], {}
for c in cases:
    if c["ref"] in old:
        cid, keep = old[c["ref"]]
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

CATALOGUE.parent.mkdir(parents=True, exist_ok=True)
CATALOGUE.write_text("# Generated by parity/extract/sonarr_naming.py; IDs are stable, edit status/decision/tests only.\n"
                     + "".join(out))
CORPUS.mkdir(parents=True, exist_ok=True)
for f, cs in sorted(by_file.items()):
    (CORPUS / f).write_text(json.dumps(cs, indent=1, ensure_ascii=False) + "\n")
print(f"{len(cases)} entries ({sum(c['in_scope'] for c in cases)} in scope), {len(by_file)} corpus files, "
      f"{len(RULED_OUT)} fixtures ruled out")
for c in cases:
    if not c["in_scope"]:
        print(f"skipped: {c['ref']}: {c['behaviour']}")
for f, why in sorted(RULED_OUT.items()):
    print(f"ruled out: {TAG} {f}: {why}")
