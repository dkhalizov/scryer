//! Parity net, area `naming` (notes/11-parity-net-design.md).
//!
//! Suites (one test fn each): `son_naming`, `rad_naming` (reference ports),
//! `son_naming_config`, `rad_naming_config` (our captured settings),
//! `son_naming_captured`, `rad_naming_captured` (real relative paths).
//!
//! # What runs
//!
//! Every name, file or folder, comes out of Scryer's rename planner
//! `build_series_rename_plan_items_from_media_files`
//! (src/library/rename.rs:5532): a Title, its Episodes and one source file
//! go in, the proposed path under the media root comes out. That is the real
//! token building (title, multi-episode numbering, joined episode titles,
//! group, dynamic range), template rendering, sanitising, extension and
//! title/season/specials folders. `clean_file_name` cases run
//! `sanitize_filesystem_component` (public).
//!
//! Movies run through the same planner with a Movie-facet Title. Scryer's
//! movie planner (rename.rs:3400 `build_movie_rename_plan_items`) is private
//! behind a private options struct; it shares every step a movie name uses
//! here (`title_rename_tokens` splits a movie's year hint by facet,
//! `resolve_rendered_rename_filename`, `configured_title_folder_path`) and
//! differs in the collection-label quality fallback and `{edition}`, which no
//! translated Radarr template uses (their quality tokens are lacking), and in
//! the parent folder: the movie item keeps a subfolder under the title folder
//! (`title_folder_path_for_renamed_file`), the series planner with season
//! folders off goes straight to the configured title folder. They agree for
//! every case here: the source is in `/downloads` (no title folder) or
//! directly in the title folder (RAD-NAMING-5008: `Old Folder`).
//!
//! # Seams this file needs (product src, minimal)
//!
//! - rename.rs:5532 `pub(crate) fn build_series_rename_plan_items_from_media_files`
//!   -> `pub fn`.
//! - rename.rs:4402 `pub(crate) struct RenamePlanningState` -> `pub struct`
//!   (the planner takes `&mut` of it; built with its derived Default).
//! - lib.rs, after `pub use library::rename::{...}` (line 497):
//!   `#[doc(hidden)] pub use library::rename::{RenamePlanningState, build_series_rename_plan_items_from_media_files};`
//!
//! # Format translation (arr token -> Scryer template)
//!
//! | arr token                          | Scryer                       |
//! |------------------------------------|------------------------------|
//! | `{Series Title}`, `{Movie Title}`  | `{title}`                    |
//! | `{Episode Title}`                  | `{episode_title}`            |
//! | `{Release Group}`                  | `{group}`                    |
//! | `{MediaInfo VideoDynamicRange}`    | `{dynamic_range}`            |
//! | `{season}`, `{season:00}`          | `{season}`, `{season:2}`     |
//! | `{episode}`, `{episode:00}`        | `{episode}`, `{episode:2}`   |
//! | `{absolute:000}`                   | `{absolute_episode:3}`       |
//! | `{Release Year}`                   | `{year}`                     |
//! | `{{`, `}}`                         | `{{`, `}}`                   |
//!
//! Token names match case-insensitively, as in the arr. A `.`, `_` or `-`
//! between the words of a text token (`{Series.Title}`) adds
//! `|space:<sep>`. A prefix or suffix inside the braces (`{-Release Group}`,
//! `{ [Release Group]}`, `{_Episode.Title_}`) becomes an optional group
//! `{?token:<prefix>{token}<suffix>}`. A number format must be all zeros; its
//! length is Scryer's pad width.
//!
//! Lacking (no Scryer token; the case is red, P5 / #470): every other token,
//! e.g. `{Quality Full}`, `{Quality Title}`, `{Quality Proper}`,
//! `{Quality Real}` (Scryer's `{quality}` is a resolution and `{revision}`
//! is read from the file name, neither is the arr's quality vocabulary),
//! `{MediaInfo Full}`, `{MEDIAINFO.FULL}`, `{MediaInfo AudioLanguages[All]}`,
//! `{MediaInfo SubtitleLanguages[:...]}`, `{Series.CleanTitle}`,
//! `{Episode.CleanTitle}`, `{Original Title}`, `{Original Filename}`,
//! `{Release Hash}`, `{Air-Date}`; also an all-upper or all-lower text token
//! (`{SERIES TITLE}`, `{series title}`: the arr changes case, Scryer has no
//! case filter) and a text token with a custom format. A case with a lacking
//! token records a fail with Scryer's render (lacking tokens dropped) in
//! the detail; it never passes.
//!
//! # Input translation (reference input -> Scryer input)
//!
//! | reference                                              | Scryer                                  |
//! |--------------------------------------------------------|-----------------------------------------|
//! | `series.title`, `movie.title`                          | `Title.name`                            |
//! | `movie.year`, captured `series.year`                   | `Title.year`                            |
//! | `episodes[]` season/episode/absolute number, title     | `Episode` rows, one file row per episode (same file id) |
//! | `episodeFile.releaseGroup`, `movie_file.release_group` | `TitleMediaFile.release_group`          |
//! | `episodeFile.mediaInfo.videoHdrFormat` (`None` = absent) | `TitleMediaFile.video_hdr_format`     |
//! | `extension`, captured file extension                   | the source file's extension             |
//! | captured series `seasonFolder` (series.json via episodefile.json `seriesId`) | `use_season_folders` |
//!
//! Fixed harness inputs: media root `/library`; source file
//! `/downloads/source.<ext>` (a neutral stem, so the release parser adds no
//! tokens); missing-metadata policy FallbackTitle (Scryer's default,
//! settings/keys.rs DEFAULT_RENAME_MISSING_METADATA_POLICY); templates a case
//! does not give are `{title}`, `Season {season}`, `Specials` (they never
//! feed the compared component); folder-only ops get a stub episode S01E01.
//! A name with no `extension` in the case is compared without Scryer's
//! `.mkv`; with one, in full.
//!
//! Ignored keys: `colonReplacementFormat`, `customColonReplacementFormat`,
//! `multiEpisodeStyle`, `replaceIllegalCharacters` in reference cases (Scryer
//! has one fixed behaviour for each; the case compares Scryer's output with
//! the arr's for that setting), `dailyEpisodeFormat`, `animeEpisodeFormat`,
//! `seriesType` (no daily or anime case is in scope), `culture`,
//! `episodeFile.quality`/`revision`, mediaInfo other than `videoHdrFormat`,
//! `sceneName`, `relativePath`, `path`, `releaseHash` (they only feed lacking
//! tokens or rename-off cases), `series.path`, `series.seasonFolder` (only in
//! unmapped build_file_path cases), `note`. Captured: `quality`,
//! `media_info` (they feed `{Quality Full}`).
//!
//! # Config checks (built from the ref'd JSON)
//!
//! A format entry and its naming-examples entry share one check: the
//! example is the arr's own render of that format for its sample
//! (Sonarr: series "The Series Title's!", S01E01-E03 "Episode Title (n)";
//! Radarr: "The Movie: Title" (2010); Organizer/FileNameSampleService.cs).
//! `multiEpisodeStyle` 5 (PrefixedRange) renders S01E01-E03 as
//! `S01E01-E03`; `colonReplacementFormat` Smart renders "CSI: Vegas" as
//! "CSI - Vegas" (ColonReplacementFixture, SON-NAMING-0012);
//! `replaceIllegalCharacters` on renders "Question Mark?" as "Question
//! Mark!", off as "Question Mark" (FileNameBuilder.CleanFileName, as in
//! SON-NAMING-0183); `autoRenameFolders` off keeps a moved movie in its
//! existing folder, on moves it to the configured folder.
//! `certificationCountry` (RAD-NAMING-5009) runs Scryer's public
//! `request_rules::us_certification_label` (request_rules/facts.rs:496;
//! Scryer fixes the country to US, it has no setting) on a snapshot with a
//! GB and a US rating, listed GB first; the expected value is the rating of
//! the configured country.
//!
//! # Unmapped (left `tests: []`)
//!
//! - SON-NAMING-0001..0007 (`build_file_path`), SON-NAMING-0058..0061 and
//!   RAD-NAMING-0041 (`build_file_name` with rename off): with renaming off the
//!   arr keeps the scene or relative name. In Scryer the rename on/off switch
//!   (`rename.enabled`, settings/runtime/media.rs:548) is applied by the
//!   import pipeline, which this pure planner harness cannot drive; the
//!   planner always renames. Belongs to the import harness.
//! - SON-NAMING-0188..0190 (`requires_episode_title`): concept absent; Scryer
//!   has no episode-title-required import gate.
//! - SON-NAMING-5001, RAD-NAMING-5001 (rename on import): Scryer's default is
//!   resolved by `AppUseCase::resolve_rename_enabled` against the settings
//!   database (settings/runtime/media.rs:548, `unwrap_or(true)`); no pure
//!   accessor. Belongs to an app-level settings harness.

