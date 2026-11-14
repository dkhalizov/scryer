//! Stream `priv_subtitles` (Stage 8, private Bazarr config): suites, IDs and
//! the Scryer functions under test. Corpus: `corpus/captured/bazarr/config/`
//! (`settings.json` key `general`, `scores.json`, `languages-profiles.json`).
//! No captured value is written in this file; every compared value is read at
//! run time.
//!
//! Suites and mapped IDs:
//! - `priv_subtitles::baz_subtitles_cfg_wanted`
//!   - BAZ-SUBTITLES-5010 (`general/upgrade_subs`, red): a satisfied language
//!     with a score below the upgrade cutoff is searched again. Scryer:
//!     `wanted::compute_missing_subtitles_from_streams` (what the search cycle
//!     calls); it ignores the score, there is no subtitle upgrade.
//!   - BAZ-SUBTITLES-5012 (`general/adaptive_searching`, red): the instants a
//!     wanted subtitle is searched, over a fixed horizon. Scryer:
//!     `search_backoff::SubtitleSearchBackoff` (`applies_to`,
//!     `holds_search_at`, `after_empty_search`), driven as the search cycle
//!     does; it backs off by empty-search count, not by Bazarr's delay/delta.
//!   - BAZ-SUBTITLES-5013 (`general/use_embedded_subs`, red): an embedded track
//!     satisfies a profile item. Scryer: `compute_missing_subtitles_from_streams`
//!     with a `SubtitleStreamDetail`. The `und` probe (a track with no language
//!     tag) fails: a stream without a language never satisfies.
//! - `priv_subtitles::baz_subtitles_cfg_files`
//!   - BAZ-SUBTITLES-5015 (`general/subfolder`, red): folder and file name of a
//!     saved subtitle. Scryer: `download::build_subtitle_path` with the code
//!     from the seam `subtitle_download_language` (`download::normalize_language`).
//!   - BAZ-SUBTITLES-5018 (`general/skip_hashing`, green): the file hash is
//!     computed. Scryer: `provider::compute_subtitle_file_hash` on two temp files
//!     of different content (hash must be Ok and differ). Scryer has no skip
//!     switch, so the green ratchets the captured setting (off) against
//!     always-hash; it turns red if the setting is turned on.
//! - `priv_subtitles::baz_subtitles_cfg_scene_name`
//!   - BAZ-SUBTITLES-5017 (`general/use_scenename`, red): where the subtitle
//!     query takes release group, source and resolution from. Bazarr parses the
//!     video file path first; the scene name only fills fields still empty,
//!     and the Sonarr/Radarr DB refiner fills last (`subtitles/utils.py:33-46`,
//!     `refine_video_with_scenename` `:104-107`, `refiners/database.py:58-61`,
//!     `:100-105`). Scryer: seam `build_subtitle_query` (+
//!     `SubtitleEpisodeContext::default()`) never reads `file_path`; the scene
//!     parse wins over the stored fields (`orchestration.rs:2175-2190`).
//!
//! Translation table, Bazarr -> Scryer:
//!
//! | Bazarr | Scryer |
//! |---|---|
//! | profile item `{language, hi, forced}` | `super::prefs(&[item])[0]` (code normalised, hi/forced via `super::flag`); items of all profiles, deduplicated |
//! | embedded probe: codec subrip/pgs/vobsub/ass (`indexer/series.py:45-49`) | `SubtitleStreamDetail.codec`; hi item -> stream name "SDH", forced item -> `forced`; expected `use_embedded_subs && !ignore_<codec>_subs` (no flag for subrip) |
//! | embedded track with no language tag, given `default_und_embedded_subtitles_lang` (`utilities/video_analyzer.py:52-58`: the default applies only when the track has no `language` key; an explicit `und` tag is not covered) | stream with `language: None` (probe label `und`); expected `use_embedded_subs` and that setting normalising to the item's code |
//! | upgrade probe (`upgrade.py:261-310`, `:377`) | a download mirroring the item, `score` = ceil(max x minimum / 100) < cutoff (reference constants 357 episode, 117 movie); expected `upgrade_subs && days_to_upgrade_subs > 0 && score < cutoff` |
//! | adaptive search (`adaptive_searching.py:26-105`) | ticks every `wanted_search_frequency` hours from a fixed instant over delay + 4 x delta (first char integer, unit d/w); expected rule: run iff `!adaptive || first run + delay > t || last run + delta <= t`; every search empty |
//! | subtitle folder `subfolder` | "current" iff the path's parent is the video's parent, else "other" |
//! | file name (`subliminal_patch/core.py:1144-1174`, `:1204-1224`) | `video` + (`.` + profile code as written unless `single_language`) + (`.forced` / `.` + `hi_extension`, no HI tag when `subzero_mods` has `remove_HI`) + `.srt`. Bazarr writes `Language.basename` (`subzero/language.py:122-123`), equal to the profile code for a plain alpha2 code; the test panics on Bazarr's custom codes `pb`, `zt`, `ea` (`languages/custom_lang.py`), whose basename differs |
//! | 5015 probes per item | `plain`; `hi` always (Bazarr takes the HI tag from the subtitle, not the profile item, and an item with hi False accepts HI subtitles, `core.py:1204-1224`); `forced` when the item is forced |
//! | `skip_hashing` | expected hash computed = `!skip_hashing`; actual: `compute_subtitle_file_hash` is Ok and content-dependent on two 128 KiB files |
//! | video from path, refined by scene name, then DB (`subtitles/utils.py:33-46`) | two probes per kind: `bare_path` (`/parity/video.mkv`, no release tokens: expected "scene" for every field) and `tagged_path` (a synthetic file name whose group, source and resolution differ from the scene name's: expected "path"). Expected labels are reference-rule constants. Actual label per `SubtitleQuery` field: "path" / "scene" if it equals Scryer's `parse_release_metadata` of the path stem / scene name, "stored" if it equals the stored-field sentinel (Scryer's analogue of the DB refiner), "none", else "other" |
//!
//! Ignored keys: `general/upgrade_frequency` (job schedule), `upgrade_manual`,
//! `embedded_subs_show_desired` (UI), `embedded_subtitles_parser`,
//! `embeddedsubtitles.*` (provider section), `subfolder_custom`,
//! `wanted_search_frequency_movie` (the tick uses the episode value),
//! `use_scenename` value (read nowhere in Bazarr v1.5.6),
//! `days_to_upgrade_subs` (only a > 0 guard; Scryer's function has no time input).
//!
//! Unmapped (tests: []), with reasons:
//! - BAZ-SUBTITLES-5002, 5003, 5028: concept absent. One global language list
//!   (`subtitles.languages`) applies to every file; no per-title profile, no
//!   assignment step, no tag-to-profile mapping.
//! - BAZ-SUBTITLES-5004: the enabled provider set is runtime state (WASM plugins
//!   plus config rows behind `AppUseCase`); provider names are captured values.
//! - BAZ-SUBTITLES-5005: AI/machine-translation filtering happens inside the
//!   provider plugin; the host only copies the settings into the query, so a
//!   test would echo the input. The hash half is checked by 5018.
//! - BAZ-SUBTITLES-5011: one interval `subtitles.search_interval_hours` for
//!   both kinds, applied by the async poller with the wall clock.
//! - BAZ-SUBTITLES-5014: Scryer's subtitle decisions never read audio
//!   languages; dropping `und` tags happens in `scryer-mediainfo`, an optional
//!   dependency behind `runtime-media-analysis` (not in the parity build).
//! - BAZ-SUBTITLES-5016: concept absent; subtitle bytes are written as given,
//!   no charset conversion.
//! - BAZ-SUBTITLES-5019: concept absent; post-processing scripts run on import
//!   only.
//! - BAZ-SUBTITLES-5020: `subtitles.sync_enabled` exists but translating the
//!   flag would echo the input, and the parity build compiles `sync_stub.rs`.
//!   Finding: Scryer defaults `sync_enabled` to true
//!   (`settings/runtime/subtitles.rs:106-109`), and a production build (feature
//!   `runtime`) compiles the real `sync.rs`; a migration must set it to match
//!   `subsync/use_subsync`.
//! - BAZ-SUBTITLES-5021: concept absent; no permission change on saved files.
//! - BAZ-SUBTITLES-5022: concept absent; no language-equivalence setting.
//! - BAZ-SUBTITLES-5024, 5025, 5029, 5030: behind the async
//!   `run_subtitle_search_cycle` with repositories (`AppUseCase`). Finding: the
//!   automatic cycle walks only monitored titles, and import-time search is
//!   `subtitles.auto_download_on_import` (default off).
//! - BAZ-SUBTITLES-5026: the code-less-file language guess lives in
//!   `external_probe.rs` behind `runtime-media-analysis`; the naming half is
//!   covered by 5015.
//! - BAZ-SUBTITLES-5027: concept absent; no subtitle content mods.
//! - BAZ-SUBTITLES-5031: a captured row count, not a decision; any test would
//!   be vacuous.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use scryer_application::parity_seams::{
    build_subtitle_query, subtitle_download_language, SubtitleEpisodeContext,
};
use scryer_application::subtitles::download::build_subtitle_path;
use scryer_application::subtitles::language::normalize_subtitle_language_code;
use scryer_application::subtitles::provider::compute_subtitle_file_hash;
use scryer_application::subtitles::search_backoff::{
    enabled_providers_key, wanted_languages_key, SubtitleSearchBackoff,
};
use scryer_application::subtitles::wanted::{
    compute_missing_subtitles_from_streams, SubtitleLanguagePref,
};
use scryer_application::{parse_release_metadata, SubtitleStreamDetail, TitleMediaFile};
use scryer_domain::Title;
use scryer_parity::{corpus, Catalogue, Suite};
use serde_json::{json, Value};

