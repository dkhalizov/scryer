//! fork (P33): the arrs' quality vocabulary and defaults (Sonarr/Radarr
//! `QualityParser.ParseQualityName`) as one post-pass over the in-scope
//! technical tokens. The beam is never edited: this only fills or overrides
//! `quality`, `source`, `is_remux` and `is_raw_hd`, and marks each value the
//! arrs would not have read from the name with a `quality_default:*` hint.

use crate::enrichment::{joined, tok};
use crate::lex::{SeparatorKind, Token};
use crate::model::{ParseFamily, ParsedReleaseMetadata, ReleaseParseCandidate, ReleaseSource};
use crate::parse::{protected_title_end, technical_recovery_token, token_source};

/// The arrs leave the source detection source `Unknown` (a default).
pub(crate) const DEFAULT_SOURCE_HINT: &str = "quality_default:source";
/// The arrs leave the resolution detection source `Unknown` (a default).
pub const DEFAULT_RESOLUTION_HINT: &str = "quality_default:resolution";

/// Which HDTV spelling gave the source; the arrs' resolution default differs.
#[derive(Clone, Copy, PartialEq)]
enum Tv {
    Hdtv,
    /// PDTV, DSR, TVRip, SDTV.
    Pdtv,
    /// `HD[-_. ]TV` (OtherSourceRegex).
    HdTv,
    /// `SD[-_. ]TV` (OtherSourceRegex).
    SdTv,
}

