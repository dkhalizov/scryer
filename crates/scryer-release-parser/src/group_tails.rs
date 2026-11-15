//! fork (P26): release-group rules the arrs apply to the raw name as text
//! (Sonarr `Parser.cs` ParseReleaseGroup, Radarr `ReleaseGroupParser.cs`).

use crate::lex::Token;

/// Groups whose releases end with `Group)` or `Group]` (arr ExceptionReleaseGroupRegex).
const SONARR_EXCEPTIONS: &[&str] = &[
    "Silence",
    "afm72",
    "Panda",
    "Ghost",
    "MONOLITH",
    "Tigole",
    "Joy",
    "ImE",
    "UTR",
    "t3nzin",
    "Anime Time",
    "Project Angel",
    "Hakata Ramen",
    "HONE",
    "Vyndros",
    "SEV",
    "Garshasp",
    "Kappa",
    "Natty",
    "RCVR",
    "SAMPA",
    "YOGI",
    "r00t",
    "EDGE2020",
    "RZeroX",
];

const RADARR_EXCEPTIONS: &[&str] = &[
    "Silence",
    "afm72",
    "Panda",
    "Ghost",
    "MONOLITH",
    "Tigole",
    "Joy",
    "ImE",
    "UTR",
    "t3nzin",
    "Anime Time",
    "Project Angel",
    "Hakata Ramen",
    "HONE",
    "GiLG",
    "Vyndros",
    "SEV",
    "Garshasp",
    "Kappa",
    "Natty",
    "RCVR",
    "SAMPA",
    "YOGI",
    "r00t",
    "EDGE2020",
    "RZeroX",
    "FreetheFish",
    "Anna",
    "Bandi",
    "Qman",
    "theincognito",
    "HDO",
    "DusIctv",
    "DHD",
    "CtrlHD",
    "-ZR-",
    "ADC",
    "XZVN",
    "RH",
    "Kametsu",
    "Celdra",
];

/// Names matched anywhere (arr ExceptionReleaseGroupRegexExact). In patterns,
/// `~` is one of `_ . space` and `?` is any one character.
const SONARR_EXACT: &[&str] = &[
    "D-Z0N3",
    "Fight-BB",
    "VARYG",
    "E.N.D",
    "KRaLiMaRKo",
    "BluDragon",
    "DarQ",
    "KCRT",
    "BEN~THE~MEN",
];

const RADARR_EXACT: &[&str] = &[
    "KRaLiMaRKo",
    "E.N.D",
    "D-Z0N3",
    "Koten_Gars",
    "BluDragon",
    "ZØNEHD",
    "HQMUX",
    "VARYG",
    "YIFY",
    "YTS?MX",
    "YTS?LT",
    "YTS?AG",
    "YTS",
    "TMd",
    "Eml HDTeam",
    "LMain",
    "DarQ",
    "BEN THE MEN",
    "TAoE",
    "QxR",
    "126811",
];

const MEDIA_EXTENSIONS: &[&str] = &[".mkv", ".mp4", ".avi", ".m4v", ".webm"];

/// The group an arr list names in `raw`. Radarr checks its exact list first
/// (`radarr`), Sonarr its exception list first; each returns the last match.
// ponytail: scans the whole name; Sonarr first cuts everything up to SxxEyy.
pub(crate) fn arr_listed_group(raw: &str, radarr: bool) -> Option<&str> {
    let text = without_media_extension(crate::strip_website_prefix(raw.trim()));
    let (exceptions, exact) = if radarr {
        (RADARR_EXCEPTIONS, RADARR_EXACT)
    } else {
        (SONARR_EXCEPTIONS, SONARR_EXACT)
    };
    let exception = |text| last_match(text, exceptions, exception_bounds);
    let exact = |text| {
        last_match(text, exact, |text, start, end| {
            exact_bounds(text, start, end, radarr)
        })
    };
    if radarr {
        exact(text).or_else(|| exception(text))
    } else {
        exception(text).or_else(|| exact(text))
    }
}

/// `Group` in a name ending `-._ [Group]` (arr ReleaseGroupRegex's last branch),
/// after a media extension and a torrent-site tag are cut. CRCs, digits and
/// `[Sxx]`/`[Exx]` are never a group.
pub(crate) fn trailing_bracket_word(raw: &str) -> Option<&str> {
    let mut text = without_media_extension(raw.trim());
    for tag in ["[ettv]", "[rartv]", "[rarbg]", "[cttv]", "[publichd]"] {
        if let Some(cut) = strip_suffix_ignore_case(text, tag) {
            text = cut;
            break;
        }
    }
    let inner = text.strip_suffix(']')?;
    let open = inner.rfind('[')?;
    let word = &inner[open + 1..];
    let before = inner[..open].chars().next_back()?;
    let season_or_episode = word.len() > 1
        && matches!(word.as_bytes()[0], b's' | b'S' | b'e' | b'E')
        && word[1..].bytes().all(|byte| byte.is_ascii_digit());
    let crc = word.len() == 8 && word.bytes().all(|byte| byte.is_ascii_hexdigit());
    (matches!(before, '-' | '.' | '_' | ' ')
        && !word.is_empty()
        && word.bytes().all(|byte| byte.is_ascii_alphanumeric())
        && !word.bytes().all(|byte| byte.is_ascii_digit())
        && !season_or_episode
        && !crc)
        .then_some(word)
}

