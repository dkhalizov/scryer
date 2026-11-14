//! Parity net, stream `decision_context`: the specifications Scryer decides
//! inside the auto-candidate evaluator (`evaluate_auto_candidate_with_step`) or
//! the already-imported guard (`candidate_matches_existing_media_file`).
//!
//! Suites (nextest names `decision_context::<fn>`):
//!
//! - `son_decision_context` (27 IDs): SON-DECISION-0040..0048
//!   (AlreadyImported), 0089 (FullSeason, non-season release), 0105, 0106,
//!   0107, 0113 (MonitoredEpisode, multi-episode and pack cases), 0114,
//!   0115 (MultiSeason), 1219, 1220 (Search.Series), 1229..1231 (AnimeSearch),
//!   1232..1237 (StandardEpisodeSearch).
//! - `rad_decision_context` (11 IDs): RAD-DECISION-0012..0020
//!   (AlreadyImported), 0097, 0098 (MovieSpecification).
//!
//! Selection (before `wants`, so adoption never takes foreign IDs):
//! MonitoredEpisode: `series.monitored == true` and (`release.full_season` or
//! every episode list of length >= 2; a single-episode subject never reaches
//! the gate, see Unmapped). FullSeason: `release.full_season != true`
//! (the root `full_season()` skips exactly these). Every other file: all cases.
//! Radarr: MovieSpecification and AlreadyImported only.
//!
//! # Scryer functions under test
//!
//! - `evaluate_auto_candidate_with_step` (seam) with an `AutoCandidateEvaluationContext`
//!   built from struct literals (the candidate carries `parse_release_metadata` of
//!   its title, the production parser),
//! - `candidate_matches_existing_media_file` (seam): the anti-loop guard,
//! - `evaluate_admission` + `episode_is_unaired` (FullSeason 0089, no seam).
//!
//! # Translation table (reference term -> Scryer term)
//!
//! Concern codes. The evaluator returns one code for the whole grab, so each
//! spec compares only its own concern: accepted = the code is not in the set.
//!
//! | Reference spec | Refusal codes (`ReleaseAutoDecisionCode`) |
//! |---|---|
//! | MonitoredEpisode | `EpisodeNotMonitored` |
//! | Search.Series, MovieSpecification | `TitleMismatch` |
//! | SingleEpisodeSearchMatch (Anime, Standard) | `EpisodeMismatch`, `PackForSingleEpisode` (Sonarr's WrongSeason / WrongEpisode / FullSeason) |
//! | AlreadyImported | the guard returns true |
//!
//! Gate-order guard: the evaluator's gates run in the order SubtitlesOnly,
//! NumberingPackUnresolved, ParseAmbiguous, ParseUnparseable, TitleYearMismatch,
//! EpisodicReleaseForMovie, TitleMismatch, EpisodeMismatch, PackForSingleEpisode,
//! EpisodeNotMonitored (release_search.rs:2326-2417). A code returned from a gate
//! that runs before the concern's first code is recorded red ("decided by X
//! before the gate"), so "not refused" cannot be vacuous. This guard only turns a
//! result red; it never decides acceptance.
//!
//! Names: series/movie id 1 -> "Amber Circuit", 2 -> "Northern Lantern" (two
//! words, so no alias sets requires_external_id); none -> 1. `Title.id` and
//! `subject.title_id` are `title-<N>`; `Title.monitored` = series.monitored.
//!
//! Release titles (harness glue, dots for spaces): one episode
//! `<Name>.S01E01.720p.HDTV.x264-GRP`; two `<Name>.S01E01E02...`; full season
//! `<Name>.S01.720p.HDTV.x264-GRP`; Standard `<Name>.S{season:02}` + one
//! `E{n:02}` per `release.episodes` (none = pack); movie
//! `<Name>.2018.1080p.WEB-DL.x264-GRP`. FullSeason/MultiSeason `release.title`
//! contradicts the flags on purpose and is ignored.
//!
//! Search -> lane/scope: `search_criteria` null -> RSS (`is_rss_lane`,
//! `subject_kind` Rss); Single/AnimeEpisodeSearchCriteria -> automatic episode
//! search (`Episode{e1}`, `user_invoked` false, season/episode from the search,
//! none for Anime); Anime `is_season_search` -> `Collection{c1}`, season 1.
//! Movie: `Title` scope, facet Movie, year 2018, not RSS, `user_invoked` false
//! (both cases are automatic searches; `search.kind` is not read).
//! MonitoredEpisode coverage (subject scope and candidate `coverage_scope`): 1
//! episode -> `Episode{e1}`; 2 -> `EpisodeSet{e1,e2}` (season/episode none);
//! `full_season` -> `Collection{c1}`. An episode with `monitored` false puts
//! `e<i+1>` in `unmonitored_episode_ids`. Facet Series (Anime for AnimeSearch).
//!
//! AlreadyImported: a stored file (`episode_file_id`/`movie_file_id` > 0) is an
//! `Incumbent` `f<id>` (covering `e1` for Sonarr) plus a `MediaFileCutoffSummary`.
//! History -> `grabbed_release_title`: the first "Grabbed" row; when a
//! "DownloadFolderImported" row has the same `download_id` (null equals null,
//! as in C#) the file gets that grab's `source_title`, else none (a Scryer file
//! knows its grab title only if imported from that grab). Candidate title =
//! `release.title` or "Title1" (NBuilder default); `release.protocol` ->
//! `source_kind`; `info_hash` -> `extra["info_hash"]`.
//! `enable_completed_download_handling == false` -> red (no Scryer setting).
//!
//! FullSeason 0089: a non-season release over 3 episodes is a batch, which a
//! grab resolves to `EpisodeSet` coverage; the production subject builder makes
//! an `EpisodeSet` grab `.per_member().with_unaired_members(n)`
//! (quality/canonical_context.rs:555-578, `is_pack_grab`), the same shape as the
//! root `full_season()`. `n` from `episode_is_unaired`; accepted = reason is not
//! `SeasonIncomplete`.
//!
//! # Ignored keys
//!
//! `release.mapped_season` (the reference spec never reads it), `search.kind`
//! beyond the mapping above (Movie: `search.kind`, `search.user_invoked`), `note`;
//! MultiSeason: every input key (`release.*`, `episodes[].air_date_offset_hours`;
//! red by reason, nothing computed); AlreadyImported `episodes[].season/number`,
//! `release.quality/revision`, `parsed_quality`, `history[].quality/revision`
//! (Scryer's guard is quality-blind), `movie.id`, `release.protocol` (no effect
//! on the guard), `info_hash` (carried on the candidate only; stored files have
//! no hash).
//!
//! # Known reds
//!
//! son 0089 (Scryer applies the not-yet-aired check to every multi-episode batch
//! grab, Sonarr's FullSeasonSpecification only to full-season releases), 0113
//! (Scryer exempts Collection from the unmonitored gate), 0114 and 0115
//! (no multi-season rule, red by reason), 0040 and rad 0012 (no completed
//! download handling switch), 0047, 0048, rad 0019, 0020 (the guard matches
//! release names only, stored files carry no info hash).
//!
//! Vacuity warning: the AlreadyImported greens show only that the name guard does
//! not fire on a release named "Title1"; no fixture names a release by the grabbed
//! title, so they cannot catch a guard that never fires. The MonitoredEpisode
//! greens are the all-monitored or refused `EpisodeSet` shapes only.
//!
//! # Unmapped (tests: []), with reasons
//!
//! - SON-DECISION-0103, 0108: series monitoring. Scryer never reads
//!   `Title.monitored` in the grab evaluator; an unmonitored title is skipped in
//!   the RSS loop (rss.rs:2174, AppUseCase with the catalog repository) and in
//!   wanted-target derivation. A mapped case would echo the input.
//! - SON-DECISION-0102, 0104, 0109, 0110, 0111, 0112: single-episode monitoring is
//!   target derivation and the RSS scope filter (`rss_episode_scope_is_target`,
//!   rss.rs:2790), not the grab gate (the gate binds only `EpisodeSet` coverage,
//!   release_search.rs:2395-2417), so the evaluator accepts a single-episode
//!   subject whatever its flags. 0102 asserts one single-episode and one
//!   two-episode subject in one test, so half its comparison would be constant.
//!   Search-lane monitoring is likewise target derivation. All need an AppUseCase.
//! - RAD-DECISION-0073..0076: movie monitoring; `Title.monitored` is read only by
//!   the RSS loop and target derivation (AppUseCase with repositories).
//! - SON-DECISION-1183..1188 (DeletedEpisodeFile): Scryer has no grab-time check
//!   that an incumbent file exists on disk and no setting to unmonitor episodes
//!   whose file went missing; liveness comes from scan-maintained rows
//!   (`live_media_files_for_episode_ids`, AppUseCase).

