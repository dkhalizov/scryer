use chrono::NaiveDate;
use scryer_domain::{Collection, Episode, MediaFacet, Title};

pub use scryer_release_parser::{
    AudioCodec, ContextAlias, ContextEpisode, ContextFacetHint, ContextTitle,
    ContextTitleMatchKind, ExternalIdSource, ParseDisposition, ParsedEpisodeMetadata,
    ParsedEpisodeReleaseType, ParsedReleaseMetadata, ParsedSpecialKind, ParsedStereoscopy,
    ReleaseParseAnalysis, ReleaseParseContext, ReleaseSource, StereoEncoding, StereoLayout,
    StereoPresentation, StereoSampling, StreamingService, TargetedReleaseParseAnalysis, VideoCodec,
    analyze_release_against_targets, analyze_release_for_target, best_parse_for_target,
};

pub fn parse_release_metadata(raw: &str) -> ParsedReleaseMetadata {
    project_analysis(raw, &neutral_release_analysis(raw))
}

#[doc(hidden)] // fork: parity net seam (was pub(crate))
pub fn neutral_release_analysis(raw: &str) -> ReleaseParseAnalysis {
    analyze_release_for_target(raw, &synthesize_release_parse_context(raw))
}

pub(crate) fn parsed_release_source_type(parsed: &ParsedReleaseMetadata) -> Option<String> {
    if parsed.is_remux {
        return Some("Remux".to_string());
    }
    if parsed.is_raw_hd {
        return Some("RawHD".to_string()); // fork (P33e): tier key RAWHD-1080P
    }

    parsed.source.as_ref().map(ToString::to_string)
}

pub fn parse_release_metadata_for_target(
    raw: &str,
    context: &ReleaseParseContext,
) -> ParsedReleaseMetadata {
    project_analysis(raw, &analyze_release_for_target(raw, context))
}

pub fn build_release_parse_context(
    title: &Title,
    episode: Option<&Episode>,
    _collection: Option<&Collection>,
    facet_hint: Option<&str>,
) -> ReleaseParseContext {
    build_release_parse_context_from_episodes(title, episode, facet_hint)
}

pub fn build_release_parse_context_for_title(
    title: &Title,
    episodes: &[Episode],
    facet_hint: Option<&str>,
) -> ReleaseParseContext {
    build_release_parse_context_from_episodes(title, episodes.iter(), facet_hint)
}

pub(crate) fn build_release_parse_context_from_episodes<'a>(
    title: &Title,
    episodes: impl IntoIterator<Item = &'a Episode>,
    facet_hint: Option<&str>,
) -> ReleaseParseContext {
    let title = title.with_custom_search_aliases();
    let aliases = title
        .aliases
        .iter()
        .map(|alias| ContextAlias {
            name: alias.clone(),
        })
        .chain(title.tagged_aliases.iter().map(|alias| ContextAlias {
            name: alias.name.clone(),
        }))
        .collect::<Vec<_>>();

    let imdb_ids = title
        .external_ids
        .iter()
        .filter(|external_id| external_id.source.eq_ignore_ascii_case("imdb"))
        .map(|external_id| external_id.value.clone())
        .collect::<Vec<_>>();

    let episodes = episodes
        .into_iter()
        .map(domain_episode_to_parse_context)
        .collect::<Vec<_>>();

    ReleaseParseContext {
        facet_hint: parse_context_facet_hint(facet_hint, &title.facet),
        title: ContextTitle {
            name: title.name.clone(),
        },
        aliases,
        known_years: title.year.into_iter().collect(),
        imdb_ids,
        episodes,
    }
}

fn domain_episode_to_parse_context(episode: &Episode) -> ContextEpisode {
    ContextEpisode {
        season: episode
            .season_number
            .as_deref()
            .and_then(|value| value.parse::<u32>().ok()),
        episode: episode
            .episode_number
            .as_deref()
            .and_then(|value| value.parse::<u32>().ok()),
        absolute_number: episode
            .absolute_number
            .as_deref()
            .and_then(|value| value.parse::<u32>().ok()),
        air_date: episode
            .air_date
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()),
        title: episode.title.clone(),
        title_aliases: episode.title.clone().into_iter().collect(),
    }
}

pub fn build_candidate_bank_contexts<'a>(
    titles: impl IntoIterator<Item = &'a Title>,
    episode: Option<&Episode>,
    collection: Option<&Collection>,
    facet_hint: Option<&str>,
    limit: usize,
) -> Vec<ReleaseParseContext> {
    titles
        .into_iter()
        .take(limit)
        .map(|title| build_release_parse_context(title, episode, collection, facet_hint))
        .collect()
}

fn parse_context_facet_hint(facet_hint: Option<&str>, facet: &MediaFacet) -> ContextFacetHint {
    match facet_hint.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) if value.eq_ignore_ascii_case("movie") => ContextFacetHint::Movie,
        Some(value) if value.eq_ignore_ascii_case("anime") => ContextFacetHint::Anime,
        Some(value) if value.eq_ignore_ascii_case("series") || value.eq_ignore_ascii_case("tv") => {
            ContextFacetHint::Series
        }
        _ => match facet {
            MediaFacet::Movie => ContextFacetHint::Movie,
            MediaFacet::Series => ContextFacetHint::Series,
            MediaFacet::Anime => ContextFacetHint::Anime,
        },
    }
}

