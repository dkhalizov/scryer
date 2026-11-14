//! Rename-off naming suites (stage 6-7, stream `naming`).
//!
//! Suites: `son_naming_rename_off` (SON-NAMING-0001..0007 `build_file_path`,
//! 0058..0061 `build_file_name` with `renameEpisodes` false) and
//! `rad_naming_rename_off` (RAD-NAMING-0041).
//!
//! Scryer functions under test (parity_seams, "fork: parity net seam"):
//! - `episode_import_dest_path` (import/workflow/series.rs): the series
//!   import destination with the rename on/off switch, title folder and
//!   season/specials folder join. Called with `rename_enabled = false`.
//! - `preserved_import_filename` (import/workflow/paths.rs): the rename-off
//!   file name every movie import branch uses. The movie switch itself is
//!   inline in async import code and is not exercised.
//!
//! Rule: source name = relativePath ?? path ?? sceneName, because Scryer
//! keeps the source file's own name and has no scene-name input. Cases where
//! the arr prefers the scene name over relativePath (0060, 0061, RAD-0041)
//! are therefore red (D29), not unmapped. `build_file_path` cases give only
//! sceneName; it stands in for the source file's own name (the reference
//! fixture's NBuilder RelativePath is "RelativePath1", and NamingConfig.Default
//! has rename off, so the arr's name is `GetOriginalTitle` = the scene name).
//! So 0001..0006 attest the title/season/specials folder join and the rename
//! switch, not the scene-name preference (that is 0060/0061).
//!
//! # Translation
//!
//! | reference                                   | Scryer                                           |
//! |---------------------------------------------|--------------------------------------------------|
//! | naming.renameEpisodes / rename_movies false | `rename_enabled = false` / `preserved_import_filename` |
//! | standardEpisodeFormat                       | `rename_template` (not rendered with rename off) |
//! | seasonFolderFormat, specialsFolderFormat    | season / specials folder templates (via `translate`) |
//! | series.title                                | `Title.name`                                     |
//! | series.path, series.seasonFolder            | `title_folder_path`, `use_season_folders` (build_file_name: `/library/<title>`, off) |
//! | episodes[0] season/episode/absolute/title   | `season_num`, `ep_num_str`, `absolute_number`, `episode_title` |
//! | episodeFile.relativePath / path / sceneName | source file `/downloads/<name><extension>` (see rule) |
//! | movie_file.relative_path / scene_name       | source file `/downloads/<relative_path>.mkv`     |
//!
//! `build_file_path` compares the full path; `build_file_name` compares the
//! file name without the `.mkv` suffix. SON-NAMING-0007
//! (BuildFilePathFixture.cs:59, asserted at :84) checks that a colon in the
//! series title is cleaned in a `{Series Title}` season folder. Its red is
//! the colon: Scryer's sanitiser gives
//! "NCIS Los Angeles", the arr's Smart mode "NCIS - Los Angeles" (P15).
//!
//! Ignored keys: episodeFile.sceneName when relativePath or path is given,
//! replaceIllegalCharacters, colonReplacementFormat,
//! customColonReplacementFormat, multiEpisodeStyle, dailyEpisodeFormat,
//! animeEpisodeFormat, seriesFolderFormat, seriesType, episodeFile quality /
//! revision / releaseGroup, note; Radarr: scene_name, quality, revision,
//! release_group, movie.title, replace_illegal_characters,
//! colon_replacement_format, movie_folder_format, standard_movie_format.
//!
//! # Unmapped (tests: []), with reasons
//!
//! - SON-NAMING-0188..0190 (`requires_episode_title`): concept absent.
//!   Scryer has no predicate or import gate asking whether the episode
//!   template uses `{episode_title}` and holding the import until the title
//!   is known. The nearest logic, `RenameMissingMetadataPolicy::Skip`
//!   (library/rename.rs), reacts only when the whole rendered name is empty.
//!   The import counterpart (SON-IMPORT-0163..0172) is unmapped likewise.