use scryer_application::{
    MediaRequestMetadataSnapshot, RenameMissingMetadataPolicy, RenamePlanningState,
    TitleMediaFile, build_series_rename_plan_items_from_media_files,
    request_rules::us_certification_label, sanitize_filesystem_component,
};
use scryer_domain::{Episode, EpisodeType, MediaFacet, Title};
use scryer_parity::{Catalogue, Source, Suite, corpus, parity_dir};
use serde_json::Value;

const MEDIA_ROOT: &str = "/library";

// ---------------------------------------------------------------- translation

#[derive(Clone, Copy)]
enum Kind {
    Text,
    Number,
}

/// Arr token name, lowercased with separators removed -> Scryer token.
const TOKENS: &[(&str, &str, Kind)] = &[
    ("seriestitle", "title", Kind::Text),
    ("movietitle", "title", Kind::Text),
    ("episodetitle", "episode_title", Kind::Text),
    ("releasegroup", "group", Kind::Text),
    ("mediainfovideodynamicrange", "dynamic_range", Kind::Text),
    ("season", "season", Kind::Number),
    ("episode", "episode", Kind::Number),
    ("absolute", "absolute_episode", Kind::Number),
    ("releaseyear", "year", Kind::Number),
];

struct Template {
    scryer: String,
    /// Arr tokens with no Scryer equivalent, dropped from `scryer`.
    lacking: Vec<String>,
}