use std::collections::HashSet;

use scryer_application::parity_seams::{
    AdmissionPolicy, AdmissionRejectionReason, AdmissionScope, AdmissionSubject,
    AutoCandidateEvaluationContext, CandidateFacts, Incumbent, ReleaseAutoDecisionCode,
    ResolvedReleaseSearchSubject, TitleReleaseBlocklistSignatures, candidate_matches_existing_media_file,
    canonical_title_evidence, episode_is_unaired, evaluate_admission, evaluate_auto_candidate_with_step,
};
use scryer_application::{
    AcquisitionThresholds, AudioLanguageRank, IndexerSearchNumberingContext, IndexerSearchResult,
    MediaFileCutoffSummary, ReleaseSearchSubjectKind, SubmissionScope, builtin_1080p_profile,
    parse_release_metadata,
};
use scryer_domain::{MediaFacet, Title};
use scryer_parity::{Suite, corpus};
use serde_json::{Value, json};

use ReleaseAutoDecisionCode as C;

#[test]
fn son_decision_context() {
    let Some(mut suite) = Suite::open("scryer-application::parity_decision::son_decision_context") else { return };
    let dir = "corpus/reference/sonarr/decision/";
    for stem in [
        "MonitoredEpisodeSpecificationFixture",
        "Search.SeriesSpecificationFixture",
        "Search.SingleEpisodeSearchMatchSpecificationTests.AnimeSearchFixture",
        "Search.SingleEpisodeSearchMatchSpecificationTests.StandardEpisodeSearch",
        "MultiSeasonSpecificationFixture",
        "FullSeasonSpecificationFixture",
        "AlreadyImportedSpecificationFixture",
    ] {
        let cases: Vec<Value> = corpus(&format!("{dir}{stem}.json"));
        for case in &cases {
            let g = &case["input"]["given"];
            let selected = match stem {
                "MonitoredEpisodeSpecificationFixture" => {
                    g["series"]["monitored"].as_bool() == Some(true)
                        && (g["release"]["full_season"].as_bool() == Some(true)
                            || episode_lists(g).iter().all(|l| l.len() >= 2))
                }
                "FullSeasonSpecificationFixture" => g["release"]["full_season"].as_bool() != Some(true),
                _ => true,
            };
            let id = case["id"].as_str().unwrap();
            if !selected || !suite.wants(id) {
                continue;
            }
            let outcome = match stem {
                "MonitoredEpisodeSpecificationFixture" => monitored_episode(g),
                "Search.SeriesSpecificationFixture" => search_series(g),
                "MultiSeasonSpecificationFixture" => Err(
                    "no Scryer rule refuses multi-season releases (MultiSeasonSpecification); Scryer takes them through coverage resolution and the series-pack missing-ratio threshold inside private scoring (catalog/release_search.rs:321)"
                        .into(),
                ),
                "FullSeasonSpecificationFixture" => full_season_span(g),
                "AlreadyImportedSpecificationFixture" => already_imported(g, false),
                _ => single_episode_search(g, stem.ends_with("AnimeSearchFixture")),
            };
            super::record(&mut suite, id, &case["expect"], Some(outcome));
        }
    }
    suite.finish();
}

