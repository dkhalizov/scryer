//! Parity net, phase 5: parser property tests (notes/11-parity-net-design.md
//! § Checks beyond examples).
//!
//! "Parses" means `disposition != Unparseable`, the parity harness's
//! translation of the reference's null result (crates/scryer-application/tests/parity_parse.rs).
//! The random-string properties port CrapParserFixture.should_not_parse_random
//! (SON-PARSE-0225/0226, RAD-PARSE-0029/0030), which the reference runs
//! unseeded; here the seed is fixed. Context: the title guess production makes
//! for a one-token name (the name itself) per facet, plus an unrelated series.

use proptest::prelude::*;
use proptest::string::string_regex;
use proptest::test_runner::{Config as ProptestConfig, RngSeed};
use scryer_release_parser::{
    ContextFacetHint, ContextTitle, ParseDisposition, ReleaseParseContext,
    analyze_release_for_target, best_parse_for_target,
};

fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        rng_seed: RngSeed::Fixed(0x5c71_e7a3),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

fn context(facet_hint: ContextFacetHint, title: &str) -> ReleaseParseContext {
    ReleaseParseContext {
        facet_hint,
        title: ContextTitle {
            name: title.to_string(),
        },
        aliases: Vec::new(),
        known_years: Vec::new(),
        imdb_ids: Vec::new(),
        episodes: Vec::new(),
    }
}

/// Contexts a series lookup would use (SON-PARSE-0225/0226).
fn series_contexts(raw: &str) -> Vec<ReleaseParseContext> {
    vec![
        context(ContextFacetHint::Series, raw),
        context(ContextFacetHint::Anime, raw),
        context(ContextFacetHint::Series, "Known Series"),
    ]
}

/// Contexts a movie lookup, or no facet at all, would use (RAD-PARSE-0029/0030).
fn movie_contexts(raw: &str) -> Vec<ReleaseParseContext> {
    vec![
        context(ContextFacetHint::Movie, raw),
        context(ContextFacetHint::Unknown, raw),
        context(ContextFacetHint::Movie, "Known Movie"),
        context(ContextFacetHint::Unknown, ""),
    ]
}

/// 32 or 40 chars: the reference's charset (alphanumeric), or hex like an md5/sha1.
fn random_name() -> impl Strategy<Value = String> {
    let pattern = |class: &str, len: usize| {
        string_regex(&format!("[{class}]{{{len}}}")).expect("valid regex")
    };
    prop_oneof![
        pattern("A-Za-z0-9", 32),
        pattern("A-Za-z0-9", 40),
        pattern("0-9a-f", 32),
        pattern("0-9a-f", 40),
        pattern("0-9A-F", 32),
        pattern("0-9A-F", 40),
    ]
}

fn assert_never_parses(raw: &str, targets: Vec<ReleaseParseContext>) -> Result<(), TestCaseError> {
    let parsed_under: Vec<String> = targets
        .iter()
        .filter_map(|target| {
            let analysis = analyze_release_for_target(raw, target);
            (analysis.disposition != ParseDisposition::Unparseable).then(|| {
                format!(
                    "{:?}/{:?} -> {:?}",
                    target.facet_hint, target.title.name, analysis.disposition
                )
            })
        })
        .collect();
    prop_assert!(
        parsed_under.is_empty(),
        "{:?} parsed under {:?}",
        raw,
        parsed_under
    );
    Ok(())
}

/// Weaker than unparseable: no season, episode, absolute number or air date.
fn assert_no_episode(raw: &str) -> Result<(), TestCaseError> {
    for target in series_contexts(raw).into_iter().chain(movie_contexts(raw)) {
        let parsed = best_parse_for_target(raw, &target);
        if let Some(episode) = parsed.episode {
            prop_assert!(
                episode.season.is_none()
                    && episode.episode_numbers.is_empty()
                    && episode.absolute_episode.is_none()
                    && episode.absolute_episode_numbers.is_empty()
                    && episode.air_date.is_none(),
                "{:?} under {:?} read as {:?}",
                raw,
                target.facet_hint,
                episode
            );
        }
    }
    Ok(())
}

