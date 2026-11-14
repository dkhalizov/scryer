"""Port the Bazarr subtitle tests our config relies on to parity/corpus/reference/bazarr/subtitles
and parity/catalogue/subtitles.bazarr.yaml (notes/11-parity-net-design.md, phase 2).

Run from anywhere:  python3 -I parity/extract/bazarr_subtitles.py

Scope (our config: parity/corpus/captured/bazarr/config/*.json): profile 'English' (en, normal or
HI, not forced, SRT), providers opensubtitlescom, podnapisi, tvsubtitles, our score weights,
min score 90 % / 70 %, upgrades on, language_equals empty, subzero_mods empty.
Every test of a ported file gets an entry; tests that are not data, or that only exercise a
setting we leave empty, are in_scope false with no corpus case. Behaviours our config relies
on that no reference test covers are catalogued with a ref to the implementing line (SOURCE
below), catalogue only: there is no test to take an expected value from.

Files ruled out entirely (no entries), with the config fact that rules them out:
- tests/subliminal_patch/test_utils.py: no enabled provider imports
  subliminal_patch/providers/utils.py (podnapisi unpacks archives with
  ProviderSubtitleArchiveMixin, mixins.py:64; opensubtitlescom and tvsubtitles never do).
- test_mods.py (subzero_mods empty), test_embeddedsubtitles.py (provider not enabled),
  the provider files (addic7ed, subdl, titlovi, karagarga, napiprojekt, subf2m, gestdown,
  hosszupuska, supersubtitles, subtitrarinoi, animesubinfo, animetosho, argenteam_dump).
- tests/bazarr/test_logging_filters.py, test_ui.py: logging and login, not subtitles.
- tests/bazarr/test_utilities_video_analyzer.py, test_sonarr_sync_episodes.py: area scan.
- frontend/src/**/*.test.ts(x) (31 files, 69 it()/test()): web UI rendering and form state;
  Scryer has its own UI. The two about languages are UI too: pages/Settings/Languages/
  equals.test.ts parses the language_equals setting string (ours is empty) and
  components/bazarr/Language.test.tsx renders language labels.

IDs: the key (YAML comment above each entry) is path | test or source slug | row; an entry keeps
its ID and its status/decision/tests lines on re-run; new keys take max + 1 in file/line order.
"""
import ast
import json
import re
from pathlib import Path

TAG = "Bazarr@v1.5.6"
SRC = Path.home() / "PycharmProjects/arr-reference/bazarr@v1.5.6"
OUT = Path(__file__).resolve().parents[1]
CATALOGUE = OUT / "catalogue/subtitles.bazarr.yaml"
CORPUS = OUT / "corpus/reference/bazarr/subtitles"
PREFIX = "BAZ-SUBTITLES-"
_lines = {}


def lines(rel):
    if rel not in _lines:
        _lines[rel] = (SRC / rel).read_text(encoding="utf-8").splitlines()
    return _lines[rel]


def def_line(rel, name):
    hits = [i + 1 for i, l in enumerate(lines(rel)) if re.match(rf"def {name}\(", l)]
    assert len(hits) == 1, (rel, name, hits)
    return hits[0]


def row_line(rel, name, needle):
    """Line of the parametrize row containing `needle`, between the previous def and `name`."""
    end = def_line(rel, name)
    start = max([i + 1 for i, l in enumerate(lines(rel)[:end - 1]) if l.startswith("def ")] + [0])
    hits = [i + 1 for i in range(start, end - 1) if needle in lines(rel)[i]]
    assert len(hits) == 1, (rel, name, needle, hits)
    return hits[0]


def src_line(rel, needle, after=None):
    """First line containing `needle` (after the first line containing `after`, if given)."""
    ls = lines(rel)
    start = 0 if after is None else next(i for i, l in enumerate(ls) if after in l)
    return next(i + 1 for i in range(start, len(ls)) if needle in ls[i])


entries = []  # (rel, line, slug, behaviour, in_scope, file, input, expect, note)


def test(rel, name, behaviour, file=None, input=None, expect=None, note=None, row=None, in_scope=True):
    line = row_line(rel, name, row) if row else def_line(rel, name)
    entries.append((rel, line, name + (f"|{row}" if row else ""), behaviour, in_scope, file, input, expect, note))


