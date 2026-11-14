//! Parity net: grab-decision harness (notes/11-parity-net-design.md, phase 4).
//!
//! Six suites here, one test fn each; the submodules `parity_decision/decision_delay.rs`,
//! `parity_decision/decision_rank.rs`, `parity_decision/decision_absent.rs`, `parity_decision/decision_context.rs`, `parity_decision/decision_guards.rs` and `parity_decision/priv_decision.rs` (the private config entries) add their own (see their headers):
//!
//! | Suite | Corpus |
//! |---|---|
//! | `son_decision` / `rad_decision` | `parity/corpus/reference/<app>/decision/*.json` (0001.., 1000..) |
//! | `son_decision_config` / `rad_decision_config` | catalogue entries with `source: config` (5001..); the check is built from the JSON their `ref` points at |
//! | `son_decision_captured` / `rad_decision_captured` | `parity/corpus/captured/<app>/data/cases/decision.json` (6001..) |
//!
//! Every mapped case compares a real Scryer output with the corpus `expect`.
//! A case whose input names something Scryer has no field for is recorded as
//! a failure with the reason (red); a case whose concept Scryer lacks entirely
//! is not recorded (stays `unmapped`, reasons below).
//!
//! # Scryer functions under test
//!
//! - `evaluate_profile_requirements` + `apply_min_score_gate` (profile
//!   requirements, min format score),
//! - `analyzed_cutoff_quality_for_scope` -> `QualityProfileCriteria::quality_cutoff_met`
//!   -> `incumbent_at_cutoff` -> `cutoff_refusal` -> `evaluate_admission`: the grab
//!   lane's cutoff and upgrade ladder, called in the order
//!   `release_search::evaluate_auto_candidate_recording` calls them, with the grab
//!   `AdmissionPolicy` it builds (min delta from `AcquisitionThresholds::for_profile`),
//! - `DelayProfile::evaluate_delay_decision_with_context`, `resolve_delay_profile`,
//! - `meets_minimum_seeders`, `episode_is_unaired`, `parse_release_metadata`.
//!
//! # Translation table (reference model -> Scryer)
//!
//! Quality name -> (resolution, source, flags). A Scryer tier is a bare
//! resolution (`1080P`: every source not listed at it) or a source key
//! (`WEBDL-1080P`), built by the product's `quality_tier_label` from the
//! resolution and the source spelled as `parsed_release_source_type` does
//! (`Remux` when `is_remux`), ledger D48. Per case a resolution is *split* into
//! source keys only when (D54) two named qualities at it (release, files, queue
//! items, cutoff) have different keys, or a named one is outside the explicit
//! allowed list while another allowed one at that resolution is in it. A split
//! resolution lists the key of each allowed quality there, ranked by its
//! position in the allowed list (the reference item order), and no bare entry;
//! colliding keys (DVD, DVD-R, REGIONAL -> `DVD-480P`; HDTV-1080p, Raw-HD ->
//! `HDTV-1080P`) take the rank of the member the case names, else the best.
//! Other resolutions stay bare, and
//! resolutions are ordered highest first.
//!
//! | Reference quality | Scryer resolution | `ReleaseSource` | flags |
//! |---|---|---|---|
//! | Unknown | none (`allow_unknown_quality` when allowed) | none | |
//! | SDTV | 480P | Hdtv | |
//! | DVD, DVD-R, REGIONAL | 480P | Dvd | |
//! | WEBDL-480p / WEBRip-480p | 480P | WebDl / WebRip | |
//! | Bluray-480p / Bluray-576p | 480P / 576P | BluRay | |
//! | HDTV-720p, WEBDL-720p, WEBRip-720p, Bluray-720p | 720P | Hdtv, WebDl, WebRip, BluRay | |
//! | HDTV-1080p, Raw-HD | 1080P | Hdtv | |
//! | WEBDL/WEBRip/Bluray-1080p | 1080P | WebDl/WebRip/BluRay | |
//! | Remux-1080p, Bluray-1080p Remux | 1080P | BluRay | `is_remux` |
//! | HDTV/WEBDL/WEBRip/Bluray-2160p | 2160P | Hdtv/WebDl/WebRip/BluRay | |
//! | Remux-2160p, Bluray-2160p Remux | 2160P | BluRay | `is_remux` |
//! | BR-DISK | 1080P | BrDisk | `is_bd_disk`; allowed -> `allow_bd_disk` |
//! | CAM, TELESYNC, TELECINE, DVDSCR, WORKPRINT | none | Cam, Telesync, Telecine, DvdScr, Workprint | |
//!
//! Profile -> `QualityProfileCriteria` (starting from `builtin_1080p_profile()`):
//!
//! - `quality_tiers`: distinct tiers of the allowed qualities (split as
//!   above), best (highest resolution, then item order) first. Allowed = the
//!   explicit `allowed` / `items_allowed` list; `"all"` or no list = every
//!   quality of `qualities` / `items`, or
//!   every quality in the table above when the fixture lists none.
//! - `source_allowlist`: distinct sources of the allowed qualities, only when
//!   the fixture gives an explicit allowed list; empty for "all".
//! - `cutoff_tier`: tier (or split key) of `cutoff`; `Unknown` or absent (the
//!   reference model's default cutoff, quality id 0 = Unknown) -> the lowest
//!   tier.
//! - `allow_upgrades` = `upgrade_allowed`; `min_score_to_grab` = `min_format_score`;
//!   `cutoff_score` = `cutoff_format_score`; `min_upgrade_score_delta` =
//!   `min_upgrade_format_score`. Absent keys take the reference model's
//!   defaults: UpgradeAllowed false, MinFormatScore 0, CutoffFormatScore 0,
//!   MinUpgradeFormatScore 1.
//! - `downloadPropersAndRepacks`: PreferAndUpgrade (or absent) ->
//!   `set_ignore_proper_repack(false)`; DoNotPrefer -> `set_ignore_proper_repack(true)`;
//!   DoNotUpgrade -> no Scryer field: the case fails.
//! - Captured profiles (`config/qualityprofile.json`): allowed = quality items
//!   with `allowed` (a group's children when the group and child are allowed);
//!   `cutoff` id resolves to a quality id or a group id (group -> its first
//!   child); the other fields as above; propers from `mediamanagement.json`.
//!
//! Release / file facts:
//!
//! - Revision: reference `version` v -> Scryer `revision` v - 1 (Scryer 0 is an
//!   original). A `real` count > 0 has no Scryer field: the case fails.
//! - Custom formats: Scryer has no custom-format objects; a matched format's
//!   profile score is a rule score contribution (`ScoringSource::UserRule`)
//!   on the release decision, and a file's / release's score in the upgrade
//!   ladder is the summed format score (an explicit `custom_format_score`
//!   wins). Resolution bonus points are not added to ladder scores.
//! - Release group `""` or null -> `None`, except in the Proper family: the
//!   reference ProperSpecification never reads a group and its fixtures name
//!   none, while Scryer only counts a revision between same-group releases, so
//!   a group-less release and file both get the group `PARITY` (else Scryer's
//!   group gate, not the spec's old-file check, decided every case).
//! - Protocol usenet -> `DownloadSourceKind::NzbUrl`; torrent -> `TorrentFile`;
//!   `ReleaseInfo` (not a `TorrentInfo`) -> no source kind.
//! - Indexer id 0 -> no indexer id. Minimum seeders -> the threshold of the
//!   release's indexer, 0 when it has none.
//! - Delay profile: `usenet_delay`/`torrent_delay` -> minutes;
//!   `bypass_if_above_custom_format_score` -> `bypass_score_threshold =
//!   Some(minimum_custom_format_score)`; `order` -> `priority`; tag ids -> tag
//!   labels from `tag.json`. Global `minimumAge` -> every delay profile's
//!   `min_age_minutes`.
//! - Time: one fixed `NOW`; ages and air dates are offsets from it.
//!
//! Spec-only fixtures (a single reference specification asserted) compare
//! Scryer's refusal *for that concern* rather than the whole grab verdict:
//!
//! | Reference spec | Scryer counterpart (accepted = not refused by) |
//! |---|---|
//! | QualityAllowedByProfile | requirement codes `quality_not_in_profile_tiers`, `quality_missing_and_profile_disallows_unknown`, `source_*` |
//! | CustomFormatAllowedByProfile | `score_below_minimum`, `score_at_or_below_block_threshold` |
//! | RawDisk | `bd_disk_not_allowed` |
//! | Language (Radarr) | `required_audio_language_missing` |
//! | Protocol | `DelayDecisionReason::ProtocolDisabled` |
//! | MinimumAge | `DelayDecisionReason::MinimumAge` |
//! | Delay | `DelayDecisionReason::{PendingDelay, ReleaseAgeUnknown}` |
//! | TorrentSeeding | `meets_minimum_seeders` |
//! | UpgradeAllowed | `AdmissionRejectionReason::UpgradesDisabled` |
//! | Proper | `ReleaseAutoDecisionCode::ProperForOldFile` |
//! | SameEpisodes | `AdmissionRejectionReason::BroaderIncumbentSpan` |
//! | FullSeason | `AdmissionRejectionReason::SeasonIncomplete` (per-member pack subject, unaired count from `episode_is_unaired`) |
//! | Queue, Upgrade, UpgradeDisk, Repack, AnimeVersionUpgrade | whole grab verdict (cutoff gate + admission) |
//!
//! Upgrade `rejection` names: `CutoffReached` -> QualityCutoff,
//! `LowerQualityTier` -> BetterQuality, `LowerRevision` -> BetterRevision,
//! `UpgradesDisabled` -> UpgradesNotAllowed, `FormatCutoffReached` ->
//! CustomFormatCutoff, `NotAnUpgrade` -> CustomFormatScore when the candidate
//! does not beat the incumbent, else MinCustomFormatScore; admitted -> None.
//!
//! Captured cutoff: Sonarr `cutoff_met` is the file's `qualityCutoffNotMet`
//! flag negated, a quality-only cutoff, so it is compared with
//! `quality_cutoff_met`. Radarr `cutoff_not_met` likewise. Radarr
//! `in_wanted_cutoff` (file listed by `GET /wanted/cutoff`, monitored and
//! unmonitored) is quality-only too (`MovieCutoffService.cs:27-52`: quality ids
//! below `UpgradeAllowed ? Cutoff : FirstAllowedQuality`, no format score), as
//! is Scryer's cutoff-unmet list (`compute_cutoff_unmet_items_for_search`,
//! titles.rs:612), which past its guards (upgrades on, file tier in the
//! profile) is `!quality_cutoff_met`; when a guard fails the case fails with
//! the reason (that list needs repositories, not callable here).
//!
//! Ignored keys (no bearing on the Scryer call): `series.runtime` outside size
//! cases, `position_in_season`, `season_episodes`, `languages` on releases,
//! files and queue items (Scryer's language rank needs title preferences;
//! neutral rank used), `pending_releases` (always empty), `series_id` /
//! `movie.id` beyond queue matching, NBuilder id notes, `scene_name`,
//! `remote_movie`, `search.kind` except where named below.
//!
//! # Unmapped (not recorded; the catalogue keeps them `unmapped`)
//!
//! - MaximumSize, ReleaseRestrictions and IndexerTag are red (D29, no Scryer
//!   field) in `parity_decision/decision_absent.rs`. Retention, FreeSpace (grab
//!   time; the import check is mapped in `parity_import`), BlockedIndexer,
//!   DownloadDecisionMaker, SingleEpisodeAgeDownloadDecision and Pending stay
//!   unmapped, reasons in that file. Private config catalogue: `maximumSize`,
//!   `releaseprofile`, `tag.json`, `allowHardcodedSubs` and `preferIndexerFlags` are
//!   red by reason, `retention` and `autotagging` stay unmapped, see
//!   `parity_decision/priv_decision.rs`.
//! - Movie, Search.Series, SingleEpisodeSearchMatch, AlreadyImported, MultiSeason
//!   (red) and the MonitoredEpisode multi-episode / pack cases are mapped in
//!   `parity_decision/decision_context.rs` (seams `evaluate_auto_candidate_with_step`,
//!   `candidate_matches_existing_media_file`). MonitoredMovie, the MonitoredEpisode
//!   series-monitoring / single-episode cases and DeletedEpisodeFile stay unmapped,
//!   reasons in that file's header. Config minimum availability and
//!   `allowHardcodedSubs` are mapped in `priv_decision`; config `monitored` (title,
//!   season and single-episode monitoring, applied by target derivation and the RSS
//!   scope filter) stays unmapped (reason there).
//! - History (Sonarr RssSync.History, Radarr History): mapped in
//!   `parity_decision/decision_guards.rs` through the loader's row selection
//!   (`grabbed_release_record`, `recent_missing_grab_record`).
//! - PrioritizeDownloadDecision: the cases the search rank head decides are
//!   mapped in `parity_decision/decision_rank.rs` (seam
//!   `compare_release_search_results`). The rest (protocol, coverage, episode
//!   number, indexer priority, seeders/peers, usenet age) stay unmapped: they
//!   need `SearchRank`, built only inside the private `evaluate_candidate`
//!   (see that file's header). Config `downloadPropersAndRepacks` is mapped
//!   in `priv_decision` (the head carries the revision step); `preferIndexerFlags`
//!   is red by reason there (no flag step in the rank).
//! - Config `formatItems/format=N`: mapped red in `priv_decision` (Scryer's import
//!   derives no score), the same comparison as `parity_quality` `son_quality_config`.
//! - Delay cases whose outcome rests on "highest quality in profile" (bypass
//!   on) or "same-tier revision over the file on disk": mapped in
//!   `parity_decision/decision_delay.rs` (`son_decision_delay`,
//!   `rad_decision_delay`) through the `automatic_candidate_delay_decision`
//!   seam; the reference suites here still skip them.
//! - Spec applicability guards (Repack / Proper / AnimeVersionUpgrade cases where
//!   the reference spec does not evaluate): mapped in
//!   `parity_decision/decision_guards.rs` where the reference's whole pipeline
//!   gives the same answer; SON 1094, 1099, 0051, 0056 and RAD 1081, 1088 stay
//!   unmapped, reasons in that file's header. (FullSeason with a non-season
//!   release: mapped, red, in `decision_context`.)
//! - Queue cases with a queue item in a state other than Downloading: mapped in
//!   `parity_decision/decision_guards.rs` through `submission_is_queued`.
//!
//! # Product seams this file needs (visibility only, see the integrator's list)
//!
//! `scryer_application::parity_seams` re-exports the pub(crate) decision
//! pieces used here, and `QualityProfileCriteria::quality_cutoff_met` is `pub`.
//! Queue cases select queued submissions with the loader's own scope filter,
//! `submission_scope_intersects` (and `ScopeMembership`), widened to `pub` in
//! `acquisition/workflow/search.rs` for that re-export. `decision_context` uses `evaluate_auto_candidate_with_step` and
//! `candidate_matches_existing_media_file`; `decision_rank` also
//! uses `compare_release_search_results` (search rank head); `decision_delay` uses
//! `automatic_candidate_delay_decision`; `decision_guards` uses `submission_is_queued`,
//! `grabbed_release_record`, `recent_missing_grab_record` and `GrabbedReleaseRecord`
//! (plus `submission_scope_intersects` as the history rows' scope filter);
//! `priv_decision` uses `revision_preference`, `movie_availability_decision`,
//! `MovieAvailabilitySettings::policy` and `ReleaseAutoDecisionCode::with_movie_availability`.

