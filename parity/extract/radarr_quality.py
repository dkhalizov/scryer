"""Port Radarr's quality-model suites to parity/corpus/reference/radarr/quality
and parity/catalogue/quality.radarr.yaml (notes/11-parity-net-design.md, phase 2).

Run from anywhere:  python3 -I parity/extract/radarr_quality.py
Same shape as sonarr_quality.py. Every ref line is located in the reference
source by anchor, so a wrong anchor fails loudly. IDs are keyed by ref
(path:line, unique per entry here) and preserved on re-run, with their
status/decision/tests lines.
"""
import json
import re
from pathlib import Path

TAG = "Radarr@v6.4.4.10685"
SRC = Path.home() / "PycharmProjects/arr-reference/radarr@v6.4.4.10685"
OUT = Path(__file__).resolve().parents[1]
CATALOGUE = OUT / "catalogue/quality.radarr.yaml"
CORPUS = OUT / "corpus/reference/radarr/quality"
PREFIX = "RAD-QUALITY-"

T = "src/NzbDrone.Core.Test/"
C = "src/NzbDrone.Core/"
_lines = {}


def lines(rel):
    if rel not in _lines:
        _lines[rel] = (SRC / rel).read_text(encoding="utf-8-sig").splitlines()
    return _lines[rel]


def method_line(rel, method):
    hits = [i for i, l in enumerate(lines(rel)) if re.search(rf"\bvoid {method}\(", l)]
    assert len(hits) == 1, (rel, method, hits)
    return hits[0]


def test_ref(rel, method):
    """Ref of the [Test] attribute above `method`."""
    i = method_line(rel, method)
    while not lines(rel)[i].strip().startswith("[Test]"):
        i -= 1
        assert i >= 0, (rel, method)
    return f"{TAG} {rel}:{i + 1}"


def testcase_rows(rel, method):
    """[(ref, args)] of the [TestCase(...)] rows directly above `method`."""
    i = method_line(rel, method) - 1
    rows = []
    while (s := lines(rel)[i].strip()).startswith("[TestCase(") or s == "":
        if s:
            rows.append((f"{TAG} {rel}:{i + 1}", re.fullmatch(r"\[TestCase\((.*)\)\]", s)[1]))
        i -= 1
    assert rows, (rel, method)
    return rows[::-1]


def line_ref(rel, pattern, start=0):
    hits = [i for i, l in enumerate(lines(rel)) if i >= start and re.search(pattern, l)]
    assert len(hits) == 1, (rel, pattern, hits)
    return hits[0], f"{TAG} {rel}:{hits[0] + 1}"


def src_at(rel, pattern):
    """'path:line' (no tag) of the one line matching pattern, for notes."""
    return f"{rel}:{line_ref(rel, pattern)[0] + 1}"


cases = []  # dicts: file, ref, behaviour, in_scope, input?, expect?, note?


def add(file, ref, behaviour, input=None, expect=None, note=None, in_scope=True):
    assert (input is None) == (not in_scope), ref
    cases.append(dict(file=file, ref=ref, behaviour=behaviour, in_scope=in_scope,
                      input=input, expect=expect, note=note))


# Quality names as Radarr declares them (Quality.cs), keyed by C# member name.
Q_CS = C + "Qualities/Quality.cs"
QNAME, QDECL = {}, []
for n, l in enumerate(lines(Q_CS)):
    m = re.search(r"public static Quality (\w+) => new Quality\((\d+), \"([^\"]+)\", QualitySource\.(\w+), (\d+)(?:, Modifier\.(\w+))?\);", l)
    if m and not l.strip().startswith("//"):
        QNAME[m[1]] = m[3]
        QDECL.append((n, m[1], int(m[2]), m[3], m[4], int(m[5]), m[6] or "NONE"))
assert len(QDECL) == 30, len(QDECL)


def q(expr):
    return QNAME[expr.removeprefix("Quality.").strip()]


REV_DEFAULT = (f"Revision() defaults to version 1, real 0 ({src_at(C + 'Qualities/Revision.cs', r'public Revision\(\)')}); "
               f"QualityModel(quality) uses it ({src_at(C + 'Qualities/QualityModel.cs', r'Revision = revision \?\? new Revision\(\)')})")

# --- CustomFormats/Specifications/LanguageSpecification -------------------------
LS = T + "CustomFormats/Specifications/LanguageSpecification/"


