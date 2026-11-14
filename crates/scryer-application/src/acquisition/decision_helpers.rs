use crate::acquisition_search_queries::tvdb_id_from_external_ids;
use crate::{
    AcquisitionThresholds, AppError, AppResult, AppUseCase, QualityProfile, TitleMediaFile,
    app_usecase_discovery::QualityProfileLookup,
};
use scryer_domain::Title;

/// A season pack that failed within this window is not retried as a pack —
/// its episodes search individually instead.
pub(crate) const FAILED_GRAB_RESEARCH_COOLDOWN_MINUTES: i64 = 20;

pub(crate) fn extract_grabbed_release_title(raw: Option<&str>) -> Option<String> {
    raw.and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .and_then(|value| {
            value
                .get("title")
                .and_then(|title| title.as_str())
                .map(str::to_string)
        })
}

/// The one retryability decision every acquisition path (pending processing,
/// RSS, both task-runner submissions, catalog queueing) makes about a failed
/// download submission: retry later without burning the release only for the
/// typed `DownloadSubmitUnavailable` / `DownloadSubmitFailoverExhausted`
/// failures, or a title temporarily locked by a location operation. Message text is never inspected — an old
/// "all prioritized download clients failed" repository string, a rendered
/// typed error wrapped in another error, or any near-match is a definitive
/// failure, not failover evidence.
pub(crate) fn is_download_submit_unavailable_error(err: &AppError) -> bool {
    err.is_retryable_download_submit_failure() || matches!(err, AppError::LocationOperationBusy(_))
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedUpgradeContext {
    pub(crate) profile: QualityProfile,
    pub(crate) thresholds: AcquisitionThresholds,
    pub(crate) cutoff_reached: bool,
}

pub(crate) fn upgrade_context_category<'a>(
    title: &'a Title,
    category_hint: Option<&'a str>,
) -> &'a str {
    category_hint
        .map(str::trim)
        .filter(|category| !category.is_empty())
        .unwrap_or_else(|| title.facet.as_str())
}

/// What "the quality already on disk" means for one submission scope.
///
/// A pack is the case that forced this to be a type rather than two `Option`s:
/// a `SubmissionScope::Collection` has neither an episode id nor a link id, so
/// passing those two alone matched the *title-scoped* files — of which a series
/// has none. The cutoff check then always said "not reached" and a season
/// entirely at cutoff could be re-fetched as a pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CutoffScope {
    Title,
    Episode(String),
    SeriesMovieLink(String),
    /// Every episode the scope covers — a multi-episode span or a season's
    /// monitored members.
    Episodes(Vec<String>),
}

/// The quality a scope has already reached, or `None` when it has not reached
/// one.
///
/// **Chosen by quality, not by the stored `acquisition_score`.** A persisted
/// score is only valid while the profile, persona, rule packs and algorithm that
/// wrote it are unchanged, so electing the cutoff-defining file with it let an
/// old-scale row (or a −10 000 one written before vetoes became verdicts) decide
/// whether a scope had reached cutoff. Quality is the thing the cutoff is about;
/// recency breaks ties.
///
/// For a **multi-member** scope the answer is the **weakest** member's quality,
/// and `None` if any member is empty: a season has reached cutoff only when all
/// of it has. Taking the best member would call a season satisfied on the
/// strength of one good episode.
///
/// Ordering is by resolution, descending, read with the same parser
/// `quality_tier_index` normalizes through. fork (P11): the answer is the tier
/// key (`quality_tier_label` of the file's quality and stored source), so a
/// profile with source tiers ranks it like the candidate and the incumbent.
// ponytail: ties within a resolution break on a fixed source order (`cutoff_rank`),
// not the profile's tiers (callers resolve the profile from this value); pass the
// tiers in if a profile ever ranks sources differently.
pub fn analyzed_cutoff_quality_for_scope<'a, F: CutoffFile>(
    existing_files: &'a [F],
    scope: &CutoffScope,
) -> Option<String> {
    match scope {
        CutoffScope::Episodes(episode_ids) => {
            if episode_ids.is_empty() {
                return None;
            }
            let mut weakest: Option<String> = None;
            for episode_id in episode_ids {
                let member = best_cutoff_quality_for(existing_files, |file| {
                    file.episode_id == Some(episode_id.as_str())
                })?;
                // `cutoff_rank` is lower-is-better, so the weakest member is
                // the one with the highest rank.
                weakest = Some(match weakest {
                    Some(current) if cutoff_rank(&current) >= cutoff_rank(&member) => current,
                    _ => member,
                });
            }
            weakest
        }
        CutoffScope::Episode(episode_id) => best_cutoff_quality_for(existing_files, |file| {
            file.episode_id == Some(episode_id.as_str())
        }),
        CutoffScope::SeriesMovieLink(link_id) => best_cutoff_quality_for(existing_files, |file| {
            file.series_movie_link_ids
                .iter()
                .any(|candidate| candidate == link_id)
        }),
        CutoffScope::Title => best_cutoff_quality_for(existing_files, |file| {
            file.episode_id.is_none() && file.series_movie_link_ids.is_empty()
        }),
    }
}