#[test]
fn rad_decision_context() {
    let Some(mut suite) = Suite::open("scryer-application::parity_decision::rad_decision_context") else { return };
    let dir = "corpus/reference/radarr/decision/";
    for stem in ["MovieSpecificationFixture", "AlreadyImportedSpecificationFixture"] {
        let cases: Vec<Value> = corpus(&format!("{dir}{stem}.json"));
        for case in &cases {
            let id = case["id"].as_str().unwrap();
            if !suite.wants(id) {
                continue;
            }
            let g = &case["input"]["given"];
            let outcome = if stem.starts_with("Movie") { movie_spec(g) } else { already_imported(g, true) };
            super::record(&mut suite, id, &case["expect"], Some(outcome));
        }
    }
    suite.finish();
}

// ── Concern comparison ─────────────────────────────────────────────────────

const GATE_ORDER: [C; 10] = [
    C::SubtitlesOnly,
    C::NumberingPackUnresolved,
    C::ParseAmbiguous,
    C::ParseUnparseable,
    C::TitleYearMismatch,
    C::EpisodicReleaseForMovie,
    C::TitleMismatch,
    C::EpisodeMismatch,
    C::PackForSingleEpisode,
    C::EpisodeNotMonitored,
];

/// `Ok(accepted)`; `Err` when an earlier gate decided (red, never vacuous).
fn concern_outcome(code: C, concern: &[C], spec: &str) -> Result<bool, String> {
    if concern.contains(&code) {
        return Ok(false);
    }
    let pos = |c: C| GATE_ORDER.iter().position(|x| *x == c);
    let first = concern.iter().filter_map(|c| pos(*c)).min().unwrap();
    if pos(code).is_some_and(|p| p < first) {
        return Err(format!("decided by {code:?} before the {spec} gate"));
    }
    Ok(true)
}