def lang_cases(fixture, languages, tests):
    rel = LS + fixture
    for method, value, flags, given_langs, matches in tests:
        spec = {"implementation": "LanguageSpecification", "value": value,
                "negate": "negate" in flags, "except_language": "except" in flags}
        langs = given_langs or languages
        words = ("negated " if spec["negate"] else "") + ("except-" if spec["except_language"] else "")
        add("custom_format_language.json", test_ref(rel, method),
            f"{'An' if words.startswith('except') else 'A'} {words}language format for {value} {'matches' if matches else 'does not match'} "
            f"a release in {' + '.join(langs)} (movie original language English)",
            {"op": "format_matches",
             "given": {"spec": spec, "release": {"languages": langs, "movie_original_language": "English"}}},
            {"matches": matches})


lang_cases("MultiLanguageFixture.cs", ["English", "French"], [
    ("should_match_one_language", "French", "", None, True),
    ("should_match_language_if_other_languages_are_present", "French", "except", None, True),
    ("should_match_language_if_not_original_language_is_present", "Original", "except", None, True),
    ("should_not_match_different_language", "Spanish", "", None, False),
    ("should_not_match_negated_when_one_language_matches", "French", "negate", None, False),
    ("should_not_match_negated_when_all_languages_do_not_match", "Spanish", "negate", None, True),
    ("should_not_match_negate_language_if_other_languages_are_present", "Spanish", "except negate", None, False),
])
lang_cases("OriginalLanguageFixture.cs", ["French"], [
    ("should_match_same_single_language", "Original", "", ["English"], True),
    ("should_not_match_different_single_language", "Original", "", None, False),
    ("should_not_match_negated_same_single_language", "Original", "negate", ["English"], False),
    ("should_match_negated_different_single_language", "Original", "negate", None, True),
])
lang_cases("SingleLanguageFixture.cs", ["French"], [
    ("should_match_same_language", "French", "", None, True),
    ("should_not_match_different_language", "Spanish", "", None, False),
    ("should_not_match_negated_same_language", "French", "negate", None, False),
    ("should_match_negated_different_language", "Spanish", "negate", None, True),
    ("should_match_negated_except_language_if_language_is_only_present_language", "French", "except negate", None, True),
])

# --- Profiles/Delay/DelayProfileServiceFixture -----------------------------------
DP = T + "Profiles/Delay/DelayProfileServiceFixture.cs"
DP_GIVEN = [{"id": 1, "order": 2147483647}, {"id": 2, "order": 1}, {"id": 3, "order": 2}, {"id": 4, "order": 3}]
DP_NOTE = ("Ids 1-4 come from NBuilder's sequential ids (Setup, line 21); id 1 starts at int.MaxValue and Reorder never renumbers id 1 "
           f"({src_at(C + 'Profiles/Delay/DelayProfileService.cs', r'if \(delayProfile\.Id == 1\)')}).")
for method, moving, after, expect, beh in [
    ("should_move_to_first_if_afterId_is_null", 4, None, {"sorted_by_order": {"0": {"id": 4, "order": 1}}},
     "Reordering a delay profile with no 'after' moves it first with order 1"),
    ("should_move_after_if_afterId_is_not_null", 4, 2, {"sorted_by_order": {"1": {"id": 4, "order": 2}}},
     "Reordering a delay profile after another puts it right after, with that profile's order + 1"),
    ("should_reorder_delay_profiles_that_are_after_moved", 4, None,
     {"sorted_by_order": {"1": {"order": 2}, "2": {"order": 3}}, "id_1_order": 2147483647},
     "Profiles after a moved delay profile are renumbered index + 1, the default profile (id 1) keeps int.MaxValue"),
    ("should_not_change_afters_order_if_moving_was_after", 4, 2, {"sorted_by_order": {"0": {"id": 2, "order": 1}}},
     "Moving a later delay profile after an earlier one leaves the earlier one's order unchanged"),
    ("should_change_afters_order_if_moving_was_before", 2, 4, {"order_of_id_4_less_than": 3},
     "Moving an earlier delay profile after a later one lowers the later one's order"),
]:
    note = DP_NOTE
    if method == "should_move_after_if_afterId_is_not_null":
        note += " Expected order is after.Order + 1; after.Order stays 1 (should_not_change_afters_order_if_moving_was_after)."
    if method == "should_reorder_delay_profiles_that_are_after_moved":
        note += " Test asserts order i+1 for sorted index i >= 1 except id 1, which sorts last at int.MaxValue."
    add("delay_profile_reorder.json", test_ref(DP, method), beh,
        {"op": "delay_profile_reorder", "given": {"profiles": DP_GIVEN, "moving_id": moving, "after_id": after}},
        expect, note)