fn translate(format: &str) -> Template {
    let chars: Vec<char> = format.chars().collect();
    let mut scryer = String::new();
    let mut lacking = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if matches!(c, '{' | '}') && chars.get(i + 1) == Some(&c) {
            scryer.push(c);
            scryer.push(c);
            i += 2;
            continue;
        }
        if c == '{'
            && let Some(len) = chars[i + 1..].iter().position(|x| *x == '}')
        {
            let inner: String = chars[i + 1..i + 1 + len].iter().collect();
            match translate_token(&inner) {
                Some(token) => scryer.push_str(&token),
                None => lacking.push(format!("{{{inner}}}")),
            }
            i += len + 2;
            continue;
        }
        scryer.push(c);
        i += 1;
    }
    Template { scryer, lacking }
}

/// One arr token (the text between its braces) as a Scryer token, or None
/// when Scryer has no equivalent.
fn translate_token(inner: &str) -> Option<String> {
    let prefix_len = inner
        .find(|c: char| !"- ._[(".contains(c))
        .unwrap_or(inner.len());
    let (prefix, rest) = inner.split_at(prefix_len);
    let core_len = rest
        .trim_end_matches(|c: char| "- ._)]".contains(c))
        .len();
    let (core, suffix) = rest.split_at(core_len);
    let (name, format) = match core.split_once(':') {
        Some((name, format)) => (name, Some(format)),
        None => (core, None),
    };
    let is_sep = |c: char| "- ._".contains(c);
    let key = name.split(is_sep).collect::<String>().to_lowercase();
    let sep: String = name.chars().filter(|c| is_sep(*c)).collect();
    let &(_, scryer, kind) = TOKENS.iter().find(|(arr, _, _)| *arr == key)?;
    let token = match kind {
        Kind::Number => match format {
            None => format!("{{{scryer}}}"),
            Some(f) if !f.is_empty() && f.chars().all(|c| c == '0') => {
                format!("{{{scryer}:{}}}", f.len())
            }
            Some(_) => return None,
        },
        Kind::Text => {
            let letters: Vec<char> = name.chars().filter(|c| c.is_alphabetic()).collect();
            let all_upper = letters.iter().all(|c| c.is_uppercase());
            let all_lower = letters.iter().all(|c| c.is_lowercase());
            if format.is_some() || all_upper || all_lower {
                return None;
            }
            match sep.as_str() {
                "" | " " => format!("{{{scryer}}}"),
                "." | "_" | "-" => format!("{{{scryer}|space:{sep}}}"),
                _ => return None,
            }
        }
    };
    Some(if prefix.is_empty() && suffix.is_empty() {
        token
    } else {
        format!("{{?{scryer}:{prefix}{token}{suffix}}}")
    })
}

// ------------------------------------------------------------------ planner

#[derive(Clone)]
struct Ep {
    season: u32,
    number: u32,
    absolute: Option<u32>,
    title: String,
}

fn stub_episode(season: u32) -> Ep {
    Ep {
        season,
        number: 1,
        absolute: None,
        title: "Episode".into(),
    }
}

#[derive(Default)]
struct Plan {
    facet: MediaFacet,
    name: String,
    year: Option<i32>,
    episodes: Vec<Ep>,
    release_group: Option<String>,
    hdr_format: Option<String>,
    /// Without the dot.
    extension: String,
    file_template: String,
    folder_template: String,
    season_template: String,
    specials_template: String,
    use_season_folders: bool,
    /// The title's recorded folder; the source file sits in it when set.
    folder_path: Option<String>,
}

fn epoch() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).expect("epoch")
}

fn episode(index: usize, ep: &Ep) -> Episode {
    Episode {
        id: format!("episode-{index}"),
        title_id: "title-1".into(),
        collection_id: None,
        episode_type: EpisodeType::Standard,
        episode_number: Some(ep.number.to_string()),
        season_number: Some(ep.season.to_string()),
        episode_label: None,
        title: Some(ep.title.clone()),
        air_date: None,
        duration_seconds: None,
        has_multi_audio: false,
        has_subtitle: false,
        is_filler: false,
        is_recap: false,
        absolute_number: ep.absolute.map(|n| n.to_string()),
        contiguous_absolute_number: None,
        overview: None,
        tvdb_id: None,
        tmdb_id: None,
        image_url: None,
        monitored: true,
        created_at: epoch(),
    }
}

impl Plan {
    fn new(facet: MediaFacet, name: &str) -> Self {
        Self {
            facet,
            name: name.into(),
            extension: "mkv".into(),
            file_template: "{title}".into(),
            folder_template: "{title}".into(),
            season_template: "Season {season}".into(),
            specials_template: "Specials".into(),
            ..Self::default()
        }
    }