use std::cmp::Reverse;

use chrono::{DateTime, Duration, Utc};
use scryer_application::parity_seams::{
    AdmissionPolicy, AdmissionRejectionReason, AdmissionScope, AdmissionSubject, CandidateFacts,
    CutoffScope, DelayDecisionReason, DelayPolicyContext, Incumbent, QueuedRelease,
    ReleaseAutoDecisionCode, ScopeMembership, analyzed_cutoff_quality_for_scope,
    apply_min_score_gate, cutoff_refusal, episode_is_unaired, evaluate_admission,
    evaluate_profile_requirements, incumbent_at_cutoff, meets_minimum_seeders,
    quality_tier_label, submission_scope_intersects,
};
use scryer_application::{
    AcquisitionThresholds, AudioLanguageRank, DelayProfile, DownloadSourceKind,
    MediaFileCutoffSummary, ParsedReleaseMetadata, PreferredProtocol, QualityProfile,
    QualityProfileDecision, ReleaseSource, ScoringSource, SubmissionScope, builtin_1080p_profile,
    parse_release_metadata, resolve_delay_profile,
};
use scryer_domain::MediaFacet;
use scryer_parity::{Catalogue, Source, Suite, corpus, parity_dir};
use serde_json::{Value, json};

#[path = "parity_decision/decision_delay.rs"]
mod decision_delay;
#[path = "parity_decision/decision_rank.rs"]
mod decision_rank;
#[path = "parity_decision/decision_absent.rs"]
mod decision_absent;
#[path = "parity_decision/decision_context.rs"]
mod decision_context;
#[path = "parity_decision/decision_guards.rs"]
mod decision_guards;
#[path = "parity_decision/priv_decision.rs"]
mod priv_decision;

const NOW: &str = "2026-01-15T12:00:00Z";

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(NOW).unwrap().with_timezone(&Utc)
}

/// `None`: not mapped (not recorded). `Some(Err)`: input Scryer has no field
/// for (recorded as a failure). `Some(Ok(actual))`: compared with `expect`.
type Outcome = Option<Result<Value, String>>;

fn record(suite: &mut Suite, id: &str, expect: &Value, outcome: Outcome) {
    match outcome {
        None => {}
        Some(Ok(actual)) => suite.check(id, expect.clone(), actual),
        Some(Err(why)) => suite.record(id, false, why),
    }
}

fn accepted(value: bool) -> Value {
    json!({ "accepted": value })
}

// ── Suites ─────────────────────────────────────────────────────────────────

#[test]
fn son_decision() {
    reference_suite("sonarr", "scryer-application::parity_decision::son_decision");
}

#[test]
fn rad_decision() {
    reference_suite("radarr", "scryer-application::parity_decision::rad_decision");
}

#[test]
fn son_decision_config() {
    config_suite("SON-", "scryer-application::parity_decision::son_decision_config");
}

#[test]
fn rad_decision_config() {
    config_suite("RAD-", "scryer-application::parity_decision::rad_decision_config");
}

#[test]
fn son_decision_captured() {
    let Some(mut suite) = Suite::open("scryer-application::parity_decision::son_decision_captured") else { return };
    let cases: Vec<Value> = corpus("corpus/captured/sonarr/data/cases/decision.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let input = &case["input"];
        let outcome = (|| -> Result<Value, String> {
            let profile_id = input["profile_id"].as_i64().ok_or("no profile_id")?;
            let profile = build_profile(&captured_spec("sonarr", profile_id)?, &[])?;
            let name = input["file_quality"]["name"].as_str().ok_or("no file quality")?;
            let tier = quality(name)?.tier;
            Ok(json!({ "cutoff_met": profile.criteria.quality_cutoff_met(tier) }))
        })();
        record(&mut suite, id, &case["expect"], Some(outcome));
    }
    suite.finish();
}

#[test]
fn rad_decision_captured() {
    let Some(mut suite) = Suite::open("scryer-application::parity_decision::rad_decision_captured") else { return };
    let cases: Vec<Value> = corpus("corpus/captured/radarr/data/cases/decision.json");
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        if !suite.wants(id) {
            continue;
        }
        let given = &case["input"]["given"];
        let outcome = (|| -> Result<Value, String> {
            let name = given["file_quality"].as_str().ok_or("no file quality")?;
            let spec = fixture_spec(&given["profile"], &Value::Null)?;
            let profile = build_profile(&spec, &[quality(name)?])?;
            let crit = &profile.criteria;
            let (version, real) = revision_of(&given["revision"]);
            if real > 0 {
                return Err("no Scryer field for a REAL revision".into());
            }
            let file = Side {
                q: quality(name)?,
                name: name.to_string(),
                revision: (version - 1) as i32,
                group: None,
                score: given["custom_format_score"].as_i64().unwrap_or(0) as i32,
            };
            let quality_met = crit.quality_cutoff_met(key(&file.q).as_deref());
            // Scryer's cutoff-unmet list (catalog/workflow/titles.rs:612) skips
            // profiles with upgrades off and files whose tier is not in the
            // profile; past those guards it is `!quality_cutoff_met`.
            let in_profile = tier_index(&profile, &file).is_some();
            if !crit.allow_upgrades || !in_profile {
                return Err(
                    "Scryer's cutoff-unmet list is not callable here; its guards (upgrades on, file tier in the profile) do not hold"
                        .into(),
                );
            }
            Ok(json!({ "cutoff_not_met": !quality_met, "in_wanted_cutoff": !quality_met }))
        })();
        record(&mut suite, id, &case["expect"], Some(outcome));
    }
    suite.finish();
}