/// `source_seen`: an in-scope token carried a source for independent
/// recovery (a conflict included), so a sourceless name gets no default.
pub(crate) fn apply(
    tokens: &[Token],
    candidate: &ReleaseParseCandidate,
    source_seen: bool,
    p: &mut ParsedReleaseMetadata,
) {
    use ReleaseSource::{BluRay, BrDisk, Dvd, Hdtv, WebDl, WebRip};
    let end = protected_title_end(candidate);
    let scope: Vec<usize> = (0..tokens.len())
        .filter(|&i| technical_recovery_token(candidate, i, &tokens[i], end))
        .collect();
    let any = |f: &dyn Fn(usize) -> bool| scope.iter().any(|&i| f(i));
    let word = |words: &[&str]| any(&|i| words.contains(&tok(tokens, i)));
    // A source spelling read as the group (`[SDTV]`, `-xvidvd`) is still a
    // source to the arrs, whose regexes see the whole name.
    let group = candidate.zones.release_group_span;
    let in_group = |i: usize| group.is_some_and(|g| (g.start_token..g.end_token).contains(&i));
    let spelled = |words: &[&str]| {
        word(words) || (0..tokens.len()).any(|i| in_group(i) && words.contains(&tok(tokens, i)))
    };
    let pair = |a: &str, b: &str| {
        (0..tokens.len()).any(|i| {
            (scope.contains(&i) || in_group(i))
                && tok(tokens, i) == a
                && joined(tokens, i)
                && tok(tokens, i + 1) == b
        })
    };

    // P33e: Raw-HD first, as in ParseQualityName. MPEG2Regex is
    // case-sensitive (raw text); Radarr skips it for a disc.
    let raw_hd = (0..tokens.len()).any(|i| {
        tok(tokens, i) == "RAWHD"
            || (tok(tokens, i) == "RAW" && joined(tokens, i) && tok(tokens, i + 1) == "HD")
    });
    let mpeg2 = (0..tokens.len()).any(|i| {
        tokens[i].raw == "MPEG2"
            || (tokens[i].raw == "MPEG"
                && tokens.get(i + 1).is_some_and(|n| n.raw == "2")
                && (joined(tokens, i) || tokens[i + 1].span.start == tokens[i].span.end))
    });
    if !p.is_bd_disk && (raw_hd || (mpeg2 && p.source == Some(Hdtv))) {
        p.is_raw_hd = true;
        p.source = Some(Hdtv);
        p.quality = Some("1080p".into());
        p.is_remux = false;
        return;
    }

    // P33c: the arr resolution tokens; a single class sets the quality, two
    // keep Scryer's conflict reading.
    let mut classes: Vec<u32> = scope
        .iter()
        .filter_map(|&i| arr_resolution(tok(tokens, i)))
        .collect();
    classes.sort_unstable();
    classes.dedup();
    if let [class] = classes[..]
        && p.quality.as_deref().and_then(class_of) != Some(class)
    {
        p.quality = Some(format!("{class}p"));
    }
    if let Some(q) = p.quality.as_deref() {
        let digits: String = q.chars().take_while(char::is_ascii_digit).collect();
        let to = match digits.as_str() {
            "360" | "540" => "480",
            "960" => "720",
            "1440" => "1080",
            _ => "",
        };
        if !to.is_empty() {
            p.quality = Some(format!("{to}{}", &q[digits.len()..]));
        }
    }

    // P33a: WEBMux is a WEBRip; a bare `WEB` outside every arr context and a
    // final `.webm` are no source.
    let mut tv = (p.source == Some(Hdtv)).then_some(Tv::Hdtv);
    if word(&["WEBMUX"]) && matches!(p.source, None | Some(WebDl)) {
        p.source = Some(WebRip);
    } else if p.source == Some(WebDl) && p.streaming_service.is_none() {
        let webs: Vec<usize> = scope
            .iter()
            .copied()
            .filter(|&i| tok(tokens, i).starts_with("WEB") && !tok(tokens, i).starts_with("WEBRI"))
            .collect();
        let weak = |i: usize| {
            (tok(tokens, i) == "WEBM" && i + 1 == tokens.len())
                || (tok(tokens, i) == "WEB" && !web_context(tokens, i))
        };
        if !webs.is_empty() && webs.iter().all(|&i| weak(i)) {
            p.source = None;
        }
    }

    // P33b: source spellings the beam does not read, only for a name whose
    // in-scope tokens carried no source.
    if p.source.is_none() && !source_seen {
        let (source, kind) = if spelled(&["PDTV", "DSR", "TVRIP", "SDTV"]) {
            (Some(Hdtv), Some(Tv::Pdtv))
        } else if pair("HD", "TV") {
            (Some(Hdtv), Some(Tv::HdTv))
        } else if pair("SD", "TV") {
            (Some(Hdtv), Some(Tv::SdTv))
        } else if spelled(&[
            "AMAZONHD",
            "AMAZONSD",
            "ITUNESHD",
            "MAXDOMEHD",
            "NETFLIXHD",
            "NETFLIXUHD",
            "WEBHD",
            "HBOMAXHD",
            "DISNEYHD",
        ]) {
            (Some(WebDl), None)
        } else if spelled(&["BDMUX", "BDLIGHT", "UHDBD", "UHD2BD", "UHDBDRIP", "MBLURAY"])
            || any(&|i| tok(tokens, i) == "BD" && bracketed(&tokens[i]))
        {
            (Some(BluRay), None)
        } else if spelled(&["XVIDVD"]) || any(&|i| dvd_spelling(tokens, i)) {
            (Some(Dvd), None)
        } else if any(&|i| {
            matches!(tok(tokens, i), "PAL" | "NTSC")
                && [i.wrapping_sub(1), i + 1]
                    .into_iter()
                    .any(|j| scope.contains(&j) && pal_partner(tokens, j))
        }) {
            (Some(Dvd), None)
        } else {
            (None, None)
        };
        p.source = source;
        tv = kind;
        if matches!(kind, Some(Tv::HdTv | Tv::SdTv)) && p.quality.is_some() {
            // Sonarr reads `1080p HD-TV` as a sourceless 1080p (HDTV default).
            hint(p, DEFAULT_SOURCE_HINT);
        }
    }

    // P33f: Radarr GermanRemuxRegex, movies only.
    if candidate.family == ParseFamily::Movie
        && p.source == Some(BluRay)
        && !p.is_bd_disk
        && german_remux(tokens)
    {
        p.is_remux = true;
    }

    // P33d: remux survives only on a Bluray or sourceless name at 1080p+.
    if p.is_remux && !matches!(p.source, None | Some(BluRay | BrDisk)) {
        p.is_remux = false;
    }
    if p.is_remux {
        match p.quality.as_deref().and_then(class_of) {
            None if p.source.is_none() => p.is_remux = false,
            None => {
                p.quality = Some("1080p".into());
                hint(p, DEFAULT_RESOLUTION_HINT);
            }
            Some(class) => {
                if class < 1080 {
                    p.is_remux = false;
                }
                if p.source.is_none() {
                    p.source = Some(BluRay);
                    hint(p, DEFAULT_SOURCE_HINT);
                }
            }
        }
    }

    // P33d: the arr defaults. A sourceless default needs a name with no
    // source spelling anywhere (the arrs read the whole name; a source the
    // title swallowed must not turn into HDTV).
    let sourceless = !source_seen
        && !(0..tokens.len()).any(|i| {
            let final_group_bd = i + 1 == tokens.len()
                && tok(tokens, i) == "BD"
                && tokens[i].separator_before == SeparatorKind::Hyphen;
            let extension = i + 1 == tokens.len()
                && tokens[i].separator_before == SeparatorKind::Dot
                && matches!(tok(tokens, i), "TS" | "M2TS" | "WEBM");
            !final_group_bd
                && !extension
                && (token_source(tok(tokens, i)).is_some() || vocabulary(tokens, i))
        });
    match (p.source, p.quality.is_some()) {
        // Two resolution classes are a conflict, not a missing resolution.
        (Some(source), false) if classes.len() < 2 => {
            let bracketed_word = |w: &str| {
                any(&|i| {
                    tokens[i].raw == w
                        && bracketed(&tokens[i])
                        && tokens[i].separator_after == SeparatorKind::CloseBracket
                })
            };
            let height = match source {
                _ if p.is_bd_disk => Some(if source == Dvd { 480 } else { 1080 }),
                BluRay | BrDisk => Some(
                    if word(&["XVID", "DIVX"])
                        || pair("X", "VID")
                        || word(&["BDRIP", "BRRIP", "BDLIGHT", "UHDBDRIP", "HDDVDRIP"])
                    {
                        480
                    } else {
                        720
                    },
                ),
                WebDl => Some(
                    if bracketed_word("WEBDL") || any(&|i| anime_web(tokens, i)) {
                        720
                    } else {
                        480
                    },
                ),
                WebRip | Dvd => Some(480),
                Hdtv => Some(match tv {
                    Some(Tv::Pdtv)
                        if (0..tokens.len()).any(|i| {
                            tok(tokens, i) == "HR"
                                && joined(tokens, i)
                                && tok(tokens, i + 1) == "WS"
                        }) =>
                    {
                        // HighDefPdtvRegex: the arrs read this as a name resolution.
                        p.quality = Some("720p".into());
                        return;
                    }
                    Some(Tv::HdTv) => 720,
                    Some(Tv::Hdtv) if bracketed_word("HDTV") => 720,
                    _ => 480,
                }),
                _ => None,
            };
            if let Some(height) = height {
                p.quality = Some(format!("{height}p"));
                hint(p, DEFAULT_RESOLUTION_HINT);
            }
        }
        (None, true) if sourceless && !p.is_bd_disk => {
            p.source = Some(Hdtv);
            hint(p, DEFAULT_SOURCE_HINT);
        }
        (None, false) if p.is_bd_disk => {
            p.source = Some(BluRay);
            p.quality = Some("1080p".into());
            hint(p, DEFAULT_SOURCE_HINT);
            hint(p, DEFAULT_RESOLUTION_HINT);
        }
        (None, false)
            if sourceless
                && scope
                    .iter()
                    .map(|&i| tok(tokens, i))
                    .find(|t| matches!(*t, "X264" | "H264" | "XVIDHD" | "XVID" | "DIVX"))
                    == Some("X264") =>
        {
            // Sonarr: an x264 name with neither source nor resolution is SDTV.
            p.source = Some(Hdtv);
            p.quality = Some("480p".into());
            hint(p, DEFAULT_SOURCE_HINT);
            hint(p, DEFAULT_RESOLUTION_HINT);
        }
        _ => {}
    }
}