# --- Profiles/ProfileRepositoryFixture --------------------------------------------
add(None, test_ref(T + "Profiles/ProfileRepositoryFixture.cs", "should_be_able_to_read_and_write"),
    "A quality profile round-trips through Radarr's database repository (name, cutoff, items, format items)", in_scope=False)

# --- Profiles/ProfileServiceFixture -------------------------------------------------
PS = T + "Profiles/ProfileServiceFixture.cs"
QPS_DELETE = src_at(C + "Profiles/Qualities/QualityProfileService.cs", r"_movieService\.GetAllMovies\(\)\.Any")
add("quality_profile_service.json", test_ref(PS, "init_should_add_default_profiles"),
    "On startup with no quality profiles and no custom formats, 6 default profiles are created",
    {"op": "default_profiles", "given": {"existing_profiles": 0, "custom_formats": 0}},
    {"profiles_inserted": 6})
add("quality_profile_service.json", test_ref(PS, "Init_should_skip_if_any_profiles_already_exist"),
    "On startup, default profiles are not re-added if any quality profile exists",
    {"op": "default_profiles", "given": {"existing_profiles": 2}},
    {"profiles_inserted": 0})
DEL_NOTE = f"Delete checks movies, then import lists, then collections, short-circuiting ({QPS_DELETE}); a list the test does not set up is omitted."
for method, deleting, used, expect, beh, extra in [
    ("should_not_be_able_to_delete_profile_if_assigned_to_movie", 2, {"movies": True, "import_lists": False},
     {"error": "QualityProfileInUseException", "deleted": False},
     "Deleting a quality profile used by a movie fails and deletes nothing",
     " 3 movies, one random movie set to profile 2; all import lists use profile 1."),
    ("should_not_be_able_to_delete_profile_if_assigned_to_list", 2, {"movies": False, "import_lists": True},
     {"error": "QualityProfileInUseException", "deleted": False},
     "Deleting a quality profile used by an import list fails and deletes nothing",
     " All movies use profile 1; 3 import lists, one random list set to profile 2."),
    ("should_not_be_able_to_delete_profile_if_assigned_to_collection", 2, {"movies": False, "collections": True},
     {"error": "QualityProfileInUseException", "deleted": False},
     "Deleting a quality profile used by a collection fails and deletes nothing",
     " All movies use profile 1, all collections use profile 2. Import lists are omitted: one random list is set to"
     " profile 1, the others keep NBuilder's sequential QualityProfileId, so whether one uses profile 2 is not controlled."),
    ("should_delete_profile_if_not_assigned_to_movie_or_list", 1, {"movies": False, "import_lists": False, "collections": False},
     {"deleted": True},
     "Deleting a quality profile no movie, import list or collection uses deletes it",
     " All movies, import lists and collections use profile 2."),
]:
    add("quality_profile_service.json", test_ref(PS, method), beh,
        {"op": "profile_delete", "given": {"profile_id": deleting, "in_use_by": used}}, expect, DEL_NOTE + extra)
add("quality_profile_service.json", test_ref(PS, "get_acceptable_languages_should_return_profile_language"),
    "A quality profile with language German and no format items accepts only German",
    {"op": "acceptable_languages", "given": {"profile_language": "German", "format_items": []}},
    {"language_set": ["German"]},
    "The test asserts count and membership, not order. FormatItems defaults to an empty list (QualityProfile constructor).")
add("quality_profile_service.json", test_ref(PS, "get_acceptable_languages_should_return_custom_format_positive_languages"),
    "A quality profile (German) also accepts the language of a positively scored language custom format (French), not a negatively scored one (English)",
    {"op": "acceptable_languages", "given": {"profile_language": "German", "format_items": [
        {"format": {"name": "My Format 1", "specifications": [{"implementation": "LanguageSpecification", "value": "English", "negate": False}]}, "score": -2},
        {"format": {"name": "My Format 2", "specifications": [{"implementation": "LanguageSpecification", "value": "French", "negate": False}]}, "score": 1},
    ]}},
    {"language_set": ["French", "German"]},
    "The test asserts count 2 and membership, not order. Scores come from CustomFormatsTestHelpers.GetSampleFormatItems "
    f"({src_at(T + 'CustomFormats/CustomFormatsTestHelpers.cs', r'var allowedItems')}): allowed formats score 2^index, the others -2^allowedCount.")