    fn title(&self) -> Title {
        Title {
            id: "title-1".into(),
            library_id: scryer_domain::default_library_id_for_facet(&self.facet),
            name: self.name.clone(),
            facet: self.facet.clone(),
            monitored: true,
            tags: vec![],
            external_ids: vec![],
            root_folder_id: scryer_domain::root_folder_id_for_path(MEDIA_ROOT),
            created_by: None,
            created_at: epoch(),
            year: self.year,
            overview: None,
            poster_url: None,
            poster_source_url: None,
            background_url: None,
            background_source_url: None,
            sort_title: None,
            catalog_sort_key: String::new(),
            slug: None,
            imdb_id: None,
            runtime_minutes: None,
            popularity: None,
            canonical_tags: vec![],
            content_status: None,
            language: None,
            first_aired: None,
            network: None,
            studio: None,
            country: None,
            aliases: vec![],
            tagged_aliases: vec![],
            metadata_language: None,
            metadata_fetched_at: None,
            min_availability: None,
            digital_release_date: None,
            movie_release_dates: None,
            folder_path: self.folder_path.clone(),
        }
    }

    /// Scryer's proposed path for the one source file, as components under
    /// the media root (title folder[/season folder]/file). None when Scryer
    /// proposes no path.
    fn run(&self) -> Option<Vec<String>> {
        let source = match &self.folder_path {
            Some(folder) => format!("{folder}/source.{}", self.extension),
            None => format!("/downloads/source.{}", self.extension),
        };
        let episodes: Vec<Episode> = self
            .episodes
            .iter()
            .enumerate()
            .map(|(i, ep)| episode(i, ep))
            .collect();
        let row = |episode_id: Option<String>| TitleMediaFile {
            id: "file-1".into(),
            title_id: "title-1".into(),
            episode_id,
            file_path: source.clone(),
            release_group: self.release_group.clone(),
            video_hdr_format: self.hdr_format.clone(),
            ..TitleMediaFile::default()
        };
        let media_files: Vec<TitleMediaFile> = if episodes.is_empty() {
            vec![row(None)]
        } else {
            episodes.iter().map(|e| row(Some(e.id.clone()))).collect()
        };
        let items = build_series_rename_plan_items_from_media_files(
            &self.title(),
            self.use_season_folders,
            Vec::new(),
            episodes,
            media_files,
            MEDIA_ROOT,
            &self.folder_template,
            &self.season_template,
            &self.specials_template,
            &self.file_template,
            &RenameMissingMetadataPolicy::FallbackTitle,
            &mut RenamePlanningState::default(),
        );
        assert_eq!(items.len(), 1, "one source file plans one item");
        let path = items[0].proposed_path.as_deref()?;
        let relative = path.strip_prefix(MEDIA_ROOT)?.strip_prefix('/')?;
        Some(relative.split('/').map(str::to_string).collect())
    }

    fn file_name(&self) -> Option<String> {
        self.run().and_then(|c| c.last().cloned())
    }

    /// The file name without Scryer's extension.
    fn file_stem(&self) -> Option<String> {
        let suffix = format!(".{}", self.extension);
        self.file_name()
            .map(|n| n.strip_suffix(&suffix).map(str::to_string).unwrap_or(n))
    }

    fn title_folder(&self) -> Option<String> {
        self.run().map(|c| c[..c.len() - 1].join("/"))
    }

    fn season_folder(&self) -> Option<String> {
        self.run()
            .and_then(|c| c.len().checked_sub(2).map(|i| c[i].clone()))
    }
}

// ------------------------------------------------------------------ helpers

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key]
        .as_str()
        .unwrap_or_else(|| panic!("missing string `{key}` in {v}"))
}

fn opt_text(v: &Value, key: &str) -> Option<String> {
    v[key].as_str().map(str::to_string)
}

fn number(v: &Value) -> u32 {
    v.as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or_else(|| panic!("not a number: {v}"))
}

fn year(v: &Value) -> Option<i32> {
    v.as_i64().map(|y| i32::try_from(y).expect("year fits i32"))
}

/// Records the comparison, or a fail when the arr format used a token
/// Scryer lacks.
fn record(suite: &mut Suite, id: &str, lacking: &[String], expected: &str, actual: Option<String>) {
    if lacking.is_empty() {
        suite.check(id, Some(expected.to_string()), actual);
    } else {
        suite.record(
            id,
            false,
            format!(
                "Scryer has no token for {} (P5, #470); rendered without it: {actual:?}; expected {expected:?}",
                lacking.join(" ")
            ),
        );
    }
}