/// The best quality among the primary files a predicate selects.
fn best_cutoff_quality_for<'a, F: CutoffFile>(
    existing_files: &'a [F],
    matches: impl Fn(&CutoffFileFacts<'a>) -> bool,
) -> Option<String> {
    existing_files
        .iter()
        .map(CutoffFile::cutoff_facts)
        .filter(|file| file.primary)
        .filter(|file| matches(file))
        .filter_map(|file| {
            // fork (P11): the tier key, so a source breaks a resolution tie
            let key =
                crate::quality_profile::quality_tier_label(file.quality_label, file.source_type)?;
            Some((file, key))
        })
        .min_by(|(left_file, left), (right_file, right)| {
            cutoff_rank(left)
                .cmp(&cutoff_rank(right))
                .then_with(|| right_file.created_at.cmp(left_file.created_at))
        })
        .map(|(_, key)| key)
}

/// fork (P11): a tier key's place, lower is better: resolution first, then the
/// source (REMUX > BLURAY > WEBDL > WEBRIP > HDTV > DVD > any other or none).
fn cutoff_rank(key: &str) -> (u32, u8) {
    let source = key.rsplit_once('-').map(|(source, _)| source);
    let source_rank = ["REMUX", "BLURAY", "WEBDL", "WEBRIP", "HDTV", "DVD"]
        .iter()
        .position(|known| Some(*known) == source)
        .unwrap_or(6) as u8;
    (resolution_rank(key), source_rank)
}

pub struct CutoffFileFacts<'a> {
    episode_id: Option<&'a str>,
    series_movie_link_ids: &'a [String],
    quality_label: Option<&'a str>,
    source_type: Option<&'a str>, // fork (P11)
    created_at: &'a str,
    primary: bool,
}

/// The cutoff gate reads the same facts from full records and compact rows.
pub trait CutoffFile {
    fn cutoff_facts(&self) -> CutoffFileFacts<'_>;
}

impl CutoffFile for TitleMediaFile {
    fn cutoff_facts(&self) -> CutoffFileFacts<'_> {
        CutoffFileFacts {
            episode_id: self.episode_id.as_deref(),
            series_movie_link_ids: &self.series_movie_link_ids,
            quality_label: self.quality_label.as_deref(),
            source_type: self.source_type.as_deref(), // fork (P11)
            created_at: &self.created_at,
            primary: self.role.is_primary(),
        }
    }
}

impl CutoffFile for crate::MediaFileCutoffSummary {
    fn cutoff_facts(&self) -> CutoffFileFacts<'_> {
        CutoffFileFacts {
            episode_id: self.episode_id.as_deref(),
            series_movie_link_ids: &self.series_movie_link_ids,
            quality_label: self.quality_label.as_deref(),
            source_type: self.source_type.as_deref(), // fork (P11)
            created_at: &self.created_at,
            primary: true,
        }
    }
}

/// Position of a quality in the resolution ordering; **lower is better**, so it
/// composes with `quality_tier_index` without an inversion.
///
/// Reads the label through [`crate::quality_profile::resolution_lines`], the
/// same parser the profile lookup normalizes with, so the two cannot disagree
/// about what a label says. A label that names no resolution ranks last, which
/// is the only honest answer — but it is now genuinely rare rather than "any
/// label that does not end in `p`", which used to bury `1080i` and every
/// Sonarr-style compound below 480p.
#[doc(hidden)] // fork: parity net seam (was private; tests/props_quality.rs)
pub fn resolution_rank(quality: &str) -> u32 {
    crate::quality_profile::resolution_lines(Some(quality))
        .map_or(u32::MAX, |lines| u32::MAX - lines)
}