use super::{bazarr_max, catalogue_ref, download, flag, prefs, score_list};

/// Profile items of every profile, deduplicated on (language, hi, forced).
fn profile_items() -> Vec<Value> {
    let profiles: Vec<Value> = corpus("corpus/captured/bazarr/config/languages-profiles.json");
    let mut seen = Vec::new();
    let mut items = Vec::new();
    for profile in &profiles {
        for item in profile["items"].as_array().unwrap() {
            let key = (
                item["language"].as_str().unwrap().to_string(),
                flag(&item["hi"]),
                flag(&item["forced"]),
            );
            if !seen.contains(&key) {
                seen.push(key);
                items.push(item.clone());
            }
        }
    }
    items
}

fn pref_of(item: &Value) -> SubtitleLanguagePref {
    prefs(&json!([item.clone()])).remove(0)
}

fn settings() -> Value {
    let all: Value = corpus("corpus/captured/bazarr/config/settings.json");
    all["general"].clone()
}

fn check_ref(catalogue: &Catalogue, id: &str, key: &str) {
    assert!(
        catalogue_ref(catalogue, id).ends_with(&format!("#general/{key}")),
        "{id} does not point at general/{key}"
    );
}

/// Bazarr `adaptive_searching.py:69-82`: first character an integer, unit d/w.
fn parse_span(s: &str) -> Duration {
    let n: i64 = s[..1].parse().unwrap();
    match &s[1..2] {
        "d" => Duration::days(n),
        "w" => Duration::weeks(n),
        other => panic!("unknown adaptive searching unit {other}"),
    }
}

