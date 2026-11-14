//! The library scan's filename parser takes any relative path under a library
//! root without panicking, and never reports a season of 10000 or more.
#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use scryer_application::parity_seams::{
    LibraryFilenameParseInput, LibraryFilenameParseMode, parse_library_filename,
};

const ROOT: &str = "/library/tv";

fn check_seasons(
    what: &str,
    path: &str,
    episode: Option<&scryer_application::ParsedEpisodeMetadata>,
) {
    let Some(episode) = episode else { return };
    if let Some(season) = episode.season {
        assert!(season < 10_000, "{what} season {season} for {path:?}");
    }
    if let Some(season) = episode
        .season_numbers
        .iter()
        .find(|season| **season >= 10_000)
    {
        panic!("{what} season_numbers has {season} for {path:?}");
    }
}

fuzz_target!(|input: (bool, bool, String)| {
    let (title_scan, rooted, relative) = input;
    let full = format!("{ROOT}/{relative}");
    let path = Path::new(if rooted { &full } else { &relative });
    let mut parse_input =
        LibraryFilenameParseInput::title_only(path, rooted.then(|| Path::new(ROOT)));
    if title_scan {
        parse_input.mode = LibraryFilenameParseMode::TitleScan;
    }
    let parse = parse_library_filename(&parse_input);
    check_seasons(
        "parsed_release",
        &relative,
        parse.parsed_release.episode.as_ref(),
    );
    check_seasons(
        "episode_identity",
        &relative,
        parse.episode_identity.as_ref(),
    );
});
