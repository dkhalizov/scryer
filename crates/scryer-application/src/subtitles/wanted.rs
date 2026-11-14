use scryer_domain::SubtitleDownload;
use std::collections::BTreeMap;

use super::language::same_subtitle_language;

/// Represents a subtitle that is wanted but not yet downloaded.
#[derive(Debug, Clone)]
pub struct WantedSubtitle {
    pub media_file_id: String,
    pub title_id: String,
    pub episode_id: Option<String>,
    pub language: String,
    pub hearing_impaired: bool,
    pub forced: bool,
    /// File path to the video on disk.
    pub video_path: String,
    /// Title name (for metadata-based search).
    pub title_name: String,
    /// Content facet (movie, series, anime).
    pub facet: Option<String>,
    /// Release year.
    pub year: Option<i32>,
    /// IMDb ID.
    pub imdb_id: Option<String>,
    /// Season number (series only).
    pub season: Option<i32>,
    /// Episode number (series only).
    pub episode: Option<i32>,
    /// Absolute episode number when available.
    pub absolute_episode: Option<i32>,
    /// Provider-specific external identifiers grouped by normalized source key.
    pub external_ids: BTreeMap<String, Vec<String>>,
}

/// Language preference for subtitle downloading.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubtitleLanguagePref {
    /// ISO 639-2 code (e.g., "eng").
    pub code: String,
    /// Whether to specifically request hearing-impaired subs.
    pub hearing_impaired: bool,
    /// Whether to specifically request forced subs.
    pub forced: bool,
}

/// Determine which subtitles are missing for a set of media files.
///
/// Compares the wanted languages against:
/// 1. Already-downloaded external subtitles (from `subtitle_downloads` table)
/// 2. Embedded subtitle streams (from the stored media analysis, `media_file_analyses`)
///
/// Returns only the combinations that are still needed.
pub fn compute_missing_subtitles(
    wanted_languages: &[SubtitleLanguagePref],
    existing_downloads: &[SubtitleDownload],
    embedded_languages: &[String],
) -> Vec<SubtitleLanguagePref> {
    wanted_languages
        .iter()
        .filter(|want| {
            let lang = &want.code;

            if !want.forced
                && embedded_languages
                    .iter()
                    .any(|embedded| same_subtitle_language(embedded, lang))
            {
                return false;
            }

            !existing_downloads.iter().any(|dl| {
                same_subtitle_language(&dl.language, lang)
                    && subtitle_kind_satisfies(want, dl.hearing_impaired, dl.forced)
            })
        })
        .cloned()
        .collect()
}

pub fn compute_missing_subtitles_from_streams(
    wanted_languages: &[SubtitleLanguagePref],
    existing_downloads: &[SubtitleDownload],
    embedded_streams: &[crate::SubtitleStreamDetail],
) -> Vec<SubtitleLanguagePref> {
    wanted_languages
        .iter()
        .filter(|want| {
            let lang = &want.code;

            if embedded_streams
                .iter()
                .any(|stream| embedded_stream_satisfies(stream, want))
            {
                return false;
            }

            !existing_downloads.iter().any(|dl| {
                same_subtitle_language(&dl.language, lang)
                    && subtitle_kind_satisfies(want, dl.hearing_impaired, dl.forced)
            })
        })
        .cloned()
        .collect()
}

// fork (P7): Bazarr's rule. A forced item takes only forced, an HI item only HI,
// and a plain item (hi=false, forced=false) takes a plain or an HI subtitle.
pub fn subtitle_kind_satisfies(
    want: &SubtitleLanguagePref,
    hearing_impaired: bool,
    forced: bool,
) -> bool {
    if want.forced {
        forced
    } else if forced {
        false
    } else {
        hearing_impaired || !want.hearing_impaired
    }
}

/// fork (P7): the download pick's selection step. Best-scoring result at or above
/// `min_score` whose kind satisfies `want`.
pub(crate) fn pick_best_subtitle<'a>(
    results: &[&'a super::provider::SubtitleMatch],
    want: &SubtitleLanguagePref,
    min_score: i32,
) -> Option<&'a super::provider::SubtitleMatch> {
    results
        .iter()
        .filter(|r| r.score >= min_score)
        .filter(|r| subtitle_kind_satisfies(want, r.hearing_impaired, r.forced))
        // Equal scores (a hash match drops the scorer's +1 HI point): prefer the exact HI match.
        .max_by_key(|r| (r.score, r.hearing_impaired == want.hearing_impaired))
        .copied()
}