fn hint(p: &mut ParsedReleaseMetadata, h: &str) {
    if !p.parse_hints.iter().any(|x| x == h) {
        p.parse_hints.push(h.to_string());
    }
}

/// A P33b source spelling at token `i`.
fn vocabulary(tokens: &[Token], i: usize) -> bool {
    let pair = |b: &str| joined(tokens, i) && tok(tokens, i + 1) == b;
    matches!(
        tok(tokens, i),
        "PDTV"
            | "DSR"
            | "TVRIP"
            | "SDTV"
            | "AMAZONHD"
            | "AMAZONSD"
            | "ITUNESHD"
            | "MAXDOMEHD"
            | "NETFLIXHD"
            | "NETFLIXUHD"
            | "WEBHD"
            | "HBOMAXHD"
            | "DISNEYHD"
            | "BDMUX"
            | "BDLIGHT"
            | "UHDBD"
            | "UHD2BD"
            | "UHDBDRIP"
            | "MBLURAY"
            | "XVIDVD"
    ) || (matches!(tok(tokens, i), "HD" | "SD") && pair("TV"))
        || dvd_spelling(tokens, i)
}

/// The arr ResolutionRegex class of one token.
fn arr_resolution(token: &str) -> Option<u32> {
    Some(match token {
        "360P" => 360,
        "480P" | "480I" | "640X480" | "848X480" => 480,
        "540P" => 540,
        "576P" => 576,
        "720P" | "1280X720" | "960P" => 720,
        "1080P" | "1920X1080" | "1440P" | "FHD" | "1080I" | "4KTO1080P" => 1080,
        "2160P" | "3840X2160" => 2160,
        _ => return None,
    })
}

/// The arr resolution class of a quality value (`1440p` is 1080).
fn class_of(quality: &str) -> Option<u32> {
    let digits: String = quality.chars().take_while(char::is_ascii_digit).collect();
    match digits.parse().ok()? {
        960 => Some(720),
        1440 => Some(1080),
        height => Some(height),
    }
}

fn bracketed(token: &Token) -> bool {
    matches!(
        token.separator_before,
        SeparatorKind::OpenBracket | SeparatorKind::OpenParen
    ) || matches!(
        token.separator_after,
        SeparatorKind::CloseBracket | SeparatorKind::CloseParen
    )
}