def out(rel, name, what, why, row=None):
    test(rel, name, f"{what}; not ported: {why}", row=row, in_scope=False)


def source(rel, needle, slug, behaviour, after=None):
    entries.append((rel, src_line(rel, needle, after), slug,
                    "No reference test; implementing source. " + behaviour, True, None, None, None, None))


def literal(rel, name):
    for node in ast.parse((SRC / rel).read_text(encoding="utf-8")).body:
        if isinstance(node, ast.Assign) and any(getattr(t, "id", None) == name for t in node.targets):
            return ast.literal_eval(node.value)
    raise KeyError(name)


# Fixture videos (tests/subliminal_patch/conftest.py:24, :120), reference terms kept.
DUNE = {"kind": "Movie", "name": "Dune.2021.1080p.WEBRip.DD5.1.x264-SHITBOX", "title": "Dune", "year": 2021,
        "resolution": "1080p", "source": "Web", "imdb_id": "tt1160419", "alternative_titles": ["Dune: Part One"],
        "audio_codec": "Dolby Digital", "video_codec": "H.264", "release_group": "SHITBOX"}
BREAKING_BAD = {"kind": "Episode", "name": "Breaking.Bad.S01E01.720p.BluRay.X264-REWARD.mkv", "series": "Breaking Bad",
                "season": 1, "episode": 1, "source": "Blu-Ray", "series_tvdb_id": 81189,
                "series_imdb_id": "tt0903747", "release_group": "REWARD", "resolution": "720p",
                "video_codec": "H.264"}

# --- tests/subliminal_patch/test_score.py ------------------------------------------------------
SC = "tests/subliminal_patch/test_score.py"
DEFAULTS = literal("custom_libs/subliminal_patch/score.py", "DEFAULT_SCORES")
assert [l for l in lines(SC) if l.startswith("def test")] == [
    "def test_compute_score_set_var(movies, languages):",
    "def test_compute_score_set_var_w_episode(episodes, languages):",
    "def test_compute_score_defaults():",
    "def test_compute_score_custom_invalid():",
    "def test_compute_score_custom_valid():"]
SMOKE = "the test only asserts that scoring does not raise"
test(SC, "test_compute_score_set_var",
     "Scoring an English subtitle against a movie with matches {hash} does not raise", "score.json",
     {"op": "compute_score", "given": {"matches": ["hash"], "subtitle": {"language": "en", "provider": "karagarga"},
                                       "video": DUNE}},
     {"no_error": True}, SMOKE)
test(SC, "test_compute_score_set_var_w_episode",
     "Scoring an English subtitle against an episode with matches {hash} does not raise", "score.json",
     {"op": "compute_score", "given": {"matches": ["hash"], "subtitle": {"language": "en", "provider": "karagarga"},
                                       "video": BREAKING_BAD}},
     {"no_error": True}, SMOKE)
test(SC, "test_compute_score_defaults",
     "With no custom weights the scorer uses the default weights (episode hash 359 ... movie hash 119)",
     "score.json", {"op": "compute_score_weights", "given": {"custom": None}}, {"weights": DEFAULTS},
     "Expected table is score.DEFAULT_SCORES (custom_libs/subliminal_patch/score.py:31), read from the source.")
test(SC, "test_compute_score_custom_invalid",
     "Custom weights whose non-hash weights minus 1 do not equal the hash weight are rejected and the defaults used",
     "score.json",
     {"op": "compute_score_weights", "given": {"custom": {"movie": {"hash": 120}, "episode": {"hash": 321}}}},
     {"weights": DEFAULTS},
     "Rule in score.py:62 (_check_hash_sum): sum of the other weights - 1 must equal hash.")
custom = json.loads(json.dumps(DEFAULTS))
custom["movie"].update(release_group=12, source=8)
test(SC, "test_compute_score_custom_valid",
     "Custom movie weights release_group 12 and source 8 (sum unchanged) are accepted and used", "score.json",
     {"op": "compute_score_weights", "given": {"custom": custom}},
     {"weights": {"movie": {"release_group": 12, "source": 8}}},
     "The test copies DEFAULT_SCORES shallowly and edits two movie weights; only those two are asserted.")