/// fork (P7): whether a subtitle already saved this cycle, as (language, hi, forced),
/// satisfies `want`, so a later item for the same language needs no download.
pub(crate) fn satisfied_by_saved(
    saved: &[(String, bool, bool)],
    want: &SubtitleLanguagePref,
) -> bool {
    saved.iter().any(|(lang, hi, forced)| {
        same_subtitle_language(lang, &want.code) && subtitle_kind_satisfies(want, *hi, *forced)
    })
}

fn embedded_stream_satisfies(
    stream: &crate::SubtitleStreamDetail,
    wanted: &SubtitleLanguagePref,
) -> bool {
    let Some(language) = stream.language.as_deref() else {
        return false;
    };
    if !same_subtitle_language(language, &wanted.code) {
        return false;
    }

    if wanted.forced {
        return stream.forced;
    }
    if stream.forced {
        return false;
    }

    let embedded_hi = stream_name_is_hearing_impaired(stream.name.as_deref());
    subtitle_kind_satisfies(wanted, embedded_hi, false)
}

fn stream_name_is_hearing_impaired(name: Option<&str>) -> bool {
    let Some(name) = name else {
        return false;
    };
    let upper = name.to_ascii_uppercase();
    if upper.contains("HEARING IMPAIRED") {
        return true;
    }

    upper
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .any(|token| matches!(token, "SDH" | "HI" | "CC"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_download(lang: &str, forced: bool, hi: bool) -> SubtitleDownload {
        SubtitleDownload {
            id: "test".into(),
            media_file_id: "mf1".into(),
            title_id: "t1".into(),
            episode_id: None,
            source_kind: scryer_domain::ExternalSubtitleSourceKind::Downloaded,
            language: lang.into(),
            provider: Some("opensubtitles".into()),
            provider_file_id: None,
            file_path: "/tmp/test.srt".into(),
            score: Some(300),
            hearing_impaired: hi,
            forced,
            ai_translated: false,
            machine_translated: false,
            uploader: None,
            release_info: None,
            synced: false,
            downloaded_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn make_stream(lang: &str, forced: bool, name: Option<&str>) -> crate::SubtitleStreamDetail {
        crate::SubtitleStreamDetail {
            codec: Some("srt".into()),
            language: Some(lang.into()),
            name: name.map(str::to_string),
            forced,
            default: false,
        }
    }

    #[test]
    fn nothing_missing_when_all_downloaded() {
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: false,
            forced: false,
        }];
        let downloads = vec![make_download("eng", false, false)];
        let missing = compute_missing_subtitles(&wanted, &downloads, &[]);
        assert!(missing.is_empty());
    }

    #[test]
    fn missing_when_no_downloads() {
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: false,
            forced: false,
        }];
        let missing = compute_missing_subtitles(&wanted, &[], &[]);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].code, "eng");
    }

    #[test]
    fn embedded_satisfies_non_forced() {
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: false,
            forced: false,
        }];
        let embedded = vec!["eng".to_string()];
        let missing = compute_missing_subtitles(&wanted, &[], &embedded);
        assert!(missing.is_empty());
    }

    #[test]
    fn embedded_does_not_satisfy_forced() {
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: false,
            forced: true,
        }];
        let embedded = vec!["eng".to_string()];
        let missing = compute_missing_subtitles(&wanted, &[], &embedded);
        assert_eq!(missing.len(), 1);
    }

    // ── Multiple languages, some satisfied ──────────────────────────

    #[test]
    fn multiple_languages_some_satisfied() {
        let wanted = vec![
            SubtitleLanguagePref {
                code: "eng".into(),
                hearing_impaired: false,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "spa".into(),
                hearing_impaired: false,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "fre".into(),
                hearing_impaired: false,
                forced: false,
            },
        ];
        let downloads = vec![make_download("eng", false, false)];
        let missing = compute_missing_subtitles(&wanted, &downloads, &[]);
        assert_eq!(missing.len(), 2);
        let codes: Vec<&str> = missing.iter().map(|m| m.code.as_str()).collect();
        assert!(codes.contains(&"spa"));
        assert!(codes.contains(&"fre"));
        assert!(!codes.contains(&"eng"));
    }

    #[test]
    fn multiple_languages_all_satisfied() {
        let wanted = vec![
            SubtitleLanguagePref {
                code: "eng".into(),
                hearing_impaired: false,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "spa".into(),
                hearing_impaired: false,
                forced: false,
            },
        ];
        let downloads = vec![
            make_download("eng", false, false),
            make_download("spa", false, false),
        ];
        let missing = compute_missing_subtitles(&wanted, &downloads, &[]);
        assert!(missing.is_empty());
    }

    #[test]
    fn multiple_languages_one_embedded_one_downloaded() {
        let wanted = vec![
            SubtitleLanguagePref {
                code: "eng".into(),
                hearing_impaired: false,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "spa".into(),
                hearing_impaired: false,
                forced: false,
            },
        ];
        let downloads = vec![make_download("spa", false, false)];
        let embedded = vec!["eng".to_string()];
        let missing = compute_missing_subtitles(&wanted, &downloads, &embedded);
        assert!(missing.is_empty());
    }

    // ── HI wanted but non-HI downloaded ─────────────────────────────

    #[test]
    fn hi_wanted_but_non_hi_downloaded_still_missing() {
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: true,
            forced: false,
        }];
        // Download has hearing_impaired = false
        let downloads = vec![make_download("eng", false, false)];
        let missing = compute_missing_subtitles(&wanted, &downloads, &[]);
        assert_eq!(missing.len(), 1);
        assert!(missing[0].hearing_impaired);
    }

    #[test]
    fn hi_wanted_and_hi_downloaded_is_satisfied() {
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: true,
            forced: false,
        }];
        let downloads = vec![make_download("eng", false, true)];
        let missing = compute_missing_subtitles(&wanted, &downloads, &[]);
        assert!(missing.is_empty());
    }

    #[test]
    fn non_hi_wanted_is_satisfied_by_hi_download() {
        // fork (P7): Bazarr's rule, a plain item accepts a plain or an HI subtitle.
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: false,
            forced: false,
        }];
        let downloads = vec![make_download("eng", false, true)];
        assert!(compute_missing_subtitles(&wanted, &downloads, &[]).is_empty());
        assert!(compute_missing_subtitles_from_streams(&wanted, &downloads, &[]).is_empty());
        let sdh = [make_stream("eng", false, Some("English SDH"))];
        assert!(compute_missing_subtitles_from_streams(&wanted, &[], &sdh).is_empty());
    }

    fn make_match(
        id: &str,
        score: i32,
        hi: bool,
        forced: bool,
    ) -> super::super::provider::SubtitleMatch {
        super::super::provider::SubtitleMatch {
            provider: "p".into(),
            provider_file_id: id.into(),
            language: "eng".into(),
            release_info: None,
            score,
            score_percent: 0,
            hearing_impaired: hi,
            forced,
            ai_translated: false,
            machine_translated: false,
            uploader: None,
            download_count: None,
            hash_matched: false,
        }
    }

    fn pref(hi: bool, forced: bool) -> SubtitleLanguagePref {
        SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: hi,
            forced,
        }
    }

    #[test]
    fn pick_plain_item_takes_hi_result_when_only_one() {
        let hi = make_match("hi", 300, true, false);
        let forced = make_match("f", 400, false, true);
        let best = pick_best_subtitle(&[&hi, &forced], &pref(false, false), 200);
        assert_eq!(best.unwrap().provider_file_id, "hi");
    }

    #[test]
    fn pick_plain_result_beats_hi_with_higher_score() {
        // The scorer gives a plain result for a plain item +1 over an HI one.
        let plain = make_match("plain", 301, false, false);
        let hi = make_match("hi", 300, true, false);
        let best = pick_best_subtitle(&[&hi, &plain], &pref(false, false), 200);
        assert_eq!(best.unwrap().provider_file_id, "plain");
    }

    #[test]
    fn pick_equal_scores_prefer_exact_hi_match_in_either_order() {
        let plain = make_match("plain", 300, false, false);
        let hi = make_match("hi", 300, true, false);
        for order in [[&plain, &hi], [&hi, &plain]] {
            let best = pick_best_subtitle(&order, &pref(false, false), 200);
            assert_eq!(best.unwrap().provider_file_id, "plain");
            let best = pick_best_subtitle(&order, &pref(true, false), 200);
            assert_eq!(best.unwrap().provider_file_id, "hi");
        }
    }

    #[test]
    fn saved_hi_subtitle_satisfies_later_plain_and_hi_items_only() {
        let saved = [("eng".to_string(), true, false)];
        assert!(satisfied_by_saved(&saved, &pref(false, false)));
        assert!(satisfied_by_saved(&saved, &pref(true, false)));
        assert!(!satisfied_by_saved(&saved, &pref(false, true)));
        let saved_plain = [("eng".to_string(), false, false)];
        assert!(!satisfied_by_saved(&saved_plain, &pref(true, false)));
        assert!(!satisfied_by_saved(&[], &pref(false, false)));
    }

    #[test]
    fn forced_item_and_forced_hi_subtitle_accept_each_other() {
        // Bazarr: forced wins, hi is not compared for a forced item.
        assert!(subtitle_kind_satisfies(&pref(false, true), true, true));
        assert!(subtitle_kind_satisfies(&pref(true, true), false, true));
        let wanted = vec![pref(false, true)];
        assert!(
            compute_missing_subtitles(&wanted, &[make_download("eng", true, true)], &[]).is_empty()
        );
        let wanted = vec![pref(true, true)];
        assert!(
            compute_missing_subtitles_from_streams(
                &wanted,
                &[make_download("eng", true, false)],
                &[]
            )
            .is_empty()
        );
    }

    #[test]
    fn pick_hi_item_never_takes_plain_result() {
        let plain = make_match("plain", 400, false, false);
        assert!(pick_best_subtitle(&[&plain], &pref(true, false), 200).is_none());
    }

    #[test]
    fn pick_respects_min_score() {
        let hi = make_match("hi", 100, true, false);
        assert!(pick_best_subtitle(&[&hi], &pref(false, false), 200).is_none());
    }

    #[test]
    fn forced_wanted_is_not_satisfied_by_hi_download() {
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: false,
            forced: true,
        }];
        let downloads = vec![make_download("eng", false, true)];
        assert_eq!(compute_missing_subtitles(&wanted, &downloads, &[]).len(), 1);
        assert_eq!(
            compute_missing_subtitles_from_streams(&wanted, &downloads, &[]).len(),
            1
        );
    }

    // ── Forced wanted, non-forced embedded ──────────────────────────

    #[test]
    fn forced_wanted_non_forced_embedded_still_missing() {
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: false,
            forced: true,
        }];
        // Embedded tracks are non-forced (embedded_languages are just language codes)
        let embedded = vec!["eng".to_string()];
        let missing = compute_missing_subtitles(&wanted, &[], &embedded);
        assert_eq!(missing.len(), 1);
        assert!(missing[0].forced);
    }

    #[test]
    fn forced_wanted_non_forced_downloaded_still_missing() {
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: false,
            forced: true,
        }];
        // Download has forced = false
        let downloads = vec![make_download("eng", false, false)];
        let missing = compute_missing_subtitles(&wanted, &downloads, &[]);
        assert_eq!(missing.len(), 1);
        assert!(missing[0].forced);
    }

    #[test]
    fn forced_wanted_forced_downloaded_is_satisfied() {
        let wanted = vec![SubtitleLanguagePref {
            code: "eng".into(),
            hearing_impaired: false,
            forced: true,
        }];
        let downloads = vec![make_download("eng", true, false)];
        let missing = compute_missing_subtitles(&wanted, &downloads, &[]);
        assert!(missing.is_empty());
    }

    // ── All satisfied by mix of embedded + downloads ────────────────

    #[test]
    fn all_satisfied_by_mix() {
        let wanted = vec![
            SubtitleLanguagePref {
                code: "eng".into(),
                hearing_impaired: false,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "spa".into(),
                hearing_impaired: false,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "fre".into(),
                hearing_impaired: true,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "jpn".into(),
                hearing_impaired: false,
                forced: true,
            },
        ];
        let downloads = vec![
            make_download("fre", false, true), // satisfies fre HI
            make_download("jpn", true, false), // satisfies jpn forced
        ];
        let embedded = vec!["eng".to_string(), "spa".to_string()];
        let missing = compute_missing_subtitles(&wanted, &downloads, &embedded);
        assert!(
            missing.is_empty(),
            "all should be satisfied, missing: {missing:?}"
        );
    }

    #[test]
    fn mix_of_satisfied_and_unsatisfied() {
        let wanted = vec![
            SubtitleLanguagePref {
                code: "eng".into(),
                hearing_impaired: false,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "spa".into(),
                hearing_impaired: true,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "fre".into(),
                hearing_impaired: false,
                forced: true,
            },
        ];
        let downloads = vec![make_download("eng", false, false)];
        let embedded = vec![];
        let missing = compute_missing_subtitles(&wanted, &downloads, &embedded);
        assert_eq!(missing.len(), 2);
        let codes: Vec<&str> = missing.iter().map(|m| m.code.as_str()).collect();
        assert!(codes.contains(&"spa"));
        assert!(codes.contains(&"fre"));
    }

    // ── Empty wanted list ───────────────────────────────────────────

    #[test]
    fn empty_wanted_returns_empty_missing() {
        let wanted: Vec<SubtitleLanguagePref> = vec![];
        let downloads = vec![make_download("eng", false, false)];
        let embedded = vec!["spa".to_string()];
        let missing = compute_missing_subtitles(&wanted, &downloads, &embedded);
        assert!(missing.is_empty());
    }

    #[test]
    fn empty_wanted_empty_downloads_returns_empty() {
        let missing = compute_missing_subtitles(&[], &[], &[]);
        assert!(missing.is_empty());
    }

    // ── Same language, different flags ───────────────────────────────

    #[test]
    fn same_language_different_flags_tracked_independently() {
        let wanted = vec![
            SubtitleLanguagePref {
                code: "eng".into(),
                hearing_impaired: false,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "eng".into(),
                hearing_impaired: true,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "eng".into(),
                hearing_impaired: false,
                forced: true,
            },
        ];
        // Only the normal eng is downloaded
        let downloads = vec![make_download("eng", false, false)];
        let missing = compute_missing_subtitles(&wanted, &downloads, &[]);
        assert_eq!(missing.len(), 2);
    }

    #[test]
    fn embedded_streams_distinguish_forced_and_hi() {
        let wanted = vec![
            SubtitleLanguagePref {
                code: "eng".into(),
                hearing_impaired: true,
                forced: false,
            },
            SubtitleLanguagePref {
                code: "eng".into(),
                hearing_impaired: false,
                forced: true,
            },
        ];
        let embedded = vec![
            make_stream("eng", false, Some("English SDH")),
            make_stream("eng", true, Some("English Forced")),
        ];

        let missing = compute_missing_subtitles_from_streams(&wanted, &[], &embedded);
        assert!(missing.is_empty());
    }

    #[test]
    fn embedded_streams_use_canonical_language_matching() {
        let wanted = vec![SubtitleLanguagePref {
            code: "fre".into(),
            hearing_impaired: false,
            forced: false,
        }];
        let embedded = vec![make_stream("fra", false, Some("French"))];

        let missing = compute_missing_subtitles_from_streams(&wanted, &[], &embedded);
        assert!(missing.is_empty());
    }

    #[test]
    fn hi_detection_ignores_hi_substrings_inside_words() {
        assert!(!stream_name_is_hearing_impaired(Some("Thai")));
        assert!(!stream_name_is_hearing_impaired(Some("Hindi")));
        assert!(!stream_name_is_hearing_impaired(Some("Simplified Chinese")));
        assert!(stream_name_is_hearing_impaired(Some("English HI")));
        assert!(stream_name_is_hearing_impaired(Some("English SDH")));
        assert!(stream_name_is_hearing_impaired(Some("English CC")));
        assert!(stream_name_is_hearing_impaired(Some(
            "English Hearing Impaired"
        )));
    }
}