fn reference_suite(app: &str, name: &str) {
    let Some(mut suite) = Suite::open(name) else { return };
    let dir = parity_dir().join(format!("corpus/reference/{app}/decision"));
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    for file in files {
        let stem = file.file_stem().unwrap().to_string_lossy().into_owned();
        let family = stem
            .trim_start_matches("RssSync.")
            .trim_start_matches("Search.")
            .to_string();
        let rel = format!("corpus/reference/{app}/decision/{stem}.json");
        let cases: Vec<Value> = corpus(&rel);
        for case in &cases {
            let id = case["id"].as_str().unwrap();
            if !suite.wants(id) {
                continue;
            }
            let outcome = reference_case(app, &family, &case["input"]["given"], &case["expect"]);
            record(&mut suite, id, &case["expect"], outcome);
        }
    }
    suite.finish();
}

fn reference_case(app: &str, family: &str, g: &Value, expect: &Value) -> Outcome {
    let has = |prefix: &str| family.starts_with(prefix);
    if has("QualityAllowedByProfileSpecification") {
        Some(quality_allowed(app, g))
    } else if has("CustomFormatAllowedByProfileSpecification") {
        Some(custom_format_allowed(app, g))
    } else if has("RawDiskSpecification") {
        Some(raw_disk(g))
    } else if has("LanguageSpecification") {
        Some(language(g))
    } else if has("ProtocolSpecification") {
        Some(protocol(g))
    } else if has("MinimumAgeSpecification") {
        Some(minimum_age(g))
    } else if has("DelaySpecification") {
        delay(app, g)
    } else if has("TorrentSeedingSpecification") {
        Some(torrent_seeding(app, g))
    } else if has("AcceptableSizeSpecification") {
        Some(Err(
            "no Scryer field for per-quality size limits (quality definition min/max MB per minute)"
                .into(),
        ))
    } else if has("UpgradeAllowedSpecification") {
        Some(upgrade_allowed(g))
    } else if has("UpgradeDiskSpecification") {
        Some(upgrade_disk(app, g))
    } else if has("UpgradeSpecification") {
        Some(upgradable(g, expect))
    } else if has("QueueSpecification") {
        queue(app, g)
    } else if has("RepackSpecification") {
        repack(app, g)
    } else if has("AnimeVersionUpgradeSpecification") {
        anime_version(g)
    } else if has("ProperSpecification") {
        proper(app, g)
    } else if has("FullSeasonSpecification") {
        full_season(g)
    } else if has("SameEpisodesSpecification") {
        Some(same_episodes(g))
    } else {
        // See the header: concept absent, or only reachable through the
        // whole auto-candidate evaluator.
        None
    }
}

// ── Translation ────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Q {
    tier: Option<&'static str>,
    source: Option<ReleaseSource>,
    remux: bool,
    bd_disk: bool,
    /// The reference name when it shares a key with another (`COLLIDING`).
    alias: Option<&'static str>,
}

/// Reference qualities that share a tier key (header, D54).
const COLLIDING: &[&str] = &["DVD", "DVD-R", "REGIONAL", "HDTV-1080p", "Raw-HD"];

/// Every quality the translation table knows, for "all qualities allowed"
/// when a fixture lists none.
const VOCAB: &[&str] = &[
    "Unknown", "SDTV", "WEBDL-480p", "WEBRip-480p", "DVD", "Bluray-480p", "Bluray-576p",
    "HDTV-720p", "WEBDL-720p", "WEBRip-720p", "Bluray-720p", "HDTV-1080p", "Raw-HD",
    "WEBDL-1080p", "WEBRip-1080p", "Bluray-1080p", "Remux-1080p", "HDTV-2160p", "WEBDL-2160p",
    "WEBRip-2160p", "Bluray-2160p", "Remux-2160p",
];

fn quality(name: &str) -> Result<Q, String> {
    use ReleaseSource::*;
    let q = |tier: Option<&'static str>, source: Option<ReleaseSource>| Q {
        tier,
        source,
        remux: false,
        bd_disk: false,
        alias: None,
    };
    let remux = |tier: &'static str| Q {
        tier: Some(tier),
        source: Some(BluRay),
        remux: true,
        bd_disk: false,
        alias: None,
    };
    let mut found = match name {
        "Unknown" => q(None, None),
        "SDTV" => q(Some("480P"), Some(Hdtv)),
        "DVD" | "DVD-R" | "REGIONAL" => q(Some("480P"), Some(Dvd)),
        "WEBDL-480p" => q(Some("480P"), Some(WebDl)),
        "WEBRip-480p" => q(Some("480P"), Some(WebRip)),
        "Bluray-480p" => q(Some("480P"), Some(BluRay)),
        "Bluray-576p" => q(Some("576P"), Some(BluRay)),
        "HDTV-720p" => q(Some("720P"), Some(Hdtv)),
        "WEBDL-720p" => q(Some("720P"), Some(WebDl)),
        "WEBRip-720p" => q(Some("720P"), Some(WebRip)),
        "Bluray-720p" => q(Some("720P"), Some(BluRay)),
        "HDTV-1080p" | "Raw-HD" => q(Some("1080P"), Some(Hdtv)),
        "WEBDL-1080p" => q(Some("1080P"), Some(WebDl)),
        "WEBRip-1080p" => q(Some("1080P"), Some(WebRip)),
        "Bluray-1080p" => q(Some("1080P"), Some(BluRay)),
        "Remux-1080p" | "Bluray-1080p Remux" => remux("1080P"),
        "HDTV-2160p" => q(Some("2160P"), Some(Hdtv)),
        "WEBDL-2160p" => q(Some("2160P"), Some(WebDl)),
        "WEBRip-2160p" => q(Some("2160P"), Some(WebRip)),
        "Bluray-2160p" => q(Some("2160P"), Some(BluRay)),
        "Remux-2160p" | "Bluray-2160p Remux" => remux("2160P"),
        "BR-DISK" => Q {
            tier: Some("1080P"),
            source: Some(BrDisk),
            remux: false,
            bd_disk: true,
            alias: None,
        },
        "CAM" => q(None, Some(Cam)),
        "TELESYNC" => q(None, Some(Telesync)),
        "TELECINE" => q(None, Some(Telecine)),
        "DVDSCR" => q(None, Some(DvdScr)),
        "WORKPRINT" => q(None, Some(Workprint)),
        other => return Err(format!("no Scryer translation for quality '{other}'")),
    };
    found.alias = COLLIDING.iter().copied().find(|n| *n == name);
    Ok(found)
}

fn tier_lines(tier: &str) -> u32 {
    let resolution = tier.rsplit('-').next().unwrap_or(tier);
    resolution.trim_end_matches('P').parse().unwrap_or(0)
}

/// The source as the product stores it (`parsed_release_source_type`).
fn source_name(q: &Q) -> Option<String> {
    if q.remux { Some("Remux".into()) } else { q.source.map(|s| s.to_string()) }
}

/// The product's tier key for a quality (`WEBDL-1080P`, `REMUX-2160P`).
fn key(q: &Q) -> Option<String> {
    quality_tier_label(q.tier, source_name(q).as_deref())
}

/// Resolutions this case splits into source keys (header, D54). `named`: the
/// case's release, files and queue items; the cutoff is added here.
fn split_lines(s: &ProfileSpec, named: &[Q]) -> Result<Vec<u32>, String> {
    let mut named = named.to_vec();
    if let Some(cutoff) = s.cutoff.as_deref() {
        named.push(quality(cutoff)?);
    }
    let allowed = s.names.iter().map(|n| quality(n)).collect::<Result<Vec<_>, _>>()?;
    let keys_at = |list: &[Q], tier: &str| -> Vec<Option<String>> {
        list.iter().filter(|q| q.tier == Some(tier)).map(key).collect()
    };
    let mut split = Vec::new();
    for q in &named {
        let Some(tier) = q.tier else { continue };
        let allowed_here = keys_at(&allowed, tier);
        let differ = keys_at(&named, tier).iter().any(|k| *k != key(q));
        let outside = s.explicit && !allowed_here.is_empty() && !allowed_here.contains(&key(q));
        if (differ || outside) && !split.contains(&tier_lines(tier)) {
            split.push(tier_lines(tier));
        }
    }
    Ok(split)
}

struct ProfileSpec {
    names: Vec<String>,
    explicit: bool,
    cutoff: Option<String>,
    upgrade_allowed: bool,
    min_format_score: i32,
    cutoff_format_score: i32,
    min_upgrade_format_score: i32,
    ignore_revisions: bool,
}

/// `downloadPropersAndRepacks` -> `ignore_proper_repack`.
fn propers(value: &Value) -> Result<bool, String> {
    match value.as_str().map(str::to_ascii_lowercase).as_deref() {
        None | Some("preferandupgrade") => Ok(false),
        Some("donotprefer") => Ok(true),
        Some("donotupgrade") => Err(
            "no Scryer equivalent for downloadPropersAndRepacks=DoNotUpgrade (prefer revisions but never grab them as upgrades)"
                .into(),
        ),
        Some(other) => Err(format!("unknown downloadPropersAndRepacks '{other}'")),
    }
}

fn strings(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
}

fn int(value: &Value, default: i64) -> i32 {
    value.as_i64().unwrap_or(default) as i32
}

/// A fixture's profile object plus its `config` object.
fn fixture_spec(p: &Value, config: &Value) -> Result<ProfileSpec, String> {
    let vocab = strings(&p["qualities"])
        .or_else(|| strings(&p["items"]))
        .unwrap_or_else(|| VOCAB.iter().map(|s| s.to_string()).collect());
    let (names, explicit) = match strings(&p["allowed"]).or_else(|| strings(&p["items_allowed"])) {
        Some(list) => (list, true),
        None => (vocab, false),
    };
    Ok(ProfileSpec {
        names,
        explicit,
        cutoff: p["cutoff"].as_str().map(str::to_string),
        upgrade_allowed: p["upgrade_allowed"].as_bool().unwrap_or(false),
        min_format_score: int(&p["min_format_score"], 0),
        cutoff_format_score: int(&p["cutoff_format_score"], 0),
        min_upgrade_format_score: int(&p["min_upgrade_format_score"], 1),
        ignore_revisions: propers(&config["download_propers_and_repacks"])?,
    })
}