fn project_analysis(raw: &str, analysis: &ReleaseParseAnalysis) -> ParsedReleaseMetadata {
    let mut projected = analysis
        .best_candidate()
        .map(|candidate| candidate.projected.clone())
        .unwrap_or_else(|| ParsedReleaseMetadata::empty(raw, analysis.parser_version));

    projected.ambiguity_margin = analysis.ambiguity_margin;
    projected.is_ambiguous = analysis.is_ambiguous;
    projected.disposition = analysis.disposition;
    projected.scoring_model_version = analysis.scoring_model_version;

    merge_analysis_hints(&mut projected, analysis);
    projected
}

fn merge_analysis_hints(parsed: &mut ParsedReleaseMetadata, analysis: &ReleaseParseAnalysis) {
    for hint in &analysis.parse_hints {
        push_unique_hint(&mut parsed.parse_hints, hint.clone());
    }

    push_unique_hint(
        &mut parsed.parse_hints,
        format!(
            "parse_status:{}",
            match parsed.disposition {
                ParseDisposition::Parsed => "parsed",
                ParseDisposition::Ambiguous => "ambiguous",
                ParseDisposition::Unparseable => "unparseable",
            }
        ),
    );
    push_unique_hint(
        &mut parsed.parse_hints,
        format!("parse_ambiguity_margin:{}", analysis.ambiguity_margin),
    );
    push_unique_hint(
        &mut parsed.parse_hints,
        format!("scoring_model_version:{}", analysis.scoring_model_version),
    );
    if parsed.is_ambiguous {
        push_unique_hint(&mut parsed.parse_hints, "v2:ambiguous".to_string());
    }
    if matches!(parsed.disposition, ParseDisposition::Unparseable) {
        push_unique_hint(&mut parsed.parse_hints, "v2:unparseable".to_string());
    }
}

fn push_unique_hint(hints: &mut Vec<String>, value: String) {
    if !hints.iter().any(|existing| existing == &value) {
        hints.push(value);
    }
}

/// Strip leading `[group]` tags (and separator noise after them) from a raw
/// release name, returning the remainder and each stripped group's content so
/// a bracket-styled *title* can still be recovered from it.
fn strip_leading_bracket_groups(raw_title: &str) -> (&str, Vec<&str>) {
    let mut groups = Vec::new();
    let mut rest = raw_title.trim_start();
    loop {
        if let Some(stripped) = rest.strip_prefix('[')
            && let Some(end) = stripped.find(']')
        {
            groups.push(stripped[..end].trim());
            rest = stripped[end + 1..]
                .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '_' | '.'));
            continue;
        }
        return (rest, groups);
    }
}

fn synthesize_release_parse_context(raw: &str) -> ReleaseParseContext {
    // fork (P17): the parser drops a site tag; keep it out of the context title.
    let raw = scryer_release_parser::strip_website_prefix(raw);
    let tokenize = |value: &str| {
        value
            .split(|ch: char| {
                matches!(
                    ch,
                    '.' | '_' | ' ' | '-' | '/' | '[' | ']' | '(' | ')' | '{' | '}'
                )
            })
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let tokens = tokenize(raw);

    // Leading `[group]` tags (including multi-token ones like `[Erai-raws]` or
    // `[Anime Time]`, and stacked tags) are release provenance, not title text.
    // Strip them bracket-aware before guessing the title span; a token-count
    // skip cannot know how many tokens the bracket group spans.
    let (stripped, leading_groups) = strip_leading_bracket_groups(raw);
    let stripped_tokens = tokenize(stripped);
    let (scan_tokens, title_start) = if stripped_tokens.is_empty() {
        // Nothing but bracket groups — fall back to the raw token stream.
        let title_start = usize::from(raw.trim_start().starts_with('[') && tokens.len() > 1);
        (&tokens, title_start)
    } else {
        // fork (P35 B): `(1995) Movie Name` names its year first (Radarr).
        let leading_year = stripped.trim_start().starts_with('(')
            && stripped_tokens.len() > 1
            && parse_context_year(&stripped_tokens[0]).is_some()
            && !looks_like_release_metadata_token(&stripped_tokens[1], &stripped_tokens);
        (&stripped_tokens, usize::from(leading_year))
    };
    let title_boundary = scan_tokens
        .iter()
        .enumerate()
        .skip(title_start)
        .find(|(index, token)| {
            (looks_like_release_metadata_token(token, scan_tokens) || series_season_at(*index, scan_tokens))
                && !numeric_title_word(token, &scan_tokens[index + 1..], scan_tokens)
        })
        .map(|(index, _)| index)
        .unwrap_or(scan_tokens.len());
    let title_tokens = scan_tokens
        .iter()
        .skip(title_start)
        .take(title_boundary.saturating_sub(title_start))
        .take(12)
        .cloned()
        .collect::<Vec<_>>();
    // A name whose only title text lives in a leading bracket group is a
    // bracket-styled title (`[Nagi no Ko].S02E01...`), not a group tag — a
    // group tag is always followed by title text. Prefer the last such group
    // that carries non-metadata tokens (tags stack before the title).
    let bracket_styled_title = if title_tokens.is_empty() {
        leading_groups.iter().rev().find_map(|group| {
            let group_tokens = tokenize(group);
            (!group_tokens.is_empty()
                && group_tokens
                    .iter()
                    .any(|token| !looks_like_release_metadata_token(token, &group_tokens)))
            .then(|| group_tokens.join(" "))
        })
    } else {
        None
    };
    let title = if let Some(bracket_styled_title) = bracket_styled_title {
        bracket_styled_title
    } else if title_tokens.is_empty() && leading_identity(&scan_tokens[title_start.min(scan_tokens.len())..]) {
        // fork (P34a): `S02E03-04-05.720p` has no title; its first token is
        // identity, not a context title the parse would then link as alias.
        String::new()
    } else if title_tokens.is_empty() {
        scan_tokens
            .get(title_start)
            .or_else(|| scan_tokens.first())
            .or_else(|| tokens.first())
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string())
    } else {
        title_tokens.join(" ")
    };

    ReleaseParseContext {
        facet_hint: guess_context_facet(raw, &tokens),
        title: ContextTitle { name: title },
        aliases: Vec::new(),
        known_years: tokens
            .iter()
            .filter_map(|token| parse_context_year(token))
            .chain(bracketed_old_year(raw))
            .collect(),
        imdb_ids: tokens
            .iter()
            .filter_map(|token| normalize_context_imdb_id(token))
            .collect(),
        episodes: Vec::new(),
    }
}