// ── Evaluator harness ──────────────────────────────────────────────────────

struct Params {
    facet: MediaFacet,
    series_id: i64,
    monitored: bool,
    year: Option<i32>,
    release_title: String,
    scope: SubmissionScope,
    coverage: Option<SubmissionScope>,
    season: Option<u32>,
    episode: Option<u32>,
    kind: ReleaseSearchSubjectKind,
    rss: bool,
    unmonitored: HashSet<String>,
}

fn name(id: i64) -> &'static str {
    if id == 2 { "Northern Lantern" } else { "Amber Circuit" }
}

fn dotted(id: i64) -> String {
    name(id).replace(' ', ".")
}

fn evaluate(p: &Params) -> C {
    let mut title = title(p.facet.clone(), name(p.series_id), p.monitored);
    title.id = format!("title-{}", p.series_id);
    title.year = p.year;
    let subject = ResolvedReleaseSearchSubject {
        title_id: title.id.clone(),
        title_tags: title.tags.clone(),
        title_evidence: canonical_title_evidence(&title),
        queries: vec![title.name.clone()],
        imdb_id: None,
        tmdb_id: None,
        tvdb_id: None,
        anidb_id: None,
        mal_id: None,
        category: title.facet.as_str().to_string(),
        owner_facet: title.facet.clone(),
        search_facet: title.facet.clone(),
        id_search_facet: None,
        newznab_categories: Vec::new(),
        runtime_minutes: title.runtime_minutes,
        season: p.season,
        episode: p.episode,
        numbering_context: IndexerSearchNumberingContext::default(),
        absolute_episode: None,
        subject_kind: p.kind,
        submission_scope: p.scope.clone(),
        pre_release: false,
        episode_search_not_before: None,
    };
    let mut candidate = candidate(&p.release_title);
    candidate.coverage_scope = p.coverage.clone();
    let admission = AdmissionSubject::new(AdmissionScope::Title, Vec::new());
    let profile = builtin_1080p_profile();
    let thresholds = AcquisitionThresholds::default();
    let now = super::now();
    let db_blocklist = TitleReleaseBlocklistSignatures::default();
    let minimum_seeders = Default::default();
    let context = AutoCandidateEvaluationContext {
        title: &title,
        subject: &subject,
        admission: &admission,
        profile: &profile,
        thresholds: &thresholds,
        incumbent_at_cutoff: false,
        is_rss_lane: p.rss,
        user_invoked: false,
        oldest_overlapping_pending_published_at: None,
        now: &now,
        dl_snapshot: None,
        db_blocklist: &db_blocklist,
        existing_files: &[],
        delay_profiles: &[],
        failed_routes: None,
        minimum_seeders: &minimum_seeders,
        unmonitored_episode_ids: &p.unmonitored,
    };
    evaluate_auto_candidate_with_step(&candidate, &context).0
}