# --- Profiles/Qualities/QualityIndexCompareToFixture -------------------------------
QI = T + "Profiles/Qualities/QualityIndexCompareToFixture.cs"
for method, respect in [("should_match_expected_when_respect_group_order_is_true", True),
                        ("should_match_expected_when_respect_group_order_is_false", False)]:
    for ref, args in testcase_rows(QI, method):
        li, lg, ri, rg, exp = (int(a) for a in args.split(","))
        add("quality_index_compare.json", ref,
            f"QualityIndex({li},{lg}) compared to ({ri},{rg}) with respect_group_order={str(respect).lower()} is {exp}",
            {"op": "quality_index_compare",
             "given": {"left": {"index": li, "group_index": lg}, "right": {"index": ri, "group_index": rg},
                       "respect_group_order": respect}},
            {"result": exp})

# --- Qualities/QualityFinderFixture ---------------------------------------------------
QF = T + "Qualities/QualityFinderFixture.cs"
for method, quality in [("should_return_CAM", "CAM"), ("should_return_Unknown", "Unknown"),
                        ("should_return_DVD_Remux", "DVD-R"), ("should_return_DVD", "DVD"),
                        ("should_return_SDTV", "SDTV"), ("should_return_HDTV_720p", "HDTV-720p"),
                        ("should_return_HDTV_1080p", "HDTV-1080p"), ("should_return_Bluray720p", "Bluray-720p")]:
    body = " ".join(lines(QF)[method_line(QF, method):method_line(QF, method) + 4])
    assert q(re.search(r"Should\(\)\.Be\((Quality\.\w+)\)", body)[1]) == quality, method
    for ref, args in testcase_rows(QF, method):
        src, res, mod = (a.strip() for a in args.split(","))
        src, mod = src.removeprefix("QualitySource."), mod.removeprefix("Modifier.")
        add("quality_finder.json", ref, f"Source {src} at {res}p with modifier {mod} resolves to quality {quality}",
            {"op": "find_by_source_and_resolution", "given": {"source": src, "resolution": int(res), "modifier": mod}},
            {"quality": quality})

# --- Qualities/QualityFixture ------------------------------------------------------------
QX = T + "Qualities/QualityFixture.cs"
for array, op in [("FromIntCases", "from_id"), ("ToIntCases", "to_id")]:
    start, _ = line_ref(QX, rf"public static object\[\] {array}")
    i = start + 1
    while "};" not in lines(QX)[i]:
        m = re.search(r"new object\[\] \{ (.+?), (.+?) \}", lines(QX)[i])
        if m and not lines(QX)[i].strip().startswith("//"):
            ref = f"{TAG} {QX}:{i + 1}"
            if op == "from_id":
                n, name = int(m[1]), q(m[2])
                add("quality_id.json", ref, f"Quality id {n} is {name}",
                    {"op": op, "given": {"id": n}}, {"quality": name})
            else:
                name, n = q(m[1]), int(m[2])
                add("quality_id.json", ref, f"Quality {name} has id {n}",
                    {"op": op, "given": {"quality": name}}, {"id": n})
        i += 1

# --- Qualities/QualityModelComparerFixture ------------------------------------------------
QM = T + "Qualities/QualityModelComparerFixture.cs"
gd_start, _ = line_ref(QX, r"public static List<QualityProfileQualityItem> GetDefaultQualities")
DEFAULT_Q = []
for l in lines(QX)[gd_start + 4:]:
    if "};" in l:
        break
    DEFAULT_Q.append(re.fullmatch(r"\s*Quality\.(\w+),?", l)[1])
assert len(DEFAULT_Q) == 22, DEFAULT_Q


def default_items(*allowed):
    """QualityFixture.GetDefaultQualities: disallowed first, then allowed in given order."""
    allowed = allowed or tuple(DEFAULT_Q)
    order = [x for x in DEFAULT_Q if x not in allowed] + list(allowed)
    return [{"quality": q(x), "allowed": x in allowed} for x in order]


GROUPED = [{"quality": "SDTV", "allowed": False}, {"quality": "DVD", "allowed": False},
           {"allowed": True, "items": [{"quality": "HDTV-720p", "allowed": True}, {"quality": "WEBDL-720p", "allowed": True}]},
           {"quality": "Bluray-720p", "allowed": True}]