/// The reference's length bound on a name, when it has one and the name
/// breaks it (`max_length` counts UTF-16 units, as .NET's String.Length).
fn over_limit(expect: &Value, actual: Option<&str>) -> Option<String> {
    let actual = actual.unwrap_or_default();
    let units = actual.encode_utf16().count() as u64;
    let bytes = actual.len() as u64;
    match (expect["max_length"].as_u64(), expect["max_utf8_bytes"].as_u64()) {
        (Some(max), _) if units > max => Some(format!("{units} UTF-16 units, over max_length {max}: {actual:?}")),
        (_, Some(max)) if bytes > max => Some(format!("{bytes} bytes, over max_utf8_bytes {max}: {actual:?}")),
        _ => None,
    }
}

fn reference_cases(app: &str) -> Vec<Value> {
    let rel = format!("corpus/reference/{app}/naming");
    let dir = parity_dir().join(&rel);
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".json"))
        .collect();
    files.sort();
    files
        .iter()
        .flat_map(|file| corpus::<Vec<Value>>(&format!("{rel}/{file}")))
        .collect()
}

/// (id, ref'd file name, ref'd field) of the app's naming config entries.
fn config_entries(prefix: &str) -> Vec<(String, String, String)> {
    Catalogue::load()
        .entries
        .into_iter()
        .filter(|(area, e)| area == "naming" && e.source == Source::Config && e.id.starts_with(prefix))
        .map(|(_, e)| {
            let (path, field) = e.reference.split_once('#').expect("config ref has #field");
            let file = path.rsplit('/').next().unwrap_or(path).to_string();
            let field = field.to_string();
            (e.id, file, field)
        })
        .collect()
}

/// `title` named through `format`, against the arr's render of it.
fn title_check(suite: &mut Suite, id: &str, facet: MediaFacet, format: &str, title: &str, expected: &str) {
    let template = translate(format);
    let mut plan = Plan::new(facet, title);
    plan.file_template = template.scryer.clone();
    record(suite, id, &template.lacking, expected, plan.file_stem());
}

/// `replaceIllegalCharacters`: the arr replaces '?' with '!' when on and
/// drops it when off (FileNameBuilder.CleanFileName).
fn illegal_check(suite: &mut Suite, id: &str, facet: MediaFacet, format: &str, on: bool) {
    let expected = if on { "Question Mark!" } else { "Question Mark" };
    title_check(suite, id, facet, format, "Question Mark?", expected);
}

/// Sonarr's naming sample: series "The Series Title's!", season 1, episodes
/// 1..=count titled "Episode Title (n)".
fn son_sample(count: u32) -> Plan {
    let mut plan = Plan::new(MediaFacet::Series, "The Series Title's!");
    plan.episodes = (1..=count)
        .map(|n| Ep {
            season: 1,
            number: n,
            absolute: Some(n),
            title: format!("Episode Title ({n})"),
        })
        .collect();
    plan
}

/// Radarr's naming sample: "The Movie: Title" (2010).
fn rad_sample() -> Plan {
    let mut plan = Plan::new(MediaFacet::Movie, "The Movie: Title");
    plan.year = Some(2010);
    plan
}

// -------------------------------------------------------------------- suites

#[test]
fn son_naming() {
    let Some(mut suite) = Suite::open("scryer-application::parity_naming::son_naming") else { return };
    for case in reference_cases("sonarr") {
        let id = text(&case, "id");
        if !suite.wants(id) {
            continue;
        }
        let op = text(&case["input"], "op");
        let given = &case["input"]["given"];
        let naming = &given["naming"];
        let expect = &case["expect"];
        match op {
            "clean_file_name" => suite.check(
                id,
                text(expect, "name").to_string(),
                sanitize_filesystem_component(text(given, "name")),
            ),
            // Unmapped, see the header.
            "build_file_path" | "requires_episode_title" => continue,
            "build_file_name" if naming["renameEpisodes"].as_bool() == Some(false) => continue,
            "build_file_name" => {
                let template = translate(text(naming, "standardEpisodeFormat"));
                let file = &given["episodeFile"];
                let mut plan = Plan::new(MediaFacet::Series, text(&given["series"], "title"));
                plan.episodes = given["episodes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|e| Ep {
                        season: number(&e["seasonNumber"]),
                        number: number(&e["episodeNumber"]),
                        absolute: e.get("absoluteEpisodeNumber").filter(|n| !n.is_null()).map(number),
                        title: e["title"].as_str().unwrap_or_default().to_string(),
                    })
                    .collect();
                plan.release_group = opt_text(file, "releaseGroup");
                plan.hdr_format = file["mediaInfo"]["videoHdrFormat"]
                    .as_str()
                    .filter(|f| *f != "None")
                    .map(str::to_string);
                plan.file_template = template.scryer.clone();
                let actual = match given["extension"].as_str() {
                    Some(ext) => {
                        plan.extension = ext.trim_start_matches('.').to_string();
                        plan.file_name()
                    }
                    None => plan.file_stem(),
                };
                match over_limit(expect, actual.as_deref()) {
                    Some(why) => suite.record(id, false, why),
                    None => record(&mut suite, id, &template.lacking, text(expect, "name"), actual),
                }
            }
            "get_series_folder" => {
                let template = translate(text(naming, "seriesFolderFormat"));
                let mut plan = Plan::new(MediaFacet::Series, text(&given["series"], "title"));
                plan.episodes = vec![stub_episode(1)];
                plan.folder_template = template.scryer.clone();
                record(&mut suite, id, &template.lacking, text(expect, "name"), plan.title_folder());
            }
            "get_season_folder" => {
                let template = translate(text(naming, "seasonFolderFormat"));
                let mut plan = Plan::new(MediaFacet::Series, text(&given["series"], "title"));
                plan.episodes = vec![stub_episode(number(&given["seasonNumber"]))];
                plan.use_season_folders = true;
                plan.season_template = template.scryer.clone();
                record(&mut suite, id, &template.lacking, text(expect, "name"), plan.season_folder());
            }
            other => panic!("{id}: no mapping for op {other}"),
        }
    }
    suite.finish();
}