fn series_params(series_id: i64, release_title: String) -> Params {
    Params {
        facet: MediaFacet::Series,
        series_id,
        monitored: true,
        year: None,
        release_title,
        scope: SubmissionScope::Episode { episode_id: "e1".into() },
        coverage: None,
        season: Some(1),
        episode: Some(1),
        kind: ReleaseSearchSubjectKind::Episode,
        rss: false,
        unmonitored: HashSet::new(),
    }
}

fn candidate(release_title: &str) -> IndexerSearchResult {
    IndexerSearchResult {
        indexer_id: None,
        source: "parity".into(),
        title: release_title.into(),
        link: None,
        download_url: None,
        source_kind: None,
        size_bytes: None,
        published_at: None,
        thumbs_up: None,
        thumbs_down: None,
        indexer_languages: None,
        indexer_subtitles: None,
        indexer_grabs: None,
        password_hint: None,
        parsed_release_metadata: Some(parse_release_metadata(release_title)),
        quality_profile_decision: None,
        extra: Default::default(),
        response_attributes: Default::default(),
        guid: None,
        info_url: None,
        provenance: None,
        candidate_token: None,
        queue_scope: None,
        coverage_scope: None,
        auto_eligible: None,
        auto_decision_code: None,
        auto_decision_summary: None,
        release_listing_json: None,
        http: None,
    }
}

fn title(facet: MediaFacet, name: &str, monitored: bool) -> Title {
    Title {
        id: "parity-title".into(),
        library_id: scryer_domain::default_library_id_for_facet(&facet),
        name: name.into(),
        facet,
        monitored,
        tags: vec![],
        external_ids: vec![],
        root_folder_id: scryer_domain::root_folder_id_for_path("/parity"),
        created_by: None,
        created_at: super::now(),
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
        folder_path: None,
    }
}

// ── Cases ──────────────────────────────────────────────────────────────────

fn episode_lists(g: &Value) -> Vec<Vec<Value>> {
    if let Some(subjects) = g["subjects"].as_array() {
        subjects.iter().map(|s| s["episodes"].as_array().cloned().unwrap_or_default()).collect()
    } else {
        vec![g["episodes"].as_array().cloned().unwrap_or_default()]
    }
}