# --- tests/subliminal_patch/test_subtitle.py ---------------------------------------------------
ST = "tests/subliminal_patch/test_subtitle.py"
for name, video_ed, guess_ed, matched in [
        ("test_guess_matches_w_edition_only_video", "Director's Cut", None, False),
        ("test_guess_matches_w_edition_only_guess", None, "Director's Cut", False),
        ("test_guess_matches_w_edition_both", "Director's Cut", "Director's Cut", True),
        ("test_guess_matches_w_edition_both_empty", None, None, True)]:
    side = {(True, False): "only the video has", (False, True): "only the release guess has",
            (True, True): "both have", (False, False): "neither has"}[(bool(video_ed), bool(guess_ed))]
    test(ST, name, f"When {side} an edition, 'edition' is {'' if matched else 'not '}a match", "subtitle.json",
         {"op": "guess_matches", "given": {"video": {**DUNE, "edition": video_ed},
                                           "guess": {"edition": guess_ed} if guess_ed else {}}},
         {"edition_in_matches": matched})
assert sum(l.startswith("def test") for l in lines(ST)) == 4

# --- tests/subliminal_patch/test_core.py -------------------------------------------------------
CO = "tests/subliminal_patch/test_core.py"
for name, fname, kind in [("test_scan_video_movie", "Taxi Driver 1976 Bluray 720p x264.mkv", "Movie"),
                          ("test_scan_video_episode", "The Wire S01E01 Bluray 720p x264.mkv", "Episode")]:
    test(CO, name, f"A video file named '{fname}' is scanned as a {kind}", "core.json",
         {"op": "scan_video", "given": {"file_name": fname}}, {"kind": kind},
         "The test creates an empty file with this name and scans it.")


def pool(name, behaviour, update, providers, discarded=None, before_discarded=None):
    given = {"providers": ["opensubtitlescom"], "update_providers": update}
    if before_discarded is not None:
        given["discarded_providers"] = before_discarded
    expect = {"providers": providers}
    if discarded is not None:
        expect["discarded_providers"] = discarded
    test(CO, name, behaviour, "core.json", {"op": "provider_pool_update", "given": given}, expect,
         "Pool built with providers {opensubtitlescom}; update() takes the enabled provider set.")


pool("test_pool_update_w_nothing", "Updating the provider pool with no enabled providers leaves no providers and none discarded",
     [], [], [])
pool("test_pool_update_w_multiple_providers", "Updating the provider pool with two enabled providers makes exactly those the pool's providers",
     ["opensubtitlescom", "subf2m"], ["opensubtitlescom", "subf2m"])
pool("test_pool_update_discarded_providers",
     "A provider discarded at run time that is still enabled is used again after a pool update and leaves the discarded set",
     ["opensubtitlescom", "subf2m"], ["opensubtitlescom", "subf2m"], [], ["opensubtitlescom"])
pool("test_pool_update_discarded_providers_2",
     "A provider discarded at run time that is no longer enabled stays discarded after a pool update",
     ["subf2m"], ["subf2m"], ["opensubtitlescom"], ["opensubtitlescom"])
LE = "language_equals is empty in our settings (BAZ-SUBTITLES-5022)"
out(CO, "test_language_equals_init", "Language equivalence spa -> spa-MX can be built", LE)
out(CO, "test_language_equals_init_invalid", "A language equivalence with one language raises ValueError", LE)
test(CO, "test_language_equals_init_empty_list_gracefully",
     "An empty language-equivalence list is accepted and equals an empty list", "core.json",
     {"op": "language_equals", "given": {"equals": []}}, {"equals": []})
for row in ['core.Language("spa"), core.Language("spa", "MX")', 'core.Language("por"), core.Language("por", "BR")',
            'core.Language("zho"), core.Language("zho", "TW")']:
    out(CO, "test_language_equals_check_set", "A language set is widened with the equivalent regional language", LE, row=row)
NOTE_LANG = "Languages in Bazarr's language_equals notation (get_providers.py:147): code[-COUNTRY][@forced|@hi]."
test(CO, "test_language_equals_check_set_do_nothing",
     "A language set is left unchanged when no equivalence starts from one of its languages (eng -> spa, set {spa})",
     "core.json", {"op": "language_equals_check_set", "given": {"equals": [["eng", "spa"]], "languages": ["spa"]}},
     {"languages": ["spa"]}, NOTE_LANG)