PROFILES = {"default": default_items(), "custom": default_items("Bluray720p", "DVD"), "grouped": GROUPED}
GD_NOTE = f" Profile items built by QualityFixture.GetDefaultQualities ({QX}:{gd_start + 1})."
for method, prof, first, second, respect, sign in [
    ("should_be_greater_when_first_quality_is_greater_than_second", "default", ("Bluray-1080p", None), ("DVD", None), None, 1),
    ("should_be_lesser_when_second_quality_is_greater_than_first", "default", ("DVD", None), ("Bluray-1080p", None), None, -1),
    ("should_be_greater_when_first_quality_is_a_proper_for_the_same_quality", "default", ("Bluray-1080p", 2), ("Bluray-1080p", 1), None, 1),
    ("should_be_greater_when_using_a_custom_profile", "custom", ("DVD", None), ("Bluray-720p", None), None, 1),
    ("should_ignore_group_order_by_default", "grouped", ("HDTV-720p", None), ("WEBDL-720p", None), None, 0),
    ("should_respect_group_order", "grouped", ("HDTV-720p", None), ("WEBDL-720p", None), True, -1),
]:
    def model(m):
        return {"quality": m[0], "revision": {"version": m[1] or 1, "real": 0}}
    given = {"profile_items": PROFILES[prof], "first": model(first), "second": model(second)}
    if respect is not None:
        given["respect_group_order"] = respect
    word = {1: "greater than", -1: "less than", 0: "equal to"}[sign]
    vs = lambda m: m[0] + (f" v{m[1]}" if m[1] else "")
    add("quality_model_compare.json", test_ref(QM, method),
        f"In the {prof} profile{' respecting group order' if respect else ''}, {vs(first)} compares {word} {vs(second)}",
        {"op": "compare", "given": given}, {"sign": sign},
        REV_DEFAULT + "." + (GD_NOTE if prof != "grouped" else "") + " The test asserts the sign of Compare only.")

# --- Qualities/RevisionComparableFixture ---------------------------------------------------
RC = T + "Qualities/RevisionComparableFixture.cs"


def rev(version=1, real=0):
    return {"version": version, "real": real}


for method, a, b, expect, beh in [
    ("should_be_greater_when_first_quality_is_a_real", rev(real=1), rev(), {"sign": 1}, "A REAL revision is greater than the default revision"),
    ("should_be_greater_when_first_quality_is_a_proper", rev(version=2), rev(), {"sign": 1}, "A PROPER (version 2) is greater than the default revision"),
    ("should_be_greater_when_first_is_a_proper_for_a_real", rev(2, 1), rev(real=1), {"sign": 1}, "A PROPER REAL is greater than a REAL"),
    ("should_be_lesser_when_second_quality_is_a_real", rev(), rev(real=1), {"sign": -1}, "The default revision is less than a REAL"),
    ("should_be_lesser_when_second_quality_is_a_proper", rev(), rev(version=2), {"sign": -1}, "The default revision is less than a PROPER"),
    ("should_be_lesser_when_second_is_a_proper_for_a_real", rev(real=1), rev(2, 1), {"sign": -1}, "A REAL is less than a PROPER REAL"),
    ("should_be_equal_when_both_real_and_version_match", rev(), rev(), {"sign": 0}, "Two default revisions compare equal"),
    ("should_be_equal_when_both_real_and_version_match_for_real", rev(real=1), rev(real=1), {"sign": 0}, "Two REAL revisions compare equal"),
    ("should_be_equal_when_both_real_and_version_match_for_real_proper", rev(2, 1), rev(2, 1), {"sign": 0}, "Two PROPER REAL revisions compare equal"),
    ("equal_operator_tests", rev(), rev(),
     {"gt": False, "lt": False, "ne": False, "ge": True, "le": True, "eq": True}, "Comparison operators agree for equal revisions"),
    ("greater_than_operator_tests", rev(version=2), rev(),
     {"gt": True, "lt": False, "ne": True, "ge": True, "le": False, "eq": False}, "Comparison operators agree when the first revision is greater"),
    ("less_than_operator_tests", rev(), rev(version=2),
     {"gt": False, "lt": True, "ne": True, "ge": False, "le": True, "eq": False}, "Comparison operators agree when the first revision is less"),
]:
    add("revision_compare.json", test_ref(RC, method), beh,
        {"op": "revision_compare", "given": {"first": a, "second": b}}, expect, REV_DEFAULT)
add("revision_compare.json", test_ref(RC, "operating_on_nulls"),
    "A revision compared with a missing (null) revision is greater",
    {"op": "revision_compare", "given": {"first": rev(), "second": None}},
    {"gt": True, "ge": True, "lt": False, "le": False},
    REV_DEFAULT + f"; operators treat a null right side as lower ({src_at(C + 'Qualities/Revision.cs', r'public static bool operator >\(')}). The test asserts only these four operators.")