fn monitored_one(series_id: i64, monitored: bool, full_season: bool, episodes: &[Value]) -> Result<bool, String> {
    let n = dotted(series_id);
    let (release_title, scope, season, episode) = if full_season {
        (format!("{n}.S01.720p.HDTV.x264-GRP"), SubmissionScope::Collection { collection_id: "c1".into() }, Some(1), None)
    } else if episodes.len() >= 2 {
        let ids = (1..=episodes.len()).map(|i| format!("e{i}")).collect();
        (format!("{n}.S01E01E02.720p.HDTV.x264-GRP"), SubmissionScope::EpisodeSet { episode_ids: ids }, None, None)
    } else {
        (format!("{n}.S01E01.720p.HDTV.x264-GRP"), SubmissionScope::Episode { episode_id: "e1".into() }, Some(1), Some(1))
    };
    let mut p = series_params(series_id, release_title);
    p.monitored = monitored;
    p.coverage = Some(scope.clone());
    p.scope = scope;
    p.season = season;
    p.episode = episode;
    p.kind = ReleaseSearchSubjectKind::Rss;
    p.rss = true;
    p.unmonitored = episodes
        .iter()
        .enumerate()
        .filter(|(_, e)| e["monitored"].as_bool() == Some(false))
        .map(|(i, _)| format!("e{}", i + 1))
        .collect();
    concern_outcome(evaluate(&p), &[C::EpisodeNotMonitored], "MonitoredEpisode")
}

fn monitored_episode(g: &Value) -> Result<Value, String> {
    let monitored = g["series"]["monitored"].as_bool().unwrap_or(true);
    let full = g["release"]["full_season"].as_bool() == Some(true);
    let eps = episode_lists(g).remove(0);
    Ok(super::accepted(monitored_one(1, monitored, full, &eps)?))
}

fn search_series(g: &Value) -> Result<Value, String> {
    let release_id = g["release"]["series_id"].as_i64().unwrap_or(1);
    let search_id = g["search"]["series_id"].as_i64().unwrap_or(1);
    // The subject is the searched series; the release names the release's series.
    let mut p = series_params(search_id, format!("{}.S01E01.720p.HDTV.x264-GRP", dotted(release_id)));
    p.season = Some(1);
    Ok(super::accepted(concern_outcome(evaluate(&p), &[C::TitleMismatch], "Search.Series")?))
}