#[test]
fn rad_naming() {
    let Some(mut suite) = Suite::open("scryer-application::parity_naming::rad_naming") else { return };
    for case in reference_cases("radarr") {
        let id = text(&case, "id");
        if !suite.wants(id) {
            continue;
        }
        let op = text(&case["input"], "op");
        let given = &case["input"]["given"];
        let naming = &given["naming"];
        let expect = &case["expect"];
        let movie = &given["movie"];
        match op {
            "clean_file_name" => suite.check(
                id,
                text(expect, "name").to_string(),
                sanitize_filesystem_component(text(given, "name")),
            ),
            // Unmapped, see the header.
            "build_file_name" if naming["rename_movies"].as_bool() == Some(false) => continue,
            "build_file_name" => {
                let template = translate(text(naming, "standard_movie_format"));
                let mut plan = Plan::new(MediaFacet::Movie, text(movie, "title"));
                plan.year = year(&movie["year"]);
                plan.release_group = opt_text(&given["movie_file"], "release_group");
                plan.file_template = template.scryer.clone();
                record(&mut suite, id, &template.lacking, text(expect, "name"), plan.file_stem());
            }
            "get_movie_folder" => {
                let template = translate(text(naming, "movie_folder_format"));
                let mut plan = Plan::new(MediaFacet::Movie, text(movie, "title"));
                plan.year = year(&movie["year"]);
                plan.folder_template = template.scryer.clone();
                record(&mut suite, id, &template.lacking, text(expect, "name"), plan.title_folder());
            }
            other => panic!("{id}: no mapping for op {other}"),
        }
    }
    suite.finish();
}

#[test]
fn son_naming_config() {
    let Some(mut suite) = Suite::open("scryer-application::parity_naming::son_naming_config") else { return };
    let naming: Value = corpus("corpus/captured/sonarr/config/naming.json");
    let examples: Value = corpus("corpus/captured/sonarr/config/naming-examples.json");
    let arr_format = |key: &str| translate(text(&naming, key));
    for (id, file, field) in config_entries("SON-NAMING-") {
        if !suite.wants(&id) {
            continue;
        }
        match (file.as_str(), field.as_str()) {
            // Unmapped, see the header.
            ("naming.json", "renameEpisodes") => continue,
            ("naming.json", "standardEpisodeFormat") | ("naming-examples.json", "singleEpisodeExample") => {
                let template = arr_format("standardEpisodeFormat");
                let mut plan = son_sample(1);
                plan.file_template = template.scryer.clone();
                let expected = text(&examples, "singleEpisodeExample");
                record(&mut suite, &id, &template.lacking, expected, plan.file_stem());
            }
            ("naming-examples.json", "multiEpisodeExample") => {
                let template = arr_format("standardEpisodeFormat");
                let mut plan = son_sample(3);
                plan.file_template = template.scryer.clone();
                let expected = text(&examples, "multiEpisodeExample");
                record(&mut suite, &id, &template.lacking, expected, plan.file_stem());
            }
            ("naming.json", "seriesFolderFormat") | ("naming-examples.json", "seriesFolderExample") => {
                let template = arr_format("seriesFolderFormat");
                let mut plan = son_sample(1);
                plan.folder_template = template.scryer.clone();
                let expected = text(&examples, "seriesFolderExample");
                record(&mut suite, &id, &template.lacking, expected, plan.title_folder());
            }
            ("naming.json", "seasonFolderFormat") | ("naming-examples.json", "seasonFolderExample") => {
                let template = arr_format("seasonFolderFormat");
                let mut plan = son_sample(1);
                plan.use_season_folders = true;
                plan.season_template = template.scryer.clone();
                let expected = text(&examples, "seasonFolderExample");
                record(&mut suite, &id, &template.lacking, expected, plan.season_folder());
            }
            ("naming.json", "specialsFolderFormat") | ("naming-examples.json", "specialsFolderExample") => {
                let template = arr_format("specialsFolderFormat");
                let mut plan = son_sample(1);
                plan.episodes = vec![stub_episode(0)];
                plan.use_season_folders = true;
                plan.specials_template = template.scryer.clone();
                let expected = text(&examples, "specialsFolderExample");
                record(&mut suite, &id, &template.lacking, expected, plan.season_folder());
            }
            ("naming.json", "multiEpisodeStyle") => {
                let expected = match naming["multiEpisodeStyle"].as_u64() {
                    Some(5) => "S01E01-E03", // PrefixedRange
                    other => panic!("{id}: no sample for multiEpisodeStyle {other:?}"),
                };
                let template = translate("S{season:00}E{episode:00}");
                let mut plan = son_sample(3);
                plan.file_template = template.scryer.clone();
                record(&mut suite, &id, &template.lacking, expected, plan.file_stem());
            }
            ("naming.json", "colonReplacementFormat") => {
                let expected = match naming["colonReplacementFormat"].as_u64() {
                    Some(4) => "CSI - Vegas", // Smart
                    other => panic!("{id}: no sample for colonReplacementFormat {other:?}"),
                };
                title_check(&mut suite, &id, MediaFacet::Series, "{Series Title}", "CSI: Vegas", expected);
            }
            ("naming.json", "replaceIllegalCharacters") => {
                let on = naming["replaceIllegalCharacters"].as_bool().expect("bool");
                illegal_check(&mut suite, &id, MediaFacet::Series, "{Series Title}", on);
            }
            (file, field) => panic!("{id}: no check for {file}#{field}"),
        }
    }
    suite.finish();
}