fn t0() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(1_767_225_600, 0).unwrap()
}

fn hours_since_t0(t: DateTime<Utc>) -> i64 {
    (t - t0()).num_hours()
}

#[test]
fn baz_subtitles_cfg_wanted() {
    let Some(mut suite) =
        Suite::open("scryer-application::parity_subtitles::baz_subtitles_cfg_wanted")
    else {
        return;
    };
    let catalogue = Catalogue::load();
    let general = settings();
    let scores: Value = corpus("corpus/captured/bazarr/config/scores.json");
    let items = profile_items();

    let id = "BAZ-SUBTITLES-5010";
    if suite.wants(id) {
        check_ref(&catalogue, id, "upgrade_subs");
        let upgrade =
            flag(&general["upgrade_subs"]) && general["days_to_upgrade_subs"].as_i64().unwrap() > 0;
        let mut expected = BTreeMap::new();
        let mut actual = BTreeMap::new();
        // Bazarr@v1.5.6 bazarr/subtitles/upgrade.py:310 (episode), :377 (movie).
        for (kind, list, minimum, cutoff) in [
            ("episode", "series_scores", "minimum_score", 357i64),
            ("movie", "movie_scores", "minimum_score_movie", 117i64),
        ] {
            let max = bazarr_max(&score_list(&scores[list]));
            let min = general[minimum].as_i64().unwrap();
            let probe = (max * min + 99) / 100;
            for (i, item) in items.iter().enumerate() {
                let pref = pref_of(item);
                let mut dl = download(
                    item["language"].as_str().unwrap(),
                    pref.hearing_impaired,
                    pref.forced,
                );
                dl.score = Some(i32::try_from(probe).unwrap());
                let searched_again =
                    !compute_missing_subtitles_from_streams(&[pref], &[dl], &[]).is_empty();
                expected.insert((kind, i), upgrade && probe < cutoff);
                actual.insert((kind, i), searched_again);
            }
        }
        suite.check(id, expected, actual);
    }

    let id = "BAZ-SUBTITLES-5012";
    if suite.wants(id) {
        check_ref(&catalogue, id, "adaptive_searching");
        let adaptive = flag(&general["adaptive_searching"]);
        let delay = parse_span(general["adaptive_searching_delay"].as_str().unwrap());
        let delta = parse_span(general["adaptive_searching_delta"].as_str().unwrap());
        let step = Duration::hours(general["wanted_search_frequency"].as_i64().unwrap());
        let all_prefs: Vec<_> = items.iter().map(pref_of).collect();
        let pref = all_prefs[0].clone();
        let file = TitleMediaFile {
            id: "parity-file".into(),
            file_path: "/parity/video.mkv".into(),
            size_bytes: 1,
            ..Default::default()
        };
        let wk = wanted_languages_key(&all_prefs);
        let pk = enabled_providers_key(["parity-provider"]);
        let horizon = delay + delta * 4;

        let mut expected = Vec::new();
        let (mut first, mut last): (Option<DateTime<Utc>>, Option<DateTime<Utc>>) = (None, None);
        let mut actual = Vec::new();
        let mut row: Option<SubtitleSearchBackoff> = None;
        let mut t = t0();
        while t - t0() <= horizon {
            // Bazarr rule (adaptive_searching.py:26-105).
            let bazarr_runs = !adaptive
                || first.is_none()
                || first.unwrap() + delay > t
                || last.unwrap() + delta <= t;
            if bazarr_runs {
                expected.push(hours_since_t0(t));
                first.get_or_insert(t);
                last = Some(t);
            }
            // Scryer: orchestration.rs:2468-2487, :2570-2580.
            let held = row
                .as_ref()
                .filter(|r| r.applies_to(&file, &pref, &wk, &pk))
                .is_some_and(|r| r.holds_search_at(t));
            if !held {
                actual.push(hours_since_t0(t));
                row = Some(SubtitleSearchBackoff::after_empty_search(
                    row.as_ref(),
                    &file,
                    &pref,
                    &wk,
                    &pk,
                    t,
                ));
            }
            t += step;
        }
        suite.check(id, expected, actual);
    }

    let id = "BAZ-SUBTITLES-5013";
    if suite.wants(id) {
        check_ref(&catalogue, id, "use_embedded_subs");
        let on = flag(&general["use_embedded_subs"]);
        let und_lang = general["default_und_embedded_subtitles_lang"]
            .as_str()
            .unwrap()
            .to_string();
        let mut expected = BTreeMap::new();
        let mut actual = BTreeMap::new();
        for (i, item) in items.iter().enumerate() {
            let pref = pref_of(item);
            let satisfied = |stream: SubtitleStreamDetail| {
                compute_missing_subtitles_from_streams(&[pref.clone()], &[], &[stream]).is_empty()
            };
            let language = normalize_subtitle_language_code(item["language"].as_str().unwrap());
            // Bazarr indexer/series.py:45-49: codec names after its own mapping.
            for (codec, ignore) in [
                ("subrip", None),
                ("pgs", Some("ignore_pgs_subs")),
                ("vobsub", Some("ignore_vobsub_subs")),
                ("ass", Some("ignore_ass_subs")),
            ] {
                expected.insert((i, codec), on && !ignore.is_some_and(|k| flag(&general[k])));
                actual.insert(
                    (i, codec),
                    satisfied(SubtitleStreamDetail {
                        codec: Some(codec.into()),
                        language: language.clone(),
                        name: pref.hearing_impaired.then(|| "SDH".to_string()),
                        forced: pref.forced,
                        default: false,
                    }),
                );
            }
            let und_matches =
                !und_lang.is_empty() && normalize_subtitle_language_code(&und_lang) == language;
            expected.insert((i, "und"), on && und_matches);
            actual.insert(
                (i, "und"),
                satisfied(SubtitleStreamDetail {
                    codec: Some("subrip".into()),
                    language: None,
                    name: pref.hearing_impaired.then(|| "SDH".to_string()),
                    forced: pref.forced,
                    default: false,
                }),
            );
        }
        suite.check(id, expected, actual);
    }
    suite.finish();
}