/// fork (P35 B): a number that only looks like an absolute episode is title
/// text when an episode, season or year marker follows (`30 Series S01E01`,
/// `(500) Days Of Movie (2009)`); a year is title text only when another year
/// follows it directly (`Klasse von 1999.1990`). Sonarr/Radarr read numeric
/// titles the same way.
fn numeric_title_word(token: &str, later: &[String], tokens: &[String]) -> bool {
    let normalized = normalize_alphanumeric_upper(token);
    let year = |t: &String| parse_context_year(&normalize_alphanumeric_upper(t)).is_some();
    if parse_context_year(&normalized).is_some() {
        // fork (P34h): `series.2009.416.hdtv`: the year is title text.
        return later.first().is_some_and(year)
            || later.first().is_some_and(|next| compact_number(&normalize_alphanumeric_upper(next)) && compact_follows(&later[1..]));
    }
    looks_like_absolute_episode_token(&normalized)
        && later.iter().any(|t| {
            let next = normalize_alphanumeric_upper(t);
            looks_like_standard_episode_token(&next)
                || looks_like_season_pack_marker(&next, tokens)
                || spelled_season_marker(&next, tokens)
                || year(t)
        })
}

/// fork (P34a): a name that opens with a strict `SxxEyy`/`NxNN` identity or a
/// y-m-d date. Strict, so `Se7en`, `Sense8` and `S1m0ne` keep their titles.
fn leading_identity(tokens: &[String]) -> bool {
    let Some(first) = tokens.first() else {
        return false;
    };
    strict_identity_prefix(first)
        || looks_like_daily_token(first)
        || looks_like_daily_token_sequence(&tokens[..tokens.len().min(3)])
}

fn strict_identity_prefix(token: &str) -> bool {
    let bytes = token.as_bytes();
    let digit_run = |from: usize| bytes[from.min(bytes.len())..].iter().take_while(|b| b.is_ascii_digit()).count();
    let after = if bytes.first().is_some_and(|b| b.eq_ignore_ascii_case(&b'S')) {
        let season = digit_run(1);
        if !(1..=4).contains(&season) || !bytes.get(1 + season).is_some_and(|b| b.eq_ignore_ascii_case(&b'E')) {
            return false;
        }
        let episode = digit_run(2 + season);
        if episode == 0 {
            return false;
        }
        2 + season + episode
    } else {
        let season = digit_run(0);
        if !(1..=4).contains(&season) || !bytes.get(season).is_some_and(|b| b.eq_ignore_ascii_case(&b'X')) {
            return false;
        }
        let episode = digit_run(season + 1);
        if episode == 0 {
            return false;
        }
        season + 1 + episode
    };
    match bytes.get(after) {
        None => true,
        Some(b) if !b.is_ascii_alphanumeric() => true,
        // `E01b` split letter, or a chained `E05`/`x05`.
        Some(b) => {
            matches!(b.to_ascii_lowercase(), b'a'..=b'd') && bytes.get(after + 1).is_none_or(|n| !n.is_ascii_alphanumeric())
                || matches!(b.to_ascii_lowercase(), b'e' | b'x') && bytes.get(after + 1).is_some_and(u8::is_ascii_digit)
        }
    }
}

fn guess_context_facet(raw: &str, tokens: &[String]) -> ContextFacetHint {
    if tokens
        .iter()
        .any(|token| looks_like_standard_episode_token(token))
        // fork (P34b3): `Series.S02.2010.1080p` is a season pack, not a
        // movie; `Title S2 - 05` (anime) stays out.
        || tokens.iter().enumerate().any(|(index, token)| {
            looks_like_season_pack_marker(&normalize_alphanumeric_upper(token), tokens)
                && !tokens
                    .get(index + 1)
                    .is_some_and(|next| !next.is_empty() && next.chars().all(|ch| ch.is_ascii_digit()) && parse_context_year(next).is_none())
        })
        // fork (P34b1): Spanish `TEMPORADA 2`.
        || temporada_marker(tokens)
        || tokens.iter().any(|token| chain_marker(&normalize_alphanumeric_upper(token), tokens))
        || (0..tokens.len()).any(|index| series_season_at(index, tokens))
        // fork (P34h): `Series.and.a.Title.103.720p.HDTV`; spaced names are not.
        || (!raw.contains(' ') && (0..tokens.len()).any(|index| compact_episode_at(index, tokens)))
        || tokens.iter().any(|token| bare_episode_token(&normalize_alphanumeric_upper(token), tokens))
        || tokens.iter().any(|token| looks_like_daily_token(token))
        || looks_like_daily_token_sequence(tokens)
        || looks_like_season_pack_release(tokens)
    {
        return ContextFacetHint::Series;
    }
    let has_year = tokens
        .iter()
        .any(|token| parse_context_year(token).is_some())
        || bracketed_old_year(raw).is_some();
    if raw.contains('[')
        && tokens
            .iter()
            .any(|token| looks_like_absolute_episode_token(token))
        && !has_year
    {
        return ContextFacetHint::Anime;
    }
    if has_year {
        return ContextFacetHint::Movie;
    }
    ContextFacetHint::Unknown
}