#[test]
fn rad_naming_config() {
    let Some(mut suite) = Suite::open("scryer-application::parity_naming::rad_naming_config") else { return };
    let naming: Value = corpus("corpus/captured/radarr/config/naming.json");
    let examples: Value = corpus("corpus/captured/radarr/config/naming-examples.json");
    let media: Value = corpus("corpus/captured/radarr/config/mediamanagement.json");
    for (id, file, field) in config_entries("RAD-NAMING-") {
        if !suite.wants(&id) {
            continue;
        }
        match (file.as_str(), field.as_str()) {
            // Unmapped, see the header.
            ("naming.json", "renameMovies") => continue,
            ("metadataconfig.json", "certificationCountry") => {
                let config: Value = corpus("corpus/captured/radarr/config/metadataconfig.json");
                let country = text(&config, "certificationCountry");
                let ratings = [("GB", "15"), ("US", "R")];
                let expected = ratings
                    .iter()
                    .find(|(c, _)| c.eq_ignore_ascii_case(country))
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_else(|| panic!("{id}: no sample rating for country {country}"));
                let snapshot: MediaRequestMetadataSnapshot = serde_json::from_value(serde_json::json!({
                    "content_ratings": ratings
                        .iter()
                        .map(|(c, v)| serde_json::json!({"country": c, "certifications": [{"value": v}]}))
                        .collect::<Vec<_>>()
                }))
                .expect("snapshot");
                suite.check(&id, Some(expected), us_certification_label(&snapshot));
            }
            ("naming.json", "standardMovieFormat") | ("naming-examples.json", "movieExample") => {
                let template = translate(text(&naming, "standardMovieFormat"));
                let mut plan = rad_sample();
                plan.file_template = template.scryer.clone();
                let expected = text(&examples, "movieExample");
                record(&mut suite, &id, &template.lacking, expected, plan.file_stem());
            }
            ("naming.json", "movieFolderFormat") | ("naming-examples.json", "movieFolderExample") => {
                let template = translate(text(&naming, "movieFolderFormat"));
                let mut plan = rad_sample();
                plan.folder_template = template.scryer.clone();
                let expected = text(&examples, "movieFolderExample");
                record(&mut suite, &id, &template.lacking, expected, plan.title_folder());
            }
            ("naming.json", "colonReplacementFormat") => {
                let expected = match naming["colonReplacementFormat"].as_str() {
                    Some("smart") => "CSI - Vegas",
                    other => panic!("{id}: no sample for colonReplacementFormat {other:?}"),
                };
                title_check(&mut suite, &id, MediaFacet::Movie, "{Movie Title}", "CSI: Vegas", expected);
            }
            ("naming.json", "replaceIllegalCharacters") => {
                let on = naming["replaceIllegalCharacters"].as_bool().expect("bool");
                illegal_check(&mut suite, &id, MediaFacet::Movie, "{Movie Title}", on);
            }
            ("mediamanagement.json", "autoRenameFolders") => {
                let template = translate(text(&naming, "movieFolderFormat"));
                let mut plan = rad_sample();
                plan.folder_template = template.scryer.clone();
                plan.folder_path = Some(format!("{MEDIA_ROOT}/Old Folder"));
                if media["autoRenameFolders"].as_bool().expect("bool") {
                    let expected = text(&examples, "movieFolderExample");
                    record(&mut suite, &id, &template.lacking, expected, plan.title_folder());
                } else {
                    suite.check(&id, Some("Old Folder".to_string()), plan.title_folder());
                }
            }
            (file, field) => panic!("{id}: no check for {file}#{field}"),
        }
    }
    suite.finish();
}