test(CO, "test_language_equals_check_set_do_nothing_w_forced",
     "A forced-only equivalence (spa@forced -> spa-MX) does not widen a plain {spa} language set",
     "core.json",
     {"op": "language_equals_check_set", "given": {"equals": [["spa@forced", "spa-MX"]], "languages": ["spa"]}},
     {"languages": ["spa"]}, NOTE_LANG)
LIVE = "it lists subtitles from the live OpenSubtitles.com API (no cassette), so nothing in it is fixed data"
out(CO, "test_language_equals_pool_intance_list_subtitles",
    "With spa -> spa-MX, Spanish subtitles listed for Dune all come back as spa-MX", f"{LE}; {LIVE}")
out(CO, "test_language_equals_pool_intance_list_subtitles_reversed",
    "With spa-MX -> spa, Spanish subtitles listed for Dune all come back as spa", f"{LE}; {LIVE}")
out(CO, "test_language_equals_pool_intance_list_subtitles_empty_lang_equals",
    "With no language equivalences, Spanish subtitles listed for Dune are not all relabelled spa-MX", LIVE)
out(CO, "test_language_equals_pool_intance_list_subtitles_return_nothing",
    "With spa-MX -> eng and spa -> eng, nothing is downloaded for a Spanish search", f"{LE}; {LIVE}")
assert sum(l.startswith("def test") for l in lines(CO)) == 16

# --- tests/bazarr/test_app_get_providers.py ----------------------------------------------------
GP = "tests/bazarr/test_app_get_providers.py"
AUTH = "checks the internal provider-credential wiring (dict shapes, constructor parameters), not a behaviour"
out(GP, "test_get_providers_auth", "Every provider auth entry is a dict", AUTH)
out(GP, "test_get_providers_auth_with_provider_registry", "Every provider auth key is a constructor parameter of its provider", AUTH)
out(GP, "test_get_providers_auth_embeddedsubtitles", "embeddedsubtitles auth entry has typed fields", AUTH + "; provider not enabled")
out(GP, "test_get_providers_auth_karagarga", "karagarga auth entry has credentials", AUTH + "; provider not enabled")
test(GP, "test_get_language_equals_default_settings",
     "With default settings (language_equals unset) the parsed language-equivalence list is a list", "app_get_providers.json",
     {"op": "get_language_equals", "given": {"settings": "default"}}, {"is_list": True},
     "The default is [] (bazarr/app/config.py:168), the same as ours; the test asserts only the type.")
out(GP, "test_get_language_equals_injected_settings_invalid", "An entry without ':' is ignored, giving an empty list", LE)
out(GP, "test_get_language_equals_injected_settings_valid", "'spa:spa-MX' parses to (spa, spa-MX)", LE)
for row in ['["spa:spl"]', '["por:pob"]', '["zho:zht"]']:
    out(GP, "test_get_language_equals_injected_settings_custom_lang_alpha3",
        f"Custom alpha3 code {row} parses to its regional language", LE, row=row)
out(GP, "test_get_language_equals_injected_settings_multiple", "Four entries with @hi parse to four pairs", LE)
out(GP, "test_get_language_equals_injected_settings_valid_multiple", "Two entries parse to two pairs in order", LE)
out(GP, "test_get_language_equals_injected_settings_hi", "'eng@hi:eng' parses to (eng HI, eng)", LE)
out(GP, "test_get_traceback_info", "Provider error tracebacks are cut to 100 characters", "logging internals")
assert sum(l.startswith("def test") for l in lines(GP)) == 12

# --- tests/bazarr/test_config.py, test_subtitles_pool.py ---------------------------------------
CF = "tests/bazarr/test_config.py"
out(CF, "test_get_settings", "get_settings returns a dict", "type check on an internal API")
out(CF, "test_get_scores", "get_scores returns movie and episode weight dicts", "type check on an internal API")
SP = "tests/bazarr/test_subtitles_pool.py"
out(SP, "test_init_pool", "A movie provider pool can be built from settings", "smoke test of internals that needs a full app config")
out(SP, "test_pool_update", "A movie provider pool can be updated from settings", "smoke test of internals that needs a full app config")

