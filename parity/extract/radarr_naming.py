"""Port Radarr's organizer (naming) tests that our naming config touches to
parity/corpus/reference/radarr/naming and parity/catalogue/naming.radarr.yaml
(notes/11-parity-net-design.md, phase 2).

Run from anywhere:  python3 -I parity/extract/radarr_naming.py

Our config (corpus/captured/radarr/config/naming.json): rename on, file
'{Movie Title} ({Release Year}) {Quality Full}', folder '{Movie Title} ({Release Year})',
colon 'smart', replace illegal characters on. Scope, per row: the format uses only
those tokens as we write them, or the test is an illegal-character / path-cleaning case
(colon replacement, CleanFileName, reserved device names). Every live [Test] and
[TestCase] row of the five fixtures below gets an entry (the scanner fails on an
unhandled method); out-of-scope rows have no corpus case. Fixtures that only test
tokens we do not use are not ported (see SKIPPED). Hardcoded literals are checked
against the method body. IDs are keyed by '# key: path|method|args|n' (n counts identical
rows of one method), so a new tag that shifts line numbers keeps them; an entry whose
row disappears is kept with a warning and its ID is never reused.
In-scope rows whose naming settings differ from our captured naming.json (illegal-path
guards run under another colon mode, renaming off, or a token we do not use) say so in
the case note ("Off our config: ...") so the harness phase can tell them apart.
"""
import json
import re
import sys
from pathlib import Path

TAG = "Radarr@v6.4.4.10685"
SRC = Path.home() / "PycharmProjects/arr-reference/radarr@v6.4.4.10685"
OUT = Path(__file__).resolve().parents[1]
CATALOGUE = OUT / "catalogue/naming.radarr.yaml"
CORPUS = OUT / "corpus/reference/radarr/naming"
PREFIX = "RAD-NAMING-"

T = "src/NzbDrone.Core.Test/OrganizerTests/"
C = "src/NzbDrone.Core/"
FB = T + "FileNameBuilderTests/FileNameBuilderFixture.cs"
CR = T + "FileNameBuilderTests/ColonReplacementFixture.cs"
RD = T + "FileNameBuilderTests/ReservedDeviceNameFixture.cs"
CF = T + "CleanFilenameFixture.cs"
GF = T + "GetMovieFolderFixture.cs"

SKIPPED = {  # fixture -> token it tests that our formats do not use
    "EditionTagsFixture.cs": "{Edition Tags}",
    "CleanCollectionTheFixture.cs": "{Movie CleanCollectionThe}",
    "CleanTitleTheFixture.cs": "{Movie CleanTitleThe}",
    "IdFixture.cs": "{ImdbId}/{TmdbId}",
    "TruncatedReleaseGroupFixture.cs": "{ReleaseGroup:n}",
    "CleanTitleFixture.cs": "{Movie CleanTitle}",
    "OriginalTitleFixture.cs": "{Original Title}",
    "CollectionTheFixture.cs": "{Movie CollectionThe}",
    "MovieTitleFirstCharacterFixture.cs": "{Movie TitleFirstCharacter}",
    "TitleTheFixture.cs": "{Movie TitleThe}",
    "TruncatedMovieTitleFixture.cs": "{Movie Title:n}",
    "CustomFormatsFixture.cs": "{Custom Formats}/{Custom Format}",
}

_lines = {}


def lines(rel):
    if rel not in _lines:
        _lines[rel] = (SRC / rel).read_text(encoding="utf-8-sig").splitlines()
    return _lines[rel]


def cs_args(s):
    """C# attribute arguments -> Python values (strings, chars, ints, null, Enum.Member)."""
    out = []
    for tok in re.findall(r'"(?:[^"\\]|\\.)*"|\'(?:[^\'\\]|\\.)\'|[^,\s][^,]*', s):
        tok = tok.strip()
        if tok[0] in "\"'":
            out.append(json.loads('"' + tok[1:-1] + '"'))
        elif tok == "null":
            out.append(None)
        elif re.fullmatch(r"-?\d+", tok):
            out.append(int(tok))
        else:
            out.append(tok.split(".")[-1])
    return out