/// fork (P35 B): a `(1897)` or `[1897]` year, which Radarr reads; a bare
/// 18xx number stays title text (`1883.S01E01`), as in the parser.
fn bracketed_old_year(raw: &str) -> Option<i32> {
    raw.as_bytes().windows(6).find_map(|w| {
        let digits = &w[1..5];
        (matches!((w[0], w[5]), (b'(', b')') | (b'[', b']'))
            && digits.starts_with(b"18")
            && digits.iter().all(u8::is_ascii_digit))
        .then(|| std::str::from_utf8(digits).ok()?.parse().ok())
        .flatten()
    })
}

fn parse_context_year(token: &str) -> Option<i32> {
    let normalized = token
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (normalized.len() == 4)
        .then(|| normalized.parse::<i32>().ok())
        .flatten()
        .filter(|year| (1900..=2099).contains(year))
}

fn normalize_context_imdb_id(token: &str) -> Option<String> {
    let normalized = token
        .trim()
        .trim_matches(|ch| matches!(ch, '{' | '}' | '[' | ']'));
    let imdb = normalized.strip_prefix("tt").unwrap_or(normalized);
    (!imdb.is_empty() && imdb.chars().all(|ch| ch.is_ascii_digit())).then(|| format!("tt{imdb}"))
}

fn looks_like_release_metadata_token(token: &str, tokens: &[String]) -> bool {
    let normalized = token
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();

    parse_context_year(&normalized).is_some()
        || looks_like_standard_episode_token(&normalized)
        || looks_like_daily_token(&normalized)
        || looks_like_absolute_episode_token(&normalized)
        || looks_like_season_pack_marker(&normalized, tokens)
        || spelled_season_marker(&normalized, tokens)
        || looks_like_release_provenance_normalized(&normalized)
        // fork (P34b1): `S42 E01`, `TEMPORADA 2`, `COMPLETE SERIES`.
        || season_before_episode(&normalized, tokens)
        || bare_episode_token(&normalized, tokens)
        || (normalized == "TEMPORADA" && temporada_marker(tokens))
        || complete_before_season(&normalized, tokens)
        || chain_marker(&normalized, tokens)
}

/// fork (P34h): `Series.and.a.Title.103.720p.HDTV`: a bare 3-4 digit number
/// (not a year or resolution) after the title, followed by a TV source, a
/// resolution and TV source, or the end. Mirrors `parse_compact_episode_at`.
fn compact_number(normalized: &str) -> bool {
    let Ok(number) = normalized.parse::<u32>() else {
        return false;
    };
    (normalized.len() == 3 || normalized.len() == 4 && parse_context_year(normalized).is_none())
        && !normalized.starts_with('0')
        && !matches!(number, 240 | 360 | 480 | 540 | 576 | 720 | 864 | 900 | 960 | 1080 | 1200 | 1440 | 1600 | 2160 | 2880 | 4320 | 8640)
        && number % 100 != 0
}

fn compact_follows(rest: &[String]) -> bool {
    let upper = |at: usize| rest.get(at).map(|token| normalize_alphanumeric_upper(token));
    let source = |token: Option<String>| {
        token.is_some_and(|t| matches!(t.as_str(), "HDTV" | "PDTV" | "SDTV" | "DSR" | "TVRIP" | "WEB" | "WEBDL" | "WEBRIP"))
    };
    let resolution = upper(0).is_some_and(|t| t.strip_suffix('P').is_some_and(|d| d.len() >= 3 && d.bytes().all(|b| b.is_ascii_digit())));
    rest.is_empty() || source(upper(0)) || (resolution && source(upper(1)))
}

fn compact_episode_at(index: usize, tokens: &[String]) -> bool {
    index > 0
        && compact_number(&normalize_alphanumeric_upper(&tokens[index]))
        && compact_follows(&tokens[index + 1..])
        && (parse_context_year(&tokens[index - 1]).is_some() || tokens[index - 1].parse::<u32>().is_err())
        && !tokens.iter().any(|token| looks_like_standard_episode_token(token))
}

/// fork (P34e): ` - Series 1 (1970)`: a later `Series N` names a season.
fn series_season_at(index: usize, tokens: &[String]) -> bool {
    let upper = |at: usize| tokens.get(at).map(|token| normalize_alphanumeric_upper(token));
    index > 0
        && upper(index).as_deref() == Some("SERIES")
        && upper(index + 1).is_some_and(|n| looks_like_small_season_number(&n))
        && upper(index + 2)
            .is_some_and(|t| parse_context_year(&t).is_some() || looks_like_release_provenance_normalized(&t))
}