impl AppUseCase {
    /// The [`CutoffScope`] a submission scope stands for.
    ///
    /// Resolves a season's member episodes, which is the one case that needs a
    /// catalog read; every other scope is a field rename.
    pub(crate) async fn cutoff_scope_for(&self, scope: &crate::SubmissionScope) -> CutoffScope {
        use crate::SubmissionScope;
        match scope {
            SubmissionScope::Episode { episode_id } => CutoffScope::Episode(episode_id.clone()),
            SubmissionScope::EpisodeSet { episode_ids } => {
                CutoffScope::Episodes(episode_ids.clone())
            }
            SubmissionScope::SeriesMovie {
                series_movie_link_id,
            } => CutoffScope::SeriesMovieLink(series_movie_link_id.clone()),
            SubmissionScope::Collection { collection_id } => CutoffScope::Episodes(
                self.services
                    .catalog
                    .shows
                    .list_episodes_for_collection(collection_id)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|episode| episode.monitored)
                    .map(|episode| episode.id)
                    .collect(),
            ),
            SubmissionScope::Title | SubmissionScope::Orphan => CutoffScope::Title,
        }
    }

    /// The quality profile that governs a title, and nothing else.
    ///
    /// [`Self::resolve_upgrade_context_for_title_with_category_and_quality`] also
    /// resolves the persona thresholds and the cutoff verdict, which a read-only
    /// caller — re-deriving an incumbent's bar for display — does not need and
    /// should not pay for.
    pub(crate) async fn resolve_quality_profile_for_title(
        &self,
        title: &Title,
    ) -> AppResult<QualityProfile> {
        self.resolve_quality_profile_for_title_with_category(title, None)
            .await
    }

    /// The profile half of
    /// [`Self::resolve_upgrade_context_for_title_with_category_and_quality`]:
    /// the same lookup (title tag, category override, library, global) under
    /// the same category, without the persona and acquisition thresholds.
    ///
    /// The full upgrade context resolves its profile through this helper, so a
    /// caller that needs only the profile — the convergence fingerprint — can
    /// never resolve a different one than the grab path would.
    pub(crate) async fn resolve_quality_profile_for_title_with_category(
        &self,
        title: &Title,
        category_hint: Option<&str>,
    ) -> AppResult<QualityProfile> {
        self.resolve_quality_profile(QualityProfileLookup {
            title_tags: &title.tags,
            library_id: Some(title.library_id.as_str()),
            imdb_id: title.imdb_id.as_deref(),
            tvdb_id: tvdb_id_from_external_ids(&title.external_ids).as_deref(),
            category_hint: Some(upgrade_context_category(title, category_hint)),
        })
        .await
    }

    pub(crate) async fn resolve_upgrade_context_for_title_with_category_and_quality(
        &self,
        title: &Title,
        category_hint: Option<&str>,
        analyzed_quality: Option<&str>,
    ) -> AppResult<ResolvedUpgradeContext> {
        // Resolution failures propagate: scoring against a substitute profile
        // silently makes the wrong upgrade decision, which is exactly the
        // failure mode the strict resolver exists to prevent.
        let profile = self
            .resolve_quality_profile_for_title_with_category(title, category_hint)
            .await?;

        // **The cutoff is a fact about files.** It used to fall back to parsing
        // the anchor state row's `grabbed_release` when no file supplied a
        // quality, which is wrong twice over: a grab that has not landed says
        // nothing about what the scope holds, and for a multi-member scope
        // `analyzed_cutoff_quality_for_scope` returns `None` the moment *one*
        // member is empty — so a season with eleven 1080p episodes and a missing
        // twelfth read the first episode's grabbed release, reported "cutoff
        // reached", and the pack that would have filled E12 was never evaluated.
        //
        // "A release for this scope is already in flight" is a real question, and
        // it now has a real answer: D18's queued pseudo-incumbents, which compare
        // the in-flight release on the full tier → revision → score ladder
        // instead of asking whether it happened to clear the cutoff.
        //
        // A score-first profile has no quality half: its cutoff is
        // `cutoff_score` alone, and none means upgrades never stop on a cutoff.
        let cutoff_reached = profile.criteria.quality_cutoff_met(analyzed_quality);

        // The same-tier delta belongs to the profile that judges the title.
        let thresholds = AcquisitionThresholds::for_profile(&profile.criteria);

        Ok(ResolvedUpgradeContext {
            profile,
            thresholds,
            cutoff_reached,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use scryer_domain::{MediaFacet, Title};

    fn make_title(facet: MediaFacet) -> Title {
        Title {
            id: "title-1".to_string(),
            name: "Example".to_string(),
            library_id: scryer_domain::default_library_id_for_facet(&facet),
            root_folder_id: scryer_domain::root_folder_id_for_path("/data/test"),
            facet,
            monitored: true,
            tags: vec![],
            canonical_tags: vec![],
            external_ids: vec![],
            created_by: None,
            created_at: Utc::now(),
            year: None,
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
            folder_path: None,
        }
    }

    #[test]
    fn upgrade_context_category_prefers_explicit_hint() {
        let title = make_title(MediaFacet::Movie);
        assert_eq!(upgrade_context_category(&title, Some("anime")), "anime");
    }

    #[test]
    fn upgrade_context_category_falls_back_to_facet_for_blank_hint() {
        let title = make_title(MediaFacet::Series);
        assert_eq!(upgrade_context_category(&title, Some("  ")), "series");
    }
}