/// Sonarr's `(?!.+?(?:480p|…|2160p))`: a group is never followed by a resolution.
pub(crate) fn followed_by_resolution(tokens: &[Token], end_token: usize) -> bool {
    tokens.iter().skip(end_token).any(|token| {
        ["480P", "576P", "720P", "1080P", "2160P"]
            .iter()
            .any(|resolution| token.normalized.contains(resolution))
    })
}

fn without_media_extension(text: &str) -> &str {
    MEDIA_EXTENSIONS
        .iter()
        .find_map(|extension| strip_suffix_ignore_case(text, extension))
        .unwrap_or(text)
}

fn strip_suffix_ignore_case<'a>(text: &'a str, suffix: &str) -> Option<&'a str> {
    let cut = text.len().checked_sub(suffix.len())?;
    (text.is_char_boundary(cut) && text[cut..].eq_ignore_ascii_case(suffix)).then(|| &text[..cut])
}

fn last_match<'a>(
    text: &'a str,
    patterns: &[&str],
    bounds: impl Fn(&str, usize, usize) -> bool,
) -> Option<&'a str> {
    // Left-to-right non-overlapping matches, first alternative wins (.NET Matches).
    let mut found = None;
    let mut resume = 0;
    for (start, _) in text.char_indices() {
        if start < resume {
            continue;
        }
        if let Some(end) = patterns.iter().find_map(|pattern| {
            match_at(text, start, pattern).filter(|end| bounds(text, start, *end))
        }) {
            found = Some(&text[start..end]);
            resume = end;
        }
    }
    found
}

fn match_at(text: &str, start: usize, pattern: &str) -> Option<usize> {
    let mut chars = text[start..].chars();
    let mut end = start;
    for wanted in pattern.chars() {
        let character = chars.next()?;
        let matched = match wanted {
            '~' => matches!(character, '_' | '.' | ' '),
            '?' => true,
            _ => character.to_lowercase().eq(wanted.to_lowercase()),
        };
        if !matched {
            return None;
        }
        end += character.len_utf8();
    }
    Some(end)
}

/// `(?<=[._ \[])Group(?=\]|\))`
fn exception_bounds(text: &str, start: usize, end: usize) -> bool {
    text[..start]
        .chars()
        .next_back()
        .is_some_and(|before| matches!(before, '.' | '_' | ' ' | '['))
        && text[end..]
            .chars()
            .next()
            .is_some_and(|after| matches!(after, ']' | ')'))
}

/// `Group\b` (Sonarr) or `\bGroup\b` (Radarr), with .NET word characters.
fn exact_bounds(text: &str, start: usize, end: usize, leading: bool) -> bool {
    let word = |character: char| character.is_alphanumeric() || character == '_';
    !text[end..].chars().next().is_some_and(word)
        && !(leading && text[..start].chars().next_back().is_some_and(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_follow_each_arr_order() {
        let name = "Movie Title (2016) (1080p AMZN WEB-DL x265 HEVC 10bit EAC3 5 1 RZeroX) QxR";
        assert_eq!(arr_listed_group(name, false), Some("RZeroX"));
        assert_eq!(arr_listed_group(name, true), Some("QxR"));
        assert_eq!(
            arr_listed_group("Movie [YIFY] [YTS.LT].mkv", true),
            Some("YTS.LT")
        );
        assert_eq!(arr_listed_group("Movie.2022-yts.bz", true), Some("yts"));
        assert_eq!(
            arr_listed_group("A.MKV-BEN.THE_MEN-xpost", false),
            Some("BEN.THE_MEN")
        );
        assert_eq!(arr_listed_group("A.Title.2020-xDarQ", true), None);
        assert_eq!(arr_listed_group("A.Title.2020-xDarQ", false), Some("DarQ"));
        // Multi-byte text never splits a character.
        assert_eq!(
            arr_listed_group("İstanbul (2020) (x265 Joy)", false),
            Some("Joy")
        );
    }

    #[test]
    fn trailing_bracket_word_refuses_crcs_and_numbers() {
        assert_eq!(trailing_bracket_word("A 10x11 - B [rl].avi"), Some("rl"));
        assert_eq!(trailing_bracket_word("A-GRP [TGx][rarbg]"), Some("TGx"));
        assert_eq!(trailing_bracket_word("A [6AFFEF6B]"), None);
        assert_eq!(trailing_bracket_word("A [2160]"), None);
        assert_eq!(trailing_bracket_word("A [S01]"), None);
        assert_eq!(trailing_bracket_word("A[x]"), None);
    }
}