def scan(rel):
    """[(method, line_no, args or None for [Test], key)] for every live test row, file order."""
    rows, pending, in_comment = [], [], False
    for i, l in enumerate(lines(rel)):
        s = l.strip()
        if in_comment:
            in_comment = "*/" not in s
            continue
        if s.startswith("/*"):
            in_comment = "*/" not in s
            continue
        if s == "[Test]":
            pending.append((i + 1, None))
        elif m := re.fullmatch(r"\[TestCase\((.*)\)\](?:\s*//.*)?", s):
            pending.append((i + 1, cs_args(m[1])))
        elif "[TestCase" in s or "[Test]" in s:
            raise AssertionError(f"unparsed test attribute {rel}:{i + 1}: {s}")
        elif m := re.search(r"public void (\w+)\(", s):
            rows += [(m[1], n, a) for n, a in pending]
            pending = []
    seen, out = {}, []
    for method, n, a in rows:
        base = f"{rel}|{method}|{json.dumps(a, ensure_ascii=False)}"
        seen[base] = seen.get(base, -1) + 1
        out.append((method, n, a, f"{base}|{seen[base]}"))
    return out


def body(rel, method):
    ls = lines(rel)
    start = next(i for i, l in enumerate(ls) if re.search(rf"\bvoid {method}\(", l))
    end = next(i for i in range(start + 1, len(ls)) if ls[i].startswith("        }"))
    return "\n".join(ls[start:end + 1])


def check(rel, method, *literals):
    """Every hardcoded value must appear, as a C# literal, in the test method."""
    b = body(rel, method)
    for v in literals:
        lit = json.dumps(v, ensure_ascii=False) if isinstance(v, str) else str(v)
        assert lit in b, (rel, method, lit)


def src_at(rel, pattern):
    hits = [i for i, l in enumerate(lines(rel)) if re.search(pattern, l)]
    assert len(hits) == 1, (rel, pattern, hits)
    return f"{rel}:{hits[0] + 1}"


# NamingConfig.Default, read from the source (every fixture starts from it).
NC = C + "Organizer/NamingConfig.cs"
_nc = "\n".join(lines(NC))
DEFAULT = {
    "rename_movies": re.search(r"RenameMovies = (\w+)", _nc)[1] == "true",
    "replace_illegal_characters": re.search(r"ReplaceIllegalCharacters = (\w+)", _nc)[1] == "true",
    "colon_replacement_format": re.search(r"ColonReplacementFormat = ColonReplacementFormat\.(\w+)", _nc)[1],
    "movie_folder_format": re.search(r'MovieFolderFormat = "([^"]+)"', _nc)[1],
    "standard_movie_format": re.search(r'StandardMovieFormat = "([^"]+)"', _nc)[1],
}
assert DEFAULT == {"rename_movies": False, "replace_illegal_characters": True, "colon_replacement_format": "Smart",
                   "movie_folder_format": "{Movie Title} ({Release Year})",
                   "standard_movie_format": "{Movie Title} ({Release Year}) {Quality Full}"}, DEFAULT
DEFAULT_NOTE = f"Naming starts from NamingConfig.Default ({src_at(NC, r'public static NamingConfig Default')})."
REV_NOTE = (f"Revision() defaults to version 1, real 0 ({src_at(C + 'Qualities/Revision.cs', r'public Revision\(\)')}).")
OUR_TOKENS = ["{Movie Title}", "{Release Year}", "{Quality Full}"]

# Our live naming config (captured read-only in phase 1), in the corpus's keys.
_ours = json.loads((OUT / "corpus/captured/radarr/config/naming.json").read_text())
OURS = {"rename_movies": _ours["renameMovies"], "replace_illegal_characters": _ours["replaceIllegalCharacters"],
        "colon_replacement_format": _ours["colonReplacementFormat"],
        "movie_folder_format": _ours["movieFolderFormat"], "standard_movie_format": _ours["standardMovieFormat"]}