#[test]
fn baz_subtitles_cfg_files() {
    let Some(mut suite) =
        Suite::open("scryer-application::parity_subtitles::baz_subtitles_cfg_files")
    else {
        return;
    };
    let catalogue = Catalogue::load();
    let general = settings();
    let items = profile_items();

    let id = "BAZ-SUBTITLES-5015";
    if suite.wants(id) {
        check_ref(&catalogue, id, "subfolder");
        let single = flag(&general["single_language"]);
        let hi_ext = general["hi_extension"].as_str().unwrap().to_string();
        // core.py:1204, :1224: the remove_HI mod drops the HI tag.
        let remove_hi = general["subzero_mods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "remove_HI");
        let video = Path::new("/parity/video.mkv");
        let mut expected = BTreeMap::new();
        let mut actual = BTreeMap::new();
        for (i, item) in items.iter().enumerate() {
            let language = item["language"].as_str().unwrap();
            // Bazarr custom codes (languages/custom_lang.py) have a basename
            // other than the profile code; the expected name below would be wrong.
            assert!(
                !["pb", "zt", "ea"].contains(&language),
                "profile item {i}: Bazarr custom language code, basename differs"
            );
            let code = subtitle_download_language(language).unwrap();
            // HI always: the tag comes from the subtitle, not the profile item.
            let mut probes = vec![("plain", false, false), ("hi", true, false)];
            if flag(&item["forced"]) {
                probes.push(("forced", false, true));
            }
            for (label, hi, forced) in probes {
                let tag = if forced {
                    ".forced".to_string()
                } else if hi && !remove_hi {
                    format!(".{hi_ext}")
                } else {
                    String::new()
                };
                let lang = if single {
                    String::new()
                } else {
                    format!(".{language}")
                };
                expected.insert(
                    (i, label),
                    (
                        general["subfolder"].as_str().unwrap().to_string(),
                        format!("video{lang}{tag}.srt"),
                    ),
                );
                let path = build_subtitle_path(video, &code, "srt", forced, hi);
                let folder = if path.parent() == video.parent() {
                    "current"
                } else {
                    "other"
                };
                actual.insert(
                    (i, label),
                    (
                        folder.to_string(),
                        path.file_name().unwrap().to_str().unwrap().to_string(),
                    ),
                );
            }
        }
        suite.check(id, expected, actual);
    }

    let id = "BAZ-SUBTITLES-5018";
    if suite.wants(id) {
        check_ref(&catalogue, id, "skip_hashing");
        // Probe: two same-size files with different content. Scryer has no
        // skip switch, so "hash computed" means a real, content-dependent hash.
        let (a, b) = (
            tempfile::NamedTempFile::new().unwrap(),
            tempfile::NamedTempFile::new().unwrap(),
        );
        std::fs::write(a.path(), vec![0u8; 131_072]).unwrap();
        std::fs::write(b.path(), vec![1u8; 131_072]).unwrap();
        let computed = match (
            compute_subtitle_file_hash(a.path()),
            compute_subtitle_file_hash(b.path()),
        ) {
            (Ok(x), Ok(y)) => !x.is_empty() && x != y,
            _ => false,
        };
        suite.check(id, !flag(&general["skip_hashing"]), computed);
    }
    suite.finish();
}

#[test]
fn baz_subtitles_cfg_scene_name() {
    let Some(mut suite) =
        Suite::open("scryer-application::parity_subtitles::baz_subtitles_cfg_scene_name")
    else {
        return;
    };
    let id = "BAZ-SUBTITLES-5017";
    if suite.wants(id) {
        let catalogue = Catalogue::load();
        check_ref(&catalogue, id, "use_scenename");
        assert!(settings().get("use_scenename").is_some());
        const GROUP: &str = "PARITYFILEGROUP";
        const SOURCE: &str = "parity-file-source";
        const RESOLUTION: &str = "parity-file-resolution";
        let mut expected = BTreeMap::new();
        let mut actual = BTreeMap::new();
        for (kind, scene, tagged_path) in [
            (
                "series",
                "Parity.Series.S01E02.720p.WEB-DL.x264-PARITYSCENE",
                "/parity/Parity.Series.S01E02.1080p.HDTV.x264-PARITYPATH.mkv",
            ),
            (
                "movie",
                "Parity.Movie.2001.1080p.BluRay.x264-PARITYSCENE",
                "/parity/Parity.Movie.2001.2160p.WEB-DL.x265-PARITYPATH.mkv",
            ),
        ] {
            let title: Title = serde_json::from_value(json!({
                "id": "parity-title",
                "library_id": "parity-library",
                "name": "Parity Title",
                "facet": kind,
                "monitored": true,
                "tags": [],
                "external_ids": [],
                "root_folder_id": "parity-root",
                "created_at": "2025-01-01T00:00:00Z",
                "catalog_sort_key": "parity title",
                "canonical_tags": [],
                "aliases": [],
                "tagged_aliases": []
            }))
            .unwrap();
            // Label glue: the field values Scryer's parser reads off each name.
            let fields = |raw: &str| {
                let parsed = parse_release_metadata(raw);
                [
                    parsed.release_group,
                    parsed.source.map(|s| s.to_string()),
                    parsed.quality,
                ]
            };
            let scene_values = fields(scene);
            let path_stem = Path::new(tagged_path)
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap();
            let path_values = fields(path_stem);
            for (s, p) in scene_values.iter().zip(&path_values) {
                assert!(
                    s.is_some() && p.is_some() && s != p,
                    "probe names must differ per field"
                );
            }
            // Bazarr: path parse first, scene name fills gaps (utils.py:33-46).
            for (probe, path, want) in [
                ("bare_path", "/parity/video.mkv", "scene"),
                ("tagged_path", tagged_path, "path"),
            ] {
                let file = TitleMediaFile {
                    file_path: path.into(),
                    scene_name: Some(scene.into()),
                    release_group: Some(GROUP.into()),
                    source_type: Some(SOURCE.into()),
                    resolution: Some(RESOLUTION.into()),
                    ..Default::default()
                };
                let query = build_subtitle_query(
                    &title,
                    &file,
                    &SubtitleEpisodeContext::default(),
                    &["parity".to_string()],
                    None,
                    false,
                    false,
                );
                for (n, (field, value, sentinel)) in [
                    ("release_group", &query.release_group, GROUP),
                    ("source", &query.source, SOURCE),
                    ("resolution", &query.resolution, RESOLUTION),
                ]
                .into_iter()
                .enumerate()
                {
                    let label = match value {
                        None => "none",
                        Some(v) if v == sentinel => "stored",
                        v if probe == "tagged_path" && *v == path_values[n] => "path",
                        v if *v == scene_values[n] => "scene",
                        Some(_) => "other",
                    };
                    expected.insert((kind, probe, field), want);
                    actual.insert((kind, probe, field), label);
                }
            }
        }
        suite.check(id, expected, actual);
    }
    suite.finish();
}