/// A profile from `corpus/captured/<app>/config/qualityprofile.json`.
fn captured_spec(app: &str, id: i64) -> Result<ProfileSpec, String> {
    let profiles: Vec<Value> = corpus(&format!("corpus/captured/{app}/config/qualityprofile.json"));
    let media: Value = corpus(&format!("corpus/captured/{app}/config/mediamanagement.json"));
    let p = profiles
        .iter()
        .find(|p| p["id"].as_i64() == Some(id))
        .ok_or_else(|| format!("{app} profile {id} not captured"))?;
    let mut names = Vec::new();
    let mut ids: Vec<(i64, String)> = Vec::new();
    for item in p["items"].as_array().into_iter().flatten() {
        if item["quality"].is_object() {
            let name = item["quality"]["name"].as_str().unwrap_or_default().to_string();
            if item["allowed"].as_bool() == Some(true) {
                names.push(name.clone());
            }
            ids.push((item["quality"]["id"].as_i64().unwrap_or(-1), name));
        } else {
            let group_allowed = item["allowed"].as_bool() == Some(true);
            let mut first = None;
            for child in item["items"].as_array().into_iter().flatten() {
                let name = child["quality"]["name"].as_str().unwrap_or_default().to_string();
                if group_allowed && child["allowed"].as_bool() == Some(true) {
                    names.push(name.clone());
                }
                ids.push((child["quality"]["id"].as_i64().unwrap_or(-1), name.clone()));
                first.get_or_insert(name);
            }
            if let (Some(gid), Some(first)) = (item["id"].as_i64(), first) {
                ids.push((gid, first));
            }
        }
    }
    let cutoff_id = p["cutoff"].as_i64().ok_or("profile without cutoff")?;
    let cutoff = ids
        .iter()
        .find(|(qid, _)| *qid == cutoff_id)
        .map(|(_, name)| name.clone())
        .ok_or_else(|| format!("cutoff id {cutoff_id} not in profile {id}"))?;
    Ok(ProfileSpec {
        names,
        explicit: true,
        cutoff: Some(cutoff),
        upgrade_allowed: p["upgradeAllowed"].as_bool().unwrap_or(false),
        min_format_score: int(&p["minFormatScore"], 0),
        cutoff_format_score: int(&p["cutoffFormatScore"], 0),
        min_upgrade_format_score: int(&p["minUpgradeFormatScore"], 1),
        ignore_revisions: propers(&media["downloadPropersAndRepacks"])?,
    })
}

fn build_profile(s: &ProfileSpec, named: &[Q]) -> Result<QualityProfile, String> {
    let split = split_lines(s, named)?;
    let tier_of = |q: &Q| -> Option<String> {
        let tier = q.tier?;
        if split.contains(&tier_lines(tier)) { key(q) } else { Some(tier.to_string()) }
    };
    // A collision takes the rank of the member the case names, else the best.
    let cutoff_q = s.cutoff.as_deref().map(quality).transpose()?;
    let named_alias: Vec<&str> = named.iter().chain(&cutoff_q).filter_map(|q| q.alias).collect();
    // (tier, rank, pinned): rank = position in the allowed list
    let mut tiers: Vec<(String, usize, bool)> = Vec::new();
    let mut sources: Vec<ReleaseSource> = Vec::new();
    let (mut unknown, mut bd_disk) = (false, false);
    for (rank, name) in s.names.iter().enumerate() {
        let q = quality(name)?;
        let pinned = q.alias.is_some_and(|a| named_alias.contains(&a));
        match tier_of(&q) {
            Some(tier) => match tiers.iter_mut().find(|(t, ..)| *t == tier) {
                Some(entry) if pinned => *entry = (tier, rank, true),
                Some(entry) if !entry.2 => entry.1 = entry.1.max(rank),
                Some(_) => {}
                None => tiers.push((tier, rank, pinned)),
            },
            None => unknown |= name == "Unknown",
        }
        if let Some(source) = q.source {
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
        bd_disk |= q.bd_disk;
    }
    tiers.sort_by_key(|(tier, rank, _)| Reverse((tier_lines(tier), *rank)));
    let tiers: Vec<String> = tiers.into_iter().map(|(tier, ..)| tier).collect();
    let lowest = tiers.last().cloned();
    let cutoff = match s.cutoff.as_deref() {
        None | Some("Unknown") => lowest,
        Some(_) => cutoff_q.as_ref().and_then(|q| tier_of(q)).or(lowest),
    };
    let mut profile = builtin_1080p_profile();
    profile.id = "parity".into();
    profile.name = "parity".into();
    let c = &mut profile.criteria;
    c.archival_quality = tiers.first().cloned();
    c.quality_tiers = tiers;
    c.allow_unknown_quality = unknown;
    c.source_allowlist = if s.explicit { sources } else { Vec::new() };
    c.allow_bd_disk = bd_disk;
    c.allow_upgrades = s.upgrade_allowed;
    c.set_ignore_proper_repack(s.ignore_revisions);
    c.cutoff_tier = cutoff;
    c.min_score_to_grab = Some(s.min_format_score);
    c.cutoff_score = Some(s.cutoff_format_score);
    c.min_upgrade_score_delta = Some(s.min_upgrade_format_score);
    Ok(profile)
}

/// A release or a file as the ladder sees it.
#[derive(Clone)]
struct Side {
    q: Q,
    name: String,
    /// Scryer revision: reference version - 1.
    revision: i32,
    group: Option<String>,
    score: i32,
}

/// `revision` as a number (`2`) or an object (`{"version": 2, "real": 1}`).
fn revision_of(r: &Value) -> (i64, i64) {
    match r {
        Value::Number(n) => (n.as_i64().unwrap_or(1), 0),
        Value::Object(_) => (r["version"].as_i64().unwrap_or(1), r["real"].as_i64().unwrap_or(0)),
        _ => (1, 0),
    }
}

/// `{"quality": "DVD", "revision": 2, "real": 1}` or
/// `{"quality": {"quality": "DVD", "revision": {"version": 2}}}`.
fn quality_of(v: &Value) -> Option<(String, i64, i64)> {
    match &v["quality"] {
        Value::String(name) => {
            let (version, real) = revision_of(&v["revision"]);
            Some((name.clone(), version, v["real"].as_i64().unwrap_or(real)))
        }
        Value::Object(_) => {
            let inner = &v["quality"];
            let name = inner["quality"].as_str()?;
            let (version, real) = revision_of(&inner["revision"]);
            Some((name.to_string(), version, real))
        }
        _ => None,
    }
}

fn group(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|g| !g.is_empty()).map(str::to_string)
}

fn side(v: &Value, group: Option<String>, score: i32) -> Result<Side, String> {
    let (name, version, real) = quality_of(v).ok_or("no quality in fixture")?;
    if real > 0 {
        return Err("no Scryer field for a REAL revision".into());
    }
    Ok(Side {
        q: quality(&name)?,
        name,
        revision: (version - 1) as i32,
        group,
        score,
    })
}

fn format_score(profile: &Value, format: &str) -> i32 {
    profile["format_items"]
        .as_array()
        .and_then(|items| items.iter().find(|i| i["format"].as_str() == Some(format)))
        .and_then(|i| i["score"].as_i64())
        .unwrap_or(0) as i32
}

/// Custom-format score of a release or file object.
fn cf_score(profile: &Value, v: &Value) -> i32 {
    if let Some(score) = v["custom_format_score"].as_i64() {
        return score as i32;
    }
    v["custom_formats"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .filter_map(Value::as_str)
                .map(|n| format_score(profile, n))
                .sum()
        })
        .unwrap_or(0)
}

fn parsed(s: &Side) -> ParsedReleaseMetadata {
    ParsedReleaseMetadata {
        quality: s.q.tier.map(str::to_string),
        source: s.q.source,
        is_remux: s.q.remux,
        is_bd_disk: s.q.bd_disk,
        revision: s.revision,
        release_group: s.group.clone(),
        ..ParsedReleaseMetadata::default()
    }
}

fn requirements(profile: &QualityProfile, s: &Side) -> QualityProfileDecision {
    evaluate_profile_requirements(profile, &parsed(s), false, None)
}

fn tier_index(profile: &QualityProfile, s: &Side) -> Option<usize> {
    requirements(profile, s).tier_index
}

fn created_at(added_days_ago: i64) -> String {
    (now() - Duration::days(added_days_ago)).to_rfc3339()
}

fn incumbent(
    profile: &QualityProfile,
    id: &str,
    s: &Side,
    episode: Option<&str>,
    added_days_ago: i64,
) -> Incumbent {
    Incumbent {
        tier_index: tier_index(profile, s),
        revision: s.revision,
        file_id: id.to_string(),
        file_path: format!("/parity/{id}.mkv"),
        release_group: s.group.clone(),
        score: s.score,
        language: AudioLanguageRank::default(),
        covers: episode.map(str::to_string).into_iter().collect(),
        created_at: created_at(added_days_ago),
    }
}

fn source_kind(protocol: &Value) -> Option<DownloadSourceKind> {
    match protocol.as_str().map(str::to_ascii_lowercase).as_deref() {
        Some("usenet") => Some(DownloadSourceKind::NzbUrl),
        Some("torrent") => Some(DownloadSourceKind::TorrentFile),
        _ => None,
    }
}

fn preferred(protocol: &Value) -> PreferredProtocol {
    match protocol.as_str().map(str::to_ascii_lowercase).as_deref() {
        Some("torrent") => PreferredProtocol::Torrent,
        _ => PreferredProtocol::Usenet,
    }
}

// ── The grab ladder ────────────────────────────────────────────────────────

/// A file on disk for the ladder.
struct Held {
    id: String,
    side: Side,
    episode: Option<String>,
    added_days_ago: i64,
}

enum Refusal {
    Cutoff(ReleaseAutoDecisionCode),
    Admission(AdmissionRejectionReason),
}