READS = {  # naming settings each op reads (FileNameBuilder.cs)
    "build_file_name": ("rename_movies", "replace_illegal_characters", "colon_replacement_format", "standard_movie_format"),
    "get_movie_folder": ("replace_illegal_characters", "colon_replacement_format", "movie_folder_format"),
    "clean_file_name": ("replace_illegal_characters", "colon_replacement_format"),
}


def naming(**over):
    return {**DEFAULT, **over}


def off_config(op, nm):
    """Note naming settings this op reads that differ from ours (formats only when they use other tokens)."""
    diffs = []
    for k in READS[op]:
        v, o = nm[k], OURS[k]
        if k.endswith("_format") and k != "colon_replacement_format":
            if not ours(v):
                diffs.append(f"{k} {json.dumps(v)} uses tokens we do not use (ours {json.dumps(o)})")
        elif str(v).lower() != str(o).lower():
            diffs.append(f"{k} {json.dumps(v)} (ours {json.dumps(o)})")
    return f"Off our config (corpus/captured/radarr/config/naming.json): {'; '.join(diffs)}." if diffs else None


def ours(fmt):
    """True when the format only uses the tokens our config uses, spelled as we spell them."""
    rest = fmt
    for t in OUR_TOKENS:
        rest = rest.replace(t, "")
    return "{" not in rest


def movie_file(version=1, real=0, **extra):
    return {"quality": "HDTV-720p", "revision": {"version": version, "real": real}, "release_group": "RadarrTest", **extra}


cases = []  # dicts: file, ref, behaviour, in_scope, input?, expect?, note?


def add(file, ref, behaviour, input=None, expect=None, note=None, in_scope=True):
    assert (input is None) == (not in_scope), ref
    if input is not None:
        note = " ".join(x for x in [note, off_config(input["op"], input["given"]["naming"])] if x) or None
    cases.append(dict(file=file, ref=ref, key=KEY, behaviour=behaviour, in_scope=in_scope,
                      input=input, expect=expect, note=note))


def build(file, ref, behaviour, nm, movie, mf, expect, note=None, op="build_file_name"):
    given = {"naming": nm, "movie": movie}
    if mf is not None:
        given["movie_file"] = mf
    add(file, ref, behaviour, {"op": op, "given": given}, {"name": expect},
        " ".join(x for x in [DEFAULT_NOTE, REV_NOTE if mf else None, note] if x))


