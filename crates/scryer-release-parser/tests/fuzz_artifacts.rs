//! Replays the minimized fuzz crashes in `parity/fuzz/artifacts/parser/`
//! (ledger P12). Each input must parse without a panic and without a huge
//! allocation. The allocator below refuses any single allocation over
//! 64 MiB, so an unbounded episode range aborts the test instead of eating
//! the machine's memory.

use std::alloc::{GlobalAlloc, Layout, System};

use scryer_release_parser::{
    ContextAlias, ContextFacetHint, ContextTitle, ReleaseParseContext,
    analyze_release_against_targets, analyze_release_for_target,
};

struct Capped;

const MAX_ALLOCATION: usize = 64 << 20;

// SAFETY: forwards to the system allocator; refusing returns null, which the
// standard library turns into an allocation-failure abort.
unsafe impl GlobalAlloc for Capped {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > MAX_ALLOCATION {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Capped = Capped;

fn context(facet_hint: ContextFacetHint, name: &str) -> ReleaseParseContext {
    ReleaseParseContext {
        facet_hint,
        title: ContextTitle {
            name: name.to_string(),
        },
        aliases: Vec::new(),
        known_years: Vec::new(),
        imdb_ids: Vec::new(),
        episodes: Vec::new(),
    }
}

fn words(candidate: &str) -> impl Iterator<Item = &str> {
    candidate
        .split(|ch: char| {
            matches!(
                ch,
                '.' | '_' | ' ' | '-' | '/' | '[' | ']' | '(' | ')' | '{' | '}'
            )
        })
        .filter(|part| !part.trim().is_empty())
}

/// The `analyze_release` fuzz target (crates/scryer-release-parser/fuzz).
fn replay_for_target(data: &[u8]) {
    let candidate = String::from_utf8_lossy(data);
    let title = words(&candidate).next().unwrap_or("Unknown");
    let _ = analyze_release_for_target(&candidate, &context(ContextFacetHint::Series, title));
}

/// The `analyze_release_with_context` fuzz target.
fn replay_against_targets(data: &[u8]) {
    let candidate = String::from_utf8_lossy(data);
    let words: Vec<&str> = words(&candidate).take(6).collect();
    let mut anime = context(ContextFacetHint::Anime, words.first().unwrap_or(&"Unknown"));
    anime.aliases = words
        .iter()
        .skip(1)
        .take(2)
        .map(|name| ContextAlias {
            name: (*name).to_string(),
        })
        .collect();
    anime.known_years = candidate
        .split(|ch: char| !ch.is_ascii_digit())
        .filter_map(|part| part.parse::<i32>().ok())
        .filter(|year| (1900..=2099).contains(year))
        .take(2)
        .collect();
    let series = context(
        ContextFacetHint::Series,
        words.get(1).unwrap_or(&"Fallback"),
    );
    let _ = analyze_release_against_targets(&candidate, &[anime, series]);
}

const ARTIFACTS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../parity/fuzz/artifacts/parser"
);

fn artifact(path: &str) -> Vec<u8> {
    std::fs::read(format!("{ARTIFACTS}/{path}")).expect("fuzz artifact")
}

#[test]
fn wide_absolute_range_does_not_allocate_the_whole_range() {
    replay_for_target(&artifact(
        "analyze_release_for_target/oom-range-1-4000000000",
    ));
}

#[test]
fn wide_range_in_noise_does_not_allocate_the_whole_range() {
    replay_for_target(&artifact(
        "analyze_release_for_target/oom-range-build-candidate-8f11516a",
    ));
}

#[test]
fn repeated_episode_below_the_first_does_not_underflow() {
    replay_against_targets(&artifact(
        "analyze_release_against_targets/crash-underflow-e16E1-6",
    ));
}

fn episode(raw: &str) -> scryer_release_parser::ParsedEpisodeMetadata {
    let parsed = scryer_release_parser::best_parse_for_target(
        raw,
        &context(ContextFacetHint::Series, "Show Name"),
    );
    parsed.episode.unwrap_or_default()
}

/// The widest range still listed is 9999 episodes past the first (long anime
/// batches); one more is refused.
#[test]
fn range_of_the_widest_span_keeps_every_episode() {
    assert_eq!(
        episode("Show.Name.1-10000.720p.HDTV.x264-GRP").absolute_episode_numbers,
        (1..=10000).collect::<Vec<_>>()
    );
    assert!(
        episode("Show.Name.1-10001.720p.HDTV.x264-GRP")
            .absolute_episode_numbers
            .is_empty()
    );
}

/// A repeated marker for the same episode does not list it twice.
#[test]
fn repeated_marker_for_the_same_episode_is_not_listed_twice() {
    assert_eq!(
        episode("Show.Name.S01.E01.E01.720p.HDTV.x264-GRP").episode_numbers,
        vec![1]
    );
}

/// Found in review: an unsorted episode list puts the fill start below the
/// first episode, so a span check against the first alone let it through.
#[test]
fn repeated_episode_after_an_unsorted_list_does_not_allocate_the_whole_range() {
    for raw in [
        "Show S01.E4000000000E1-4000000500",
        "e4000000000E1-4000000500",
    ] {
        replay_for_target(raw.as_bytes());
        replay_against_targets(raw.as_bytes());
    }
}

/// The fill from the last listed episode is bounded on its own: `E1500E10`
/// then `-2000` is 500 past the first but 1990 past the last.
#[test]
fn repeated_episode_fill_is_bounded_from_the_last_listed_episode() {
    let listed = episode("Show.Name.S01.E1500E10-2000.720p.HDTV.x264-GRP").episode_numbers;
    assert!(listed.len() < 1000, "{} episodes", listed.len());
}

/// No season of 10000 or more, in `season` or `season_numbers` (ledger P14).
fn assert_no_huge_season(raw: &str) {
    let parsed = episode(raw);
    assert!(
        parsed.season.is_none_or(|season| season < 10_000)
            && parsed.season_numbers.iter().all(|season| *season < 10_000),
        "{raw}: {parsed:?}"
    );
}

/// A season token of five or more digits is not a season (ledger P14); four
/// digits (`S1936E18`) still are.
#[test]
fn five_digit_season_is_not_read() {
    let real = episode("Show.Name.S12345E02.S03E04.720p.HDTV.x264-GRP");
    assert_eq!(real.season, Some(3), "{real:?}");
    assert_eq!(real.episode_numbers, vec![4], "{real:?}");
    let four = episode("Show.Name.S1936E18.720p.HDTV.x264-GRP");
    assert_eq!(four.season, Some(1936), "{four:?}");
    assert_no_huge_season("Show.Name.S12345E02.720p.HDTV.x264-GRP");
}

/// The NxM route.
#[test]
fn five_digit_season_nxm_is_not_read() {
    assert_no_huge_season("Show.Name.12345x03.720p.HDTV.x264-GRP");
    let real = episode("Show.Name.12345x03.S03E04.720p.HDTV.x264-GRP");
    assert_eq!(real.season, Some(3), "{real:?}");
}

/// Every other route that reads a season word or token.
#[test]
fn five_digit_season_word_routes_are_not_read() {
    for raw in [
        "Show.Name.Season.12345.Episode.3.720p.HDTV.x264-GRP",
        "Show.Name.Complete.Season.12345.720p.HDTV.x264-GRP",
        "Show.Name.CompleteSeason12345.720p.HDTV.x264-GRP",
        "Show.Name.Season.12345.(01-12).720p.HDTV.x264-GRP",
        "Show Name (Season 12345) (01-12) 720p HDTV x264-GRP",
        "Show.Name.Season.12345.Complete.(01-12).720p.HDTV.x264-GRP",
        "Show.Name.Season.12345.Batch.01-12.720p.HDTV.x264-GRP",
        "Show.Name.2019.Season 12345 Episode 2-3.720p.HDTV.x264-GRP",
        "Show Name - Season 99999999 Episode 2-3 720p HDTV x264-GRP",
        "Show.Name.S12345.720p.HDTV.x264-GRP",
    ] {
        assert_no_huge_season(raw);
    }
}