# --- tests/subliminal_patch/test_video.py ------------------------------------------------------
VI = "tests/subliminal_patch/test_video.py"
test(VI, "test_video_fromguess_episode",
     "A video built from an episode guess keeps streaming_service and leaves other unset", "video.json",
     {"op": "video_fromguess", "given": {"name": "Breaking.Bad.S01E01.Bluray.mkv",
                                         "guess": {"type": "episode", "streaming_service": "foo", "random_key": "bar"}}},
     {"kind": "Episode", "streaming_service": "foo", "other": None})
test(VI, "test_video_fromguess_movie",
     "A video built from a movie guess keeps edition and other", "video.json",
     {"op": "video_fromguess", "given": {"name": "Taxi.Driver.1976.Bluray.mkv",
                                         "guess": {"type": "movie", "edition": "foo", "random_key": "bar", "other": "Proper"}}},
     {"kind": "Movie", "edition": "foo", "other": "Proper"})
test(VI, "test_video_fromname_episode",
     "'Breaking.Bad.S01E01.NF.WEB-DL.1080p.x264-FOO.mkv' parses to Breaking Bad S01E01, Web, Netflix, 1080p, H.264, group FOO",
     "video.json", {"op": "video_fromname", "given": {"name": "Breaking.Bad.S01E01.NF.WEB-DL.1080p.x264-FOO.mkv"}},
     {"series": "Breaking Bad", "title": None, "season": 1, "episode": 1, "source": "Web",
      "streaming_service": "Netflix", "resolution": "1080p", "video_codec": "H.264", "release_group": "FOO"})
test(VI, "test_video_fromname_movie",
     "'Some.Flick.2022.UHD.Bluray.Proper.2160p.FLAC.HEVC-FOO.mkv' parses to Some Flick (2022), Ultra HD Blu-ray, Proper, 2160p, H.265, FLAC",
     "video.json", {"op": "video_fromname", "given": {"name": "Some.Flick.2022.UHD.Bluray.Proper.2160p.FLAC.HEVC-FOO.mkv"}},
     {"source": "Ultra HD Blu-ray", "title": "Some Flick", "year": 2022, "other": "Proper", "resolution": "2160p",
      "video_codec": "H.265", "audio_codec": "FLAC"})
assert sum(l.startswith("def test") for l in lines(VI)) == 4

# --- Implementing source for behaviours our config uses that no test covers --------------------
DL, UT, UP = "bazarr/subtitles/download.py", "bazarr/subtitles/utils.py", "bazarr/subtitles/upgrade.py"
CORE, SCORE = "custom_libs/subliminal_patch/core.py", "custom_libs/subliminal_patch/score.py"
source(DL, 'hi_required = "force HI"', "hi_required",
       "Hearing-impaired subtitles are forced only when every searched language is HI-only; a profile item with hi "
       "'False' (ours) searches with 'don't prefer'")
source(SCORE, "and subtitle.hearing_impaired == hearing_impaired", "hi_match",
       "With 'don't prefer' the scorer is called with hearing_impaired False (core.py:552), so a non-HI subtitle gains "
       "the hearing_impaired match (+1, our weight) over an otherwise equal HI one")
source(UP, "results.append(item['language'])", "profile_hi_false_languages",
       "A profile item with hi 'False' and forced 'False' is satisfied by either the plain or the HI subtitle "
       "(desired languages 'en' and 'en:hi')", after="def _language_from_items")
source(UT, '"podnapisi": {\'also_foreign\'', "podnapisi_forced",
       "With no forced item in the profile (ours), podnapisi is queried with also_foreign and only_foreign False, so "
       "its foreign-only (forced) subtitles are dropped (podnapisi.py:234)")
source(SCORE, 'if subtitle.hash_verifiable and "hash" in matches', "hash_validation",
       "A hash match from a hash-verifiable provider (opensubtitlescom, opensubtitlescom.py:78) counts only when "
       "series, season, episode and source also match (movies: video_codec and source; specials are not checked); "
       "then every match but hash is dropped and hearing_impaired is added back when it matches (score.py:143), so "
       "under 'don't prefer' a non-HI subtitle scores 359 + 1 = 360 (movies 119 + 1 = 120) and an HI one 359 / 119; "
       "otherwise the hash match is dropped")
source(CORE, "if s.language.basename not in [x.basename for x in languages]", "best_language_filter",
       "Listed subtitles in a language that was not searched are skipped before scoring")