/// The grab lane's cutoff gate then admission, as
/// `evaluate_auto_candidate_recording` runs them. `members`: `None` for a
/// title scope (a movie, or a file pair with no episodes), else the episode
/// ids the release covers.
fn grab_refusal(
    profile: &QualityProfile,
    cand: &Side,
    held: &[Held],
    members: Option<&[String]>,
    queued: Vec<QueuedRelease>,
    is_rss: bool,
) -> Option<Refusal> {
    let crit = &profile.criteria;
    let now = now();
    let candidate = CandidateFacts::new(tier_index(profile, cand), cand.revision, cand.score)
        .with_profile_policy(cand.group.as_deref(), crit);
    let (admission_scope, cutoff_scope) = match members {
        None => (AdmissionScope::Title, CutoffScope::Title),
        Some(ids) => (
            AdmissionScope::Episodes(ids.to_vec()),
            CutoffScope::Episodes(ids.to_vec()),
        ),
    };
    let incumbents: Vec<(Incumbent, bool)> = held
        .iter()
        .map(|h| {
            (
                incumbent(profile, &h.id, &h.side, h.episode.as_deref(), h.added_days_ago),
                true,
            )
        })
        .collect();
    let subject = AdmissionSubject::new(admission_scope, incumbents).with_queued(queued);
    let summaries: Vec<MediaFileCutoffSummary> = held
        .iter()
        .map(|h| MediaFileCutoffSummary {
            id: h.id.clone(),
            file_path: format!("/parity/{}.mkv", h.id),
            original_file_path: None,
            scene_name: None,
            grabbed_release_title: None,
            episode_id: h.episode.clone(),
            series_movie_link_ids: Vec::new(),
            quality_label: h.side.q.tier.map(str::to_string),
            source_type: source_name(&h.side.q),
            created_at: created_at(h.added_days_ago),
        })
        .collect();
    let reached = crit.quality_cutoff_met(analyzed_cutoff_quality_for_scope(&summaries, &cutoff_scope).as_deref());
    let at_cutoff = incumbent_at_cutoff(reached, &subject, crit.cutoff_score, &crit.decision_pipeline);
    if let Some(code) = cutoff_refusal(candidate, &subject, at_cutoff, is_rss, &now) {
        return Some(Refusal::Cutoff(code));
    }
    let policy = AdmissionPolicy {
        allow_upgrades: crit.allow_upgrades,
        min_delta: AcquisitionThresholds::for_profile(crit).same_tier_min_delta,
        cutoff_score: crit.cutoff_score,
        manual_override: false,
        applies_to_queue: true,
    };
    evaluate_admission(&subject, candidate, &policy)
        .rejection()
        .map(|r| Refusal::Admission(r.reason.clone()))
}

/// The reference model's `UpgradeableRejectReason` name for a refusal.
fn rejection_name(refusal: &Option<Refusal>) -> String {
    match refusal {
        None => "None".into(),
        Some(Refusal::Cutoff(ReleaseAutoDecisionCode::CutoffReached)) => "QualityCutoff".into(),
        Some(Refusal::Cutoff(code)) => format!("{code:?}"),
        Some(Refusal::Admission(reason)) => match reason {
            AdmissionRejectionReason::LowerQualityTier => "BetterQuality".into(),
            AdmissionRejectionReason::LowerRevision { .. } => "BetterRevision".into(),
            AdmissionRejectionReason::UpgradesDisabled => "UpgradesNotAllowed".into(),
            AdmissionRejectionReason::FormatCutoffReached { .. } => "CustomFormatCutoff".into(),
            AdmissionRejectionReason::NotAnUpgrade {
                incumbent_score,
                candidate_score,
                ..
            } => {
                if candidate_score <= incumbent_score {
                    "CustomFormatScore".into()
                } else {
                    "MinCustomFormatScore".into()
                }
            }
            other => format!("{other:?}"),
        },
    }
}

// ── Reference families ─────────────────────────────────────────────────────

fn quality_allowed(app: &str, g: &Value) -> Result<Value, String> {
    let release = if app == "radarr" { &g["parsed_quality"] } else { &g["release"] };
    let release = side(release, None, 0)?;
    let profile = build_profile(&fixture_spec(&g["profile"], &g["config"])?, &[release.q])?;
    let decision = requirements(&profile, &release);
    let refused = decision.block_codes.iter().any(|code| {
        code == "quality_not_in_profile_tiers"
            || code == "quality_missing_and_profile_disallows_unknown"
            || code.starts_with("source_")
    });
    Ok(accepted(!refused))
}

fn custom_format_allowed(app: &str, g: &Value) -> Result<Value, String> {
    let p = &g["profile"];
    let profile = build_profile(&fixture_spec(p, &g["config"])?, &[])?;
    let quality = if app == "radarr" { &g["parsed_quality"] } else { &g["release"] };
    let mut decision = requirements(&profile, &side(quality, None, 0)?);
    let release = &g["release"];
    let formats: Vec<&str> = release["custom_formats"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for format in &formats {
        let score = format_score(p, format);
        if score != 0 {
            decision.log_with_source(
                format,
                score,
                ScoringSource::UserRule {
                    id: format.to_string(),
                    name: format.to_string(),
                },
            );
        }
    }
    if formats.is_empty() {
        if let Some(score) = release["custom_format_score"].as_i64().filter(|s| *s != 0) {
            decision.log_with_source(
                "custom_format_score",
                score as i32,
                ScoringSource::UserRule {
                    id: "custom_format_score".into(),
                    name: "custom_format_score".into(),
                },
            );
        }
    }
    apply_min_score_gate(&profile, &mut decision);
    let refused = decision
        .block_codes
        .iter()
        .any(|code| code == "score_below_minimum" || code == "score_at_or_below_block_threshold");
    Ok(accepted(!refused))
}

fn raw_disk(g: &Value) -> Result<Value, String> {
    let release = &g["release"];
    if let Some(container) = release["container"].as_str() {
        return Err(format!(
            "no Scryer field for an indexer-reported container ('{container}')"
        ));
    }
    let title = release["title"].as_str().ok_or("no title")?;
    let decision = evaluate_profile_requirements(
        &builtin_1080p_profile(),
        &parse_release_metadata(title),
        false,
        None,
    );
    Ok(accepted(!decision.block_codes.iter().any(|c| c == "bd_disk_not_allowed")))
}

fn language(g: &Value) -> Result<Value, String> {
    let required = match g["profile"]["language"].as_str() {
        None | Some("Any") => Vec::new(),
        Some("Original") => {
            return Err(
                "no Scryer field for a required 'Original' language (required_audio_languages takes fixed languages)"
                    .into(),
            );
        }
        Some(name) => vec![name.to_string()],
    };
    let mut profile = builtin_1080p_profile();
    profile.criteria.required_audio_languages = required;
    let release = ParsedReleaseMetadata {
        languages_audio: strings(&g["release_languages"]).unwrap_or_default(),
        ..ParsedReleaseMetadata::default()
    };
    let decision = evaluate_profile_requirements(&profile, &release, false, None);
    Ok(accepted(
        !decision
            .block_codes
            .iter()
            .any(|c| c == "required_audio_language_missing"),
    ))
}

fn delay_decision_reason(
    profile: &DelayProfile,
    protocol: &Value,
    age_minutes: Option<i64>,
    context: &DelayPolicyContext,
) -> DelayDecisionReason {
    let now = now();
    let published = age_minutes.map(|m| now - Duration::minutes(m));
    profile
        .evaluate_delay_decision_with_context(source_kind(protocol), published, context, &now)
        .reason
}

fn protocol(g: &Value) -> Result<Value, String> {
    let d = &g["delay_profile"];
    let profile = DelayProfile {
        enable_usenet: d["enable_usenet"].as_bool().unwrap_or(true),
        enable_torrent: d["enable_torrent"].as_bool().unwrap_or(true),
        ..DelayProfile::default()
    };
    let reason = delay_decision_reason(
        &profile,
        &g["release"]["protocol"],
        Some(60 * 24),
        &DelayPolicyContext::default(),
    );
    Ok(accepted(reason != DelayDecisionReason::ProtocolDisabled))
}

fn minimum_age(g: &Value) -> Result<Value, String> {
    let profile = DelayProfile {
        min_age_minutes: g["config"]["minimum_age_minutes"].as_i64().unwrap_or(0),
        ..DelayProfile::default()
    };
    let release = &g["release"];
    let reason = delay_decision_reason(
        &profile,
        &release["protocol"],
        release["age_minutes"].as_i64(),
        &DelayPolicyContext::default(),
    );
    Ok(accepted(reason != DelayDecisionReason::MinimumAge))
}

fn delay(app: &str, g: &Value) -> Outcome {
    let d = &g["delay_profile"];
    if d["bypass_if_highest_quality"].as_bool() == Some(true) {
        return None; // needs the highest-quality fact (header)
    }
    let has_file = if app == "radarr" {
        !g["existing_file"].is_null() || !g["stubs"].is_null()
    } else {
        g["episodes"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|e| e["episode_file_id"].as_i64().unwrap_or(0) != 0)
    };
    if has_file {
        return None; // needs the same-tier revision fact (header)
    }
    let profile = DelayProfile {
        usenet_delay_minutes: d["usenet_delay"].as_i64().unwrap_or(0),
        torrent_delay_minutes: d["torrent_delay"].as_i64().unwrap_or(0),
        preferred_protocol: preferred(&d["preferred_protocol"]),
        bypass_if_highest_quality: false,
        bypass_score_threshold: (d["bypass_if_above_custom_format_score"].as_bool() == Some(true))
            .then(|| int(&d["minimum_custom_format_score"], 0)),
        ..DelayProfile::default()
    };
    let release = &g["release"];
    let age = release["age_minutes"]
        .as_i64()
        .or_else(|| release["age_hours"].as_i64().map(|h| h * 60));
    let context = DelayPolicyContext {
        user_invoked: g["search"]["user_invoked"].as_bool().unwrap_or(false),
        candidate_score: int(&release["custom_format_score"], 0),
        ..DelayPolicyContext::default()
    };
    let reason = delay_decision_reason(&profile, &release["protocol"], age, &context);
    Some(Ok(accepted(!matches!(
        reason,
        DelayDecisionReason::PendingDelay | DelayDecisionReason::ReleaseAgeUnknown
    ))))
}

fn torrent_seeding(app: &str, g: &Value) -> Result<Value, String> {
    let release = &g["release"];
    let class = release["type"].as_str().or_else(|| release["release_class"].as_str());
    let kind = (class == Some("TorrentInfo")).then_some(DownloadSourceKind::TorrentFile);
    let indexer = release["indexer_id"].as_i64().unwrap_or(0);
    let indexer_id = (indexer != 0).then(|| indexer.to_string());
    let threshold = if app == "radarr" {
        let ix = &g["indexer"];
        if ix["id"].as_i64() == Some(indexer) {
            int(&ix["settings"]["minimum_seeders"], 0)
        } else {
            0
        }
    } else {
        int(&g["indexers"][indexer.to_string().as_str()]["minimum_seeders"], 0)
    };
    Ok(accepted(meets_minimum_seeders(
        kind,
        indexer_id.as_deref(),
        release["seeders"].as_i64(),
        threshold,
    )))
}

/// `current` -> `new` pair fixtures (IsUpgradable, IsUpgradeAllowed).
fn pair_refusal(g: &Value) -> Result<Option<Refusal>, String> {
    let p = &g["profile"];
    let current = side(&g["current"], None, cf_score(p, &g["current"]))?;
    let new = side(&g["new"], None, cf_score(p, &g["new"]))?;
    let profile = build_profile(&fixture_spec(p, &g["config"])?, &[current.q, new.q])?;
    let held = [Held {
        id: "f1".into(),
        side: current,
        episode: None,
        added_days_ago: 0,
    }];
    Ok(grab_refusal(&profile, &new, &held, None, Vec::new(), false))
}

fn upgradable(g: &Value, expect: &Value) -> Result<Value, String> {
    let refusal = pair_refusal(g)?;
    let mut actual = serde_json::Map::new();
    if expect.get("accepted").is_some() {
        actual.insert("accepted".into(), json!(refusal.is_none()));
    }
    if expect.get("rejection").is_some() {
        actual.insert("rejection".into(), json!(rejection_name(&refusal)));
    }
    Ok(Value::Object(actual))
}

fn upgrade_allowed(g: &Value) -> Result<Value, String> {
    let refusal = pair_refusal(g)?;
    Ok(accepted(!matches!(
        refusal,
        Some(Refusal::Admission(AdmissionRejectionReason::UpgradesDisabled))
    )))
}

fn upgrade_disk(app: &str, g: &Value) -> Result<Value, String> {
    let p = &g["profile"];
    let spec = fixture_spec(p, &g["config"])?;
    let named = |release: &Side, held: &[Held]| -> Vec<Q> {
        std::iter::once(release.q).chain(held.iter().map(|h| h.side.q)).collect()
    };
    if app == "radarr" {
        let release = side(&g["parsed_quality"], None, cf_score(p, &g["release"]))?;
        let mut held = Vec::new();
        let file = &g["existing_file"];
        if !file.is_null() {
            held.push(Held {
                id: "f1".into(),
                side: side(file, None, cf_score(p, file))?,
                episode: None,
                added_days_ago: 0,
            });
        }
        let profile = build_profile(&spec, &named(&release, &held))?;
        return Ok(accepted(
            grab_refusal(&profile, &release, &held, None, Vec::new(), false).is_none(),
        ));
    }
    let release_v = &g["release"];
    let release = side(release_v, None, cf_score(p, release_v))?;
    let episodes = g["episodes"].as_array().cloned().unwrap_or_default();
    let members: Vec<String> = (0..episodes.len()).map(|i| format!("e{i}")).collect();
    let file_score: i32 = g["existing_file_custom_formats"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .filter_map(Value::as_str)
                .map(|n| format_score(p, n))
                .sum()
        })
        .unwrap_or(0);
    let mut held = Vec::new();
    for (i, e) in episodes.iter().enumerate() {
        if e["episode_file_id"].as_i64().unwrap_or(0) == 0 || e["file"].is_null() {
            continue;
        }
        let file = &e["file"];
        held.push(Held {
            id: format!("f{i}"),
            side: side(file, None, file_score)?,
            episode: Some(members[i].clone()),
            added_days_ago: file["added_age_days"].as_i64().unwrap_or(0),
        });
    }
    let profile = build_profile(&spec, &named(&release, &held))?;
    Ok(accepted(
        grab_refusal(&profile, &release, &held, Some(members.as_slice()), Vec::new(), false).is_none(),
    ))
}