#[test]
fn son_naming_captured() {
    let Some(mut suite) = Suite::open("scryer-application::parity_naming::son_naming_captured") else { return };
    let files: Vec<Value> = corpus("corpus/captured/sonarr/data/episodefile.json");
    let series: Vec<Value> = corpus("corpus/captured/sonarr/data/series.json");
    for case in corpus::<Vec<Value>>("corpus/captured/sonarr/data/cases/naming.json") {
        let id = text(&case, "id");
        if !suite.wants(id) {
            continue;
        }
        let input = &case["input"];
        let naming: Value = corpus(text(input, "naming_config"));
        let file_id: u64 = text(&case, "ref")
            .rsplit_once("#id=")
            .and_then(|(_, n)| n.parse().ok())
            .unwrap_or_else(|| panic!("{id}: ref without #id="));
        let file = files
            .iter()
            .find(|f| f["id"].as_u64() == Some(file_id))
            .unwrap_or_else(|| panic!("{id}: episode file {file_id} not captured"));
        let show = series
            .iter()
            .find(|s| s["id"] == file["seriesId"])
            .unwrap_or_else(|| panic!("{id}: series of file {file_id} not captured"));
        let file_template = translate(text(&naming, "standardEpisodeFormat"));
        let season_template = translate(text(&naming, "seasonFolderFormat"));
        let specials_template = translate(text(&naming, "specialsFolderFormat"));
        let season = number(&input["season"]);
        let mut plan = Plan::new(MediaFacet::Series, text(&input["series"], "title"));
        plan.year = year(&input["series"]["year"]);
        plan.episodes = input["episodes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| Ep {
                season,
                number: number(&e["number"]),
                absolute: None,
                title: e["title"].as_str().unwrap_or_default().to_string(),
            })
            .collect();
        plan.release_group = opt_text(input, "release_group");
        plan.extension = std::path::Path::new(text(file, "relativePath"))
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_string();
        plan.file_template = file_template.scryer.clone();
        plan.season_template = season_template.scryer.clone();
        plan.specials_template = specials_template.scryer.clone();
        plan.use_season_folders = show["seasonFolder"].as_bool().expect("seasonFolder");
        let mut lacking = file_template.lacking.clone();
        if plan.use_season_folders {
            let folder = if season == 0 { &specials_template } else { &season_template };
            lacking.extend(folder.lacking.iter().cloned());
        }
        // Sonarr's relative path is under the series folder.
        let actual = plan.run().map(|c| c[1..].join("/"));
        record(&mut suite, id, &lacking, text(&case["expect"], "relative_path"), actual);
    }
    suite.finish();
}

#[test]
fn rad_naming_captured() {
    let Some(mut suite) = Suite::open("scryer-application::parity_naming::rad_naming_captured") else { return };
    for case in corpus::<Vec<Value>>("corpus/captured/radarr/data/cases/naming.json") {
        let id = text(&case, "id");
        if !suite.wants(id) {
            continue;
        }
        let op = text(&case["input"], "op");
        assert_eq!(op, "build_file_name", "{id}: no mapping for op {op}");
        let given = &case["input"]["given"];
        let naming = &given["naming"];
        if naming["rename_movies"].as_bool() == Some(false) {
            continue; // rename off: unmapped, see the header
        }
        let movie_file = &given["movie_file"];
        let template = translate(text(naming, "standard_movie_format"));
        let mut plan = Plan::new(MediaFacet::Movie, text(&given["movie"], "title"));
        plan.year = year(&given["movie"]["year"]);
        plan.release_group = opt_text(movie_file, "release_group");
        plan.extension = text(movie_file, "extension").trim_start_matches('.').to_string();
        plan.file_template = template.scryer.clone();
        // Radarr's relative path is under the movie folder: the file name.
        record(&mut suite, id, &template.lacking, text(&case["expect"], "relative_path"), plan.file_name());
    }
    suite.finish();
}

#[test]
fn translation_table() {
    let t = |f: &str| {
        let t = translate(f);
        (t.scryer, t.lacking)
    };
    let none: Vec<String> = vec![];
    assert_eq!(
        t("{Series Title} - S{season:00}E{episode:00} - {Episode Title} {Quality Full}"),
        ("{title} - S{season:2}E{episode:2} - {episode_title} ".into(), vec!["{Quality Full}".to_string()])
    );
    assert_eq!(t("{Series.Title}.{season:0}"), ("{title|space:.}.{season:1}".into(), none.clone()));
    assert_eq!(t("{Episode Title}{-Release Group}"), ("{episode_title}{?group:-{group}}".into(), none.clone()));
    assert_eq!(t("{_Episode.Title_}"), ("{?episode_title:_{episode_title|space:.}_}".into(), none.clone()));
    assert_eq!(t("Some {{{Series Title}}} }} {{"), ("Some {{{title}}} }} {{".into(), none.clone()));
    assert_eq!(t("{SERIES TITLE}{sErIES-tItLE}"), ("{title|space:-}".into(), vec!["{SERIES TITLE}".to_string()]));
    assert_eq!(t("{Movie Title} ({Release Year})"), ("{title} ({year})".into(), none));
}