/// fork (P34c): `5of9` and Dutch `Se.3 afl.3` both open
/// the episode identity, so they end the title and make the name a series.
fn chain_marker(normalized: &str, tokens: &[String]) -> bool {
    let small = |token: Option<&String>| {
        token.is_some_and(|token| looks_like_small_season_number(&normalize_alphanumeric_upper(token)))
    };
    let upper = |token: Option<&String>| token.map(|token| normalize_alphanumeric_upper(token));
    if let Some((n, m)) = normalized.split_once("OF") {
        let number = |value: &str| value.parse::<u32>().ok().filter(|_| value.len() <= 2);
        return number(n).zip(number(m)).is_some_and(|(n, m)| 1 <= n && n <= m);
    }
    let Some(index) = tokens.iter().position(|token| normalize_alphanumeric_upper(token) == normalized) else {
        return false;
    };
    let (next, after) = (tokens.get(index + 1), tokens.get(index + 2));
    // fork (P34e): `S 01 720p` names a season.
    let metadata = |token: Option<&String>| {
        upper(token).is_some_and(|t| parse_context_year(&t).is_some() || looks_like_release_provenance_normalized(&t))
    };
    if (normalized == "S" && small(next) && metadata(after))
    {
        return true;
    }
    (normalized == "SE" && small(next) && upper(after).as_deref() == Some("AFL") && small(tokens.get(index + 3)))
}

/// The tokens after the first token equal to `normalized`.
fn tokens_after<'a>(normalized: &str, tokens: &'a [String]) -> Option<&'a [String]> {
    let index = tokens
        .iter()
        .position(|candidate| normalize_alphanumeric_upper(candidate) == normalized)?;
    Some(&tokens[index + 1..])
}

fn season_before_episode(normalized: &str, tokens: &[String]) -> bool {
    normalized.strip_prefix('S').is_some_and(|digits| {
        (1..=4).contains(&digits.len()) && digits.chars().all(|ch| ch.is_ascii_digit())
    }) && tokens_after(normalized, tokens)
        .and_then(<[String]>::first)
        .is_some_and(|next| {
            let next = normalize_alphanumeric_upper(next);
            next.strip_prefix("EP")
                .or_else(|| next.strip_prefix('E'))
                .is_some_and(|n| !n.is_empty() && n.chars().all(|ch| ch.is_ascii_digit()))
        })
}

/// fork (P34b2): a bare `E05` is an episode, unless the name carries an
/// `SxxEyy` of its own (`E60 - S01E05 - Title`: `E60` is the title).
fn bare_episode_token(normalized: &str, tokens: &[String]) -> bool {
    normalized.strip_prefix('E').is_some_and(|n| {
        (1..=3).contains(&n.len()) && n.chars().all(|ch| ch.is_ascii_digit())
    }) && !tokens.iter().any(|token| strict_identity_prefix(token) && !token.to_ascii_uppercase().starts_with('E'))
}

fn temporada_marker(tokens: &[String]) -> bool {
    tokens.windows(2).any(|pair| {
        normalize_alphanumeric_upper(&pair[0]) == "TEMPORADA"
            && looks_like_small_season_number(&normalize_alphanumeric_upper(&pair[1]))
    })
}

fn complete_before_season(normalized: &str, tokens: &[String]) -> bool {
    normalized == "COMPLETE"
        && tokens_after(normalized, tokens)
            .and_then(<[String]>::first)
            .is_some_and(|next| {
                let next = normalize_alphanumeric_upper(next);
                matches!(next.as_str(), "SERIES" | "SEASON") || looks_like_bare_season_token(&next)
            })
}

fn looks_like_release_provenance_normalized(normalized: &str) -> bool {
    matches!(
        normalized,
        "WEB"
            | "WEBDL"
            | "WEBRIP"
            | "BLURAY"
            | "BDRIP"
            | "BRRIP"
            | "HDTV"
            | "CAM"
            | "HQCAM"
            | "TS"
            | "TC"
            | "TELESYNC"
            | "TELECINE"
            | "DVDSCR"
            | "WORKPRINT"
            | "DVDRIP"
            | "NF"
            | "AMZN"
            | "DSNP"
            | "CR"
            | "HULU"
            | "MAX"
            | "HEVC"
            | "AVC"
            | "X265"
            | "X264"
            | "H265"
            | "H264"
            | "AAC"
            | "DDP"
            | "DTS"
            | "DTSX"
            | "DTSHD"
            | "DTSMA"
            | "TRUEHD"
            | "FLAC"
            | "DUAL"
            | "DUALAUDIO"
            | "AUDIO"
            | "DUB"
            | "DUBS"
            | "DUBBED"
            | "SUB"
            | "SUBS"
            | "MULTI"
            | "MULTIAUDIO"
            | "MULTISUB"
            | "MULTISUBS"
            | "EN"
            | "ENG"
            | "ENGLISH"
            | "JP"
            | "JPN"
            | "JAPANESE"
            | "HDR"
            | "HDR10"
            | "HDR10PLUS"
            | "HDR10P"
            | "HLG"
            | "DV"
            | "DOVI"
            | "ATMOS"
            | "PROPER"
            | "REPACK"
            | "REMUX"
            | "UNCENSORED"
    ) || normalized.contains("2160")
        || normalized.contains("1080")
        || normalized.contains("720")
        || normalized.contains("480")
}

fn looks_like_season_pack_release(tokens: &[String]) -> bool {
    let normalized = tokens
        .iter()
        .map(|token| {
            token
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_uppercase()
        })
        .collect::<Vec<_>>();
    (normalized
        .iter()
        .any(|token| looks_like_bare_season_token(token))
        && normalized
            .iter()
            .any(|token| matches!(token.as_str(), "COMPLETE" | "SEASON" | "PACK" | "BATCH")))
        || spelled_season_marker("SEASON", tokens)
}