fn single_episode_search(g: &Value, anime: bool) -> Result<Value, String> {
    let (release, search) = (&g["release"], &g["search"]);
    let n = dotted(1);
    let mut p;
    if anime {
        let season_search = search["is_season_search"].as_bool() == Some(true);
        let full = release["full_season"].as_bool() == Some(true);
        let release_title =
            if full { format!("{n}.S01.720p.HDTV.x264-GRP") } else { format!("{n}.S01E01.720p.HDTV.x264-GRP") };
        p = series_params(1, release_title);
        p.facet = MediaFacet::Anime;
        p.season = None;
        p.episode = None;
        if season_search {
            p.scope = SubmissionScope::Collection { collection_id: "c1".into() };
            p.season = Some(1);
            p.kind = ReleaseSearchSubjectKind::Season;
        }
    } else {
        let season = release["season"].as_u64().unwrap_or(1);
        let eps: String = release["episodes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| format!("E{:02}", e.as_u64().unwrap_or(0)))
            .collect();
        p = series_params(1, format!("{n}.S{season:02}{eps}.720p.HDTV.x264-GRP"));
        p.season = search["season"].as_u64().map(|v| v as u32);
        p.episode = search["episode"].as_u64().map(|v| v as u32);
    }
    let code = evaluate(&p);
    Ok(super::accepted(concern_outcome(
        code,
        &[C::EpisodeMismatch, C::PackForSingleEpisode],
        "SingleEpisodeSearchMatch",
    )?))
}

fn movie_spec(g: &Value) -> Result<Value, String> {
    let release_id = g["movie"]["id"].as_i64().unwrap_or(1);
    let search_id = g["search"]["movie_id"].as_i64().unwrap_or(1);
    let mut p = series_params(search_id, format!("{}.2018.1080p.WEB-DL.x264-GRP", dotted(release_id)));
    p.facet = MediaFacet::Movie;
    p.year = Some(2018);
    p.scope = SubmissionScope::Title;
    p.kind = ReleaseSearchSubjectKind::Title;
    p.season = None;
    p.episode = None;
    Ok(super::accepted(concern_outcome(evaluate(&p), &[C::TitleMismatch], "MovieSpecification")?))
}

fn full_season_span(g: &Value) -> Result<Value, String> {
    let now = super::now();
    let episodes = g["episodes"].as_array().cloned().unwrap_or_default();
    let members: Vec<String> = (0..episodes.len()).map(|i| format!("e{i}")).collect();
    let unaired = episodes
        .iter()
        .filter(|e| {
            let air = e["air_date_offset_hours"]
                .as_i64()
                .map(|h| (now + chrono::Duration::hours(h)).to_rfc3339());
            episode_is_unaired(air.as_deref(), &now)
        })
        .count();
    // EpisodeSet grab shape of quality/canonical_context.rs (is_pack_grab).
    let subject = AdmissionSubject::new(AdmissionScope::Episodes(members), Vec::new())
        .per_member()
        .with_unaired_members(unaired);
    let policy = AdmissionPolicy {
        allow_upgrades: true,
        min_delta: 0,
        cutoff_score: None,
        manual_override: false,
        applies_to_queue: true,
    };
    let verdict = evaluate_admission(&subject, CandidateFacts::new(None, 0, 0), &policy);
    Ok(super::accepted(!matches!(
        verdict.rejection().map(|r| &r.reason),
        Some(AdmissionRejectionReason::SeasonIncomplete)
    )))
}

fn already_imported(g: &Value, radarr: bool) -> Result<Value, String> {
    if g["config"]["enable_completed_download_handling"].as_bool() == Some(false) {
        return Err("no Scryer setting to turn off completed download handling".into());
    }
    let file_id = if radarr {
        g["movie"]["movie_file_id"].as_i64()
    } else {
        g["episodes"][0]["episode_file_id"].as_i64()
    }
    .unwrap_or(0);

    // Grab title known to the stored file: the first Grabbed row, only when an
    // import row carries the same download id (null == null).
    let history = g["history"].as_array().cloned().unwrap_or_default();
    let grab = history.iter().find(|h| h["event"].as_str() == Some("Grabbed"));
    let grabbed_release_title = grab
        .filter(|gr| {
            history
                .iter()
                .any(|h| h["event"].as_str() == Some("DownloadFolderImported") && h["download_id"] == gr["download_id"])
        })
        .and_then(|gr| gr["source_title"].as_str())
        .map(str::to_string);

    let episode = (!radarr).then(|| "e1".to_string());
    let (scope, incumbents, files) = if file_id > 0 {
        let id = format!("f{file_id}");
        let path = format!("/parity/{id}.mkv");
        let inc = Incumbent {
            tier_index: None,
            revision: 0,
            file_id: id.clone(),
            file_path: path.clone(),
            release_group: None,
            score: 0,
            language: AudioLanguageRank::default(),
            covers: episode.iter().cloned().collect(),
            created_at: super::now().to_rfc3339(),
        };
        let file = MediaFileCutoffSummary {
            id,
            file_path: path,
            original_file_path: None,
            scene_name: None,
            grabbed_release_title,
            episode_id: episode.clone(),
            series_movie_link_ids: vec![],
            quality_label: None,
            created_at: super::now().to_rfc3339(),
        };
        let scope = if radarr { AdmissionScope::Title } else { AdmissionScope::Episodes(vec!["e1".into()]) };
        (scope, vec![(inc, true)], vec![file])
    } else {
        let scope = if radarr { AdmissionScope::Title } else { AdmissionScope::Episodes(vec!["e1".into()]) };
        (scope, vec![], vec![])
    };
    let subject = AdmissionSubject::new(scope, incumbents);

    let release = &g["release"];
    let mut cand = candidate(release["title"].as_str().unwrap_or("Title1"));
    cand.source_kind = super::source_kind(&release["protocol"]);
    if let Some(hash) = release["info_hash"].as_str() {
        cand.extra.insert("info_hash".into(), json!(hash));
    }
    Ok(super::accepted(!candidate_matches_existing_media_file(&cand, &files, &subject)))
}