const RESOLUTIONS: &[&str] = &["720p", "1080p", "2160p"];
const SOURCES: &[&str] = &["WEB-DL", "BluRay", "HDTV"];

/// "{Title}.S{ss}E{ee}.{res}.{src}.x264-{GROUP}" fields.
#[derive(Debug, Clone)]
struct Structured {
    title_words: Vec<String>,
    season: u32,
    episode: u32,
    resolution: &'static str,
    source: &'static str,
    group: String,
}

impl Structured {
    fn name(&self) -> String {
        format!(
            "{}.S{:02}E{:02}.{}.{}.x264-{}",
            self.title_words.join("."),
            self.season,
            self.episode,
            self.resolution,
            self.source,
            self.group
        )
    }
}

fn structured(word: &'static str) -> impl Strategy<Value = Structured> {
    structured_with(word.boxed(), group().boxed())
}

fn structured_with(
    word: BoxedStrategy<String>,
    group: BoxedStrategy<String>,
) -> impl Strategy<Value = Structured> {
    (
        prop::collection::vec(word, 1..5),
        0u32..=99,
        1u32..=99,
        prop::sample::select(RESOLUTIONS),
        prop::sample::select(SOURCES),
        group,
    )
        .prop_map(
            |(title_words, season, episode, resolution, source, group)| Structured {
                title_words,
                season,
                episode,
                resolution,
                source,
                group,
            },
        )
}

fn group() -> impl Strategy<Value = String> {
    "[A-Za-z][A-Za-z0-9]{1,9}"
}

/// Groups the reference treats specially, plus ordinary ones.
fn tricky_group() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => group(),
        1 => prop::sample::select(vec![
            "ES", "Es", "EN", "en", "CAT", "Cat", "Ca", "Eng", "Ger", "Fr", "Nf", "Dl",
            "Spa", "Rus", "Ita", "Pre", "sample", "NZBGeek", "Obfuscated", "4Planet",
            "xpost", "Rakuv", "Rakuv02", "DarQ", "KCRT", "VARYG", "BluDragon",
        ]).prop_map(String::from),
        1 => "[sSeE][0-9]{1,4}",
        1 => "[a-f][0-9a-fA-F]{7}",
        1 => "[A-Za-z]{1,4}(480|576|720|1080|2160)[pP]",
        1 => "[A-Za-z]{1,5}(DarQ|KCRT|VARYG)",
    ]
}