# --- FileNameBuilderFixture -------------------------------------------------------------
# Setup (lines 31-74): movie "South Park", file HDTV-720p, release group "RadarrTest", rename on.
SP = {"title": "South Park"}
FB_IN = {
    "should_replace_Movie_space_Title": ("{Movie Title}", {}, "South Park",
                                         "{Movie Title} renders the movie title as is"),
    "should_replace_quality_full_with_quality_title_only_when_not_a_proper":
        ("{Movie Title} [{Quality Full}]", {}, "South Park [HDTV-720p]",
         "{Quality Full} renders only the quality title when the file is not a proper"),
    "should_replace_quality_full_with_quality_title_and_proper_only_when_a_proper":
        ("{Movie Title} [{Quality Full}]", {"version": 2}, "South Park [HDTV-720p Proper]",
         "{Quality Full} appends 'Proper' for a revision 2 file"),
    "should_replace_quality_full_with_quality_title_and_real_when_a_real":
        ("{Movie Title} [{Quality Full}]", {"real": 1}, "South Park [HDTV-720p REAL]",
         "{Quality Full} appends 'REAL' for a real 1 file"),
}
# Out-of-scope [Test]s: what each one tests (tokens or settings our config does not use).
FB_OUT = {
    "should_replace_Movie_underscore_Title": "{Movie_Title} joins title words with underscores",
    "should_replace_Movie_dot_Title": "{Movie.Title} joins title words with dots",
    "should_replace_Movie_dash_Title": "{Movie-Title} joins title words with dashes",
    "should_replace_MOVIE_TITLE_with_all_caps": "{MOVIE TITLE} upper-cases the title",
    "should_replace_MOVIE_TITLE_with_random_casing_should_keep_original_casing": "{mOvIe-tItLE} keeps the title's own casing",
    "should_replace_movie_title_with_all_lower_case": "{movie title} lower-cases the title",
    "should_replace_translated_movie_title": "{Movie Title:FR} renders the French translation",
    "should_replace_translated_movie_title_with_base_title_if_invalid_code": "{Movie Title:JP} with an invalid language code falls back to the title",
    "should_replace_translated_movie_title_with_base_title_if_no_translation_exists": "{Movie Title:JA} with no translation falls back to the title",
    "should_replace_translated_movie_title_with_fallback_if_no_translation_exists": "{Movie Title:JP|FR} falls back to the second language's translation",
    "should_replace_translated_movie_title_with_original_if_no_translation_or_fallback_exists": "{Movie Title:JP|CN} with no translation in either falls back to the title",
    "should_cleanup_Movie_Title": "{Movie.CleanTitle} cleans 'South Park (1997)' to 'South.Park.1997'",
    "should_replace_movie_original_title": "{Movie OriginalTitle} renders the original-language title",
    "should_replace_movie_certification": "{Movie Certification} renders the certification",
    "should_replace_movie_collection": "{Movie Collection} renders the collection title",
    "should_be_empty_for_null_collection": "{Movie Collection} renders empty when there is no collection",
    "should_replace_quality_title": "{Quality Title} renders the quality name",
    "should_replace_quality_proper_with_proper": "{Quality Proper} renders 'Proper' for a proper",
    "should_replace_quality_real_with_real": "{Quality Real} renders 'REAL' for a real",
    "should_replace_all_contents_in_pattern": "{Movie Title} [{Quality Title}] renders both tokens",
    "use_file_name_when_sceneName_is_null": "With renaming off and no scene name, the file name is the relative path's",
    "use_path_when_sceneName_and_relative_path_are_null": "With renaming off and no scene name or relative path, the file name is the path's",
    "use_file_name_when_sceneName_is_not_null": "With renaming off, the file name is the scene name",
    "should_should_replace_release_group": "{Release Group} renders the release group",
    "should_be_able_to_use_original_title": "{Original Title} renders the scene name",
    "should_include_affixes_if_value_not_empty": "{_Quality.Title_} keeps its affixes when the value is not empty",
    "should_format_mediainfo_properly": "{MEDIAINFO.FULL} renders codecs and audio/subtitle languages",
    "should_exclude_english_in_mediainfo_audio_language": "{MEDIAINFO.FULL} leaves out English-only audio languages",
    "should_format_mediainfo_3d_properly": "{MEDIAINFO.3D} and {MediaInfo.Simple} render a 3D file",
    "should_remove_duplicate_non_word_characters": "{Movie.Title} renders 'Venture Bros.' as 'Venture.Bros'",
    "should_use_existing_filename_when_scene_name_is_not_available": "{Original Title} falls back to the existing file name without a scene name",
    "should_be_able_to_use_only_original_title": "{Original Title} alone renders the scene name",
    "should_not_include_quality_proper_when_release_is_not_a_proper": "{Quality Proper} renders empty and its separator is trimmed for a non-proper",
    "should_wrap_proper_in_square_brackets": "{[Quality Proper]} wraps 'Proper' in brackets",
    "should_not_wrap_proper_in_square_brackets_when_not_a_proper": "{[Quality Proper]} renders nothing for a non-proper",
    "should_be_able_to_use_original_filename_only": "{Original Filename} renders the relative path's name",
    "should_use_Radarr_as_release_group_when_not_available": "{Release Group} renders 'Radarr' when the file has none",
    "should_update_media_info_if_token_configured_and_revision_is_old": "A media-info token with an old media-info schema triggers a media-info update (mock verify)",
    "should_not_update_media_info_if_no_movie_path_available": "No media-info update without a movie path (mock verify)",
    "should_not_update_media_info_if_token_not_configured_and_revision_is_old": "No media-info update when no media-info token is used (mock verify)",
    "should_not_update_media_info_if_token_configured_and_revision_is_current": "No media-info update when the media-info schema is current (mock verify)",
    "should_not_update_media_info_if_token_configured_and_revision_is_newer": "No media-info update when the media-info schema is newer (mock verify)",
}
FB_OUT_ROWS = {  # parameterised out-of-scope tests: args -> sentence
    "should_format_languagecodes_properly": lambda a: f"{{MEDIAINFO.FULL}} renders subtitle language '{a[0]}' as [{a[1]}]",
    "should_trim_extra_separators_from_end_when_quality_proper_is_not_included": lambda a: f"With separator '{a[0]}', an empty {{Quality Proper}} at the end is trimmed with its separator",
    "should_trim_extra_separators_from_middle_when_quality_proper_is_not_included": lambda a: f"With separator '{a[0]}', an empty {{Quality Proper}} in the middle is trimmed with its separator",
    "should_replace_all_tokens_for_different_cultures": lambda a: f"Tokens incl. {{Movie CleanTitle}}, {{TmdbId}} and {{MediaInfo AudioCodec}} render the same under culture {a[0]}",
    "should_not_use_Radarr_as_release_group_if_pattern_has_separator": lambda a: f"'{a[0]}' renders '{a[1]}' when the file has no release group",
    "should_use_existing_casing_for_release_group": lambda a: f"{{Release Group}} keeps the casing of '{a[0]}'",
    "should_format_audio_languages": lambda a: f"{{MediaInfo AudioLanguages}} renders '{a[0]}' as '{a[1]}'",
    "should_format_audio_languages_all": lambda a: f"{{MediaInfo AudioLanguagesAll}} renders '{a[0]}' as '{a[1]}'",
    "should_format_subtitle_languages_all": lambda a: f"{{MediaInfo SubtitleLanguages{a[1]}}} renders '{a[0]}' as '{a[2]}'",
    "should_include_hdr_for_mediainfo_videodynamicrange_with_valid_properties": lambda a: f"{{MediaInfo VideoDynamicRange}} with HDR format {a[0]} renders '{a[1]}'",
}