fn queue(app: &str, g: &Value) -> Outcome {
    (|| -> Result<Option<Value>, String> {
        let p = &g["profile"];
        let (release, members, wanted): (Side, Option<Vec<String>>, Vec<String>) = if app == "radarr" {
            (side(&g["parsed_quality"], None, cf_score(p, &g["release"]))?, None, Vec::new())
        } else {
            let r = &g["release"];
            let ids: Vec<String> = r["episodes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|e| e["id"].as_i64().map(|n| n.to_string()))
                .collect();
            (side(r, None, cf_score(p, r))?, Some(ids.clone()), ids)
        };
        // Queue items name their quality even when the loader would skip them.
        let mut named = vec![release.q];
        named.extend(
            g["queue"].as_array().into_iter().flatten()
                .filter_map(|item| quality(&quality_of(item)?.0).ok()),
        );
        let profile = build_profile(&fixture_spec(p, &g["config"])?, &named)?;
        let mut queued = Vec::new();
        for item in g["queue"].as_array().into_iter().flatten() {
            let (matches, covers, state) = if app == "radarr" {
                (
                    item["movie_id"] == g["movie"]["id"],
                    Vec::new(),
                    item["tracked_download_state"].as_str(),
                )
            } else {
                let covers: Vec<String> = item["episodes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e["id"].as_i64().map(|n| n.to_string()))
                    .collect();
                // The loader keeps a submission whose scope intersects the
                // release's (`queued_releases_for_scope`, canonical_context.rs).
                let overlaps = submission_scope_intersects(
                    &SubmissionScope::EpisodeSet { episode_ids: covers.clone() },
                    &ScopeMembership { episode_ids: &wanted, ..ScopeMembership::default() },
                );
                (
                    item["series_id"] == g["release"]["series_id"] && overlaps,
                    covers,
                    item["state"].as_str(),
                )
            };
            if !matches {
                continue;
            }
            if state != Some("Downloading") {
                return Ok(None); // queue-state filter is the loader's (header)
            }
            let held = side(item, None, cf_score(p, item))?;
            queued.push(QueuedRelease {
                missing_history: false,
                history_cutoff_reached: false,
                release_group: held.group.clone(),
                title: format!("queued {}", held.name),
                covers,
                tier_index: tier_index(&profile, &held),
                revision: held.revision,
                score: held.score,
                language: AudioLanguageRank::default(),
                release_key: None,
            });
        }
        let refusal = grab_refusal(&profile, &release, &[], members.as_deref(), queued, false);
        Ok(Some(accepted(refusal.is_none())))
    })()
    .transpose()
}

/// Files of a Sonarr `episodes` list (entries with a non-zero file id).
fn episode_files(g: &Value, p: &Value) -> Result<(Vec<String>, Vec<Held>), String> {
    let episodes = g["episodes"].as_array().cloned().unwrap_or_default();
    let members: Vec<String> = (0..episodes.len()).map(|i| format!("e{i}")).collect();
    let mut held = Vec::new();
    for (i, e) in episodes.iter().enumerate() {
        if e["episode_file_id"].as_i64().unwrap_or(0) == 0 || e["file"].is_null() {
            continue;
        }
        let file = &e["file"];
        held.push(Held {
            id: format!("f{i}"),
            side: side(file, group(&file["release_group"]), cf_score(p, file))?,
            episode: Some(members[i].clone()),
            added_days_ago: file["added_days_ago"].as_i64().unwrap_or(0),
        });
    }
    Ok((members, held))
}

fn is_revision_upgrade(release: &Side, file: &Side) -> bool {
    release.name == file.name && release.revision > file.revision
}

fn repack(app: &str, g: &Value) -> Outcome {
    let p = &g["profile"];
    let config = &g["config"];
    let ignore = match propers(&config["download_propers_and_repacks"]) {
        Ok(ignore) => ignore,
        Err(why) => return Some(Err(why)),
    };
    let built = (|| -> Result<(Side, Option<Vec<String>>, Vec<Held>, bool), String> {
        if app == "radarr" {
            let pq = &g["parsed_quality"];
            let is_repack = pq["revision"]["is_repack"].as_bool() == Some(true);
            let release = side(pq, group(&g["parsed_release_group"]), 0)?;
            let mut held = Vec::new();
            let file = &g["existing_file"];
            if g["movie"]["movie_file_id"].as_i64().unwrap_or(0) != 0 && !file.is_null() {
                held.push(Held {
                    id: "f1".into(),
                    side: side(file, group(&file["release_group"]), 0)?,
                    episode: None,
                    added_days_ago: 0,
                });
            }
            Ok((release, None, held, is_repack))
        } else {
            let r = &g["release"];
            let is_repack = r["repack"].as_bool() == Some(true);
            let release = side(r, group(&r["release_group"]), 0)?;
            let (members, held) = episode_files(g, p)?;
            Ok((release, Some(members), held, is_repack))
        }
    })();
    let (release, members, held, is_repack) = match built {
        Ok(parts) => parts,
        Err(why) => return Some(Err(why)),
    };
    // RepackSpecification evaluates only a repack, under a prefer setting,
    // that is a same-quality newer revision of a file on disk.
    if !is_repack || ignore || !held.iter().any(|h| is_revision_upgrade(&release, &h.side)) {
        return None;
    }
    Some((|| -> Result<Value, String> {
        let profile = build_profile(&fixture_spec(p, config)?, &[])?;
        Ok(accepted(
            grab_refusal(&profile, &release, &held, members.as_deref(), Vec::new(), false).is_none(),
        ))
    })())
}

fn anime_version(g: &Value) -> Outcome {
    let config = &g["config"];
    let ignore = match propers(&config["download_propers_and_repacks"]) {
        Ok(ignore) => ignore,
        Err(why) => return Some(Err(why)),
    };
    let has_file = g["episodes"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|e| e["episode_file_id"].as_i64().unwrap_or(0) != 0);
    if g["series"]["series_type"].as_str() != Some("Anime") || ignore || !has_file {
        return None;
    }
    let r = &g["release"];
    let f = &g["existing_file"];
    let parts = (|| -> Result<(Side, Side), String> {
        Ok((
            side(r, group(&r["release_group"]), 0)?,
            side(f, group(&f["release_group"]), 0)?,
        ))
    })();
    let (release, file) = match parts {
        Ok(parts) => parts,
        Err(why) => return Some(Err(why)),
    };
    if !is_revision_upgrade(&release, &file) {
        return None;
    }
    Some((|| -> Result<Value, String> {
        let profile = build_profile(&fixture_spec(&g["profile"], config)?, &[])?;
        let held = [Held {
            id: "f1".into(),
            side: file,
            episode: None,
            added_days_ago: 0,
        }];
        Ok(accepted(
            grab_refusal(&profile, &release, &held, None, Vec::new(), false).is_none(),
        ))
    })())
}

/// The reference ProperSpecification reads no release group and its fixtures
/// carry none; Scryer counts a revision only between same-group releases
/// (`decision_pipeline.rs` `Facts::is_related_to`). A group-less side gets this
/// shared group so the old-file age check, not the group gate, decides.
const PROPER_GROUP: &str = "PARITY";

fn proper_group(v: &Value) -> Option<String> {
    group(v).or_else(|| Some(PROPER_GROUP.into()))
}

fn proper(app: &str, g: &Value) -> Outcome {
    let p = &g["profile"];
    let config = &g["config"];
    let is_rss = if app == "radarr" {
        g["search"]["kind"].as_str() == Some("rss")
    } else {
        g["search"].is_null()
    };
    let ignore = match propers(&config["download_propers_and_repacks"]) {
        Ok(ignore) => ignore,
        Err(why) => return Some(Err(why)),
    };
    let built = (|| -> Result<(Side, Option<Vec<String>>, Vec<Held>), String> {
        if app == "radarr" {
            let release = side(&g["parsed_quality"], proper_group(&g["parsed_release_group"]), 0)?;
            let file = &g["existing_file"];
            let held = vec![Held {
                id: "f1".into(),
                side: side(file, proper_group(&file["release_group"]), 0)?,
                episode: None,
                added_days_ago: file["added_days_ago"].as_i64().unwrap_or(0),
            }];
            Ok((release, None, held))
        } else {
            let r = &g["release"];
            let release = side(r, proper_group(&r["release_group"]), 0)?;
            let (members, mut held) = episode_files(g, p)?;
            for h in &mut held {
                h.side.group.get_or_insert_with(|| PROPER_GROUP.into());
            }
            Ok((release, Some(members), held))
        }
    })();
    let (release, members, held) = match built {
        Ok(parts) => parts,
        Err(why) => return Some(Err(why)),
    };
    // ProperSpecification evaluates only in RSS, under a prefer setting, for
    // a same-quality newer revision of a file on disk.
    if !is_rss || ignore || !held.iter().any(|h| is_revision_upgrade(&release, &h.side)) {
        return None;
    }
    Some((|| -> Result<Value, String> {
        let profile = build_profile(&fixture_spec(p, config)?, &[])?;
        let refusal = grab_refusal(&profile, &release, &held, members.as_deref(), Vec::new(), true);
        Ok(accepted(!matches!(
            refusal,
            Some(Refusal::Cutoff(ReleaseAutoDecisionCode::ProperForOldFile))
        )))
    })())
}

fn full_season(g: &Value) -> Outcome {
    if g["release"]["full_season"].as_bool() != Some(true) {
        return None; // the specification only judges season packs
    }
    let now = now();
    let episodes = g["episodes"].as_array().cloned().unwrap_or_default();
    let members: Vec<String> = (0..episodes.len()).map(|i| format!("e{i}")).collect();
    let unaired = episodes
        .iter()
        .filter(|e| {
            let air = e["air_date_offset_hours"]
                .as_i64()
                .map(|h| (now + Duration::hours(h)).to_rfc3339());
            episode_is_unaired(air.as_deref(), &now)
        })
        .count();
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
    Some(Ok(accepted(!matches!(
        verdict.rejection().map(|r| &r.reason),
        Some(AdmissionRejectionReason::SeasonIncomplete)
    ))))
}

fn same_episodes(g: &Value) -> Result<Value, String> {
    let release_episodes = g["release_episodes"].as_array().cloned().unwrap_or_default();
    let members: Vec<String> = release_episodes
        .iter()
        .filter_map(|e| e["id"].as_i64().map(|n| n.to_string()))
        .collect();
    let in_file = &g["episodes_in_file"];
    let mut file_ids: Vec<i64> = release_episodes
        .iter()
        .filter_map(|e| e["episode_file_id"].as_i64())
        .filter(|id| *id != 0)
        .collect();
    file_ids.dedup();
    let incumbents: Vec<(Incumbent, bool)> = file_ids
        .iter()
        .map(|file_id| {
            let covers_v = if in_file[file_id.to_string().as_str()].is_array() {
                &in_file[file_id.to_string().as_str()]
            } else {
                &in_file["*"]
            };
            let covers: Vec<String> = covers_v
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|n| n.as_i64().map(|n| n.to_string()))
                .collect();
            (
                Incumbent {
                    tier_index: None,
                    revision: 0,
                    file_id: format!("f{file_id}"),
                    file_path: format!("/parity/f{file_id}.mkv"),
                    release_group: None,
                    score: 0,
                    language: AudioLanguageRank::default(),
                    covers,
                    created_at: created_at(0),
                },
                true,
            )
        })
        .collect();
    let subject = AdmissionSubject::new(AdmissionScope::Episodes(members), incumbents);
    let policy = AdmissionPolicy {
        allow_upgrades: true,
        min_delta: 0,
        cutoff_score: None,
        manual_override: false,
        applies_to_queue: false,
    };
    let verdict = evaluate_admission(&subject, CandidateFacts::new(None, 0, 0), &policy);
    Ok(accepted(!matches!(
        verdict.rejection().map(|r| &r.reason),
        Some(AdmissionRejectionReason::BroaderIncumbentSpan)
    )))
}

// ── Config entries (no corpus case; the check is built from `ref`) ─────────

fn config_suite(prefix: &str, name: &str) {
    let Some(mut suite) = Suite::open(name) else { return };
    let catalogue = Catalogue::load();
    for (area, entry) in &catalogue.entries {
        if area != "decision" || entry.source != Source::Config || !entry.id.starts_with(prefix) {
            continue;
        }
        if !suite.wants(&entry.id) {
            continue;
        }
        if let Some(outcome) = config_check(&entry.reference) {
            match outcome {
                Ok(()) => suite.record(&entry.id, true, ""),
                Err(why) => suite.record(&entry.id, false, why),
            }
        }
    }
    suite.finish();
}

/// `Ok(())`: Scryer behaves as the entry says. `Err`: it does not, or the
/// entry names something Scryer has no field for. `None`: unmapped.
fn config_check(reference: &str) -> Option<Result<(), String>> {
    let (path, fragment) = reference.split_once('#')?;
    let app = path.split('/').nth(2)?; // corpus/captured/<app>/config/<file>
    let file = path.rsplit('/').next()?;
    match file {
        "qualityprofile.json" => {
            let (id_part, field) = fragment.split_once('/')?;
            let id: i64 = id_part.strip_prefix("id=")?.parse().ok()?;
            match field {
                "minFormatScore" => Some(check_min_format_score(app, id)),
                "cutoff" | "cutoffFormatScore" => Some(check_cutoff(app, id)),
                "minUpgradeFormatScore" => Some(check_min_upgrade(app, id)),
                "upgradeAllowed" => Some(check_upgrades_off(app, id)),
                "language" => Some(Err(
                    "no Scryer field for a required 'Original' language".into(),
                )),
                _ => None,
            }
        }
        "qualitydefinition.json" => Some(Err(
            "no Scryer field for per-quality size limits or a preferred size".into(),
        )),
        "delayprofile.json" => {
            let id: i64 = fragment.strip_prefix("id=")?.parse().ok()?;
            Some(check_delay_profile(app, id))
        }
        "indexerconfig.json" if fragment == "minimumAge" => Some(check_minimum_age(app)),
        _ => None,
    }
}

fn expect_that(ok: bool, what: &str, failures: &mut Vec<String>) {
    if !ok {
        failures.push(what.to_string());
    }
}

fn verdict(failures: Vec<String>) -> Result<(), String> {
    if failures.is_empty() { Ok(()) } else { Err(failures.join("; ")) }
}

/// The lowest tier of a profile, as a release side of that tier.
fn release_at(profile: &QualityProfile, tier_from_end: usize, score: i32, group: Option<&str>) -> Option<Side> {
    let tiers = &profile.criteria.quality_tiers;
    let tier = tiers.get(tiers.len().checked_sub(1 + tier_from_end)?)?;
    let tier: &'static str = match tier.as_str() {
        "480P" => "480P",
        "576P" => "576P",
        "720P" => "720P",
        "1080P" => "1080P",
        "2160P" => "2160P",
        _ => return None,
    };
    Some(Side {
        q: Q {
            tier: Some(tier),
            source: Some(ReleaseSource::WebDl),
            remux: false,
            bd_disk: false,
            alias: None,
        },
        name: format!("WEBDL-{tier}"),
        revision: 0,
        group: group.map(str::to_string),
        score,
    })
}