/// The release group Sonarr's ParseReleaseGroup returns for
/// "{Title}.S{ss}E{ee}.{res}.{src}.x264-{group}" with `group` alphanumeric and
/// starting with a letter (sonarr@v4.0.20.3014 Parser.cs:549-570, 908-966).
/// Reduced to this name shape: CleanReleaseGroupRegex drops everything up to
/// the SxxEyy token, so only the group can hit the exception lists.
fn reference_group(group: &str) -> Option<String> {
    let lower = group.to_ascii_lowercase();
    // CleanReleaseGroupRegex strips a trailing -<junk>; the rest ("...WEB-DL.x264"
    // or no dash at all) yields no group.
    const JUNK: &[&str] = &[
        "rp", "1", "nzbgeek", "obfuscated", "scrambled", "sample", "pre", "postbot", "xpost",
        "whiterev", "buymore", "asrequested", "alternativetorequested", "gerov", "z0ids3n",
        "chamele0n", "4p", "4planet", "altezachen", "repackpost",
    ];
    if JUNK.contains(&lower.as_str())
        || lower
            .strip_prefix("rakuv")
            .is_some_and(|rest| rest.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
    {
        return None;
    }
    // ExceptionReleaseGroupRegexExact: no leading boundary, \b after, so only
    // a suffix of the group can match; it wins over the generic regex.
    for exact in ["d-z0n3", "fight-bb", "varyg", "e.n.d", "kralimarko", "bludragon", "darq", "kcrt"] {
        if lower.ends_with(exact) {
            return Some(group[group.len() - exact.len()..].to_string());
        }
    }
    // ReleaseGroupRegex lookbehind: -ES/-EN/-CAT, or a group ending in a
    // resolution; no other dash in the name yields a group either.
    if ["es", "en", "cat"].contains(&lower.as_str())
        || ["480p", "576p", "720p", "1080p", "2160p"].iter().any(|res| lower.ends_with(res))
    {
        return None;
    }
    // InvalidReleaseGroupRegex ^([se]\d+|[0-9a-f]{8})$ (IgnoreCase).
    let invalid_episode_like = lower.len() > 1
        && matches!(lower.as_bytes()[0], b's' | b'e')
        && lower[1..].bytes().all(|b| b.is_ascii_digit());
    let invalid_hash = lower.len() == 8 && lower.bytes().all(|b| b.is_ascii_hexdigit());
    (!invalid_episode_like && !invalid_hash).then(|| group.to_string())
}

/// Title words for the season bound: plain words plus ones that look like
/// season or year tokens. Season digits are 3 or 5 long, never 4: the
/// reference reads any 4-digit explicit season as itself (S1936E18 -> 1936,
/// Season2010; sonarr Parser.cs:155,227,353), so those may exceed 1000.
fn seasonish_word() -> impl Strategy<Value = String> {
    prop_oneof![
        2 => "[A-Z0-9][a-z0-9]{0,5}",
        1 => "S([0-9]{3}|[0-9]{5})",
        1 => "Season([0-9]{3}|[0-9]{5})",
        1 => "S[0-9]{3}E[0-9]{2}",
        1 => "S0[1-9]\\+S([0-9]{3}|[0-9]{5})",
        1 => "S0[1-9]-S?([0-9]{3}|[0-9]{5})",
        1 => "Seasons[1-9]\\+([0-9]{3}|[0-9]{5})",
        1 => "Season\\.0[1-9]-Season\\.([0-9]{3}|[0-9]{5})",
        1 => "(19|20)[0-9]{2}",
    ]
}

fn assert_seasons_bounded(
    name: &str,
    parsed: &scryer_release_parser::ParsedReleaseMetadata,
) -> Result<(), TestCaseError> {
    if let Some(episode) = &parsed.episode {
        prop_assert!(episode.season.is_none_or(|season| season <= 1000), "season of {:?}", name);
        prop_assert!(
            episode.season_numbers.iter().all(|season| *season <= 1000),
            "seasons of {:?}: {:?}", name, episode.season_numbers
        );
    }
    Ok(())
}

fn assert_seasons_bounded_without_title(name: &str) -> Result<(), TestCaseError> {
    for target in [
        context(ContextFacetHint::Series, ""),
        context(ContextFacetHint::Series, "Known Series"),
        context(ContextFacetHint::Unknown, ""),
    ] {
        assert_seasons_bounded(name, &best_parse_for_target(name, &target))?;
    }
    Ok(())
}

fn parse_structured(fields: &Structured) -> (String, scryer_release_parser::ParsedReleaseMetadata) {
    let name = fields.name();
    let target = context(ContextFacetHint::Series, &fields.title_words.join(" "));
    let parsed = best_parse_for_target(&name, &target);
    (name, parsed)
}

fn round_trip_numbers(fields: &Structured) -> Result<(), TestCaseError> {
    let (name, parsed) = parse_structured(fields);
    let episode = parsed.episode.clone().unwrap_or_default();
    prop_assert_eq!(episode.season, Some(fields.season), "season of {:?}", name);
    prop_assert_eq!(
        &episode.episode_numbers,
        &vec![fields.episode],
        "episodes of {:?}",
        name
    );
    prop_assert_eq!(
        parsed.quality.as_deref(),
        Some(fields.resolution),
        "quality of {:?}",
        name
    );
    Ok(())
}

proptest! {
    #![proptest_config(config(512))]

    #[test]
    fn analyze_never_panics_on_arbitrary_text(
        raw in prop_oneof![any::<String>(), "[ -~]{0,200}", "\\PC{0,120}"],
        facet in prop::sample::select(vec![
            ContextFacetHint::Series, ContextFacetHint::Movie,
            ContextFacetHint::Anime, ContextFacetHint::Unknown,
        ]),
        title in prop_oneof![Just(String::new()), "[ -~]{0,40}"],
    ) {
        let target = context(facet, &title);
        let analysis = analyze_release_for_target(&raw, &target);
        let parsed = best_parse_for_target(&raw, &target);
        prop_assert_eq!(&parsed.raw_title, &raw);
        prop_assert_eq!(&analysis.raw_input, &raw);
    }

    #[test]
    fn random_names_never_parse_as_series(raw in random_name()) {
        assert_never_parses(&raw, series_contexts(&raw))?;
    }

    #[test]
    #[ignore = "known red: a 32/40-char alnum or hex string parses (Parsed) under a Movie or Unknown facet"]
    fn random_names_never_parse_as_movies(raw in random_name()) {
        assert_never_parses(&raw, movie_contexts(&raw))?;
    }

    #[test]
    fn random_names_never_read_as_episodes(raw in random_name()) {
        assert_no_episode(&raw)?;
    }

    #[test]
    fn structured_name_round_trips_season_episode_resolution(
        fields in structured("[A-Z][a-z]{2,8}"),
    ) {
        round_trip_numbers(&fields)?;
    }

    /// The group matches the reference's answer, both ways: kept where Sonarr
    /// keeps it, dropped where Sonarr drops it.
    #[test]
    #[ignore = "known red: groups that are also tags (Ca, Eng, Ger, Fr, Nf, Dl, ...) parse as None; e12/S5/abcdef12/Es kept where Sonarr gives None"]
    fn structured_name_round_trips_group(
        fields in structured_with("[A-Z][a-z]{2,8}".boxed(), tricky_group().boxed()),
    ) {
        let (name, parsed) = parse_structured(&fields);
        prop_assert_eq!(
            parsed.release_group, reference_group(&fields.group), "group of {:?}", name
        );
    }

    /// Title words may look like seasons or years (S12345, Season300,
    /// Season.01-Season.12345, 1999): no season above 1000 comes out when
    /// the context knows the title.
    #[test]
    fn structured_name_season_is_bounded(
        fields in structured_with(seasonish_word().boxed(), group().boxed()),
    ) {
        let (name, parsed) = parse_structured(&fields);
        assert_seasons_bounded(&name, &parsed)?;
    }

    /// The same names when the context does not know the title (no title, an
    /// unrelated one, no facet): still no season above 1000. Only here do the
    /// range words reach the season-range bound (MAX_EXPLICIT_SEASON).
    #[test]
    fn structured_name_season_is_bounded_without_known_title(
        fields in structured_with(seasonish_word().boxed(), group().boxed()),
    ) {
        assert_seasons_bounded_without_title(&fields.name())?;
    }

    /// A title word that is a 5-digit SxxxxxEyy: the reference never reads a
    /// 5-digit season (Parser.cs: `(?<!\d+)(?:\d{1,2}|\d{4})(?!\d+)`, \d{3}
    /// at :307), so it takes the real SxxEyy.
    #[test]
    fn structured_name_season_is_bounded_with_five_digit_season_word(
        fields in structured_with(
            prop_oneof!["[A-Z][a-z]{2,8}", "S[0-9]{5}E[0-9]{2}"].boxed(),
            group().boxed(),
        ),
    ) {
        assert_seasons_bounded_without_title(&fields.name())?;
    }

    #[test]
    #[ignore = "known red: a numeric title read as audio channels (20, 51, 71, 120) makes 20.S01E01... Unparseable"]
    fn structured_name_round_trips_with_numeric_title_words(
        fields in structured("[A-Z0-9][a-z0-9]{0,5}"),
    ) {
        round_trip_numbers(&fields)?;
    }
}