ILLEGAL = "should_replace_illegal_characters_when_renaming_is_disabled"
for method, line, args, KEY in scan(FB):
    ref = f"{TAG} {FB}:{line}"
    if method in FB_IN:
        fmt, rev, expect, beh = FB_IN[method]
        check(FB, method, fmt, expect)
        build("file_name.json", ref, f"{beh}: 'South Park' as HDTV-720p renders '{expect}'",
              naming(rename_movies=True, standard_movie_format=fmt), SP, movie_file(**rev), expect)
    elif method == ILLEGAL:
        scene, rel_path, expect = "30.Rock.S01E01.xvid:LOL", "30 Rock - S01E01 - Test", "30.Rock.S01E01.xvid-LOL"
        check(FB, method, scene, rel_path, expect)
        assert "ColonReplacementFormat.Smart" in body(FB, method)
        build("file_name.json", ref,
              f"With renaming off, the scene name is still cleaned: smart colon replacement turns '{scene}' into '{expect}'",
              naming(rename_movies=False, replace_illegal_characters=True, colon_replacement_format="Smart"), SP,
              movie_file(scene_name=scene, relative_path=rel_path), expect)
    elif method in FB_OUT:
        assert args is None, (method, line)
        add(None, ref, FB_OUT[method], in_scope=False)
    elif method in FB_OUT_ROWS:
        assert args is not None, (method, line)
        add(None, ref, FB_OUT_ROWS[method](args), in_scope=False)
    else:
        raise AssertionError(f"unhandled {FB}:{line} {method}")