source(CORE, "scored_subtitles = sorted(unsorted_subtitles", "best_order",
       "Candidates are tried best first, ordered by score then score without hash; the first one below the minimum "
       "score ends the search, and one subtitle per language is downloaded")
source(CORE, "if can_verify_series and not matches_series", "episode_series_check",
       "An episode subtitle is skipped unless season and episode matched and series (or imdb_id) matched, except a "
       "hash match from a provider that cannot verify hashes")
source("bazarr/subtitles/tools/score.py", "self.max_score * (special or min_percent) / 100", "min_score",
       "The minimum score is max score x minimum_score % / 100, where max score = hash weight + hearing_impaired "
       "weight + custom score-profile scores (score.py:71; load_profiles, :34, always leaves the profiles empty in "
       "v1.5.6); with our weights 324 of 360 for episodes (90 %) and 84 of 120 for movies (70 %)")
source(DL, "min_score = int(forced_minimum_score) + 1", "upgrade_min_score",
       "An upgrade search passes the previous score + 1 (upgrade.py:137, :242) and this adds 1 more, so a "
       "replacement must score at least the previous score + 2")
source(UP, "(TableHistory.score < 357)", "upgrade_episode_cap",
       "Only episode subtitles scoring below 357 (or translated ones with no score) are upgrade candidates; the cap is "
       "hard-coded, 3 below our maximum of 360")
source(UP, "(TableHistoryMovie.score < 117)", "upgrade_movie_cap",
       "Only movie subtitles scoring below 117 (or translated ones with no score) are upgrade candidates; the cap is "
       "hard-coded, 3 below our maximum of 120")
source(UP, "query_actions = [1, 2, 3, 4, 6]", "upgrade_actions",
       "With upgrade_manual on, subtitles whose latest history action is 1, 2, 3, 4 or 6 (6 = translated; 5 = synced "
       "is never a candidate) and newer than days_to_upgrade_subs (ours 7) are upgrade candidates; with it off only 1 and 3")
source(CORE, "if subtitle.language in set(s.language.basename for s in saved_subtitles)", "save_one_per_language",
       "Only the first subtitle per language is saved; later ones of the same language are ignored")
source(CORE, "parse_for_hi_regex(subtitle_text=", "save_hi_detect",
       "An SRT subtitle not flagged hearing-impaired whose text matches the HI regex is saved as hearing-impaired")
source(CORE, "subtitle_root = os.path.splitext(video_path)[0]", "subtitle_path",
       "A subtitle is saved as <video without extension>.<language>[.forced | .<hi_extension>].srt, the language "
       "omitted only with single_language (off for us); with our hi_extension 'hi': Movie.en.srt, Movie.en.hi.srt")
source(UT, "def refine_video_with_scenename", "scene_name_refine",
       "When Sonarr/Radarr report a scene name, the video parsed from it fills only the properties the file path left "
       "empty (get_video, utils.py:36); the use_scenename setting has no reader in v1.5.6 besides its validator "
       "(config.py:100), so this happens regardless of it")
source("bazarr/app/get_providers.py", "if now < until:", "throttled_providers",
       "An enabled provider throttled until a later time is left out of the search; once that time passes it is used "
       "again; when none is left nothing is searched ('All providers are throttled', download.py:133)")

# --- IDs, catalogue, corpus --------------------------------------------------------------------
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
    elif not in_scope:
        skipped.append((eid, ref, behaviour))

CATALOGUE.parent.mkdir(parents=True, exist_ok=True)
CORPUS.mkdir(parents=True, exist_ok=True)
CATALOGUE.write_text(f"# Bazarr subtitle behaviours ({TAG}). Generated by parity/extract/bazarr_subtitles.py;\n"
                     "# status/decision/tests are kept on re-run, everything else is regenerated.\n\n"
                     + "\n".join(rows) + "\n", encoding="utf-8")
for f, cs in sorted(corpora.items()):
    (CORPUS / f).write_text(json.dumps(cs, indent=1, ensure_ascii=False, sort_keys=True) + "\n", encoding="utf-8")
n_src = sum(e[3].startswith("No reference test") for e in entries)
print(f"{len(entries)} entries: {sum(map(len, corpora.values()))} corpus cases, {n_src} source-only, "
      f"{len(skipped)} out of scope; {len(corpora)} corpus files")
for eid, ref, b in skipped:
    print(f"  out {eid} {ref}: {b}")