fn check_min_format_score(app: &str, id: i64) -> Result<(), String> {
    let spec = captured_spec(app, id)?;
    let profile = build_profile(&spec, &[])?;
    let release = release_at(&profile, 0, 0, None).ok_or("profile has no tier")?;
    let min = spec.min_format_score;
    let gate = |total: i32| {
        let mut decision = requirements(&profile, &release);
        let delta = total - decision.release_score;
        decision.log_with_source(
            "parity-format",
            delta,
            ScoringSource::UserRule {
                id: "parity-format".into(),
                name: "parity-format".into(),
            },
        );
        apply_min_score_gate(&profile, &mut decision);
        decision.block_codes.iter().any(|c| c == "score_below_minimum")
    };
    let mut failures = Vec::new();
    expect_that(gate(min - 1), &format!("score {} not rejected", min - 1), &mut failures);
    expect_that(!gate(min), &format!("score {min} rejected"), &mut failures);
    verdict(failures)
}

fn check_cutoff(app: &str, id: i64) -> Result<(), String> {
    let spec = captured_spec(app, id)?;
    let cutoff_name = spec.cutoff.clone().ok_or("no cutoff")?;
    let cutoff_q = quality(&cutoff_name)?;
    // A Remux cutoff is probed with a WEB-DL file of its resolution (split, D54).
    let web = match (cutoff_q.remux, cutoff_q.tier) {
        (true, Some(tier)) => Some(quality(&format!("WEBDL-{}", tier.to_ascii_lowercase()))?),
        _ => None,
    };
    let profile = build_profile(&spec, web.as_slice())?;
    let crit = &profile.criteria;
    let cfs = spec.cutoff_format_score;
    let at = |q: Q, name: &str, score: i32| {
        let file = Side {
            q,
            name: name.to_string(),
            revision: 0,
            group: None,
            score,
        };
        let subject = AdmissionSubject::new(
            AdmissionScope::Title,
            [(incumbent(&profile, "f1", &file, None, 0), true)],
        );
        incumbent_at_cutoff(
            crit.quality_cutoff_met(key(&q).as_deref()),
            &subject,
            crit.cutoff_score,
            &crit.decision_pipeline,
        )
    };
    let mut failures = Vec::new();
    expect_that(at(cutoff_q, &cutoff_name, cfs), &format!("{cutoff_name} at score {cfs} not at cutoff"), &mut failures);
    expect_that(!at(cutoff_q, &cutoff_name, cfs - 1), &format!("{cutoff_name} at score {} at cutoff", cfs - 1), &mut failures);
    if let Some(web) = web {
        let name = format!("WEBDL-{}", web.tier.unwrap_or_default());
        expect_that(!at(web, &name, cfs + 1000), &format!("{name} at score {} at cutoff", cfs + 1000), &mut failures);
    }
    let below = crit
        .quality_tiers
        .iter()
        .position(|t| Some(t) == crit.cutoff_tier.as_ref())
        .and_then(|i| crit.quality_tiers.get(i + 1))
        .filter(|below| !below.contains('-'));
    if let Some(below) = below {
        let below_q = quality(&format!("WEBDL-{}", below.to_ascii_lowercase()))?;
        expect_that(
            !at(below_q, below, cfs + 1000),
            &format!("{below} at score {} at cutoff", cfs + 1000),
            &mut failures,
        );
    }
    verdict(failures)
}