# --- ColonReplacementFixture (every row: colon is an illegal path character) ---------------
# Setup (lines 22-45): movie "CSI: Vegas", file HDTV-720p, release group "RadarrTest", rename on.
for method, line, args, KEY in scan(CR):
    ref = f"{TAG} {CR}:{line}"
    if method == "should_replace_colon_followed_by_space_with_space_dash_space_by_default":
        check(CR, method, "{Movie Title}", "CSI - Vegas")
        title, mode, expect = "CSI: Vegas", DEFAULT["colon_replacement_format"], "CSI - Vegas"
        beh = f"Default colon replacement ({mode}) renders '{title}' as '{expect}'"
    elif method in ("should_replace_colon_followed_by_space_with_expected_result", "should_replace_colon_with_expected_result"):
        title, mode, expect = args
        beh = f"Colon replacement {mode} renders '{title}' as '{expect}'"
    else:
        raise AssertionError(f"unhandled {CR}:{line} {method}")
    build("colon_replacement.json", ref, beh,
          naming(rename_movies=True, standard_movie_format="{Movie Title}", colon_replacement_format=mode),
          {"title": title}, movie_file(), expect)

# --- ReservedDeviceNameFixture (every row: reserved device names are illegal paths) --------
for method, line, args, KEY in scan(RD):
    ref = f"{TAG} {RD}:{line}"
    title, year, expect = args
    fmt = "{Movie.Title} ({Release Year})"
    if method == "should_replace_reserved_device_name_in_movies_folder":
        check(RD, method, fmt)
        build("reserved_device_name.json", ref,
              f"Folder format '{fmt}' renders '{title}' ({year}) as '{expect}' (reserved device name escaped)",
              naming(rename_movies=True, movie_folder_format=fmt), {"title": title, "year": year}, None, expect,
              op="get_movie_folder")
    elif method == "should_replace_reserved_device_name_in_file_name":
        check(RD, method, fmt)
        build("reserved_device_name.json", ref,
              f"File format '{fmt}' renders '{title}' ({year}) as '{expect}' (reserved device name escaped)",
              naming(rename_movies=True, standard_movie_format=fmt), {"title": title, "year": year}, movie_file(), expect)
    else:
        raise AssertionError(f"unhandled {RD}:{line} {method}")

# --- CleanFilenameFixture (every row: path cleaning) ---------------------------------------
CF_BEH = {
    "should_replace_invalid_characters": "replaces invalid characters",
    "should_remove_periods_from_start": "removes leading periods",
    "should_remove_spaces_from_start_and_end": "trims leading and trailing spaces",
}
CF_NOTE = (f"FileNameBuilder.CleanFileName(name) uses NamingConfig.Default "
           f"({src_at(C + 'Organizer/FileNameBuilder.cs', r'return CleanFileName\(name, NamingConfig\.Default\)')}).")
for method, line, args, KEY in scan(CF):
    name, expect = args
    add("clean_file_name.json", f"{TAG} {CF}:{line}",
        f"CleanFileName {CF_BEH[method]}: {json.dumps(name)} becomes {json.dumps(expect)}",
        {"op": "clean_file_name", "given": {"naming": naming(), "name": name}}, {"name": expect}, CF_NOTE)

# --- GetMovieFolderFixture (rows using our folder format) -----------------------------------
for method, line, args, KEY in scan(GF):
    ref = f"{TAG} {GF}:{line}"
    if method == "should_use_movieFolderFormat_to_build_folder_name":
        title, year, fmt, expect = args
        movie, collection = {"title": title, "year": year}, None
    elif method == "should_use_movieFolderFormat_and_CollectionFormat_to_build_folder_name":
        collection, title, year, fmt, exp_collection, exp_title = args
        movie = {"title": title, "year": year, "collection_title": collection}
        expect = f"{exp_collection}/{exp_title}" if exp_collection else exp_title  # Path.Combine on Linux
    else:
        raise AssertionError(f"unhandled {GF}:{line} {method}")
    beh = f"Folder format '{fmt}' renders '{title}' ({year})" + (f" in collection '{collection}'" if collection else "")
    if ours(fmt):
        build("movie_folder.json", ref, f"{beh} as '{expect}'", naming(movie_folder_format=fmt), movie, None, expect,
              op="get_movie_folder")
    else:
        add(None, ref, f"{beh} as '{expect}' (token not in our formats)", in_scope=False)