use super::{MEDIA_ROOT, Plan, number, record, reference_cases, text, translate};
use scryer_application::parse_release_metadata;
use scryer_application::parity_seams::{episode_import_dest_path, preserved_import_filename};
use scryer_domain::MediaFacet;
use scryer_parity::Suite;
use std::path::Path;

fn strip_mkv(name: &str) -> String {
    name.strip_suffix(".mkv").unwrap_or(name).to_string()
}

#[test]
fn son_naming_rename_off() {
    let Some(mut suite) = Suite::open("scryer-application::parity_naming::son_naming_rename_off")
    else {
        return;
    };
    for case in reference_cases("sonarr") {
        let op = text(&case["input"], "op");
        let given = &case["input"]["given"];
        let naming = &given["naming"];
        let is_path = op == "build_file_path";
        if !(is_path || (op == "build_file_name" && naming["renameEpisodes"].as_bool() == Some(false))) {
            continue;
        }
        let id = text(&case, "id");
        if !suite.wants(id) {
            continue;
        }
        let title = Plan::new(MediaFacet::Series, text(&given["series"], "title")).title();
        let ep = &given["episodes"][0];
        let file = &given["episodeFile"];
        let extension = case["input"]["given"]["extension"].as_str().unwrap_or(".mkv");
        let stem = file["relativePath"]
            .as_str()
            .or_else(|| file["path"].as_str())
            .or_else(|| file["sceneName"].as_str())
            .expect("source name");
        let source = if file["relativePath"].is_null() && file["path"].is_string() {
            format!("{stem}{extension}")
        } else {
            format!("/downloads/{stem}{extension}")
        };
        let source = Path::new(&source);
        let season_t = translate(text(naming, "seasonFolderFormat"));
        let specials_t = translate(text(naming, "specialsFolderFormat"));
        let rename_t = translate(text(naming, "standardEpisodeFormat"));
        let (title_folder, use_season) = if is_path {
            (text(&given["series"], "path").to_string(), given["series"]["seasonFolder"].as_bool().unwrap_or(false))
        } else {
            (format!("{MEDIA_ROOT}/{}", title.name), false)
        };
        let season = number(&ep["seasonNumber"]);
        let absolute = ep["absoluteEpisodeNumber"].as_u64().map(|n| n.to_string());
        let parsed = parse_release_metadata(
            source.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
        );
        let dest = episode_import_dest_path(
            &title,
            use_season,
            &parsed,
            None,
            extension.trim_start_matches('.'),
            source,
            Path::new(&title_folder),
            false,
            &rename_t.scryer,
            &season_t.scryer,
            &specials_t.scryer,
            season,
            &number(&ep["episodeNumber"]).to_string(),
            absolute.as_deref(),
            ep["title"].as_str(),
            None,
        );
        if is_path {
            let lacking: Vec<String> = if use_season {
                if season == 0 { specials_t.lacking.clone() } else { season_t.lacking.clone() }
            } else {
                vec![]
            };
            record(
                &mut suite,
                id,
                &lacking,
                text(&case["expect"], "path"),
                Some(dest.to_string_lossy().into_owned()),
            );
        } else {
            let name = dest.file_name().map(|n| strip_mkv(&n.to_string_lossy()));
            suite.check(id, Some(text(&case["expect"], "name").to_string()), name);
        }
    }
    suite.finish();
}

#[test]
fn rad_naming_rename_off() {
    let Some(mut suite) = Suite::open("scryer-application::parity_naming::rad_naming_rename_off")
    else {
        return;
    };
    for case in reference_cases("radarr") {
        let given = &case["input"]["given"];
        if text(&case["input"], "op") != "build_file_name"
            || given["naming"]["rename_movies"].as_bool() != Some(false)
        {
            continue;
        }
        let id = text(&case, "id");
        if !suite.wants(id) {
            continue;
        }
        let rel = given["movie_file"]["relative_path"]
            .as_str()
            .or_else(|| given["movie_file"]["path"].as_str())
            .expect("source name");
        let actual = preserved_import_filename(Path::new(&format!("/downloads/{rel}.mkv")));
        suite.check(id, Some(text(&case["expect"], "name").to_string()), Some(strip_mkv(&actual)));
    }
    suite.finish();
}