/// A spelled-out season marker — `SEASON` followed within a short window by a
/// small integer — is a title boundary just as `Sxx` is:
/// `[Group] Quiet Meridian - Season 1 - The Arc Name ...` must not read its
/// neutral title as "Quiet Meridian Season 1 The Arc Name". The numeric
/// successor is mandatory, which is what keeps a title that merely contains the
/// word ("Season of the ...") intact. `PART` deliberately does not participate:
/// "Part 2" is identity-bearing in sequel and multi-part titles, and bounding
/// there would strip it from the neutral title. The window leaves room for one
/// intervening separator artifact.
fn spelled_season_marker(normalized: &str, tokens: &[String]) -> bool {
    const SUCCESSOR_WINDOW: usize = 2;
    if normalized != "SEASON" {
        return false;
    }
    tokens.iter().enumerate().any(|(index, candidate)| {
        normalize_alphanumeric_upper(candidate) == normalized
            && tokens
                .iter()
                .skip(index + 1)
                .take(SUCCESSOR_WINDOW)
                .any(|next| looks_like_small_season_number(&normalize_alphanumeric_upper(next)))
    })
}

fn looks_like_small_season_number(normalized: &str) -> bool {
    matches!(normalized.len(), 1 | 2) && normalized.chars().all(|ch| ch.is_ascii_digit())
}

fn looks_like_season_pack_marker(token: &str, tokens: &[String]) -> bool {
    if !looks_like_bare_season_token(token) {
        return false;
    }
    looks_like_season_pack_release(tokens)
        || tokens.iter().any(|candidate| {
            candidate
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .collect::<String>()
                .eq_ignore_ascii_case("COMPLETE")
        })
        || bare_season_followed_by_release_metadata(token, tokens)
}

/// A bare `Sxx` trailed within a short window by unmistakable release metadata
/// (`MULTI`, a resolution, a source, a codec, a year) is a season-pack marker
/// and therefore a title boundary — `Just.Beyond.S01.iTALiAN.MULTi.1080p...`
/// must not read its neutral title as "Just Beyond S01 iTALiAN" (issue #170).
/// An isolated `Sxx` with no such trailer keeps the stricter companion-keyword
/// requirement: a title's own token that merely looks like a season must not
/// truncate the title.
fn bare_season_followed_by_release_metadata(token: &str, tokens: &[String]) -> bool {
    const METADATA_WINDOW: usize = 3;
    let target = normalize_alphanumeric_upper(token);
    tokens.iter().enumerate().any(|(index, candidate)| {
        normalize_alphanumeric_upper(candidate) == target
            && (tokens.get(index + 1).is_some_and(|next| {
                // fork (P34f, P34e): `S02.Special`/`S03.Extras` end the title.
                matches!(normalize_alphanumeric_upper(next).as_str(), "BD" | "JPBD" | "SPECIAL" | "EXTRAS" | "SUBPACK")
            }) || tokens
                .iter()
                .skip(index + 1)
                .take(METADATA_WINDOW)
                .any(|next| {
                    let normalized = normalize_alphanumeric_upper(next);
                    looks_like_release_provenance_normalized(&normalized)
                        || parse_context_year(&normalized).is_some()
                }))
    })
}

fn normalize_alphanumeric_upper(token: &str) -> String {
    token
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase()
}

fn looks_like_bare_season_token(token: &str) -> bool {
    let Some(rest) = token.strip_prefix('S') else {
        return false;
    };
    !rest.is_empty()
        && rest.len() <= 3
        && rest.chars().all(|ch| ch.is_ascii_digit())
        && !token.contains('E')
}

fn looks_like_standard_episode_token(token: &str) -> bool {
    let normalized = token.to_ascii_uppercase();
    normalized.contains('E')
        && normalized.starts_with('S')
        && normalized.chars().any(|ch| ch.is_ascii_digit())
        || normalized
            .strip_prefix("EPISODE")
            .or_else(|| normalized.strip_prefix("EP"))
            .is_some_and(|episode| {
                !episode.is_empty() && episode.chars().all(|character| character.is_ascii_digit())
            })
        || normalized.split_once('X').is_some_and(|(left, right)| {
            !left.is_empty()
                && !right.is_empty()
                && left.chars().all(|ch| ch.is_ascii_digit())
                && right.chars().all(|ch| ch.is_ascii_digit())
        })
}

fn looks_like_daily_token(token: &str) -> bool {
    let normalized = token.trim();
    matches!(normalized.len(), 8 | 10)
        && normalized
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-'))
        && normalized.chars().filter(|ch| ch.is_ascii_digit()).count() >= 8
}

fn looks_like_daily_token_sequence(tokens: &[String]) -> bool {
    tokens.windows(3).any(|window| {
        let [year, month, day] = window else {
            return false;
        };
        let Some(year) = parse_context_year(year) else {
            return false;
        };
        let Some(month) = month.parse::<u32>().ok() else {
            return false;
        };
        let Some(day) = day.parse::<u32>().ok() else {
            return false;
        };
        (1900..=2099).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day)
    })
}