# --- IDs, catalogue, corpus ------------------------------------------------------------------
SKIPPED_ROWS = {f: len(scan(T + "FileNameBuilderTests/" + f)) for f in SKIPPED}
assert set(p.name for p in (SRC / T).rglob("*Fixture.cs")) == set(SKIPPED) | {Path(x).name for x in (FB, CR, RD, CF, GF)}
cases.sort(key=lambda c: (c["ref"].split(" ", 1)[1].rsplit(":", 1)[0], int(c["ref"].rsplit(":", 1)[1])))
keys = [c["key"] for c in cases]
assert len(set(keys)) == len(keys), "key is the ID key and must be unique"

old = []  # (id, key or None, ref, carried lines, block text)
if CATALOGUE.exists():
    for block in re.split(r"(?m)^(?=- id: )", CATALOGUE.read_text()):
        if not block.startswith("- id: "):
            continue
        bkey = re.search(r"(?m)^  # key: (.*)$", block)
        old.append((re.match(r"- id: (\S+)", block)[1], json.loads(bkey[1]) if bkey else None,
                    json.loads(re.search(r"(?m)^  ref: (.*)$", block)[1]),
                    re.findall(r"(?m)^  ((?:status|decision|tests):.*(?:\n    .*)*)", block), block))
by_key = {k: o for o in old if (k := o[1])}
by_ref = {o[2]: o for o in old if o[1] is None}  # entries written before keys existed
next_n = max([int(o[0].rsplit("-", 1)[1]) for o in old] + [0]) + 1
out, by_file, used = [], {}, set()
for c in cases:
    prev = by_key.get(c["key"]) or by_ref.get(c["ref"])
    if prev:
        cid, keep = prev[0], prev[3]
    else:
        cid, keep = f"{PREFIX}{next_n:04d}", ["status: unmapped", "tests: []"]
        next_n += 1
    used.add(cid)
    out.append(f"- id: {cid}\n  # key: {json.dumps(c['key'], ensure_ascii=False)}\n  behaviour: {json.dumps(c['behaviour'], ensure_ascii=False)}\n  source: reference\n"
               f"  ref: {json.dumps(c['ref'], ensure_ascii=False)}\n  in_scope: {str(c['in_scope']).lower()}\n"
               + "".join(f"  {k}\n" for k in keep))
    if c["in_scope"]:
        case = {"id": cid, "ref": c["ref"], "input": c["input"], "expect": c["expect"]}
        if c["note"]:
            case["note"] = c["note"]
        by_file.setdefault(c["file"], []).append(case)

for o in old:
    if o[0] not in used:
        print(f"warning: {o[0]} ({o[2]}) no longer extracted; kept, never reused", file=sys.stderr)
        out.append(o[4] if o[4].endswith("\n") else o[4] + "\n")

CATALOGUE.parent.mkdir(parents=True, exist_ok=True)
CATALOGUE.write_text("# Generated by parity/extract/radarr_naming.py; IDs are stable, edit status/decision/tests only.\n"
                     + "".join(out))
CORPUS.mkdir(parents=True, exist_ok=True)
for f, cs in sorted(by_file.items()):
    (CORPUS / f).write_text(json.dumps(cs, indent=1, ensure_ascii=False) + "\n")
print("ruled out (fixture: rows):", ", ".join(f"{f[:-3]} {n}" for f, n in SKIPPED_ROWS.items()),
      f"= {sum(SKIPPED_ROWS.values())} rows in {len(SKIPPED)} fixtures")
print(f"{len(cases)} entries ({sum(c['in_scope'] for c in cases)} in scope), {len(by_file)} corpus files")