/// Radarr `\d?x?M?DVD-?[R59]`.
fn dvd_spelling(tokens: &[Token], i: usize) -> bool {
    let t = tok(tokens, i);
    let t = t.strip_prefix(|c: char| c.is_ascii_digit()).unwrap_or(t);
    let t = t.strip_prefix('X').unwrap_or(t);
    let t = t.strip_prefix('M').unwrap_or(t);
    match t.strip_prefix("DVD") {
        Some("R" | "5" | "9") => true,
        Some("") => {
            tokens.get(i + 1).is_some_and(|next| {
                next.separator_before == SeparatorKind::Hyphen
                    && next.span.start == tokens[i].span.end + 1
            }) && tok(tokens, i + 1) == "R"
        }
        _ => false,
    }
}

/// A PAL/NTSC neighbour that makes it a DVD tag, not a word.
fn pal_partner(tokens: &[Token], j: usize) -> bool {
    let t = tok(tokens, j);
    matches!(
        t,
        "X264"
            | "H264"
            | "X265"
            | "H265"
            | "XVID"
            | "DIVX"
            | "AVC"
            | "HEVC"
            | "MPEG2"
            | "DVD"
            | "DVDRIP"
    ) || dvd_spelling(tokens, j)
        || arr_resolution(t).is_some()
}

/// Sonarr/Radarr AnimeWebDlRegex: `[WEB]`, `[WEB ` or `(WEB `.
fn anime_web(tokens: &[Token], i: usize) -> bool {
    let t = &tokens[i];
    t.normalized == "WEB"
        && matches!(
            t.separator_before,
            SeparatorKind::OpenBracket | SeparatorKind::OpenParen
        )
        && (matches!(t.separator_after, SeparatorKind::Space | SeparatorKind::Dot)
            || (t.separator_before == SeparatorKind::OpenBracket
                && t.separator_after == SeparatorKind::CloseBracket))
}

/// A bare `WEB` token in one of the arrs' WEB-DL contexts (union of Sonarr
/// and Radarr SourceRegex and AnimeWebDlRegex). `_` counts as a space.
fn web_context(tokens: &[Token], i: usize) -> bool {
    use SeparatorKind::{Dot, Hyphen, Slash, Space, Underscore};
    let t = &tokens[i];
    let next = |k: usize| tok(tokens, i + k);
    let gap = |a: usize| {
        tokens
            .get(a + 1)
            .and_then(|b| b.span.start.checked_sub(tokens[a].span.end))
    };
    let dot_space = |s: SeparatorKind| matches!(s, Dot | Space | Underscore);
    let hds = |s: SeparatorKind| matches!(s, Hyphen | Dot | Space | Underscore);
    let res = |t: &str| {
        t.strip_suffix('P').is_some_and(|d| {
            (3..=4).contains(&d.len()) && d.ends_with('0') && d.bytes().all(|b| b.is_ascii_digit())
        })
    };
    let before = i.checked_sub(1).filter(|&b| gap(b) == Some(1));
    let after = (gap(i) == Some(1)).then_some(i + 1);
    let after_hds = after.is_some_and(|_| hds(t.separator_after));
    let codec_next = match next(1) {
        "X264" | "X265" | "H264" | "H265" | "AVC" | "HEVC" => true,
        "X" | "H" => matches!(next(2), "264" | "265") && gap(i + 1).is_some_and(|g| g <= 1),
        "DD5" | "DDP5" => next(2) == "1" && gap(i + 1) == Some(1),
        _ => false,
    };
    // WEB-DL lexes as WEB + DL.
    (next(1) == "DL" && after.is_some())
        // `[. ]WEB[. ](x264|AVC|HEVC|DD5.1…)`
        || (before.is_some() && dot_space(t.separator_before) && after.is_some() && dot_space(t.separator_after) && codec_next)
        // `[. ]WEB$`, upper case
        || (t.raw == "WEB" && i + 1 == tokens.len() && before.is_some() && dot_space(t.separator_before))
        // `\d{3,4}0p[-. ](Hybrid[-_. ]?)?WEB[-. ]`
        || (after_hds
            && before.is_some_and(|b| {
                hds(t.separator_before)
                    && (res(tok(tokens, b))
                        || (tok(tokens, b) == "HYBRID"
                            && b.checked_sub(1).is_some_and(|r| gap(r) == Some(1) && res(tok(tokens, r)))))
            }))
        // `[-. ]WEB[-. ]\d{3,4}0p`
        || (before.is_some() && hds(t.separator_before) && after_hds && res(next(1)))
        // ` / WEB / ` (a spaced run folds to the next token's separator)
        || (t.separator_before == Slash
            && tokens.get(i + 1).is_some_and(|n| n.separator_before == Slash))
        // `(AMZN|NF|DP)[. -]WEB[. -](?!Rip)`
        || (before.is_some_and(|b| matches!(tok(tokens, b), "AMZN" | "NF" | "DP")) && hds(t.separator_before) && after_hds && !next(1).starts_with("RIP"))
        || anime_web(tokens, i)
}