fn looks_like_absolute_episode_token(token: &str) -> bool {
    let normalized = token.to_ascii_uppercase();
    if let Some((number, version)) = normalized.split_once('V')
        && !number.is_empty()
        && !version.is_empty()
        && number.chars().all(|ch| ch.is_ascii_digit())
        && version.chars().all(|ch| ch.is_ascii_digit())
    {
        return true;
    }
    normalized
        .split_once('-')
        .map(|(left, right)| {
            !left.is_empty()
                && !right.is_empty()
                && left.chars().all(|ch| ch.is_ascii_digit())
                && right.chars().all(|ch| ch.is_ascii_digit())
        })
        .unwrap_or_else(|| {
            (2..=4).contains(&normalized.len()) && normalized.chars().all(|ch| ch.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title(raw: &str) -> String {
        let analysis = neutral_release_analysis(raw);
        let candidate = analysis.best_candidate().expect(raw);
        candidate
            .title_segments
            .iter()
            .find(|s| s.kind == scryer_release_parser::TitleSegmentKind::ObservedPrimary)
            .map(|s| s.raw.clone())
            .unwrap_or_default()
    }

    // fork (P35 B): a number before an episode, season or year marker is a
    // title word (`30 Series S01E01`, `24-7 Series`); a year never is.
    #[test]
    fn numeric_title_words_before_identity() {
        for (raw, expected) in [
            ("30 Series - S01E01 - Pilot.avi", "30 Series"),
            ("666 Series Title - S01E01", "666 Series Title"),
            (
                "10.Lines.You.Know.About.Code.S02E04.Prohibition.HDTV.XviD-AFG",
                "10 Lines You Know About Code",
            ),
            ("30.Series.Season.04.HDTV.XviD-DIMENSION", "30 Series"),
            ("Warehouse.13.S01E01", "Warehouse 13"),
            ("Series 104 - S01E07 The Developers [SDTV]", "Series 104"),
            ("Series.Title.2010.S01E01", "Series Title"),
            (
                "Movie.Klasse.von.1999.1990.German.720p.HDTV.x264-NORETAiL",
                "Movie Klasse von 1999",
            ),
            ("(1995) Movie Name", "Movie Name"),
        ] {
            assert_eq!(title(raw), expected, "{raw}");
        }
        let parsed = crate::parse_release_metadata("Movie Name (1897) [DVD].mp4");
        assert_eq!(parsed.year, Some(1897));
        for raw in ["1883.S01E01.1080p.WEB-DL", "1899.S01E01"] {
            assert_eq!(title(raw), &raw[..4], "{raw}");
        }
    }

    // fork (P34a): a name that opens with its identity has no title, and the
    // numbers are read as the standard identity, never as absolutes.
    #[test]
    fn untargeted_parse_reads_leading_identity() {
        let identity = |raw: &str| {
            let parsed = crate::parse_release_metadata(raw);
            let episode = parsed.episode.clone().unwrap_or_else(|| panic!("{raw}: no episode"));
            assert!(episode.absolute_episode_numbers.is_empty(), "{raw}: absolute");
            (parsed.normalized_title, episode.season, episode.episode_numbers)
        };
        for (raw, season, episodes) in [
            ("S03E01.S03E02.720p.HDTV.X264-DIMENSION", 3, vec![1, 2]),
            ("S02E03-04-05.720p.BluRay-FUTV", 2, vec![3, 4, 5]),
            ("S01E01-E03 - Episode Title.HDTV-720p", 1, vec![1, 2, 3]),
            ("S02E10 6-50 to SLC [SDTV]", 2, vec![10]),
            ("6x13 - The Series Show 100th Episode Special", 6, vec![13]),
            ("S06E04,Marco.Polo.1080p.WEB-DL.DD5.1.H.264", 6, vec![4]),
            ("S06E19.Grill.Interrupted.1080p.WEB-DL.DD5.1.H.264", 6, vec![19]),
        ] {
            let (title, got_season, got_episodes) = identity(raw);
            assert_eq!(title, "", "{raw}");
            assert_eq!(got_season, Some(season), "{raw}");
            assert_eq!(got_episodes, episodes, "{raw}");
        }
        let daily = crate::parse_release_metadata("2018-11-14.1080.all.mp4");
        let episode = daily.episode.expect("daily");
        assert_eq!(episode.air_date.map(|d| d.to_string()).as_deref(), Some("2018-11-14"));
        assert!(episode.absolute_episode_numbers.is_empty());
        for (raw, title) in [
            ("Se7en.1995.1080p.BluRay.x264-GRP", "SE7EN"),
            ("Sense8.S01E01.1080p.WEB-DL", "SENSE8"),
            ("1234 S03E14", "1234"),
        ] {
            assert_eq!(crate::parse_release_metadata(raw).normalized_title.to_ascii_uppercase(), title, "{raw}");
        }
        assert_eq!(identity("S02E04E05.720p.HDTV").2, vec![4, 5]);
        assert_eq!(identity("2x04x05.720p.HDTV").2, vec![4, 5]);
    }

    // fork (P34b): season and episode markers end the untargeted title and
    // make the name a series; movies are untouched.
    #[test]
    fn untargeted_boundary_and_facet_read_series_markers() {
        let parsed = |raw: &str| crate::parse_release_metadata(raw);
        for (raw, title) in [
            ("Series Title S42 E01 720p HDTV", "SERIES TITLE"),
            ("Series Title Temporada 2 Cap 1901 720p", "SERIES TITLE"),
            ("Series Title Complete Series 720p BluRay", "SERIES TITLE"),
            ("Series Title E05 720p HDTV", "SERIES TITLE"),
            ("Series.Title.S02.2010.1080p.BluRay.x264-GRP", "SERIES TITLE"),
        ] {
            let p = parsed(raw);
            assert_eq!(p.normalized_title.to_ascii_uppercase(), title, "{raw}");
            assert!(p.episode.is_some(), "{raw}: no series identity");
        }
        for raw in ["Movie.2024.1080p.BluRay.x264-GRP", "Dune.Part.Two.2024.1080p.WEB-DL"] {
            assert!(parsed(raw).episode.is_none(), "{raw}");
        }
        assert_eq!(parsed("Agents.of.S.H.I.E.L.D.S01E01.720p").normalized_title.to_ascii_uppercase(), "AGENTS OF S H I E L D");
    }

    // fork (P34d, P34f): `S05.Special` is a season special, never a pack;
    // `S07.Vol.1` is part 1 of season 7; a title's own `Special` is title.
    #[test]
    fn season_specials_and_volumes_are_not_full_packs() {
        let ep = |raw: &str| crate::parse_release_metadata(raw).episode.expect(raw);
        let special = ep("Show.S05.Special.HDTV.x264-2HD");
        assert_eq!((special.season, special.full_season), (Some(5), false));
        assert!(special.special_kind.is_some());
        assert!(ep("Show.S11E00.A.Christmas.Carol.Special.720p.HDTV-GRP").special_kind.is_some());
        let volume = ep("Show.S07.Vol.1.1080p.NF.WEBRip.DD5.1.x264-NTb");
        assert_eq!((volume.season, volume.full_season, volume.is_partial_season), (Some(7), false, true));
        let guard = ep("Big.Special.Show.S05.720p.HDTV.x264-GRP");
        assert!(guard.special_kind.is_none() && guard.full_season);
    }

    // fork (P34c): chains and markers.
    #[test]
    fn episode_chains_and_markers() {
        let ep = |raw: &str| crate::parse_release_metadata(raw).episode.expect(raw);
        let list = |raw: &str| {
            let e = ep(raw);
            (e.season, e.episode_numbers)
        };
        assert_eq!(list("Series's Sonarr - 8x01_02 - Free Falling"), (Some(8), vec![1, 2]));
        assert_eq!(list("8x01_02 - Free Falling"), (Some(8), vec![1, 2]));
        assert_eq!(list("Show.S01E05_10bit.720p"), (Some(1), vec![5]));
        assert_eq!(
            list("Series falls - Temporada 1 [HDTV][Cap.111_120]"),
            (Some(1), (11..=20).collect())
        );
        assert_eq!(list("Series falls - Temporada 1 [HDTV][Cap.101]"), (Some(1), vec![1]));
        let p = crate::parse_release_metadata("John.Smith.The.Series.Title.5of9.The.Universe.Of.Development.1990.DVDRip.x264-HANDJOB");
        assert_eq!(p.normalized_title.to_ascii_uppercase(), "JOHN SMITH THE SERIES TITLE");
        let e = p.episode.expect("5of9");
        assert_eq!((e.season, e.episode_numbers), (Some(1), vec![5]));
        let e = ep("Series.Title.S06E01b.Fade.Out.Fade.in.Part.2.1080p.DSNP.WEB-DL.AAC2.0.H.264-FLUX");
        assert_eq!((e.season, e.episode_numbers, e.is_split_episode), (Some(6), vec![1], true));
        assert!(!ep("Series.Title.S06E01.1080p.WEB-DL").is_split_episode);
        // Hostile non-ASCII suffixes must not panic.
        for raw in ["Show.S06E01\u{c9}.720p", "S06E01\u{fc}", "Show.5of\u{fc}.720p"] {
            let _ = crate::parse_release_metadata(raw);
        }
        assert_eq!(list("Series T Se.3 afl.3"), (Some(3), vec![3]));
        assert_eq!(list("Series T Se.3 afl.3 en 4"), (Some(3), vec![3, 4]));
        assert_eq!(title("Series T Se.3 afl.3 en 4"), "Series T");
    }

    // fork (P34e): pack shapes.
    #[test]
    fn other_season_pack_shapes() {
        let ep = |raw: &str| crate::parse_release_metadata(raw).episode.expect(raw);
        for (raw, season, name) in [
            ("Series S 01 720p WEB DL DD 5 1 h264 TjHD", 1, "SERIES"),
            ("Series Title - Series 1 (1970) DivX", 1, "SERIES TITLE"),
            ("The.Series.S03.Extras.01.Deleted.Scenes.720p", 3, "THE SERIES"),
        ] {
            let e = ep(raw);
            assert_eq!((e.season, e.full_season), (Some(season), true), "{raw}");
            assert_eq!(crate::parse_release_metadata(raw).normalized_title.to_ascii_uppercase(), name, "{raw}");
        }
        assert!(ep("The.Series.S03.Extras.01.Deleted.Scenes.720p").is_season_extra);
        assert!(crate::parse_release_metadata("Series 7 The Contenders 2001 1080p").episode.is_none());
    }

    // fork (P34h): a bare compact number after the title is season and episode.
    #[test]
    fn compact_episode_number() {
        let list = |raw: &str| {
            let p = crate::parse_release_metadata(raw);
            let e = p.episode.unwrap_or_else(|| panic!("{raw}"));
            (p.normalized_title.to_ascii_uppercase(), e.season, e.episode_numbers)
        };
        assert_eq!(list("Series.and.a.Title.103.720p.HDTV.X264-DIMENSION"), ("SERIES AND A TITLE".into(), Some(1), vec![3]));
        assert_eq!(list("Series.and.a.Title.1013.720p.HDTV.X264-DIMENSION"), ("SERIES AND A TITLE".into(), Some(10), vec![13]));
        assert_eq!(list("Series.Title.525"), ("SERIES TITLE".into(), Some(5), vec![25]));
        assert_eq!(list("series.2009.416.hdtv-lol"), ("SERIES 2009".into(), Some(4), vec![16]));
        for raw in ["Room.237.1080p.BluRay.x264-GRP", "Movie.2010.1080p.BluRay.x264-GRP", "Movie.1408.2007.1080p.BluRay.x264-GRP"] {
            assert!(crate::parse_release_metadata(raw).episode.is_none(), "{raw}");
        }
    }
}