fn check_min_upgrade(app: &str, id: i64) -> Result<(), String> {
    let spec = captured_spec(app, id)?;
    let profile = build_profile(&spec, &[])?;
    let min = spec.min_upgrade_format_score;
    let file = release_at(&profile, 0, 0, None).ok_or("profile has no tier")?;
    let held = [Held {
        id: "f1".into(),
        side: file.clone(),
        episode: None,
        added_days_ago: 0,
    }];
    let refused = |score: i32| {
        let candidate = Side { score, ..file.clone() };
        grab_refusal(&profile, &candidate, &held, None, Vec::new(), false).is_some()
    };
    // The reference also refuses a score that does not rise at all
    // (UpgradableSpecification.cs:86), so the bar is never below +1.
    let need = min.max(1);
    let mut failures = Vec::new();
    expect_that(refused(need - 1), &format!("+{} upgrade admitted", need - 1), &mut failures);
    expect_that(!refused(need), &format!("+{need} upgrade refused"), &mut failures);
    verdict(failures)
}

fn check_upgrades_off(app: &str, id: i64) -> Result<(), String> {
    let spec = captured_spec(app, id)?;
    let profile = build_profile(&spec, &[])?;
    let file = release_at(&profile, 0, 0, Some("GRP")).ok_or("profile has no tier")?;
    let held = [Held {
        id: "f1".into(),
        side: file.clone(),
        episode: None,
        added_days_ago: 0,
    }];
    let mut failures = Vec::new();
    // Better quality: a file below the profile's lowest tier (Unknown when no
    // lower tier exists) is below cutoff, so only the upgrade switch can
    // refuse a release at the profile's lowest tier.
    let lowest = profile.criteria.quality_tiers.last().map(|t| tier_lines(t)).unwrap_or(0);
    let lower = ["WEBDL-2160p", "WEBDL-1080p", "WEBDL-720p", "Bluray-576p", "WEBDL-480p"]
        .into_iter()
        .find(|n| quality(n).ok().and_then(|q| q.tier).is_some_and(|t| tier_lines(t) < lowest))
        .unwrap_or("Unknown");
    let worse = [Held {
        id: "f0".into(),
        side: Side { q: quality(lower)?, name: lower.into(), ..file.clone() },
        episode: None,
        added_days_ago: 0,
    }];
    expect_that(
        grab_refusal(&profile, &file, &worse, None, Vec::new(), false).is_some(),
        &format!("a better-quality release was admitted over a {lower} file"),
        &mut failures,
    );
    let better = Side { score: 10_000, ..file.clone() };
    // The entry asserts the outcome (not upgraded), not which gate refuses.
    expect_that(
        grab_refusal(&profile, &better, &held, None, Vec::new(), false).is_some(),
        "a better-scored release of the same quality was admitted as an upgrade",
        &mut failures,
    );
    let proper = Side { revision: 1, ..file.clone() };
    expect_that(
        grab_refusal(&profile, &proper, &held, None, Vec::new(), false).is_none(),
        "a same-group proper of the same quality was refused",
        &mut failures,
    );
    verdict(failures)
}

fn delay_profiles(app: &str) -> Result<Vec<(i64, DelayProfile)>, String> {
    let raw: Vec<Value> = corpus(&format!("corpus/captured/{app}/config/delayprofile.json"));
    let tags: Vec<Value> = corpus(&format!("corpus/captured/{app}/config/tag.json"));
    let minimum_age: Value = corpus(&format!("corpus/captured/{app}/config/indexerconfig.json"));
    raw.iter()
        .map(|d| -> Result<(i64, DelayProfile), String> {
            let id = d["id"].as_i64().ok_or("delay profile without id")?;
            let labels = d["tags"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|t| {
                    tags.iter()
                        .find(|tag| tag["id"] == *t)
                        .and_then(|tag| tag["label"].as_str())
                        .map(str::to_string)
                })
                .collect();
            Ok((
                id,
                DelayProfile {
                    id: id.to_string(),
                    name: format!("parity {id}"),
                    usenet_delay_minutes: d["usenetDelay"].as_i64().unwrap_or(0),
                    torrent_delay_minutes: d["torrentDelay"].as_i64().unwrap_or(0),
                    preferred_protocol: preferred(&d["preferredProtocol"]),
                    min_age_minutes: minimum_age["minimumAge"].as_i64().unwrap_or(0),
                    bypass_score_threshold: (d["bypassIfAboveCustomFormatScore"].as_bool() == Some(true))
                        .then(|| int(&d["minimumCustomFormatScore"], 0)),
                    enable_usenet: d["enableUsenet"].as_bool().unwrap_or(true),
                    enable_torrent: d["enableTorrent"].as_bool().unwrap_or(true),
                    bypass_if_highest_quality: d["bypassIfHighestQuality"].as_bool().unwrap_or(false),
                    tags: labels,
                    priority: d["order"].as_i64().unwrap_or(0).min(i64::from(i32::MAX)) as i32,
                    ..DelayProfile::default()
                },
            ))
        })
        .collect()
}

fn check_delay_profile(app: &str, id: i64) -> Result<(), String> {
    let profiles = delay_profiles(app)?;
    let raw: Vec<Value> = corpus(&format!("corpus/captured/{app}/config/delayprofile.json"));
    let d = raw
        .iter()
        .find(|d| d["id"].as_i64() == Some(id))
        .ok_or_else(|| format!("delay profile {id} not captured"))?;
    let all: Vec<DelayProfile> = profiles.iter().map(|(_, p)| p.clone()).collect();
    let target = &profiles.iter().find(|(pid, _)| *pid == id).ok_or("not translated")?.1;
    let facet = if app == "radarr" { MediaFacet::Movie } else { MediaFacet::Series };
    let mut failures = Vec::new();

    // Which titles it applies to: its tags, or (untagged) titles no tagged
    // profile matches.
    let resolved = resolve_delay_profile(&all, &target.tags, &facet, "parity-library");
    expect_that(
        resolved.is_some_and(|p| p.id == target.id),
        &format!("titles tagged {:?} resolve another profile", target.tags),
        &mut failures,
    );

    let preferred_kind = match target.preferred_protocol {
        PreferredProtocol::Usenet => json!("usenet"),
        PreferredProtocol::Torrent => json!("torrent"),
    };
    expect_that(
        target.is_preferred_protocol(source_kind(&preferred_kind)),
        "preferred protocol not preferred",
        &mut failures,
    );
    for (protocol, enabled, delay) in [
        ("usenet", d["enableUsenet"].as_bool() == Some(true), d["usenetDelay"].as_i64().unwrap_or(0)),
        ("torrent", d["enableTorrent"].as_bool() == Some(true), d["torrentDelay"].as_i64().unwrap_or(0)),
    ] {
        // A fresh release: held for the delay, refused when the protocol is off.
        let reason = delay_decision_reason(target, &json!(protocol), Some(0), &DelayPolicyContext::default());
        let want = if !enabled {
            DelayDecisionReason::ProtocolDisabled
        } else if delay > 0 {
            DelayDecisionReason::PendingDelay
        } else {
            DelayDecisionReason::Eligible
        };
        expect_that(reason == want, &format!("{protocol}: {reason:?}, want {want:?}"), &mut failures);
        if !enabled || delay == 0 || preferred_kind != json!(protocol) {
            continue;
        }
        // The bypasses apply to the preferred protocol while it is delayed.
        let highest = DelayPolicyContext {
            preferred_protocol_highest_quality: true,
            ..DelayPolicyContext::default()
        };
        let bypassed = delay_decision_reason(target, &json!(protocol), Some(0), &highest)
            == DelayDecisionReason::Eligible;
        expect_that(
            bypassed == (d["bypassIfHighestQuality"].as_bool() == Some(true)),
            &format!("{protocol}: highest-quality bypass {bypassed}"),
            &mut failures,
        );
        let scored = DelayPolicyContext {
            candidate_score: int(&d["minimumCustomFormatScore"], 0).max(1) * 1000,
            ..DelayPolicyContext::default()
        };
        let bypassed = delay_decision_reason(target, &json!(protocol), Some(0), &scored)
            == DelayDecisionReason::Eligible;
        expect_that(
            bypassed == (d["bypassIfAboveCustomFormatScore"].as_bool() == Some(true)),
            &format!("{protocol}: custom-format-score bypass {bypassed}"),
            &mut failures,
        );
    }
    verdict(failures)
}

fn check_minimum_age(app: &str) -> Result<(), String> {
    let config: Value = corpus(&format!("corpus/captured/{app}/config/indexerconfig.json"));
    let minimum = config["minimumAge"].as_i64().ok_or("no minimumAge")?;
    let mut failures = Vec::new();
    for (id, profile) in delay_profiles(app)? {
        let fresh = delay_decision_reason(&profile, &json!("usenet"), Some(0), &DelayPolicyContext::default());
        let aged = delay_decision_reason(&profile, &json!("usenet"), Some(minimum), &DelayPolicyContext::default());
        expect_that(
            (fresh == DelayDecisionReason::MinimumAge) == (minimum > 0),
            &format!("profile {id}: fresh usenet release {fresh:?}"),
            &mut failures,
        );
        expect_that(
            aged != DelayDecisionReason::MinimumAge,
            &format!("profile {id}: release aged {minimum} min still held"),
            &mut failures,
        );
    }
    verdict(failures)
}