/// Radarr GermanRemuxRegex: a 4-digit run, later `German`, and after it a
/// disc codec word and `Blu-?ray`.
fn german_remux(tokens: &[Token]) -> bool {
    let four_digits = |t: &str| {
        t.as_bytes()
            .windows(4)
            .any(|w| w.iter().all(u8::is_ascii_digit))
    };
    let Some(german) = (0..tokens.len())
        .find(|&g| tok(tokens, g) == "GERMAN" && (0..g).any(|i| four_digits(tok(tokens, i))))
    else {
        return false;
    };
    let after = german + 1..tokens.len();
    let codec = after.clone().any(|i| {
        let t = tok(tokens, i);
        ["AVC", "HEVC", "VC1", "MVC", "MPEG2"]
            .iter()
            .any(|w| t.starts_with(w))
            || (t == "VC" && joined(tokens, i) && tok(tokens, i + 1).starts_with('1'))
            || (t == "MPEG" && joined(tokens, i) && tok(tokens, i + 1).starts_with('2'))
    });
    let bluray = after.into_iter().any(|i| {
        tok(tokens, i).contains("BLURAY")
            || (tok(tokens, i).ends_with("BLU")
                && tokens.get(i + 1).is_some_and(|n| {
                    n.normalized.starts_with("RAY")
                        && n.separator_before == SeparatorKind::Hyphen
                        && n.span.start == tokens[i].span.end + 1
                }))
    });
    codec && bluray
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_RESOLUTION_HINT, DEFAULT_SOURCE_HINT};
    use crate::{
        ContextFacetHint, ContextTitle, ParsedReleaseMetadata, ReleaseParseContext,
        ReleaseSource::{self, BluRay, Dvd, Hdtv, WebDl, WebRip},
        best_parse_for_target,
    };

    fn parse(facet_hint: ContextFacetHint, title: &str, raw: &str) -> ParsedReleaseMetadata {
        let target = ReleaseParseContext {
            facet_hint,
            title: ContextTitle { name: title.into() },
            aliases: vec![],
            known_years: vec![],
            imdb_ids: vec![],
            episodes: vec![],
        };
        best_parse_for_target(raw, &target)
    }

    fn series(raw: &str) -> ParsedReleaseMetadata {
        parse(ContextFacetHint::Series, "Series Title", raw)
    }

    fn movie(raw: &str) -> ParsedReleaseMetadata {
        parse(ContextFacetHint::Movie, "Movie Title", raw)
    }

    /// (source, quality, remux) of each name.
    fn check(
        parse: fn(&str) -> ParsedReleaseMetadata,
        cases: &[(&str, Option<ReleaseSource>, Option<&str>, bool)],
    ) {
        for &(raw, source, quality, remux) in cases {
            let p = parse(raw);
            assert_eq!(
                (p.source, p.quality.as_deref(), p.is_remux),
                (source, quality, remux),
                "{raw}"
            );
        }
    }

    // P33a

    #[test]
    fn amzn_and_nf_webrip_stay_webrip() {
        check(
            series,
            &[
                (
                    "Series Title S01E01 Tractoring 1080p AMZN WEBRip DDP5 1 x264-NTb",
                    Some(WebRip),
                    Some("1080p"),
                    false,
                ),
                (
                    "Series.Title.S03E09.1080p.NF.WEBRip.DD5.1.x264-ViSUM",
                    Some(WebRip),
                    Some("1080p"),
                    false,
                ),
                (
                    "Series.Title.S01.1080p.AMZN.WEB-Rip.DDP5.1.H.264-Telly",
                    Some(WebRip),
                    Some("1080p"),
                    false,
                ),
                (
                    "Series.Title.S07E01.A.New.Home.720p.DSNY.WEBRip.AAC2.0.x264-TVSmash",
                    Some(WebRip),
                    Some("720p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn webmux_reads_as_webrip() {
        check(
            series,
            &[
                (
                    "Series.Title.1x04.ITA.720p.WEBMux.x264-NovaRip",
                    Some(WebRip),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.1x04.ITA.WEBMux.x264-NovaRip",
                    Some(WebRip),
                    Some("480p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn webm_extension_and_web_title_word_are_not_a_source() {
        check(
            series,
            &[
                ("Series.Title.S01E01.webm", None, None, false),
                ("Series.Title.S01E01.The.Web.MT-dd", None, None, false),
            ],
        );
    }

    #[test]
    fn bare_web_in_arr_contexts_is_webdl() {
        for raw in [
            "Series.Title.S01E01.1080p.WEB.H264-GRP",
            "Series.Title.S01E01.WEB.x264-GRP",
            "Series.Title.S01E01.WEB.H.264-GRP",
            "Series.Title.S01E01.WEB.DDP5.1.H.264-GRP",
            "Series.Title.S01E01.2160p.Hybrid.WEB-GRP",
            "Series.Title.S01E01.WEB-1080p-GRP",
            "Series.Title.S01E01.NF.WEB.H264-GRP",
            "Series Title - 01 [WEB 1080p]",
            "Series.Title.S01E01.1080p.WEB",
        ] {
            assert_eq!(series(raw).source, Some(WebDl), "{raw}");
        }
    }

    // P33b

    #[test]
    fn tv_source_spellings_read_as_hdtv() {
        check(
            series,
            &[
                (
                    "Series.Title.S02E01.WS.PDTV.x264-TLA",
                    Some(Hdtv),
                    Some("480p"),
                    false,
                ),
                (
                    "Series Title S01E04 DSR x264 2HD",
                    Some(Hdtv),
                    Some("480p"),
                    false,
                ),
                (
                    "Series.Title.S03.TVRip.XviD-NOGRP",
                    Some(Hdtv),
                    Some("480p"),
                    false,
                ),
                (
                    "Series Title S01E01 [SDTV]",
                    Some(Hdtv),
                    Some("480p"),
                    false,
                ),
                ("Series Title S01E01 SD-TV", Some(Hdtv), Some("480p"), false),
                ("Series Title S01E01 HD.TV", Some(Hdtv), Some("720p"), false),
                (
                    "Series Title S01E01 1080p.HD.TV",
                    Some(Hdtv),
                    Some("1080p"),
                    false,
                ),
                (
                    "Series.Title.S12E17.HR.WS.PDTV.X264-DIMENSION",
                    Some(Hdtv),
                    Some("720p"),
                    false,
                ),
                (
                    "Series Title S01E07 (CZ)[TvRip][HEVC][720p]",
                    Some(Hdtv),
                    Some("720p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn german_service_suffixes_read_as_webdl() {
        check(
            series,
            &[
                (
                    "Series.Title.S04E25.720p.iTunesHD.AVC-TVS",
                    Some(WebDl),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.S01E08.German.DD51.Dubbed.DL.1080p.AmazonHD.x264-TVS",
                    Some(WebDl),
                    Some("1080p"),
                    false,
                ),
                (
                    "Series.Title.S03E05.German.720p.MaxdomeHD.AVC-TVS",
                    Some(WebDl),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.S01E01.German.DL.720p.HBOMaxHD.AVC-TVS",
                    Some(WebDl),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.S01E05.German.DL.1080p.DisneyHD.h264-4SF",
                    Some(WebDl),
                    Some("1080p"),
                    false,
                ),
                (
                    "Series.Title.S01E04.German.DL.NetflixUHD.x264",
                    Some(WebDl),
                    Some("480p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn bluray_spellings_read_as_bluray() {
        check(
            series,
            &[
                (
                    "Series.Title.1x01.ITA.720p.BDMux.x264-NovaRip",
                    Some(BluRay),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.S02E13.1080p.BDLight.x265-AVCDVD",
                    Some(BluRay),
                    Some("1080p"),
                    false,
                ),
                (
                    "Series.Title.S01E01.720p.HDDVD.x264-SiNNERS.mkv",
                    Some(BluRay),
                    Some("720p"),
                    false,
                ),
            ],
        );
        check(
            movie,
            &[
                (
                    "Movie.Title.2016.German.DTS.DL.1080p.UHDBD.x265-TDO.mkv",
                    Some(BluRay),
                    Some("1080p"),
                    false,
                ),
                (
                    "Movie.Title.2012.German.DL.1080p.UHD2BD.x264-QfG",
                    Some(BluRay),
                    Some("1080p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn dvd_spellings_read_as_dvd() {
        check(
            series,
            &[
                (
                    "Series.Title.1x13.circles.ws.xvidvd-tns",
                    Some(Dvd),
                    Some("480p"),
                    false,
                ),
                (
                    "Series.Title.S01E13.NTSC.x264-CtrlSD",
                    Some(Dvd),
                    Some("480p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn a_group_named_bd_is_not_a_source() {
        check(
            series,
            &[(
                "Series.Title.S01E01.The.Principle.720p.WEB-DL.DD5.1.H.264-BD",
                Some(WebDl),
                Some("720p"),
                false,
            )],
        );
    }

    #[test]
    fn pal_in_an_episode_title_is_not_a_source() {
        check(
            movie,
            &[("Movie.Title.2019.Pay.Pal.Story", None, None, false)],
        );
        check(
            series,
            &[("Series.Title.S01E01.Pen.Pal.Problems", None, None, false)],
        );
    }

    // P33c

    #[test]
    fn wxh_dimensions_read_as_resolution() {
        check(
            series,
            &[
                (
                    "[Group] Series Title - 363 [848x480][ADE35E38]",
                    Some(Hdtv),
                    Some("480p"),
                    false,
                ),
                (
                    "[Doki] Series Title - 07 (1280x720 Hi10P AAC) [80AF7DDE]",
                    Some(Hdtv),
                    Some("720p"),
                    false,
                ),
                (
                    "[Coalgirls]_Series_Title_01_(1920x1080_Blu-ray_FLAC)_[8370CB8F].mkv",
                    Some(BluRay),
                    Some("1080p"),
                    false,
                ),
            ],
        );
        check(
            movie,
            &[
                ("Movie.Title.2018.1280x720", Some(Hdtv), Some("720p"), false),
                (
                    "Movie.Title.2018.1920x1080.Bluray",
                    Some(BluRay),
                    Some("1080p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn fhd_reads_as_1080p() {
        check(
            series,
            &[
                (
                    "Series Title 04 vostfr FHD.mkv",
                    Some(Hdtv),
                    Some("1080p"),
                    false,
                ),
                (
                    "Series Title Away(2001) Bluray FHD Hi10P.mkv",
                    Some(BluRay),
                    Some("1080p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn nonstandard_heights_round_to_arr_classes() {
        check(
            series,
            &[
                (
                    "Series.Title.S04E13.960p.WEB-DL.AAC2.0.H.264-squalor",
                    Some(WebDl),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.S01E08.1440p.BKPL.WEB-DL.H.264-LiGHT",
                    Some(WebDl),
                    Some("1080p"),
                    false,
                ),
                (
                    "[SubsPlease] Series Title (540p) [AB649D32].mkv",
                    Some(Hdtv),
                    Some("480p"),
                    false,
                ),
                (
                    "[HorribleSubs] Series Title - 03 [360p].mkv",
                    Some(Hdtv),
                    Some("480p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn a_checksum_is_not_a_resolution() {
        check(
            series,
            &[(
                "[Doki] Series Title - 02 (848x480 XviD BD MP3) [95360783]",
                Some(BluRay),
                Some("480p"),
                false,
            )],
        );
    }

    #[test]
    fn fhd_release_group_is_not_a_resolution() {
        check(
            series,
            &[
                (
                    "Series.Title.S01E01.720p.HDTV.x264-FHD",
                    Some(Hdtv),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.S01E01.BluRay.x264-FHD",
                    Some(BluRay),
                    Some("720p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn conflicting_resolutions_stay_unknown() {
        let p = series("Series.Title.S01E01.[720p].[1080p].WEB-DL-GRP");
        assert_eq!(p.quality, None);
    }

    // P33d

    #[test]
    fn a_resolution_without_source_defaults_to_hdtv() {
        check(
            series,
            &[
                (
                    "Series.Title.S05E21.1080p.HEVC.x265-MeGusta",
                    Some(Hdtv),
                    Some("1080p"),
                    false,
                ),
                ("Series.Title.S02E15.720p", Some(Hdtv), Some("720p"), false),
                (
                    "[HorribleSubs] Series Title - 32 [480p]",
                    Some(Hdtv),
                    Some("480p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn remux_without_source_reads_bluray() {
        check(
            series,
            &[(
                "Series.Title.S03E01.The.Calm.1080p.DTS-HD.MA.5.1.AVC.REMUX-FraMeSToR",
                Some(BluRay),
                Some("1080p"),
                true,
            )],
        );
        check(
            movie,
            &[(
                "Movie.Title.2011.2160p.DD.2.0.AVC.REMUX-FraMeSToR",
                Some(BluRay),
                Some("2160p"),
                true,
            )],
        );
    }

    #[test]
    fn a_source_without_resolution_gets_the_arr_default() {
        check(
            series,
            &[
                (
                    "Series.Title.S04E05.HDTV.XviD-LOL",
                    Some(Hdtv),
                    Some("480p"),
                    false,
                ),
                (
                    "Series Title - S01E01 - Title [HDTV]",
                    Some(Hdtv),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.S03E06.DVDRip.XviD-WiDE",
                    Some(Dvd),
                    Some("480p"),
                    false,
                ),
                (
                    "SERIES.TITLE.S03E01-06.DUAL.BDRip.AC3.-HELLYWOOD",
                    Some(BluRay),
                    Some("480p"),
                    false,
                ),
                (
                    "SERIES.TITLE.S03E01-06.DUAL.XviD.Bluray.AC3.-HELLYWOOD.avi",
                    Some(BluRay),
                    Some("480p"),
                    false,
                ),
                (
                    "SERIES.TITLE.S03E01-06.DUAL.Bluray.AC3.-HELLYWOOD.avi",
                    Some(BluRay),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.2x11.Nato.Bluray.Remux.AVC.AC3.ITA",
                    Some(BluRay),
                    Some("1080p"),
                    true,
                ),
                (
                    "Series Title S07E23 - [WEBDL].mkv",
                    Some(WebDl),
                    Some("720p"),
                    false,
                ),
                (
                    "[HorribleSubs] Series Title S01 [Web][MKV][h264][AAC 2.0]",
                    Some(WebDl),
                    Some("720p"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn remux_below_1080_is_bluray() {
        check(
            series,
            &[
                (
                    "Series.Title.S01E01.Best.Hedgehog.720p.DD.2.0.AVC.REMUX-FraMeSToR",
                    Some(BluRay),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.S02.720p.Blu-ray.Remux.AVC.FLAC.2.0-SiCFoI",
                    Some(BluRay),
                    Some("720p"),
                    false,
                ),
                (
                    "Series.Title.S01.BluRay.480i.DD.2.0.AVC.REMUX-FraMeSToR",
                    Some(BluRay),
                    Some("480i"),
                    false,
                ),
            ],
        );
    }

    #[test]
    fn remux_with_a_tv_source_is_not_a_remux() {
        check(
            series,
            &[(
                "Series.Title.S01E01.HR.WS.PDTV.x264-DHD-Remux.mkv",
                Some(Hdtv),
                Some("720p"),
                false,
            )],
        );
    }

    #[test]
    fn remux_with_neither_is_unknown() {
        check(movie, &[("Movie.Title.2017.Remux", None, None, false)]);
    }

    #[test]
    fn x264_alone_reads_sdtv() {
        check(
            series,
            &[(
                "Series.Title.S01E13.x264-CtrlSD",
                Some(Hdtv),
                Some("480p"),
                false,
            )],
        );
    }

    #[test]
    fn defaults_are_marked_in_parse_hints() {
        let hints = |raw: &str| {
            let p = series(raw);
            (
                p.parse_hints.iter().any(|h| h == DEFAULT_SOURCE_HINT),
                p.parse_hints.iter().any(|h| h == DEFAULT_RESOLUTION_HINT),
            )
        };
        assert_eq!(hints("Series.Title.S02E15.720p"), (true, false));
        assert_eq!(hints("Series.Title.S01E01.HD.TV"), (false, true));
        assert_eq!(hints("Series.Title.S01E01.1080p-Remux"), (true, false));
        assert_eq!(hints("Series.Title.S01E13.x264-CtrlSD"), (true, true));
        assert_eq!(
            hints("Series.Title.S12E17.HR.WS.PDTV.X264-DIMENSION"),
            (false, false)
        );
        assert_eq!(
            hints("Series.Title.S01E01.1080p.WEB-DL.x264-GRP"),
            (false, false)
        );
    }

    #[test]
    fn a_name_with_neither_stays_unknown() {
        check(
            series,
            &[
                ("Series.Title.S07E01.The.Next.Generation", None, None, false),
                ("Series Title S01E01", None, None, false),
            ],
        );
    }

    #[test]
    fn conflicting_sources_get_no_default() {
        check(
            series,
            &[(
                "Series.Title.S01E01.1080p.BluRay.WEB-DL.x264-GRP",
                None,
                Some("1080p"),
                false,
            )],
        );
    }

    // P33e

    #[test]
    fn mpeg2_hdtv_and_raw_hd_read_as_raw_hd() {
        for raw in [
            "Series.Title.S02E11.1080i.HDTV.DD5.1.MPEG2-TrollHD",
            "Series Title S01E18 Title 720p HDTV DD5.1 MPEG2-TrollHD",
            "Series.Title.2011-08-04.1080i.HDTV.MPEG-2-CtrlHD",
            "Series Title - S03E01 - Episode Title Raw-HD.ts",
        ] {
            let p = series(raw);
            assert_eq!(
                (p.is_raw_hd, p.source, p.quality.as_deref()),
                (true, Some(Hdtv), Some("1080p")),
                "{raw}"
            );
        }
        // MPEG2Regex is case-sensitive; MPEG2 without HDTV is no broadcast.
        for raw in [
            "Series.Title.S02E11.1080i.HDTV.DD5.1.mpeg2-GRP",
            "Series.Title.S02E11.1080p.BluRay.MPEG2-GRP",
        ] {
            assert!(!series(raw).is_raw_hd, "{raw}");
        }
    }

    // P33f

    #[test]
    fn german_bluray_with_disc_codec_is_a_remux() {
        check(
            movie,
            &[
                (
                    "Movie.Title.1991.German.ML.1080p.BluRay.AVC-GeRMaNSCeNEGRoUP",
                    Some(BluRay),
                    Some("1080p"),
                    true,
                ),
                (
                    "Movie.Title.1991.German.ML.2160p.BluRay.HEVC-GeRMaNSCeNEGRoUP",
                    Some(BluRay),
                    Some("2160p"),
                    true,
                ),
            ],
        );
    }

    #[test]
    fn german_bluray_x264_is_not() {
        check(
            movie,
            &[(
                "Movie.Title.1991.German.DL.1080p.BluRay.x264-GRP",
                Some(BluRay),
                Some("1080p"),
                false,
            )],
        );
    }
}