# --- Quality.cs: declared qualities, default order, default definitions -----------------
for j, member, qid, name, source, res, mod in sorted(QDECL):
    add("ordering.json", f"{TAG} {Q_CS}:{j + 1}",
        f"Quality {name} is declared with id {qid}, source {source}, resolution {res}, modifier {mod}",
        {"op": "quality_declaration", "given": {"quality": name}},
        {"id": qid, "source": source, "resolution": res, "modifier": mod},
        None if " Modifier." in lines(Q_CS)[j] else "Modifier defaults to NONE (Quality constructor).")

all_start, all_ref = line_ref(Q_CS, r"All = new List<Quality>")
all_order = []
for l in lines(Q_CS)[all_start + 2:]:
    if "};" in l:
        break
    all_order.append(q(l.strip().rstrip(",")))
assert len(all_order) == 30, all_order
add("ordering.json", all_ref, "Quality.All lists every quality in Radarr's declared order",
    {"op": "all_qualities", "given": {}}, {"order": all_order})

# --- Qualities/QualityDefinitionServiceFixture --------------------------------------
# Each test verifies one repository call on ApplicationStartedEvent; expect holds only that call's count.
QD = T + "Qualities/QualityDefinitionServiceFixture.cs"
SDTV_DEF = {"quality": q("Quality.SDTV"), "weight": 1, "min_size": 0, "max_size": 100, "id": 20}
QD_NOTE = f"Quality.All has {len(all_order)} qualities ({all_ref.split(' ', 1)[1]})."
for method, existing, expect, beh in [
    ("init_should_add_all_definitions", [], {"inserted": len(all_order)},
     f"On startup with no definitions, one definition per known quality ({len(all_order)}) is inserted"),
    ("init_should_insert_any_missing_definitions", [SDTV_DEF], {"inserted": len(all_order) - 1},
     f"On startup with an SDTV definition already stored, the other {len(all_order) - 1} definitions are inserted"),
    ("init_should_update_existing_definitions", [SDTV_DEF], {"updated": 1},
     "On startup with an SDTV definition already stored, that one definition is updated"),
    ("init_should_remove_old_definitions",
     [{"quality": {"id": 100, "name": "Test"}, "weight": 1, "min_size": 0, "max_size": 100, "id": 20}], {"deleted": 1},
     "On startup, a stored definition for an unknown quality (id 100) is deleted"),
]:
    add("quality_definition_service.json", test_ref(QD, method), beh,
        {"op": "definition_init", "given": {"existing_definitions": existing}}, expect, QD_NOTE)

n_def = 0
for j, l in enumerate(lines(Q_CS)):
    m = re.search(r"new QualityDefinition\(Quality\.(\w+)\)\s*\{(.*)\}", l)
    if not m:
        continue
    props = dict(re.findall(r"(\w+) = ([^,]+?)\s*(?:,|$)", m[2].strip()))
    num = lambda v: None if v == "null" else (float(v) if "." in v else int(v))
    expect = {"weight": num(props["Weight"]), "min_size": num(props["MinSize"]), "max_size": num(props["MaxSize"]),
              "preferred_size": num(props["PreferredSize"]), "group_name": props.get("GroupName", "null").strip('"')}
    if expect["group_name"] == "null":
        expect["group_name"] = None
    name = q(m[1])
    n_def += 1
    add("ordering.json", f"{TAG} {Q_CS}:{j + 1}",
        f"The default definition for {name} has weight {expect['weight']}, min size {expect['min_size']}, "
        f"max size {'none' if expect['max_size'] is None else expect['max_size']}, "
        f"preferred {'none' if expect['preferred_size'] is None else expect['preferred_size']}"
        + (f", group {expect['group_name']}" if expect["group_name"] else ""),
        {"op": "default_definition", "given": {"quality": name}}, expect)
assert n_def == 30, n_def

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
CATALOGUE.write_text("# Generated by parity/extract/radarr_quality.py; IDs are stable, edit status/decision/tests only.\n"
                     + "".join(out))
CORPUS.mkdir(parents=True, exist_ok=True)
for f, cs in sorted(by_file.items()):
    (CORPUS / f).write_text(json.dumps(cs, indent=1, ensure_ascii=False) + "\n")
print(f"{len(cases)} entries ({sum(c['in_scope'] for c in cases)} in scope), {len(by_file)} corpus files")
